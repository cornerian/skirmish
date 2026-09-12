//! Real-replay comparison ratchet against the published gameplay export, for
//! the first Falco recording: a real Falco-vs-Fox Final Destination match.
//! `docs/falco.md` covers Falco's registration
//! (`game::characters::Specials::Falco`) this ratchet exercises end to end
//! (`make-initialization` accepting a `fighters/falco.json`-shaped pack,
//! Falco's own external Slippi id resolving through the shared Fox table).
//!
//! This file exists as a standalone test (rather than a loop or table inside
//! `real_parity.rs`), following `real_parity_fox_fd_4.rs`'s own precedent:
//! a separate, concurrent batch is generalizing that harness into a shared
//! recordings list (`docs/parity.md`); until that lands on `origin/main`,
//! this recording keeps its own baseline file
//! (`falco-fox-fd-baseline.json`). Move `falco-fox-fd.slp` and this ratchet
//! into the shared list once it exists, rather than duplicating the harness
//! further.
//!
//! `tests/fixtures/slippi/parity/falco-fox-fd.slp` is
//! `12_45_21 Falco + [HAMB] Fox (FD).slp` from the CC0-1.0
//! `erickfm/slippi-public-dataset-v3.7` corpus (`batch_00`), Falco vs. Fox,
//! Final Destination, Slippi 2.0.1, ports P3/P4 (Falco first). The
//! `falco-fox-fd` pairing's `match-data.json` (`SKIRMISH_GAMEPLAY_DATA`)
//! spawns fighters in participant order, matching this recording's own
//! Falco-then-Fox order regardless of which physical ports it used.
//!
//! See `real_parity.rs`'s own doc comment for the shared background on the
//! three verification levels, the `SKIRMISH_GAMEPLAY_DATA` skip path, and
//! the report/ratchet mechanics; this file only changes which replay,
//! pairing and baseline are compared. Unlike `fox-fd`/`fox-fd-4` (both
//! Fox-vs-Fox), the current measurement's first divergence is deep in the
//! pre-game Entry warp-in (frame -30), not mid-match; see
//! `falco-fox-fd-baseline.json`'s own note for what was and was not ruled
//! out in this batch.
use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const PAIRING: &str = "falco-fox-fd";
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/falco-fox-fd.slp"
);
const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/falco-fox-fd-baseline.json"
);

#[test]
fn ratchets_the_first_divergent_frame_against_the_real_falco_fox_fd_gameplay_export() {
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
                     baseline {base} (tests/fixtures/slippi/parity/falco-fox-fd-baseline.json)"
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
