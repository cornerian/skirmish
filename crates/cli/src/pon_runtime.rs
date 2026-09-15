//! CLI startup loading for explicit or executable-relative Pon releases.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use skirmish_script_runtime::{StandardLibrary, configure_standard_library};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

const RESOURCE_DIR: &str = "resources/pon-stdlib";
const ARCHIVE_NAME: &str = "pon-stdlib.tar.gz";
const IDENTITY_NAME: &str = "identity.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackagedIdentity {
    format: String,
    archive: String,
    archive_sha256: String,
    identity: String,
}

type PackagedRelease = (PathBuf, [u8; 32], [u8; 32]);

/// Configure an explicit release, or a release beside the current executable.
/// Explicit options always take precedence over packaged resources.
pub fn configure(archive: Option<PathBuf>, sha256: Option<String>) -> Result<()> {
    match (archive, sha256) {
        (None, None) => match packaged_default()? {
            Some((archive, expected, identity)) => {
                configure_archive(&archive, expected, Some(identity))
            }
            None => Ok(()),
        },
        (Some(_), None) => {
            bail!("--pon-stdlib requires --pon-stdlib-sha256 (64 hexadecimal characters)")
        }
        (None, Some(_)) => bail!("--pon-stdlib-sha256 requires --pon-stdlib <archive>"),
        (Some(archive), Some(sha256)) => configure_archive(&archive, parse_sha256(&sha256)?, None),
    }
}

fn packaged_default() -> Result<Option<PackagedRelease>> {
    let executable = std::env::current_exe().context("cannot locate current executable")?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current executable has no parent directory"))?
        .join(RESOURCE_DIR);
    if !directory.exists() {
        return Ok(None);
    }
    let archive = directory.join(ARCHIVE_NAME);
    let identity_path = directory.join(IDENTITY_NAME);
    let bytes = std::fs::read(&identity_path).with_context(|| {
        format!(
            "packaged Pon stdlib identity manifest is missing: {}",
            identity_path.display()
        )
    })?;
    let manifest: PackagedIdentity = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "invalid packaged Pon stdlib identity manifest {}",
            identity_path.display()
        )
    })?;
    ensure!(
        manifest.format == "skirmish-pon-stdlib-release-v1",
        "unsupported packaged Pon stdlib identity manifest format"
    );
    ensure!(
        manifest.archive == ARCHIVE_NAME,
        "packaged Pon stdlib identity manifest names unexpected archive"
    );
    Ok(Some((
        archive,
        parse_digest(&manifest.archive_sha256, "archive_sha256")?,
        parse_digest(&manifest.identity, "identity")?,
    )))
}

fn parse_digest(value: &str, field: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit),
        "packaged Pon stdlib {field} must contain exactly 64 hexadecimal characters"
    );
    parse_sha256(value)
}

fn configure_archive(
    archive: &Path,
    expected: [u8; 32],
    expected_identity: Option<[u8; 32]>,
) -> Result<()> {
    let reader = File::open(archive)
        .with_context(|| format!("cannot read Pon stdlib archive {}", archive.display()))?;
    let library = StandardLibrary::from_archive(reader, expected).map_err(|error| {
        anyhow::anyhow!("invalid Pon stdlib archive {}: {error}", archive.display())
    })?;
    if let Some(identity) = expected_identity {
        ensure!(
            library.identity_digest() == identity,
            "packaged Pon stdlib identity does not match archive manifest"
        );
    }
    let cache_root = std::env::temp_dir().join("skirmish-pon-stdlib");
    let materialized = library.materialize(&cache_root).map_err(|error| {
        anyhow::anyhow!(
            "cannot materialize Pon stdlib archive {} in {}: {error}",
            archive.display(),
            cache_root.display()
        )
    })?;
    configure_standard_library(materialized)
        .map_err(|error| anyhow::anyhow!("cannot configure Pon stdlib: {error}"))
}

fn parse_sha256(value: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit),
        "--pon-stdlib-sha256 must contain exactly 64 hexadecimal characters"
    );
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).expect("validated hex");
    }
    Ok(digest)
}
