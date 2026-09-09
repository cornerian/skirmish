//! Runs two explicit trace-producing adapters on the same input, with deadlines.
use crate::trace::{self, Comparison};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub comparison: Comparison,
    pub input_sha256: String,
    pub reference_sha256: String,
    pub candidate_sha256: String,
    pub reference: Adapter,
    pub candidate: Adapter,
}

fn hash(path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    std::io::copy(&mut File::open(path)?, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

fn execute(
    adapter: &Adapter,
    input: &Path,
    trace: &Path,
    stderr: &Path,
    timeout: Duration,
) -> Result<()> {
    ensure!(
        adapter.program.is_absolute(),
        "adapter program must be an absolute path"
    );
    let mut child = Command::new(&adapter.program)
        .args(&adapter.args)
        .envs(&adapter.env)
        .stdin(File::open(input)?)
        .stdout(File::create(trace)?)
        .stderr(Stdio::from(File::create(stderr)?))
        .spawn()
        .with_context(|| format!("start {}", adapter.program.display()))?;
    // std avoids installing a process-global SIGCHLD handler in callers.
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(
                    Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())),
                );
            }
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                match outcome {
                    Err(error) => return Err(error.into()),
                    _ => bail!(
                        "{} exceeded deadline {timeout:?}; see {}",
                        adapter.program.display(),
                        stderr.display()
                    ),
                }
            }
        }
    };
    ensure!(
        status.success(),
        "{} exited {status}; see {}",
        adapter.program.display(),
        stderr.display()
    );
    Ok(())
}

/// Output directory must be new, so evidence from a previous run cannot be reused.
/// Programs receive identical stdin bytes and must emit the documented JSONL
/// protocol. This does not supply a completed Rust game.
pub fn compare_binaries(
    reference: Adapter,
    candidate: Adapter,
    input: &Path,
    output: &Path,
    timeout: Duration,
) -> Result<RunReport> {
    ensure!(!timeout.is_zero(), "timeout must be positive");
    let input_bytes = fs::read(input).context("read shared scenario input")?;
    fs::create_dir(output)
        .with_context(|| format!("create new run directory {}", output.display()))?;
    let shared_input = output.join("input.bin");
    fs::write(&shared_input, input_bytes)?;
    let input_sha256 = hash(&shared_input)?;
    let reference_sha256 = hash(&reference.program)?;
    let candidate_sha256 = hash(&candidate.program)?;
    // Save provenance before execution so failing comparisons remain reproducible.
    fs::write(
        output.join("invocation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "reference": &reference, "candidate": &candidate,
            "input_sha256": &input_sha256,
            "reference_sha256": &reference_sha256, "candidate_sha256": &candidate_sha256,
            "timeout_ms": timeout.as_millis(),
        }))?,
    )?;
    let left = output.join("reference.jsonl");
    let right = output.join("candidate.jsonl");
    execute(
        &reference,
        &shared_input,
        &left,
        &output.join("reference.stderr"),
        timeout,
    )?;
    ensure!(
        hash(&shared_input)? == input_sha256,
        "reference modified shared scenario input"
    );
    execute(
        &candidate,
        &shared_input,
        &right,
        &output.join("candidate.stderr"),
        timeout,
    )?;
    ensure!(
        hash(&shared_input)? == input_sha256,
        "candidate modified shared scenario input"
    );
    ensure!(
        hash(&reference.program)? == reference_sha256
            && hash(&candidate.program)? == candidate_sha256,
        "adapter executable changed during comparison"
    );
    let comparison = trace::compare(
        BufReader::new(File::open(left)?),
        BufReader::new(File::open(right)?),
    )?;
    let report = RunReport {
        comparison,
        input_sha256,
        reference_sha256,
        candidate_sha256,
        reference,
        candidate,
    };
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}
