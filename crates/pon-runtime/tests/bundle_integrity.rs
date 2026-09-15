use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Error, SourceBundle};

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "skirmish-pon-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn same_version_different_bytes_get_distinct_materializations() {
    let root = temp_root("digest");
    let first = SourceBundle::new("same-version")
        .with_file("mod.py", "value = 1\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let second = SourceBundle::new("same-version")
        .with_file("mod.py", "value = 2\n")
        .unwrap()
        .materialize(&root)
        .unwrap();

    assert_ne!(first.root(), second.root());
    assert_eq!(
        fs::read_to_string(first.root().join("mod.py")).unwrap(),
        "value = 1\n"
    );
    assert_eq!(
        fs::read_to_string(second.root().join("mod.py")).unwrap(),
        "value = 2\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bundle_paths_must_be_canonical_safe_relative_paths() {
    for path in [
        "",
        ".",
        "./mod.py",
        "pkg//mod.py",
        "../mod.py",
        "/tmp/mod.py",
        "pkg\\mod.py",
    ] {
        let result = SourceBundle::new("v").with_file(path, "value = 1\n");
        assert!(matches!(result, Err(Error::Value(_))), "accepted {path:?}");
    }
}

#[cfg(unix)]
#[test]
fn from_directory_rejects_symlink_entries() {
    use std::os::unix::fs::symlink;

    let root = temp_root("symlink");
    let outside = temp_root("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("escape.py"), "value = 9\n").unwrap();
    symlink(outside.join("escape.py"), root.join("escape.py")).unwrap();
    let result = SourceBundle::from_directory("v", &root);
    assert!(matches!(result, Err(Error::Io(_))));
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn existing_cache_is_verified_before_reuse() {
    let root = temp_root("corrupt");
    let bundle = SourceBundle::new("v")
        .with_file("mod.py", "value = 1\n")
        .unwrap();
    let materialized = bundle.materialize(&root).unwrap();
    fs::write(materialized.root().join("mod.py"), "value = 99\n").unwrap();
    assert!(matches!(bundle.materialize(&root), Err(Error::Io(_))));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_first_publication_produces_one_valid_bundle() {
    let root = temp_root("concurrent");
    let bundle = Arc::new(
        SourceBundle::new("v")
            .with_file("pkg/__init__.py", "value = 1\n")
            .unwrap()
            .with_file("pkg/mod.py", "value = 2\n")
            .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let bundle = Arc::clone(&bundle);
        let barrier = Arc::clone(&barrier);
        let root = root.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            bundle.materialize(root).unwrap()
        }));
    }
    let first = handles.remove(0).join().unwrap();
    let second = handles.remove(0).join().unwrap();
    assert_eq!(first.root(), second.root());
    assert_eq!(
        fs::read_to_string(first.root().join("pkg/mod.py")).unwrap(),
        "value = 2\n"
    );
    fs::remove_dir_all(root).unwrap();
}
