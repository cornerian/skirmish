//! Conformance proof for the public class based fighter package under Pon.
//!
//! Every authoring module is embedded into the test binary. The test exercises
//! the pinned Pon importer and native execution without CPython or an installed
//! `fighter_api` package. Pon's compile-time stdlib/vendor path remains an
//! explicit runtime prerequisite; this fixture does not package those assets.

use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn authoring_bundle() -> SourceBundle {
    SourceBundle::new("fighter-api-conformance-v1")
        .with_file(
            "fighter/actions.py",
            include_str!("../../../scripts/api/fighter/actions.py"),
        )
        .unwrap()
        .with_file(
            "fighter/transitions.py",
            include_str!("../../../scripts/api/fighter/transitions.py"),
        )
        .unwrap()
        .with_file(
            "fighter/__init__.py",
            include_str!("../../../scripts/api/fighter/__init__.py"),
        )
        .unwrap()
        .with_file(
            "fighter/api.py",
            include_str!("../../../scripts/api/fighter/api.py"),
        )
        .unwrap()
        .with_file(
            "fighter/compat.py",
            include_str!("../../../scripts/api/fighter/compat.py"),
        )
        .unwrap()
        .with_file(
            "fighter/events.py",
            include_str!("../../../scripts/api/fighter/events.py"),
        )
        .unwrap()
        .with_file(
            "fighter/registry.py",
            include_str!("../../../scripts/api/fighter/registry.py"),
        )
        .unwrap()
        .with_file(
            "skirmish/__init__.py",
            include_str!("../../../scripts/api/skirmish/__init__.py"),
        )
        .unwrap()
        .with_file(
            "skirmish/api.py",
            include_str!("../../../scripts/api/skirmish/api.py"),
        )
        .unwrap()
        .with_file(
            "skirmish/events.py",
            include_str!("../../../scripts/api/skirmish/events.py"),
        )
        .unwrap()
        .with_file(
            "skirmish/registry.py",
            include_str!("../../../scripts/api/skirmish/registry.py"),
        )
        .unwrap()
        .with_file(
            "native_fighter.py",
            include_str!("../../../scripts/api/tests/fixtures/native_fighter.py"),
        )
        .unwrap()
}

#[test]
fn whole_public_fighter_package_conforms_to_native_pon() {
    let _guard = TEST_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!(
        "skirmish-pon-fighter-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bundle = authoring_bundle().materialize(&root).unwrap();
    let source = "from native_fighter import exported_probe, decorated_bound_callback, incomplete_is_rejected\n";
    let mut program = Program::new(
        source,
        "fighter_package.py",
        [
            "exported_probe",
            "decorated_bound_callback",
            "incomplete_is_rejected",
        ],
    )
    .prepare_for_thread_in_bundle(&bundle)
    .expect("the complete public fighter package must compile under pinned Pon");

    assert_eq!(
        program.invoke("exported_probe", &[]).unwrap(),
        Value::List(vec![
            Value::String("native".into()),
            Value::String("native_laser".into()),
            Value::Bool(true),
            Value::Bool(true),
            Value::Bool(true),
            Value::Bool(true),
            Value::Bool(true),
            Value::Int(4),
            Value::Int(41),
        ])
    );
    assert_eq!(
        program
            .invoke("decorated_bound_callback", &[Value::Int(41)])
            .unwrap(),
        Value::Int(42)
    );
    assert_eq!(
        program.invoke("incomplete_is_rejected", &[]).unwrap(),
        Value::Bool(true)
    );

    drop(program);
    fs::remove_dir_all(root).unwrap();
}
