use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "skirmish-pon-frozen-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn prepared_imports_survive_removed_files_and_reject_new_source_before_compile() {
    let _lock = TEST_LOCK.lock().unwrap();
    let root = root();
    let bundle = SourceBundle::new("frozen")
        .with_file("known.py", "value = 41\n")
        .unwrap()
        .with_file("unseen.py", "raise Exception('must not compile')\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\ndef ok():\n    import known\n    return known.value + 1\ndef unseen():\n    import unseen\n    return 0\n";
    let mut program = Program::new(source, "main.py", ["ok", "unseen"])
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(program.invoke("ok", &[]).unwrap(), Value::Int(42));
    let error = program.invoke("unseen", &[]).unwrap_err();
    assert!(error.to_string().contains("new source module"), "{error}");
}

#[test]
fn frozen_dynamic_code_and_swapped_cached_modules_are_rejected() {
    let _lock = TEST_LOCK.lock().unwrap();
    let root = root();
    let bundle = SourceBundle::new("frozen-dynamic")
        .with_file("known.py", "value = 7\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\nimport sys\ndef eval_code():\n    return eval('1 + 1')\ndef exec_code():\n    exec('x = 1')\n    return 1\ndef compile_code():\n    return compile('1 + 1', 'dynamic.py', 'eval')\ndef swap_with_file():\n    class Fake: pass\n    m = Fake()\n    m.__file__ = '/tmp/forged.py'\n    sys.modules['known'] = m\n    import known\n    return known.value\ndef swap_without_file():\n    class Fake: pass\n    sys.modules['known'] = Fake()\n    import known\n    return known.value\n";
    let mut program = Program::new(
        source,
        "main.py",
        [
            "eval_code",
            "exec_code",
            "compile_code",
            "swap_with_file",
            "swap_without_file",
        ],
    )
    .prepare_for_thread_in_bundle(&bundle)
    .unwrap();
    let eval_error = program.invoke("eval_code", &[]).unwrap_err();
    assert!(
        eval_error.to_string().contains("dynamic source"),
        "{eval_error}"
    );
    let exec_error = program.invoke("exec_code", &[]).unwrap_err();
    assert!(
        exec_error.to_string().contains("dynamic source"),
        "{exec_error}"
    );
    let compile_error = program.invoke("compile_code", &[]).unwrap_err();
    assert!(
        compile_error.to_string().contains("dynamic source"),
        "{compile_error}"
    );
    let file_error = program.invoke("swap_with_file", &[]).unwrap_err();
    assert!(file_error.to_string().contains("identity"), "{file_error}");
    let no_file_error = program.invoke("swap_without_file", &[]).unwrap_err();
    assert!(
        no_file_error.to_string().contains("identity"),
        "{no_file_error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_callback_recovers_after_error_and_isolates_second_program() {
    let _lock = TEST_LOCK.lock().unwrap();
    let root = root();
    let bundle = SourceBundle::new("frozen-recovery")
        .with_file("known.py", "value = 5\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\ndef set_value(value):\n    import known\n    known.value = value\n    return value\ndef ok(value):\n    import known\n    return known.value + value\ndef fail():\n    raise Exception('boom')\n";
    let mut first = Program::new(source, "first.py", ["set_value", "ok", "fail"])
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    let mut second = Program::new(source, "second.py", ["set_value", "ok", "fail"])
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    assert_eq!(
        first.invoke("set_value", &[Value::Int(10)]).unwrap(),
        Value::Int(10)
    );
    assert_eq!(
        second.invoke("set_value", &[Value::Int(20)]).unwrap(),
        Value::Int(20)
    );
    assert_eq!(
        first.invoke("ok", &[Value::Int(1)]).unwrap(),
        Value::Int(11)
    );
    assert!(first.invoke("fail", &[]).is_err());
    assert_eq!(
        first.invoke("ok", &[Value::Int(2)]).unwrap(),
        Value::Int(12)
    );
    assert_eq!(
        second.invoke("ok", &[Value::Int(3)]).unwrap(),
        Value::Int(23)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn same_name_load_order_does_not_accept_path_spoofed_cached_module() {
    let _lock = TEST_LOCK.lock().unwrap();
    let root = root();
    let first_bundle = SourceBundle::new("spoof-first")
        .with_file("known.py", "import gc\nvalue = 11\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let second_bundle = SourceBundle::new("spoof-second")
        .with_file("known.py", "import gc\nvalue = 22\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let first_source = "import known\nimport sys\ndef spoof():\n    class Fake: pass\n    m = Fake()\n    m.__file__ = known.__file__\n    sys.modules['known'] = m\n    return 1\ndef read():\n    import known\n    return known.value\ndef collect():\n    import gc\n    gc.collect()\n    import known\n    return known.value\n";
    let mut first = Program::new(first_source, "first.py", ["spoof", "read", "collect"])
        .prepare_for_thread_in_bundle(&first_bundle)
        .unwrap();
    let spoof_error = first.invoke("spoof", &[]).unwrap_err();
    assert!(
        spoof_error.to_string().contains("identity"),
        "{spoof_error}"
    );
    let second_source = "import known\ndef read():\n    import known\n    return known.value\ndef collect():\n    import gc\n    gc.collect()\n    import known\n    return known.value\n";
    let mut second = Program::new(second_source, "second.py", ["read", "collect"])
        .prepare_for_thread_in_bundle(&second_bundle)
        .unwrap();
    assert_eq!(second.invoke("read", &[]).unwrap(), Value::Int(22));
    assert_eq!(first.invoke("read", &[]).unwrap(), Value::Int(11));
    assert_eq!(second.invoke("collect", &[]).unwrap(), Value::Int(22));
    assert_eq!(first.invoke("collect", &[]).unwrap(), Value::Int(11));
    fs::remove_dir_all(root).unwrap();
}
