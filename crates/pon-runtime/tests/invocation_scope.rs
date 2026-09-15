use std::{
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "skirmish-pon-invocation-scope-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn scoped_callbacks_observe_state_from_previous_callbacks() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut program = Program::new(
        "counter = 0\ndef increment(value):\n    global counter\n    counter = counter + value\n    return counter\ndef read(value):\n    return counter\n",
        "invocation-scope-state.py",
        ["increment", "read"],
    )
    .prepare_for_thread()
    .unwrap();
    let increment = program.callback_index("increment").unwrap();
    let read = program.callback_index("read").unwrap();

    let values = program
        .with_invocation_scope(|scope| {
            let first = scope.invoke_index(increment, &[Value::Int(2)])?;
            let second = scope.invoke_index(read, &[Value::Int(0)])?;
            Ok((first, second))
        })
        .unwrap();
    assert_eq!(values, (Value::Int(2), Value::Int(2)));
}

#[test]
fn scope_restores_guards_when_closure_returns_error() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut program = Program::new(
        "def fail(value):\n    raise Exception('scope failure')\ndef echo(value):\n    return value\n",
        "invocation-scope-error.py",
        ["fail", "echo"],
    )
    .prepare_for_thread()
    .unwrap();
    let fail = program.callback_index("fail").unwrap();
    let echo = program.callback_index("echo").unwrap();

    let error = program
        .with_invocation_scope(|scope| {
            scope.invoke_index(fail, &[Value::Int(0)])?;
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("scope failure"));
    assert_eq!(
        program.invoke_index(echo, &[Value::Int(9)]).unwrap(),
        Value::Int(9)
    );
}

#[test]
fn separate_program_scopes_remain_isolated_when_interleaved() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut first = Program::new(
        "value = 10\ndef read(arg):\n    return value\n",
        "invocation-scope-first.py",
        ["read"],
    )
    .prepare_for_thread()
    .unwrap();
    let mut second = Program::new(
        "value = 20\ndef read(arg):\n    return value\n",
        "invocation-scope-second.py",
        ["read"],
    )
    .prepare_for_thread()
    .unwrap();
    let first_read = first.callback_index("read").unwrap();
    let second_read = second.callback_index("read").unwrap();

    let first_value = first
        .with_invocation_scope(|scope| scope.invoke_index(first_read, &[Value::Int(0)]))
        .unwrap();
    let second_value = second
        .with_invocation_scope(|scope| scope.invoke_index(second_read, &[Value::Int(0)]))
        .unwrap();
    assert_eq!(first_value, Value::Int(10));
    assert_eq!(second_value, Value::Int(20));
}

#[test]
fn forced_gc_between_scoped_callbacks_preserves_values() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let mut program = Program::new(
        "saved = None\ndef store(value):\n    global saved\n    saved = [value]\n    return 0\ndef collect_and_read(value):\n    import gc\n    gc.collect()\n    return saved\n",
        "invocation-scope-gc.py",
        ["store", "collect_and_read"],
    )
    .prepare_for_thread()
    .unwrap();
    let store = program.callback_index("store").unwrap();
    let collect_and_read = program.callback_index("collect_and_read").unwrap();

    let result = program
        .with_invocation_scope(|scope| {
            scope.invoke_index(store, &[Value::String("heap-value".into())])?;
            scope.invoke_index(collect_and_read, &[Value::Int(0)])
        })
        .unwrap();
    assert_eq!(
        result,
        Value::List(vec![Value::String("heap-value".into())])
    );
}

#[test]
fn bundled_scope_latches_frozen_identity_error_and_restores_namespace() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = root();
    let bundle = SourceBundle::new("scope-bundle")
        .with_file("known.py", "value = 41\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\nimport sys\ndef swap(value):\n    class Fake: pass\n    sys.modules['known'] = Fake()\n    import known\n    return 0\ndef read(value):\n    import known\n    return known.value\ndef store(value):\n    global saved\n    saved = [value]\n    return saved\n";
    let mut program = Program::new(source, "scope-bundle-main.py", ["swap", "read", "store"])
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    let swap = program.callback_index("swap").unwrap();
    let read = program.callback_index("read").unwrap();
    let store = program.callback_index("store").unwrap();

    let error = program
        .with_invocation_scope(|scope| {
            let first = scope.invoke_index(swap, &[Value::Int(0)]).unwrap_err();
            let second = scope.invoke_index(read, &[Value::Int(0)]).unwrap_err();
            assert_eq!(first.to_string(), second.to_string());
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("identity"), "{error}");
    assert_eq!(
        program.invoke_index(read, &[Value::Int(0)]).unwrap(),
        Value::Int(41)
    );
    assert_eq!(
        program
            .invoke_index(store, &[Value::String("heap-value".into())])
            .unwrap(),
        Value::List(vec![Value::String("heap-value".into())])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bundled_scopes_restore_owned_modules_between_programs() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = root();
    let first_bundle = SourceBundle::new("scope-first")
        .with_file("known.py", "value = 11\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let second_bundle = SourceBundle::new("scope-second")
        .with_file("known.py", "value = 22\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\ndef read(value):\n    import known\n    return known.value\n";
    let mut first = Program::new(source, "scope-first.py", ["read"])
        .prepare_for_thread_in_bundle(&first_bundle)
        .unwrap();
    let mut second = Program::new(source, "scope-second.py", ["read"])
        .prepare_for_thread_in_bundle(&second_bundle)
        .unwrap();
    let first_read = first.callback_index("read").unwrap();
    let second_read = second.callback_index("read").unwrap();
    let first_value = first
        .with_invocation_scope(|scope| scope.invoke_index(first_read, &[Value::Int(0)]))
        .unwrap();
    let second_value = second
        .with_invocation_scope(|scope| scope.invoke_index(second_read, &[Value::Int(0)]))
        .unwrap();
    let first_again = first
        .with_invocation_scope(|scope| scope.invoke_index(first_read, &[Value::Int(0)]))
        .unwrap();
    assert_eq!(
        (first_value, second_value, first_again),
        (Value::Int(11), Value::Int(22), Value::Int(11))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn callback_error_wins_over_frozen_identity_error_and_latches() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let root = root();
    let bundle = SourceBundle::new("scope-error-precedence")
        .with_file("known.py", "value = 41\n")
        .unwrap()
        .materialize(&root)
        .unwrap();
    let source = "import known\nimport sys\ndef mutate_then_fail(value):\n    class Fake: pass\n    sys.modules['known'] = Fake()\n    raise Exception('original callback cause')\ndef read(value):\n    import known\n    return known.value\n";
    let mut program = Program::new(
        source,
        "scope-error-precedence.py",
        ["mutate_then_fail", "read"],
    )
    .prepare_for_thread_in_bundle(&bundle)
    .unwrap();
    let fail = program.callback_index("mutate_then_fail").unwrap();
    let read = program.callback_index("read").unwrap();

    let error = program
        .with_invocation_scope(|scope| {
            let first = scope.invoke_index(fail, &[Value::Int(0)]).unwrap_err();
            assert!(
                first.to_string().contains("original callback cause"),
                "{first}"
            );
            let second = scope.invoke_index(read, &[Value::Int(0)]).unwrap_err();
            assert_eq!(first.to_string(), second.to_string());
            Ok(())
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("original callback cause"),
        "{error}"
    );
    assert_eq!(
        program.invoke_index(read, &[Value::Int(0)]).unwrap(),
        Value::Int(41)
    );
    fs::remove_dir_all(root).unwrap();
}
