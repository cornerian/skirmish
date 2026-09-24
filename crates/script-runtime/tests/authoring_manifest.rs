#![allow(dead_code)]

#[path = "../build_support.rs"]
mod build_support;

use std::path::Path;

#[test]
fn manifest_rendering_is_sorted_and_escapes_source_literals() {
    let rendered = build_support::render_manifest(vec![
        build_support::AuthoringFile {
            relative: "fighter/z.py".into(),
            path: "fighter/z.py".into(),
            source: "value = \"z\"\n".into(),
        },
        build_support::AuthoringFile {
            relative: "fighter/a.py".into(),
            path: "fighter/a.py".into(),
            source: "value = \"a\"\nvalue = '\\n'\n".into(),
        },
    ]);
    assert!(rendered.find("fighter/a.py").unwrap() < rendered.find("fighter/z.py").unwrap());
    assert!(rendered.contains("value = \\\"z\\\"\\n"));
}

#[test]
#[should_panic(expected = "non-printable or unsupported")]
fn malformed_path_component_is_rejected() {
    let component = Path::new("bad\nname.py").components().next().unwrap();
    let _ = build_support::validated_path_component(component);
}

#[test]
#[should_panic(expected = "outside")]
fn source_root_escape_is_rejected() {
    build_support::assert_within(
        Path::new("/workspace/escape"),
        Path::new("/workspace/repository"),
        "source root must remain inside repository",
    );
}

#[cfg(unix)]
#[test]
#[should_panic(expected = "contains symlink")]
fn symlink_entries_are_rejected_before_reading() {
    use std::{fs, os::unix::fs::symlink, time::SystemTime};

    let root = std::env::temp_dir().join(format!(
        "skirmish-authoring-manifest-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target.py");
    fs::write(&target, "value = 1\n").unwrap();
    let link = root.join("link.py");
    symlink(&target, &link).unwrap();
    let _ = build_support::checked_metadata(&link, "test entry");
    let _ = fs::remove_dir_all(root);
}
