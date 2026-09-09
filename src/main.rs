use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use sha1::{Digest, Sha1};
use skirmish::{inventory, runner, trace};
use std::{
    fs::{self, File},
    io::BufReader,
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
    /// Validate the original GALE01 1.02 main.dol when it becomes available.
    VerifyDol { path: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().command {
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
        Commands::VerifyDol { path } => {
            let mut hash = Sha1::new();
            std::io::copy(&mut File::open(path).context("open main.dol")?, &mut hash)?;
            let actual = format!("{:x}", hash.finalize());
            ensure!(
                actual == "08e0bf20134dfcb260699671004527b2d6bb1a45",
                "expected GALE01 1.02 main.dol; found SHA-1 {actual}"
            );
            println!("Verified GALE01 1.02 main.dol ({actual})");
        }
    }
    Ok(())
}
