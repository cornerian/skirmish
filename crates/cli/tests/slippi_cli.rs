use peppi_adapter::{Port, Version};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    process::{Command, Output},
};

#[path = "../../peppi-adapter/tests/support/mod.rs"]
mod support;

fn inspect(bytes: &[u8], finalized_only: bool) -> Output {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("synthetic.slp");
    fs::write(&path, bytes).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_skirmish"));
    command.arg("inspect-replay").arg(path);
    if finalized_only {
        command.arg("--finalized-only");
    }
    command.output().unwrap()
}

fn summary(bytes: &[u8], finalized_only: bool) -> Value {
    let output = inspect(bytes, finalized_only);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rejected(bytes: &[u8], finalized_only: bool) {
    let output = inspect(bytes, finalized_only);
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "failed import emitted success output: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn executable_reports_hash_ports_and_surviving_rollback_timeline() {
    let bytes = support::replay_bytes(&support::Fixture {
        frame_ids: support::ROLLBACK_FRAMES.to_vec(),
        follower: true,
        ..support::Fixture::default()
    });
    let report = summary(&bytes, false);
    assert_eq!(report["parser"], "peppi 2.1.2");
    assert_eq!(report["sha256"], format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(report["bytes"], bytes.len());
    assert_eq!(
        report["version"],
        serde_json::to_value(Version(3, 18, 0)).unwrap()
    );
    // Preserve physical P1/P3 identity (zero-based 0/2), including follower P3.
    assert_eq!(
        report["ports"],
        serde_json::to_value([Port::P1, Port::P3]).unwrap()
    );
    assert_eq!(report["stage"], 31);
    assert_eq!(report["physical_frames"], 5);
    assert_eq!(report["surviving_frames"], 3);
    assert_eq!(report["discarded_frames"], 2);
    assert_eq!(report["selected_frames"], 3);
    assert_eq!(report["first_frame"], -123);
    assert_eq!(report["last_frame"], -121);
    assert_eq!(report["latest_finalized_frame"], Value::Null);
    assert_eq!(report["timeline"], "last_recorded");
    assert_eq!(summary(&bytes, false), report);
}

#[test]
fn finalized_mode_uses_explicit_bookends_without_promoting_game_end() {
    let bytes = support::replay_bytes(&support::Fixture::default());
    let empty = summary(&bytes, true);
    assert_eq!(empty["selected_frames"], 0);
    assert_eq!(empty["first_frame"], Value::Null);
    assert_eq!(empty["last_frame"], Value::Null);
    assert_eq!(empty["latest_finalized_frame"], Value::Null);
    assert_eq!(empty["timeline"], "finalized_only");

    let bytes = support::replay_bytes(&support::Fixture {
        finalized_frames: Some(vec![-130, -129, -123]),
        ..support::Fixture::default()
    });
    let prefix = summary(&bytes, true);
    assert_eq!(prefix["physical_frames"], 3);
    assert_eq!(prefix["surviving_frames"], 3);
    assert_eq!(prefix["selected_frames"], 1);
    assert_eq!(prefix["first_frame"], -123);
    assert_eq!(prefix["last_frame"], -123);
    assert_eq!(prefix["latest_finalized_frame"], -123);
    assert_eq!(summary(&bytes, false)["selected_frames"], 3);
}

#[test]
fn incomplete_invalid_and_unsupported_replays_emit_no_success_json() {
    rejected(b"not a Slippi file", false);
    rejected(
        &support::truncated_bytes(&support::Fixture::default()),
        false,
    );
    rejected(
        &support::replay_bytes(&support::Fixture {
            ended: false,
            ..support::Fixture::default()
        }),
        false,
    );
    let old = support::replay_bytes(&support::Fixture {
        version: Version(2, 2, 0),
        ..support::Fixture::default()
    });
    assert_eq!(summary(&old, false)["selected_frames"], 3);
    rejected(&old, true);
    let gap = support::replay_bytes(&support::Fixture {
        frame_ids: vec![-123, -121],
        ..support::Fixture::default()
    });
    rejected(&gap, false);
}
