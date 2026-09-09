use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

#[test]
fn archived_replays_preserve_import_summaries_and_reject_unsupported_formats() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slippi");
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    let fixtures = manifest["files"].as_array().unwrap();
    assert_eq!(fixtures.len(), 10);

    for fixture in fixtures {
        let name = fixture["file"].as_str().unwrap();
        let path = directory.join(name);
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            bytes.len() as u64,
            fixture["bytes"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            fixture["sha256"].as_str().unwrap(),
            "{name}"
        );

        let output = Command::new(env!("CARGO_BIN_EXE_skirmish"))
            .arg("inspect-replay")
            .arg(&path)
            .output()
            .unwrap();
        if let Some(expected_error) = fixture["expected_error"].as_str() {
            assert!(!output.status.success(), "{name}");
            assert!(output.stdout.is_empty(), "{name}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(expected_error),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty(), "{name}");
            let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(summary, fixture["expected_summary"], "{name}");
        }
    }
}
