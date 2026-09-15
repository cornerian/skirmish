use flate2::{Compression, write::GzEncoder};
use serde_json::json;
use sha2::{Digest, Sha256};
use skirmish_pon_runtime::StandardLibrary;
use std::collections::BTreeMap;
use tar::{Builder, Header};

const PON: &str = "ab9067dbd2899c64c4d67a4bc27b8ad49472b126";
const SOURCE: &[u8] = b"VALUE = 1\n";
const LICENSE: &[u8] = b"license\n";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn manifest(version: &str, source: &[u8], license: &[u8]) -> Vec<u8> {
    let source_record =
        json!({"path":"dataclasses.py", "bytes":source.len(), "sha256":digest(source)});
    let license_record = json!({"path":"LICENSE", "bytes":license.len(), "sha256":digest(license)});
    let identity_payload = BTreeMap::from([
        ("cpython_revision", json!(version)),
        ("files", json!([source_record])),
        ("license", json!(license_record)),
        ("pon_revision", json!(PON)),
    ]);
    let mut identity_input = serde_json::to_vec(&identity_payload).unwrap();
    identity_input.push(b'\n');
    serde_json::to_vec(&json!({
        "format":"skirmish-pon-stdlib-v1", "pon_revision":PON, "cpython_revision":version,
        "root":"stdlib", "file_count":1, "license":license_record, "files":[source_record],
        "identity":digest(&identity_input), "excluded":{"directories":[],"suffixes":[],"files":[]}
    }))
    .unwrap()
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let gzip = GzEncoder::new(&mut output, Compression::default());
        let mut tar = Builder::new(gzip);
        for (path, bytes) in entries {
            let mut header = Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, *path, *bytes).unwrap();
        }
        tar.finish().unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    output
}

fn valid_archive() -> Vec<u8> {
    let manifest = manifest("v3.14.0", SOURCE, LICENSE);
    archive(&[
        ("manifest.json", &manifest),
        ("LICENSE", LICENSE),
        ("stdlib/dataclasses.py", SOURCE),
    ])
}

fn checked(bytes: &[u8]) -> StandardLibrary {
    StandardLibrary::from_archive(bytes, Sha256::digest(bytes).into()).unwrap()
}

#[test]
fn valid_bundle_verifies_identity_and_materializes() {
    let bytes = valid_archive();
    let library = checked(&bytes);
    assert_ne!(library.identity_digest(), [0; 32]);
    let root = tempfile::tempdir().unwrap();
    let materialized = library.materialize(root.path()).unwrap();
    assert_eq!(
        std::fs::read(materialized.root().join("dataclasses.py")).unwrap(),
        SOURCE
    );
}

#[test]
fn wrong_archive_hash_is_rejected() {
    let bytes = valid_archive();
    assert!(StandardLibrary::from_archive(bytes.as_slice(), [0; 32]).is_err());
}

#[test]
fn missing_license_tampered_source_duplicate_and_traversal_are_rejected() {
    let manifest = manifest("v3.14.0", SOURCE, LICENSE);
    let cases = [
        (
            vec![
                ("manifest.json", manifest.as_slice()),
                ("stdlib/dataclasses.py", SOURCE),
            ],
            "missing LICENSE",
        ),
        (
            vec![
                ("manifest.json", manifest.as_slice()),
                ("LICENSE", LICENSE),
                ("stdlib/dataclasses.py", b"bad\n"),
            ],
            "hash or size mismatch",
        ),
        (
            vec![
                ("manifest.json", manifest.as_slice()),
                ("LICENSE", LICENSE),
                ("stdlib/dataclasses.py", SOURCE),
                ("stdlib/dataclasses.py", SOURCE),
            ],
            "duplicate archive entry",
        ),
        (
            vec![
                ("manifest.json", manifest.as_slice()),
                ("LICENSE", LICENSE),
                ("stdlib\\escape.py", SOURCE),
            ],
            "unsafe archive path",
        ),
    ];
    for (entries, expected) in cases {
        let bytes = archive(&entries);
        let error = checked_result(&bytes).expect_err("malformed archive must fail");
        assert!(
            error.to_string().contains(expected),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn version_misalignment_is_rejected() {
    let manifest = manifest("v3.14.1", SOURCE, LICENSE);
    let bytes = archive(&[
        ("manifest.json", &manifest),
        ("LICENSE", LICENSE),
        ("stdlib/dataclasses.py", SOURCE),
    ]);
    let error = checked_result(&bytes).expect_err("misaligned version must fail");
    assert!(
        error
            .to_string()
            .contains("unsupported Pon or CPython release")
    );
}

#[test]
#[ignore = "requires an explicitly supplied release artifact"]
fn finalized_packager_artifact_verifies() {
    let path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").expect("archive path");
    let expected = std::env::var("SKIRMISH_PON_STDLIB_SHA256").expect("archive SHA-256");
    let expected_identity = std::env::var("SKIRMISH_PON_STDLIB_IDENTITY").expect("identity");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(digest(&bytes), expected);
    let library = StandardLibrary::from_archive(&bytes[..], Sha256::digest(&bytes).into()).unwrap();
    assert_eq!(
        library
            .identity_digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        expected_identity
    );
}

fn checked_result(bytes: &[u8]) -> Result<StandardLibrary, skirmish_pon_runtime::Error> {
    StandardLibrary::from_archive(bytes, Sha256::digest(bytes).into())
}
