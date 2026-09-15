//! Fresh-process acceptance for the configured verified Pon stdlib facade.

use std::{path::PathBuf, process::Command, time::SystemTime};

use skirmish_script_runtime::{CallbackHandle, CompiledProgram, NativeValue, Value};

const ARCHIVE_SHA256: &str = "5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c";
const CHILD_SUCCESS: &str = "STDLIB_GAMEPLAY_CHILD_SUCCESS";

fn temporary_directory(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("skirmish-stdlib-gameplay-{label}-{stamp}"));
    std::fs::create_dir(&path).unwrap();
    path
}

fn child() {
    let root = temporary_directory("child");

    // This is the complete authoring fixture.  Its dataclasses import is
    // resolved through the configured verified stdlib while the fighter SDK
    // itself comes from the facade's embedded bundle.
    let source = include_str!("../../../scripts/api/tests/fixtures/native_fighter.py");
    let program = CompiledProgram::new(
        source,
        "native_fighter.py",
        [
            CallbackHandle::new("exported_probe"),
            CallbackHandle::new("decorated_bound_callback"),
        ],
    )
    .unwrap();
    program.prepare_for_current_thread().unwrap();

    let metadata = program.export_metadata().unwrap();
    let NativeValue::Dict(metadata) = metadata else {
        panic!("exported fighter definition must be a dictionary");
    };
    assert_eq!(metadata["name"], NativeValue::String("native".into()));

    // Raw export handles invoke named Pon exports directly.  Logical handles
    // returned by bind_callback are reserved for host dispatch.
    let callback = program
        .callback("decorated_bound_callback")
        .expect("registered raw export callback");
    assert_eq!(
        program.invoke_values(&callback, &[Value::Int(40)]).unwrap(),
        NativeValue::Int(41)
    );
    println!("{CHILD_SUCCESS}");
    std::fs::remove_dir_all(root).unwrap();
}

fn child_with_stdlib_env(sha256: Option<&str>) -> std::process::Output {
    let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "fresh_process_configured_stdlib_runs_embedded_fighter_facade",
            "--nocapture",
            "--ignored",
        ])
        .env_clear()
        .env("SKIRMISH_PON_STDLIB_CHILD", "1")
        .env("SKIRMISH_PON_STDLIB_ARCHIVE", archive_path);
    if let Some(sha256) = sha256 {
        command.env("SKIRMISH_PON_STDLIB_SHA256", sha256);
    }
    command.output().unwrap()
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn fresh_process_configured_stdlib_runs_embedded_fighter_facade() {
    if std::env::var_os("SKIRMISH_PON_STDLIB_CHILD").is_some() {
        child();
        return;
    }

    let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap();
    let cwd = temporary_directory("parent");
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fresh_process_configured_stdlib_runs_embedded_fighter_facade",
            "--nocapture",
            "--ignored",
        ])
        .env_clear()
        .env("SKIRMISH_PON_STDLIB_CHILD", "1")
        .env("SKIRMISH_PON_STDLIB_ARCHIVE", archive_path)
        .env("SKIRMISH_PON_STDLIB_SHA256", ARCHIVE_SHA256)
        .env("PON_STDLIB_PATH", cwd.join("does-not-exist"))
        .env("PWD", &cwd)
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "fresh child failed (stdout: {stdout}, stderr: {})",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(CHILD_SUCCESS),
        "callback marker missing: {stdout}"
    );
    assert!(
        stdout.contains("1 passed"),
        "child did not run exactly one test: {stdout}"
    );
    std::fs::remove_dir_all(cwd).unwrap();
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn fresh_process_rejects_wrong_stdlib_hash() {
    let output = child_with_stdlib_env(Some(&"0".repeat(64)));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256"));
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn fresh_process_rejects_missing_stdlib_hash_pair() {
    let output = child_with_stdlib_env(None);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires SKIRMISH_PON_STDLIB_SHA256")
    );
}
