use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn temp_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "skirmish-pon-isolation-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn same_named_modules_are_isolated_between_materialized_bundles() {
    let _guard = TEST_LOCK.lock().unwrap();
    let root = temp_root();
    let first_bundle = SourceBundle::new("first")
        .with_file(
            "shared.py",
            "import _weakref\ncounter = 10\nold_marker = lambda: 1\nold_ref = _weakref.ref(old_marker)\ndef bump():\n    global counter\n    counter = counter + 1\n    return counter\ndef replace_marker():\n    global old_marker\n    old_marker = lambda: 2\n    return 1\ndef released_marker():\n    import gc\n    gc.collect()\n    return old_ref() is None\n",
        )
        .unwrap()
        .with_file("alias.py", "import shared\ndef read():\n    return shared.bump()\ndef replace_marker():\n    return shared.replace_marker()\ndef released_marker():\n    return shared.released_marker()\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let second_bundle = SourceBundle::new("second")
        .with_file(
            "shared.py",
            "import _weakref\ncounter = 20\nold_marker = lambda: 1\nold_ref = _weakref.ref(old_marker)\ndef bump():\n    global counter\n    counter = counter + 1\n    return counter\ndef replace_marker():\n    global old_marker\n    old_marker = lambda: 2\n    return 1\ndef released_marker():\n    import gc\n    gc.collect()\n    return old_ref() is None\n",
        )
        .unwrap()
        .with_file("alias.py", "import shared\ndef read():\n    return shared.bump()\ndef replace_marker():\n    return shared.replace_marker()\ndef released_marker():\n    return shared.released_marker()\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "from alias import read as eager\ndef lazy():\n    import alias\n    return alias.read()\ndef collect_lazy():\n    import alias\n    import gc\n    gc.collect()\n    return alias.read()\ndef replace_marker():\n    import alias\n    return alias.replace_marker()\ndef released_marker():\n    import alias\n    return alias.released_marker()\ndef fail():\n    import alias\n    alias.read()\n    return 1 / 0\n";

    let mut first = Program::new(
        source,
        "first.py",
        [
            "eager",
            "lazy",
            "collect_lazy",
            "replace_marker",
            "released_marker",
            "fail",
        ],
    )
    .prepare_for_thread_in_bundle(&first_bundle)
    .expect("first program compiles");
    let mut second = Program::new(
        source,
        "second.py",
        [
            "eager",
            "lazy",
            "collect_lazy",
            "replace_marker",
            "released_marker",
            "fail",
        ],
    )
    .prepare_for_thread_in_bundle(&second_bundle)
    .expect("second program compiles");

    assert_eq!(first.invoke("eager", &[]).unwrap(), Value::Int(11));
    assert_eq!(first.invoke("lazy", &[]).unwrap(), Value::Int(12));
    assert_eq!(second.invoke("eager", &[]).unwrap(), Value::Int(21));
    assert_eq!(second.invoke("lazy", &[]).unwrap(), Value::Int(22));
    assert_eq!(first.invoke("collect_lazy", &[]).unwrap(), Value::Int(13));
    assert_eq!(second.invoke("collect_lazy", &[]).unwrap(), Value::Int(23));
    assert_eq!(first.invoke("replace_marker", &[]).unwrap(), Value::Int(1));
    assert_eq!(second.invoke("replace_marker", &[]).unwrap(), Value::Int(1));
    assert_eq!(
        first.invoke("released_marker", &[]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        second.invoke("released_marker", &[]).unwrap(),
        Value::Bool(true)
    );
    let first_error = first.invoke("fail", &[]).unwrap_err();
    assert!(first_error.to_string().contains("zero"), "{first_error}");
    assert_eq!(first.invoke("lazy", &[]).unwrap(), Value::Int(15));
    let second_error = second.invoke("fail", &[]).unwrap_err();
    assert!(second_error.to_string().contains("zero"), "{second_error}");
    assert_eq!(second.invoke("lazy", &[]).unwrap(), Value::Int(25));
    fs::remove_dir_all(root).unwrap();
}
