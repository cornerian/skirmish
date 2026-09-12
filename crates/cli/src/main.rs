use anyhow::Result;
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use skirmish::{inventory, menus::Unlocks};
use skirmish_cli::{initialization, menu_cli, pack};
use skirmish_equivalence::{match_trace, runner, trace};
use skirmish_replay::{match_validation, slippi};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter},
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(
    version,
    about = "Skirmish Rust migration and semantic equivalence tools (game port in progress)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Browse the translated game menu branches in a line-oriented terminal.
    Menus {
        #[arg(long)]
        all_star: bool,
        #[arg(long)]
        sound_test: bool,
    },
    /// Run menu controller frames from JSONL and emit a semantic trace.
    RunMenus {
        /// Read from this file; otherwise read stdin. See docs/menus.md.
        #[arg(long)]
        inputs: Option<PathBuf>,
        #[arg(long)]
        all_star: bool,
        #[arg(long)]
        sound_test: bool,
    },
    /// Compare native match frames with Slippi observations from explicit initialization.
    ValidateReplay {
        path: PathBuf,
        #[arg(long)]
        initialization: PathBuf,
        #[arg(long)]
        finalized_only: bool,
        /// Also write the JSON report (including `initialization_sha256`) here.
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Build a validate-replay `Initialization` from a native `MatchData` and a completed replay.
    MakeInitialization {
        /// A `MatchData` file, either `.json` or the compact `.bin` pack
        /// format (see `skirmish pack`); dispatched on the extension.
        #[arg(long)]
        match_data: PathBuf,
        #[arg(long)]
        replay: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Override the replay's own recorded GameStart random seed.
        #[arg(long)]
        seed: Option<u32>,
    },
    /// Convert between the gameplay export's `match-data.json` and the
    /// compact, lossless `match-data.bin` pack format (see
    /// `docs/gameplay-export.md`).
    Pack {
        #[command(subcommand)]
        action: PackCommands,
    },
    /// Parse a completed Slippi replay with Peppi and summarize its timeline.
    InspectReplay {
        path: PathBuf,
        /// Include only frames explicitly finalized by Slippi >=3.7 bookends.
        #[arg(long)]
        finalized_only: bool,
    },
    /// Run the synthetic headless match fixture to completion; emit a JSONL trace.
    DemoMatch {
        #[arg(long, default_value_t = 0)]
        seed: u32,
    },
    /// Run an experimental native resource bundle with JSONL controller-pair inputs.
    RunMatch {
        #[arg(long)]
        data: PathBuf,
        /// Read controller pairs from this file; defaults to stdin for trace adapters.
        #[arg(long)]
        inputs: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        seed: u32,
    },
    /// Inventory tracked upstream C, C++, headers and assembly with content hashes.
    Inventory { upstream: PathBuf },
    /// Compare complete semantic JSONL traces without comparing machine code.
    CompareTraces {
        reference: PathBuf,
        candidate: PathBuf,
    },
    /// Execute two trace adapters using identical stdin and compare their observations.
    CompareBinaries {
        /// JSON adapter specification: absolute program, args and optional env.
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        input: PathBuf,
        /// A new output directory, preferably under /mnt/archive/runs.
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 60)]
        timeout_seconds: u64,
    },
}

