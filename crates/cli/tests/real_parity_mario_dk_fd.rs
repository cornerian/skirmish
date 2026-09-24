//! Opt-in real-replay ratchet for the Mario/DK Final Destination fixture.
//!
//! The fixture has no Slippi finalized-frame bookends, so the CLI's default
//! `LastRecorded` timeline is intentional here. The gameplay pack is supplied
//! outside the repository through `SKIRMISH_GAMEPLAY_DATA`.
use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const PAIRING: &str = "mario-donkey-kong-fd";
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/03-mario-donkey-kong-final-destination.slp"
);
const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/mario-donkey-kong-fd-baseline.json"
);

#[test]
fn ratchets_first_divergence_for_real_mario_donkey_kong_fd_replay() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!("skip: SKIRMISH_GAMEPLAY_DATA is not set; Mario/DK replay comparison skipped");
        return;
    };
    let match_data_path = PathBuf::from(&root).join(PAIRING).join("match-data.json");
    if !match_data_path.is_file() {
        println!(
            "skip: {} does not exist; the {PAIRING} export is not published",
            match_data_path.display()
        );
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let initialization_path = directory.path().join("initialization.json");
    let report_path = directory.path().join("report.json");
    let make = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .args([
            "make-initialization",
            "--match-data",
            match_data_path.to_str().unwrap(),
            "--replay",
            REPLAY,
            "--output",
            initialization_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        make.status.success(),
        "make-initialization failed against {}: {}",
        match_data_path.display(),
        String::from_utf8_lossy(&make.stderr)
    );

    // Omit --finalized-only deliberately: this fixture has no finalized frames,
    // and LastRecorded is the provisional timeline used for the gate.
    let _ = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .args([
            "validate-replay",
            REPLAY,
            "--initialization",
            initialization_path.to_str().unwrap(),
            "--report",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    let baseline: Value = serde_json::from_slice(&fs::read(BASELINE).unwrap()).unwrap();
    println!("baseline provenance: {}", baseline["provenance"]);
    let baseline_frame = baseline["first_divergent_frame"].as_i64();
    match report["outcome"]["status"].as_str().unwrap() {
        "matched" => {}
        "mismatch" => {
            let actual = report["outcome"]["frame"]
                .as_i64()
                .expect("mismatch outcome carries a frame");
            assert!(
                baseline_frame.is_none_or(|base| actual >= base),
                "regression: first divergent frame {actual} is earlier than baseline {baseline_frame:?}"
            );
        }
        "error" => panic!(
            "real-replay comparison failed: {}",
            report["outcome"]["message"]
                .as_str()
                .unwrap_or("<no message>")
        ),
        other => panic!("unexpected report status {other:?}"),
    }
}
