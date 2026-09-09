//! Reproducible source inventory. File coverage is not function or game coverage.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Count {
    pub files: usize,
    pub lines: usize,
}

#[derive(Debug, Serialize)]
pub struct Source {
    pub path: String,
    pub lines: usize,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub revision: String,
    pub tracked_changes: bool,
    pub total: Count,
    pub subsystems: BTreeMap<String, Count>,
    pub sources: Vec<Source>,
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

pub fn inventory(root: &Path) -> Result<Inventory> {
    let revision = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?
        .trim()
        .to_owned();
    let tracked_changes =
        !git(root, &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    let tracked = git(root, &["ls-files", "-z"])?;
    let mut report = Inventory {
        revision,
        tracked_changes,
        total: Count::default(),
        subsystems: BTreeMap::new(),
        sources: vec![],
    };
    for bytes in tracked.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let name = std::str::from_utf8(bytes)?;
        let path = Path::new(name);
        if !path.extension().is_some_and(|s| {
            ["c", "h", "s", "S", "cpp", "hpp"]
                .iter()
                .any(|ext| s == *ext)
        }) {
            continue;
        }
        let data =
            fs::read(root.join(path)).with_context(|| format!("read tracked source {name}"))?;
        let lines = data.split_inclusive(|&b| b == b'\n').count();
        let subsystem = path
            .iter()
            .take(2)
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let count = report.subsystems.entry(subsystem).or_default();
        count.files += 1;
        count.lines += lines;
        report.total.files += 1;
        report.total.lines += lines;
        report.sources.push(Source {
            path: name.into(),
            lines,
            sha256: format!("{:x}", Sha256::digest(&data)),
        });
    }
    Ok(report)
}
