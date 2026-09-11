//! Real-replay comparison ratchet against the published gameplay export.
//!
//! This is the third and strongest of Skirmish's three verification levels
//! (see `docs/parity.md`): the other two are function-level C-oracle
//! equivalence and self-recorded replay regression (`replay_match.rs`,
//! `slippi_corpus.rs`). This target compares the real, independently
//! recorded `tests/fixtures/slippi/parity/fox-fd.slp` (Fox vs. Fox, Final
//! Destination, Slippi 2.0.1) against the native match built from
//! `skirmish-assets`'s gameplay export, once that export exists.
//!
//! Export layout (produced by `skirmish-assets`, consumed here):
//! `<SKIRMISH_GAMEPLAY_DATA>/<pairing>/match-data.json` plus a sibling
//! `manifest.json` recording the export's own provenance, for each
//! `<pairing>` (currently just `fox-fd`). `match-data.json` is a native
//! `skirmish::game::data::MatchData` for that matchup; `make-initialization`
//! cross-checks its fighter/stage names against the replay's own recorded
//! external IDs (see `crates/cli/src/initialization.rs`).
//!
//! `SKIRMISH_GAMEPLAY_DATA` is unset in ordinary CI and locally until the
//! export is published (see `docs/gameplay-export.md`'s "Durable location"
//! section); this test then prints a skip message and passes, without
//! `#[ignore]`, so the skip path itself always runs in CI. Once the
//! directory and `fox-fd/match-data.json` exist, the full comparison runs
//! and its `first_divergent_frame` is ratcheted against
//! `tests/fixtures/slippi/parity/fox-fd-baseline.json`: the recorded frame
//! must not get worse (earlier) than the last time a reviewer updated the
//! baseline. The baseline starts at the replay's own first frame (-123),
//! the worst possible result, so it never blocks progress until a reviewer
//! tightens it after a real run.
use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const PAIRING: &str = "fox-fd";
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/fox-fd.slp"
);
const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/fox-fd-baseline.json"
);

#[test]
fn ratchets_the_first_divergent_frame_against_the_real_fox_fd_gameplay_export() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; real-replay comparison against the \
             published gameplay export is skipped. See docs/gameplay-export.md."
        );
        return;
    };
    let match_data_path = PathBuf::from(&root).join(PAIRING).join("match-data.json");
    if !match_data_path.is_file() {
        println!(
            "skip: {} does not exist; SKIRMISH_GAMEPLAY_DATA={root} is set, but the {PAIRING} \
             export has not landed there yet.",
            match_data_path.display()
        );
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let initialization_path = directory.path().join("initialization.json");
    let report_path = directory.path().join("report.json");

    let make = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("make-initialization")
        .arg("--match-data")
        .arg(&match_data_path)
        .arg("--replay")
        .arg(REPLAY)
        .arg("--output")
        .arg(&initialization_path)
        .output()
        .unwrap();
    assert!(
        make.status.success(),
        "make-initialization failed against {}: {}",
        match_data_path.display(),
        String::from_utf8_lossy(&make.stderr)
    );

    // validate-replay exits unsuccessfully on a mismatch or error outcome; it
    // still writes the report first, which is what this ratchet inspects.
    let _ = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("validate-replay")
        .arg(REPLAY)
        .arg("--initialization")
        .arg(&initialization_path)
        .arg("--report")
        .arg(&report_path)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap_or_else(|error| {
        panic!(
            "validate-replay did not produce a report at {}: {error}",
            report_path.display()
        )
    }))
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    let baseline: Value = serde_json::from_slice(&fs::read(BASELINE).unwrap()).unwrap();
    let baseline_frame = baseline["first_divergent_frame"].as_i64();

    let status = report["outcome"]["status"].as_str().unwrap();
    match status {
        "matched" => {} // A full match always satisfies the ratchet.
        "mismatch" => {
            let actual = report["outcome"]["frame"]
                .as_i64()
                .expect("mismatch outcome carries a frame");
            match baseline_frame {
                None => panic!(
                    "regression: the baseline recorded a full match (first_divergent_frame: null) \
                     but this run mismatched at frame {actual}"
                ),
                Some(base) => assert!(
                    actual >= base,
                    "regression: first divergent frame {actual} is earlier than the recorded \
                     baseline {base} (tests/fixtures/slippi/parity/fox-fd-baseline.json)"
                ),
            }
        }
        "error" => panic!(
            "real-replay comparison failed to run: {}",
            report["outcome"]["message"]
                .as_str()
                .unwrap_or("<no message>")
        ),
        other => panic!("unexpected report status {other:?}"),
    }
}
