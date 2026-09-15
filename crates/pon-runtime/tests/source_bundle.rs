use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn materialized_package_is_imported_by_pon() {
    let _guard = TEST_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!(
        "skirmish-pon-bundle-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bundle = SourceBundle::new("test-v1")
        .with_file("fighter_api/__init__.py", "name = \"fighter_api\"\n")
        .unwrap()
        .with_file("fighter_api/core.py", "value = 12\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "from fighter_api.core import value\ndef invoke():\n    return value\n";
    let mut program = Program::new(source, "mod.py", ["invoke"])
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    assert_eq!(program.invoke("invoke", &[]).unwrap(), Value::Int(12));
    fs::remove_dir_all(root).unwrap();
}
