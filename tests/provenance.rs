use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Snapshot {
    fixture: String,
    upstream: String,
    sha256: String,
}

#[test]
fn original_c_snapshots_match_the_pinned_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: Vec<Snapshot> =
        serde_json::from_slice(&fs::read(root.join("tests/oracle/sources.json")).unwrap()).unwrap();
    assert!(!manifest.is_empty());
    for entry in manifest {
        assert!(entry.upstream.starts_with("src/") || entry.upstream.starts_with("extern/"));
        let bytes = fs::read(root.join(&entry.fixture)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            entry.sha256,
            "{} was changed from {}",
            entry.fixture,
            entry.upstream
        );
    }
}
