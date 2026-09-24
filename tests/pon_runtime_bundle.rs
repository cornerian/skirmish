//! Fresh-process acceptance for bundled fighter loading with the verified Pon
//! stdlib.  The roster modules are deliberately loaded through the game
//! `Definition` path: it must preserve each source file's private module
//! identity when it builds the embedded Pon program.

use std::{path::PathBuf, process::Command, time::SystemTime};

use skirmish::game::script::definition::{AssetStore, Definition};
use skirmish_script_runtime::{StandardLibrary, configure_standard_library};

const ARCHIVE: &str = "/tmp/skirmish-stdlib-release-proof/pon-stdlib-final-sorted.tar.gz";
const ARCHIVE_SHA256: &str = "5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c";
const IDENTITY_SHA256: &str = "b14b1766c1138f9aad84664dbbd4f52242b874a1003810a7ec69410cc5f83782";
const SUCCESS: &str = "PON_RUNTIME_FOX_BUNDLE_SUCCESS";

fn digest(value: &str) -> [u8; 32] {
    (0..32)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn temp_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("skirmish-pon-fox-{label}-{stamp}"));
    std::fs::create_dir(&path).unwrap();
    path
}

fn child() {
    let archive = std::fs::File::open(ARCHIVE).expect("verified stdlib archive");
    let library = StandardLibrary::from_archive(archive, digest(ARCHIVE_SHA256)).unwrap();
    assert_eq!(library.identity_digest(), digest(IDENTITY_SHA256));
    let root = temp_dir("stdlib");
    configure_standard_library(library.materialize(root.join("stdlib")).unwrap()).unwrap();

    let source = include_str!("../scripts/fighters/fox.py");
    let definition = Definition::load_registered(source, &AssetStore::builtins())
        .expect("full Fox definition must load with verified stdlib");
    assert_eq!(definition.manifest.name, "fox");
    assert_eq!(definition.manifest.external_ids, vec![2]);
    assert!(definition.manifest.behaviors.len() >= 4);
    for id in ["move_0", "move_1", "move_2", "move_3"] {
        let behavior = definition
            .manifest
            .behaviors
            .iter()
            .find(|behavior| behavior.id.as_deref() == Some(id))
            .expect("canonical special behavior");
        assert!(behavior.validate.is_some());
        assert!(!behavior.callbacks.is_empty());
    }
    for action in [
        "ground_start",
        "ground_loop",
        "ground_end",
        "air_start",
        "air_loop",
        "air_end",
    ] {
        assert!(
            definition
                .manifest
                .behaviors
                .iter()
                .any(|behavior| behavior.actions.contains_key(action)),
            "missing {action}"
        );
    }

    // These two cases exercise the canonical filename boundary that a
    // generic `fighter.py` root used to break.  Yoshi catches the ordinary
    // filename/stem path, while Captain checks the filename-to-CSS alias.
    let yoshi = Definition::load_registered(
        include_str!("../scripts/fighters/yoshi.py"),
        &AssetStore::builtins(),
    )
    .expect("full Yoshi definition must load with verified stdlib");
    assert_eq!(yoshi.manifest.name, "yoshi");
    assert_eq!(yoshi.manifest.external_ids, vec![17]);

    let captain = Definition::load_registered(
        include_str!("../scripts/fighters/captain.py"),
        &AssetStore::builtins(),
    )
    .expect("full Captain definition must load with verified stdlib");
    assert_eq!(captain.manifest.name, "captain-falcon");
    assert_eq!(captain.manifest.external_ids, vec![0]);

    // The provenance marker is an internal linker assignment, not an authoring
    // escape hatch.  A non-builtin source that tries to claim Yoshi must still
    // fail identity resolution rather than inheriting the roster entry.
    let spoof = r#"
from skirmish import Fighter
__skirmish_canonical_module__ = "yoshi"
class Spoof(Fighter):
    pass
"#;
    let error = Definition::load_registered(spoof, &AssetStore::builtins())
        .expect_err("external source must not spoof builtin roster identity");
    assert!(
        error
            .to_string()
            .contains("needs an explicit name or external_ids"),
        "spoof should fail closed at identity resolution: {error}"
    );

    println!("{SUCCESS}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn fresh_process_loads_complete_fox_definition_with_verified_bundle() {
    if std::env::var_os("SKIRMISH_PON_FOX_CHILD").is_some() {
        child();
        return;
    }
    let cwd = temp_dir("cwd");
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fresh_process_loads_complete_fox_definition_with_verified_bundle",
            "--ignored",
            "--nocapture",
        ])
        .env_clear()
        .env("SKIRMISH_PON_FOX_CHILD", "1")
        .env("PON_STDLIB_PATH", cwd.join("does-not-exist"))
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "fresh child failed: stdout={stdout}, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains(SUCCESS), "success marker missing: {stdout}");
    assert!(
        stdout.contains("1 passed"),
        "child did not run exactly one test: {stdout}"
    );
    std::fs::remove_dir_all(cwd).unwrap();
}
