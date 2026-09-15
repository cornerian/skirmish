//! Prepared-program namespace and lifetime isolation coverage.

use std::sync::Mutex;

use skirmish_pon_runtime::{Program, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn prepared_programs_keep_independent_callbacks_after_drop() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut first = Program::new(
        "offset = 10\ncounter = 0\ndef action(value):\n    global counter\n    counter = counter + 1\n    return value + offset + counter\n",
        "module-first.py",
        ["action"],
    )
    .prepare_for_thread()
    .expect("first compile");
    let mut second = Program::new(
        "offset = 100\ncounter = 0\ndef action(value):\n    global counter\n    counter = counter + 1\n    return value + offset + counter\n",
        "module-second.py",
        ["action"],
    )
    .prepare_for_thread()
    .expect("second compile");

    assert_eq!(
        first.invoke("action", &[Value::Int(1)]).unwrap(),
        Value::Int(12)
    );
    assert_eq!(
        second.invoke("action", &[Value::Int(1)]).unwrap(),
        Value::Int(102)
    );

    drop(first);
    assert_eq!(
        second.invoke("action", &[Value::Int(2)]).unwrap(),
        Value::Int(104)
    );
}

#[test]
fn dropping_a_program_does_not_leave_dangling_jit_values_in_runtime_roots() {
    let _guard = TEST_LOCK.lock().unwrap();
    let first = Program::new(
        "marker = 'jit-owned-module-marker'\ndef action(value):\n    return marker\n",
        "module-jit-lifetime.py",
        ["action"],
    )
    .prepare_for_thread()
    .expect("first compile");
    drop(first);

    // The first module remains in Pon's import/module registry. A collection
    // must therefore not traverse string constants whose backing JIT module
    // has already been freed. This also verifies that dropping one handle does
    // not invalidate the runtime while another prepared session is active.
    let mut second = Program::new(
        "import gc\ndef collect(value):\n    gc.collect()\n    return value\n",
        "module-jit-lifetime-probe.py",
        ["collect"],
    )
    .prepare_for_thread()
    .expect("probe compile");
    assert_eq!(
        second.invoke("collect", &[Value::Int(7)]).unwrap(),
        Value::Int(7)
    );
}

#[test]
fn callback_slot_retains_callable_after_module_rebinding_and_gc() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut program = Program::new(
        "def target():\n    return 42\ndef replace():\n    global target\n    target = None\n    import gc\n    gc.collect()\n    return 0\n",
        "module-callback-slot-lifetime.py",
        ["target", "replace"],
    )
    .prepare_for_thread()
    .expect("compile callback slot lifetime probe");

    let target_slot = program
        .callback_index("target")
        .expect("target callback should have a stable slot");
    assert_eq!(program.invoke("replace", &[]).unwrap(), Value::Int(0));
    assert_eq!(
        program.invoke_index(target_slot, &[]).unwrap(),
        Value::Int(42),
        "the retained callback root must outlive replacement of its module global"
    );

    let error = program.invoke_index(usize::MAX, &[]).unwrap_err();
    assert!(
        error.to_string().contains("slot"),
        "invalid callback slots must be reported as callback errors: {error}"
    );
}
