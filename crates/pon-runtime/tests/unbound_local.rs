//! Regression for exception construction from a missing local.

use skirmish_pon_runtime::Program;

#[test]
fn unbound_local_error_returns_without_reentering_runtime_lock() {
    let source = r#"
def probe():
    if False:
        value = 1
    return value
"#;
    let mut program = Program::new(source, "unbound-local.py", ["probe"])
        .prepare_for_thread()
        .expect("program should prepare");
    let error = program.invoke("probe", &[]).expect_err("probe must fail");
    let text = error.to_string();
    assert!(text.contains("UnboundLocalError"), "{text}");
    assert!(text.contains("cannot access local variable"), "{text}");
}
