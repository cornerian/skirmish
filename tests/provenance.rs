use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Snapshot {
    fixture: String,
    upstream: String,
    sha256: String,
}

fn collect_fixtures(root: &Path, directory: &Path, fixtures: &mut BTreeSet<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect_fixtures(root, &path, fixtures);
        } else {
            assert!(
                kind.is_file(),
                "snapshot must be a regular file: {}",
                path.display()
            );
            fixtures.insert(path.strip_prefix(root).unwrap().to_owned());
        }
    }
}

#[test]
fn original_c_snapshots_match_the_pinned_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: Vec<Snapshot> =
        serde_json::from_slice(&fs::read(root.join("tests/oracle/sources.json")).unwrap()).unwrap();
    assert!(!manifest.is_empty());
    let mut listed = BTreeSet::new();
    for entry in &manifest {
        assert!(
            listed.insert(PathBuf::from(&entry.fixture)),
            "duplicate fixture: {}",
            entry.fixture
        );
    }
    let mut actual = BTreeSet::new();
    collect_fixtures(root, &root.join("tests/oracle/original"), &mut actual);
    assert_eq!(
        listed, actual,
        "manifest must list every original snapshot exactly once"
    );
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
