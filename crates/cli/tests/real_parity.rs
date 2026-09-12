//! Real-replay comparison ratchet against the published gameplay export.
//!
//! This is the third and strongest of Skirmish's three verification levels
//! (see `docs/parity.md`): the other two are function-level C-oracle
//! equivalence and self-recorded replay regression (`replay_match.rs`,
//! `slippi_corpus.rs`). This target compares the native match — initialized
//! from `skirmish-assets`'s gameplay export via `make-initialization` —
//! against each of several real, independently recorded Fox-vs-Fox Final
//! Destination replays listed in
//! [`tests/fixtures/slippi/parity/recordings.json`](../tests/fixtures/slippi/parity/recordings.json).
//! Neither the replays nor the gameplay export's resources come from
//! Skirmish's own simulator.
//!
//! Export layout (produced by `skirmish-assets`, consumed here):
//! `<SKIRMISH_GAMEPLAY_DATA>/<pairing>/match-data.json` (or its compact
//! `match-data.bin` sibling, preferred when present -- see
//! `skirmish_cli::pack`) plus a sibling `manifest.json` recording the
//! export's own provenance, for each `<pairing>` (currently just
//! `fox-fd`; every recording in `recordings.json` is a Fox-vs-Fox Final
//! Destination match, so all three currently share that one pairing's pack
//! regardless of which two ports played it — spawns are assigned by
//! participant order, not port). Either file decodes to a native
//! `skirmish::game::data::MatchData` for that matchup; `make-initialization`
//! cross-checks its fighter/stage names against each replay's own recorded
//! external IDs (see `crates/cli/src/initialization.rs`).
//!
//! `SKIRMISH_GAMEPLAY_DATA` is unset in ordinary CI and locally until the
//! export is published (see `docs/gameplay-export.md`'s "Durable location"
//! section); this test then prints a skip message and passes, without
//! `#[ignore]`, so the skip path itself always runs in CI. Once the
//! directory and a given entry's pairing's `match-data.{json,bin}` exist,
//! the full comparison runs for that recording and its own
//! `first_divergent_frame` is ratcheted against its own baseline file
//! (`tests/fixtures/slippi/parity/<id>-baseline.json`, named in
//! `recordings.json`): the recorded frame must not get worse (earlier) than
//! the last time a reviewer updated that recording's baseline. A recording
//! whose baseline still records its replay's own first frame is the worst
//! possible result and never blocks progress until a reviewer tightens it
//! after a real run. Every recording is checked before the test panics, so
//! one regression is reported alongside any others rather than hiding them.
use serde::Deserialize;
use serde_json::Value;
use skirmish_cli::pack;
use std::{env, fs, path::PathBuf, process::Command};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity"
);

#[derive(Deserialize)]
struct RecordingsFile {
    recordings: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
    id: String,
    file: String,
    pairing: String,
    baseline: String,
}

/// One recording's outcome against its own baseline: `Ok(())` if the
/// ratchet is satisfied (including a pack-not-yet-published skip), `Err`
/// describing the regression otherwise.
fn check_recording(root: &str, recording: &Recording) -> Result<(), String> {
    let pairing_dir = PathBuf::from(root).join(&recording.pairing);
    let Some(match_data_path) = pack::discover_match_data(&pairing_dir) else {
        println!(
            "skip: neither match-data.bin nor match-data.json exists under {} for recording \
             {:?}; SKIRMISH_GAMEPLAY_DATA={root} is set, but the {} export has not landed there \
             yet.",
            pairing_dir.display(),
            recording.id,
            recording.pairing
        );
        return Ok(());
    };

    let replay_path = PathBuf::from(FIXTURES).join(&recording.file);
    let baseline_path = PathBuf::from(FIXTURES).join(&recording.baseline);

    let directory = tempfile::tempdir().unwrap();
    let initialization_path = directory.path().join("initialization.json");
    let report_path = directory.path().join("report.json");

    let make = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("make-initialization")
        .arg("--match-data")
        .arg(&match_data_path)
        .arg("--replay")
        .arg(&replay_path)
        .arg("--output")
        .arg(&initialization_path)
        .output()
        .unwrap();
    if !make.status.success() {
        return Err(format!(
            "{}: make-initialization failed against {}: {}",
            recording.id,
            match_data_path.display(),
            String::from_utf8_lossy(&make.stderr)
        ));
    }

    // validate-replay exits unsuccessfully on a mismatch or error outcome; it
    // still writes the report first, which is what this ratchet inspects.
    let _ = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("validate-replay")
        .arg(&replay_path)
        .arg("--initialization")
        .arg(&initialization_path)
        .arg("--report")
        .arg(&report_path)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap_or_else(|error| {
        panic!(
            "{}: validate-replay did not produce a report at {}: {error}",
            recording.id,
            report_path.display()
        )
    }))
    .unwrap();
    println!(
        "{}: {}",
        recording.id,
        serde_json::to_string_pretty(&report).unwrap()
    );

    let baseline: Value = serde_json::from_slice(&fs::read(&baseline_path).unwrap()).unwrap();
    let baseline_frame = baseline["first_divergent_frame"].as_i64();

    let status = report["outcome"]["status"].as_str().unwrap();
    match status {
        "matched" => Ok(()), // A full match always satisfies the ratchet.
        "mismatch" => {
            let actual = report["outcome"]["frame"]
                .as_i64()
                .expect("mismatch outcome carries a frame");
            match baseline_frame {
                None => Err(format!(
                    "{}: regression: the baseline recorded a full match \
                     (first_divergent_frame: null) but this run mismatched at frame {actual}",
                    recording.id
                )),
                Some(base) if actual >= base => Ok(()),
                Some(base) => Err(format!(
                    "{}: regression: first divergent frame {actual} is earlier than the \
                     recorded baseline {base} ({})",
                    recording.id,
                    baseline_path.display()
                )),
            }
        }
        "error" => Err(format!(
            "{}: real-replay comparison failed to run: {}",
            recording.id,
            report["outcome"]["message"]
                .as_str()
                .unwrap_or("<no message>")
        )),
        other => Err(format!(
            "{}: unexpected report status {other:?}",
            recording.id
        )),
    }
}

#[test]
fn ratchets_the_first_divergent_frame_against_real_gameplay_exports() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; real-replay comparison against the \
             published gameplay export is skipped. See docs/gameplay-export.md."
        );
        return;
    };

    let recordings_path = PathBuf::from(FIXTURES).join("recordings.json");
    let recordings: RecordingsFile =
        serde_json::from_slice(&fs::read(&recordings_path).unwrap()).unwrap();
    assert!(
        !recordings.recordings.is_empty(),
        "{} listed no recordings",
        recordings_path.display()
    );

    let failures: Vec<String> = recordings
        .recordings
        .iter()
        .filter_map(|recording| check_recording(&root, recording).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} recording(s) regressed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