#[derive(Subcommand)]
enum PackCommands {
    /// Read a `MatchData` JSON export and write the compact binary pack.
    Convert { json: PathBuf, bin: PathBuf },
    /// Decode both a JSON and binary `MatchData` pack and assert they hold
    /// the exact same value (a lossless round trip, not just "both parse").
    Verify { json: PathBuf, bin: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Menus {
            all_star,
            sound_test,
        } => {
            menu_cli::interactive(
                Unlocks {
                    all_star,
                    sound_test,
                },
                io::stdin().lock(),
                io::stdout().lock(),
            )?;
        }
        Commands::RunMenus {
            inputs,
            all_star,
            sound_test,
        } => {
            let input: Box<dyn BufRead> = match inputs {
                Some(path) => Box::new(BufReader::new(File::open(path)?)),
                None => Box::new(BufReader::new(io::stdin())),
            };
            menu_cli::run(
                Unlocks {
                    all_star,
                    sound_test,
                },
                input,
                BufWriter::new(io::stdout().lock()),
            )?;
        }
        Commands::ValidateReplay {
            path,
            initialization,
            finalized_only,
            report,
        } => {
            let replay = slippi::Replay::read(BufReader::new(File::open(path)?))?;
            let bytes = fs::read(initialization)?;
            let initial: match_validation::Initialization = serde_json::from_slice(&bytes)?;
            let mut game = match_validation::initialize(&initial)?;
            let checkpoint = skirmish_replay::Checkpoint {
                next_frame: initial.next_frame,
                state: game.checkpoint(),
            };
            let policy = if finalized_only {
                slippi::Timeline::FinalizedOnly
            } else {
                slippi::Timeline::LastRecorded
            };
            let validated =
                match_validation::validate(&replay, &mut game, &checkpoint, initial.ports, policy)?;
            let mut output = serde_json::to_value(&validated)?;
            output["initialization_sha256"] = format!("{:x}", Sha256::digest(&bytes)).into();
            let text = serde_json::to_string_pretty(&output)?;
            if let Some(report) = report {
                fs::write(report, &text)?;
            }
            println!("{text}");
            anyhow::ensure!(
                validated.is_match(),
                "replay validation did not match; see JSON outcome"
            );
        }
        Commands::MakeInitialization {
            match_data,
            replay,
            output,
            seed,
        } => {
            let data = pack::load_match_data(&match_data)?;
            let replay = slippi::Replay::read(BufReader::new(File::open(replay)?))?;
            let built = initialization::build(data, &replay, seed)?;
            fs::write(&output, serde_json::to_string_pretty(&built)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "output": output,
                    "ports": built.ports,
                    "seed": built.seed,
                    "next_frame": built.next_frame,
                    "warmup_steps": built.warmup.len(),
                }))?
            );
        }
        Commands::Pack { action } => match action {
            PackCommands::Convert { json, bin } => {
                let bytes = fs::read(&json)?;
                let data: skirmish::game::data::MatchData = serde_json::from_slice(&bytes)?;
                pack::write_bin(&bin, &data)?;
                let json_len = bytes.len() as u64;
                let bin_len = fs::metadata(&bin)?.len();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "json": json,
                        "bin": bin,
                        "json_bytes": json_len,
                        "bin_bytes": bin_len,
                    }))?
                );
            }
            PackCommands::Verify { json, bin } => {
                let from_json: skirmish::game::data::MatchData =
                    serde_json::from_slice(&fs::read(&json)?)?;
                let from_bin = pack::read_bin(&bin)?;
                anyhow::ensure!(
                    from_json == from_bin,
                    "pack verify failed: {} and {} do not decode to the same MatchData",
                    json.display(),
                    bin.display()
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "json": json,
                        "bin": bin,
                        "equal": true,
                    }))?
                );
            }
        },
        Commands::InspectReplay {
            path,
            finalized_only,
        } => {
            let replay = slippi::Replay::read(BufReader::new(File::open(path)?))?;
            let policy = if finalized_only {
                slippi::Timeline::FinalizedOnly
            } else {
                slippi::Timeline::LastRecorded
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&replay.summary(policy)?)?
            );
        }
        Commands::DemoMatch { seed } => {
            let data = serde_json::from_str(include_str!(
                "../../../tests/fixtures/game/integration-match.json"
            ))?;
            match_trace::run(
                data,
                seed,
                |state| Ok(match_trace::demo_input(state)),
                BufWriter::new(io::stdout().lock()),
            )?;
        }
        Commands::RunMatch { data, inputs, seed } => {
            let data = serde_json::from_slice(&fs::read(data)?)?;
            let input: Box<dyn BufRead> = match inputs {
                Some(path) => Box::new(BufReader::new(File::open(path)?)),
                None => Box::new(BufReader::new(io::stdin())),
            };
            let mut lines = input.lines();
            match_trace::run(
                data,
                seed,
                |_| {
                    lines
                        .next()
                        .map(|line| Ok(serde_json::from_str(&line?)?))
                        .transpose()
                },
                BufWriter::new(io::stdout().lock()),
            )?;
        }
        Commands::Inventory { upstream } => println!(
            "{}",
            serde_json::to_string_pretty(&inventory::inventory(&upstream)?)?
        ),
        Commands::CompareTraces {
            reference,
            candidate,
        } => {
            let comparison = trace::compare(
                BufReader::new(File::open(reference)?),
                BufReader::new(File::open(candidate)?),
            )?;
            println!("{}", serde_json::to_string_pretty(&comparison)?);
        }
        Commands::CompareBinaries {
            reference,
            candidate,
            input,
            output,
            timeout_seconds,
        } => {
            let report = runner::compare_binaries(
                serde_json::from_slice(&fs::read(reference)?)?,
                serde_json::from_slice(&fs::read(candidate)?)?,
                &input,
                &output,
                Duration::from_secs(timeout_seconds),
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}
