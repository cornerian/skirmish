use anyhow::Result;
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use skirmish::{inventory, match_trace, runner, slippi, trace};
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
    /// Compare native match frames with Slippi observations from explicit initialization.
    ValidateReplay {
        path: PathBuf,
        #[arg(long)]
        initialization: PathBuf,
        #[arg(long)]
        finalized_only: bool,
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

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::ValidateReplay {
            path,
            initialization,
            finalized_only,
        } => {
            let replay = slippi::Replay::read(BufReader::new(File::open(path)?))?;
            let bytes = fs::read(initialization)?;
            let initial: skirmish::replay_match::Initialization = serde_json::from_slice(&bytes)?;
            let mut game = skirmish::replay_match::initialize(&initial)?;
            let checkpoint = skirmish::replay::Checkpoint {
                next_frame: initial.next_frame,
                state: game.checkpoint(),
            };
            let policy = if finalized_only {
                slippi::Timeline::FinalizedOnly
            } else {
                slippi::Timeline::LastRecorded
            };
            let report = skirmish::replay_match::validate(
                &replay,
                &mut game,
                &checkpoint,
                initial.ports,
                policy,
            )?;
            let mut output = serde_json::to_value(&report)?;
            output["initialization_sha256"] = format!("{:x}", Sha256::digest(&bytes)).into();
            println!("{}", serde_json::to_string_pretty(&output)?);
            anyhow::ensure!(
                report.is_match(),
                "replay validation did not match; see JSON outcome"
            );
        }
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
                "../tests/fixtures/game/integration-match.json"
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
