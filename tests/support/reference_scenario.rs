//! Independent native-reference scenario loader.
//!
//! Resolve `<SKIRMISH_CONFORMANCE_DIR>/<case_id>`, falling back to
//! `tests/fixtures/conformance/<case_id>`. Each directory contains `manifest.json`,
//! `data.json`, `inputs.jsonl` and a complete `expected.jsonl` semantic trace.
//! Manifest schema:
//! ```json
//! {"schema":1,"case_id":"native_stage_topology","seed":42,
//!  "provenance":{"kind":"independent_native_reference","description":"producer and method",
//!    "source_revision":"40 hex Git digits","source_sha256":"64 hex digits"},
//!  "files":{"data.json":"SHA256","inputs.jsonl":"SHA256","expected.jsonl":"SHA256"},
//!  "restore_before_frame":null}
//! ```
//! Hashes bind the supplied files; provenance is the reference author's explicit
//! declaration, not proof of independence. The RNG/restore case must specify a
//! nonzero `restore_before_frame`. No assets, golden trace or missing subsystem
//! are synthesized when a fixture is absent. Synthetic references are accepted
//! only by the separate loader regression entry point below.
#![allow(dead_code)] // Separate integration targets consume this shared helper.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use skirmish::{
    game::{Controller, Match, State, data::MatchData},
    match_trace,
    trace::{self, Record},
};
use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    IndependentNativeReference,
    SyntheticLoaderTest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    kind: ReferenceKind,
    description: String,
    source_revision: String,
    source_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    #[serde(rename = "data.json")]
    data: String,
    #[serde(rename = "inputs.jsonl")]
    inputs: String,
    #[serde(rename = "expected.jsonl")]
    expected: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    case_id: String,
    seed: u32,
    provenance: Provenance,
    files: Files,
    #[serde(default)]
    restore_before_frame: Option<usize>,
}

pub fn assert_reference(name: &str) {
    let root = std::env::var_os("SKIRMISH_CONFORMANCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance")
        });
    verify_directory(
        &root.join(name),
        name,
        ReferenceKind::IndependentNativeReference,
    )
    .unwrap_or_else(|error| panic!("independent reference case {name}: {error:#}"));
}

pub fn verify_directory(
    directory: &Path,
    name: &str,
    required: ReferenceKind,
) -> Result<trace::Comparison> {
    ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "invalid case ID"
    );
    let manifest: Manifest = serde_json::from_slice(&read(directory, "manifest.json", 65_536)?)
        .context("invalid strict reference manifest")?;
    ensure!(
        manifest.schema == 1 && manifest.case_id == name,
        "manifest schema/case_id does not match requested case"
    );
    let provenance = &manifest.provenance;
    ensure!(
        provenance.kind == required,
        "reference provenance kind is not {required:?}"
    );
    ensure!(
        !provenance.description.trim().is_empty(),
        "reference producer/method description is required"
    );
    ensure!(
        hex(&provenance.source_revision, 40),
        "source_revision must be a complete Git revision"
    );
    ensure!(
        hex(&provenance.source_sha256, 64),
        "source_sha256 must identify the reference source"
    );

    let data = verified(directory, "data.json", &manifest.files.data)?;
    let inputs = verified(directory, "inputs.jsonl", &manifest.files.inputs)?;
    let expected = verified(directory, "expected.jsonl", &manifest.files.expected)?;
    let data: MatchData = serde_json::from_slice(&data).context("invalid native match data")?;
    let inputs: Vec<[Controller; 2]> = std::str::from_utf8(&inputs)?
        .lines()
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line).with_context(|| format!("invalid input line {}", index + 1))
        })
        .collect::<Result<_>>()?;
    ensure!(
        !inputs.is_empty(),
        "reference scenario requires nonempty inputs"
    );
    ensure!(
        inputs.len() <= 100_000,
        "reference scenario exceeds 100000 input frames"
    );
    if name == "rng_consumption_and_restore" {
        ensure!(
            manifest.restore_before_frame.is_some(),
            "RNG/restore conformance requires an explicit restore_before_frame"
        );
    }
    if let Some(frame) = manifest.restore_before_frame {
        ensure!(
            frame > 0 && frame < inputs.len(),
            "restore_before_frame must lie strictly inside the input sequence"
        );
    }

    let mut script = inputs.iter().copied();
    let mut actual = Vec::new();
    match_trace::run(
        data.clone(),
        manifest.seed,
        |_| Ok(script.next()),
        &mut actual,
    )?;
    if let Some(frame) = manifest.restore_before_frame {
        replay_checkpoint(&data, manifest.seed, &inputs, frame, &mut actual)?;
    }
    trace::compare(Cursor::new(expected), Cursor::new(actual))
}

fn replay_checkpoint(
    data: &MatchData,
    seed: u32,
    inputs: &[[Controller; 2]],
    frame: usize,
    actual: &mut Vec<u8>,
) -> Result<()> {
    let mut game = Match::new(data.clone(), seed)?;
    for &input in &inputs[..frame] {
        game.step(input)?;
    }
    let checkpoint = game.checkpoint();
    // Change both mutable simulation state and seed, then restore the complete
    // native checkpoint. Every following observation still compares against
    // the independently supplied expected trace, never a generated golden.
    game.reset(seed.wrapping_add(1));
    game.restore_checkpoint(&checkpoint)?;
    let mut records: Vec<Record> = std::str::from_utf8(actual)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    for (index, &input) in inputs.iter().enumerate().skip(frame) {
        let native = game.step(input)?;
        let Record::Frame { state, events, .. } = &mut records[index + 1] else {
            anyhow::bail!("native trace did not contain the requested checkpoint frame");
        };
        state.insert("match".into(), observation(native)?);
        *events = native
            .events
            .iter()
            .map(match_trace::float_bits)
            .collect::<Result<_>>()?;
    }
    actual.clear();
    for record in records {
        serde_json::to_writer(&mut *actual, &record)?;
        actual.push(b'\n');
    }
    Ok(())
}

fn observation(state: &State) -> Result<serde_json::Value> {
    let mut value = match_trace::float_bits(state)?;
    value
        .as_object_mut()
        .context("native state must be an object")?
        .remove("events");
    Ok(value)
}

fn read(directory: &Path, name: &str, limit: u64) -> Result<Vec<u8>> {
    let path = directory.join(name);
    let file = fs::File::open(&path).with_context(|| {
        format!(
            "missing required fixture {}; supply SKIRMISH_CONFORMANCE_DIR",
            path.display()
        )
    })?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "fixture {} is not a bounded regular file",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    ensure!(
        bytes.len() as u64 <= limit,
        "fixture grew beyond its size limit: {}",
        path.display()
    );
    Ok(bytes)
}

fn verified(directory: &Path, name: &str, hash: &str) -> Result<Vec<u8>> {
    ensure!(hex(hash, 64), "{name} requires a SHA256 digest");
    let bytes = read(directory, name, 128 * 1024 * 1024)?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == hash.to_ascii_lowercase(),
        "{name} SHA256 mismatch"
    );
    Ok(bytes)
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
