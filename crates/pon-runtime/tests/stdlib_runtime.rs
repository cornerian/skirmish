use std::process::Command;

use skirmish_pon_runtime::{Program, SourceBundle, StandardLibrary, Value};

fn sdk_bundle() -> SourceBundle {
    SourceBundle::new("fighter-api-stdlib-runtime-proof-v1")
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
            "fighter/roster.py",
            include_str!("../../../scripts/api/fighter/roster.py"),
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
            "fighter/math.py",
            include_str!("../../../scripts/api/fighter/math.py"),
        )
        .unwrap()
        .with_file(
            "fighter/registry.py",
            include_str!("../../../scripts/api/fighter/registry.py"),
        )
        .unwrap()
        .with_file(
            "fighter/standard.py",
            include_str!("../../../scripts/api/fighter/standard.py"),
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
            "skirmish/_loader.py",
            include_str!("../../../scripts/api/skirmish/_loader.py"),
        )
        .unwrap()
        .with_file(
            "skirmish/_native.py",
            include_str!("../../../scripts/api/skirmish/_native.py"),
        )
        .unwrap()
        .with_file(
            "native_fighter.py",
            include_str!("../../../scripts/api/tests/fixtures/native_fighter.py"),
        )
        .unwrap()
}

fn child() -> ! {
    let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap();
    let expected = std::env::var("SKIRMISH_PON_STDLIB_SHA256").unwrap();
    let expected: [u8; 32] = (0..32)
        .map(|i| u8::from_str_radix(&expected[i * 2..i * 2 + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let archive = std::fs::read(archive_path).unwrap();
    let stdlib = StandardLibrary::from_archive(&archive[..], expected).unwrap();
    let expected_identity = std::env::var("SKIRMISH_PON_STDLIB_IDENTITY").unwrap();
    let expected_identity: [u8; 32] = (0..32)
        .map(|i| u8::from_str_radix(&expected_identity[i * 2..i * 2 + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(stdlib.identity_digest(), expected_identity);
    let root = tempfile::tempdir().unwrap();
    let stdlib = stdlib.materialize(root.path().join("stdlib")).unwrap();
    let sdk = sdk_bundle().materialize(root.path().join("sdk")).unwrap();
    let source = "from dataclasses import dataclass\nimport dataclasses\nfrom fighter.api import Fighter\n@dataclass\nclass Probe:\n    value: int\ndef callback(value):\n    return [dataclasses.__file__, Probe(value).value]\n";
    let mut program = Program::new(source, "stdlib_runtime_probe.py", ["callback"])
        .with_standard_library(&stdlib)
        .prepare_for_thread_in_bundle(&sdk)
        .unwrap();
    let result = program.invoke("callback", &[Value::Int(37)]).unwrap();
    let Value::List(values) = result else {
        panic!("callback result must be a list")
    };
    assert_eq!(values[1], Value::Int(37));
    let Value::String(file) = &values[0] else {
        panic!("dataclasses.__file__ must be a string")
    };
    assert!(
        file.starts_with(stdlib.root().to_str().unwrap()),
        "dataclasses escaped verified stdlib root: {file}"
    );
    println!("STDLIB_RUNTIME_CHILD_SUCCESS");
    std::process::exit(0);
}

#[test]
#[ignore = "requires explicitly supplied finalized stdlib artifact"]
fn standalone_child_uses_verified_stdlib_and_embedded_sdk() {
    if std::env::var_os("SKIRMISH_PON_STDLIB_CHILD").is_some() {
        child();
    }
    let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap();
    let expected = std::env::var("SKIRMISH_PON_STDLIB_SHA256").unwrap();
    let identity = std::env::var("SKIRMISH_PON_STDLIB_IDENTITY").unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let output = Command::new(executable)
        .arg("--exact")
        .arg("standalone_child_uses_verified_stdlib_and_embedded_sdk")
        .arg("--nocapture")
        .arg("--ignored")
        .env_clear()
        .env("SKIRMISH_PON_STDLIB_CHILD", "1")
        .env("SKIRMISH_PON_STDLIB_ARCHIVE", archive_path)
        .env("SKIRMISH_PON_STDLIB_SHA256", expected)
        .env("SKIRMISH_PON_STDLIB_IDENTITY", identity)
        .env("PON_STDLIB_PATH", cwd.path().join("does-not-exist"))
        .env("PWD", cwd.path())
        .current_dir(cwd.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("STDLIB_RUNTIME_CHILD_SUCCESS"),
        "child did not execute callback: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// A module placed directly in `sys.modules` bypasses the source loader.  The
/// strict import policy must evict it for the next callback and let the import
/// fail. Restoration is scoped to guard teardown and is covered by the cache
/// guard's internal state transition; every callback itself enters a new guard.
#[test]
#[ignore = "requires explicitly supplied finalized stdlib artifact"]
fn strict_policy_evicts_warm_foreign_module_from_cache() {
    let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap_or_else(|_| {
        "/tmp/skirmish-stdlib-release-proof/pon-stdlib-final-sorted.tar.gz".into()
    });
    let archive = std::fs::read(archive_path).unwrap();
    let expected = (0..32)
        .map(|i| {
            u8::from_str_radix(
                "5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c"
                    [i * 2..i * 2 + 2]
                    .as_ref(),
                16,
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let stdlib = StandardLibrary::from_archive(&archive[..], expected)
        .unwrap()
        .materialize(root.path().join("stdlib"))
        .unwrap();
    let bundle_root = tempfile::tempdir().unwrap();
    let bundle = SourceBundle::new("warm-cache-policy-test")
        .materialize(bundle_root.path().join("bundle"))
        .unwrap();
    let source = "import sys\nimport types\ndef warm(value):\n    module = types.ModuleType('foreign')\n    module.__file__ = '/tmp/ambient-foreign.py'\n    module.marker = 99\n    sys.modules['foreign'] = module\n    return value\ndef probe(value):\n    import foreign\n    return foreign.marker\n";
    let mut program = Program::new(source, "warm_cache_policy.py", ["warm", "probe"])
        .with_standard_library(&stdlib)
        .prepare_for_thread_in_bundle(&bundle)
        .unwrap();
    assert_eq!(
        program.invoke("warm", &[Value::Int(1)]).unwrap(),
        Value::Int(1)
    );
    let error = program.invoke("probe", &[Value::Int(2)]).unwrap_err();
    assert!(
        error.to_string().contains("No module named 'foreign'"),
        "{error}"
    );
}
