//! Opt-in real-replay measurement for the tracked Captain Falcon/Marth replay.
//!
//! This is a provisional timeline ratchet only.  It consumes a local
//! `captain-falcon-marth-fd` gameplay pack when
//! `SKIRMISH_CAPTAIN_MARTH_FD_DATA` points at its export root; ordinary test
//! runs skip it.  The baseline records the first observed mismatch and pack
//! provenance, not a parity claim.

use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const PAIRING: &str = "captain-falcon-marth-fd";
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/11-captain-falcon-marth-final-destination.slp"
);
const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/captain-marth-fd-baseline.json"
);

#[test]
fn ratchets_the_provisional_captain_marth_fd_timeline_when_opted_in() {
    let Ok(root) = env::var("SKIRMISH_CAPTAIN_MARTH_FD_DATA") else {
        println!(
            "skip: SKIRMISH_CAPTAIN_MARTH_FD_DATA is not set; provisional Captain/Marth replay measurement is opt-in"
        );
        return;
    };
    let match_data_path = PathBuf::from(&root).join(PAIRING).join("match-data.json");
    if !match_data_path.is_file() {
        println!(
            "skip: {} does not exist; the unmodified Captain/Marth pack is unavailable",
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

    // A mismatch is expected for this provisional measurement.  The command
    // still writes the report, which is the value ratcheted below.
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
    match report["outcome"]["status"].as_str().unwrap() {
        "matched" => {}
        "mismatch" => {
            let actual = report["outcome"]["frame"]
                .as_i64()
                .expect("mismatch outcome carries a frame");
            assert!(
                baseline_frame.is_some_and(|base| actual >= base),
                "provisional Captain/Marth first divergence {actual} regressed before baseline {baseline_frame:?}"
            );
        }
        "error" => panic!(
            "provisional replay measurement failed: {}",
            report["outcome"]["message"]
                .as_str()
                .unwrap_or("<no message>")
        ),
        other => panic!("unexpected report status {other:?}"),
    }
}
