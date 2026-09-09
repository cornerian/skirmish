#![cfg(feature = "c-oracle")]
use skirmish::runner::{Adapter, compare_binaries};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    time::Duration,
};

fn adapter(program: &Path) -> Adapter {
    Adapter {
        program: program.into(),
        args: vec![],
        env: BTreeMap::new(),
    }
}

fn reference(directory: &Path, flag: &str) -> Adapter {
    let program = directory.join("reference");
    let output = Command::new("cc")
        .args([
            "-std=c11",
            "-fwrapv",
            "-ffp-contract=off",
            "-fno-fast-math",
            flag,
        ])
        .arg("-I")
        .arg(env!("OUT_DIR"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rng_probe.c"))
        .arg("-o")
        .arg(&program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    adapter(&program)
}

fn run_probe(program: &Path, input: &[u8]) -> Output {
    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn both_probe_binaries_reject_malformed_scenarios() {
    let directory = tempfile::tempdir().unwrap();
    let reference = reference(directory.path(), "-O2");
    let candidate = Path::new(env!("CARGO_BIN_EXE_skirmish-probe"));
    let malformed: &[&[u8]] = &[
        b"",
        b"1",
        b"1 1",
        b"1 1 1 0",
        b"1 1 1suffix",
        b"1suffix 1 1",
        b"1 1suffix 1",
        b"-1 1 1",
        b"-0 1 1",
        b"1 -1 1",
        b"1 -0 1",
        b"4294967296 1 1",
        b"18446744073709551616 1 1",
        b"1 4294967296 1",
        b"1 1 2147483648",
        b"1 1 -2147483649",
        b"1 0 1",
        b"1 1000001 1",
        b"+ 1 1",
        b"1 + 1",
        b"1 1 -",
        b"++1 1 1",
        b"1 1 --1",
        b"1 1 +-1",
        b"0x1 1 1",
        b"1_0 1 1",
        b"1 1 1.0",
        b"1\0 1 1",
        b"1 1 1\0",
        b"1\xff 1 1",
        b"1 1 1\xff",
        b"1\xc0\xa0 1 1",
        b"1\xe0\x80\xa0 1 1",
        b"1\xed\xa0\x80 1 1",
        b"1\xf4\x90\x80\x80 1 1",
        b"1 1 1\xe2\x80",
        "1\u{200b}1 1".as_bytes(),
        "1\u{feff}1 1".as_bytes(),
        "１ 1 1".as_bytes(),
    ];
    for &input in malformed {
        for program in [reference.program.as_path(), candidate] {
            let output = run_probe(program, input);
            assert!(
                !output.status.success(),
                "{} accepted {input:?}",
                program.display()
            );
            assert!(
                output.stdout.is_empty(),
                "invalid input emitted a trace: {input:?}"
            );
        }
    }
}

#[test]
fn both_probe_binaries_accept_identical_integer_and_whitespace_grammar() {
    let directory = tempfile::tempdir().unwrap();
    let reference = reference(directory.path(), "-O2");
    let candidate = Path::new(env!("CARGO_BIN_EXE_skirmish-probe"));
    let whitespace: String = (0..=0x10ffff)
        .filter_map(char::from_u32)
        .filter(|point| point.is_whitespace())
        .collect();
    let zeros = "0".repeat(20_000);
    for input in [
        "0 1 0".to_owned(),
        "4294967295 1 -2147483648".to_owned(),
        "+4294967295 +1 +2147483647".to_owned(),
        "0 1 -0".to_owned(),
        format!("{whitespace}+001{whitespace}002{whitespace}-003{whitespace}"),
        format!("+{zeros}1 {zeros}2 -{zeros}3"),
    ] {
        let outputs = [reference.program.as_path(), candidate].map(|program| {
            let output = run_probe(program, input.as_bytes());
            assert!(
                output.status.success(),
                "{} rejected valid input: {}",
                program.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(outputs[0], outputs[1]);
    }
}

#[test]
fn separately_compiled_c_and_rust_binaries_have_identical_rng_traces() {
    for optimization in ["-O0", "-O2"] {
        let directory = tempfile::tempdir().unwrap();
        let reference = reference(directory.path(), optimization);
        let input = directory.path().join("scenario");
        fs::write(&input, "4294967295 4096 -2147483648\n").unwrap();
        let report = compare_binaries(
            reference,
            adapter(Path::new(env!("CARGO_BIN_EXE_skirmish-probe"))),
            &input,
            &directory.path().join("run"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(report.comparison.frames, 4096);
        assert_ne!(report.reference_sha256, report.candidate_sha256);
        assert!(directory.path().join("run/report.json").is_file());
    }
}

#[test]
fn a_deliberate_behavior_change_is_detected_at_its_first_frame() {
    let directory = tempfile::tempdir().unwrap();
    let reference = reference(directory.path(), "-DDIVERGE");
    let input = directory.path().join("scenario");
    fs::write(&input, "1 100 10").unwrap();
    let error = compare_binaries(
        reference,
        adapter(Path::new(env!("CARGO_BIN_EXE_skirmish-probe"))),
        &input,
        &directory.path().join("run"),
        Duration::from_secs(10),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("frame 37/state/hsd.rand"), "{error}");
}

#[test]
fn failed_or_hung_adapters_cannot_pass() {
    for (flag, expected) in [("-DFAIL", "exited"), ("-DHANG", "exceeded deadline")] {
        let directory = tempfile::tempdir().unwrap();
        let reference = reference(directory.path(), flag);
        let input = directory.path().join("scenario");
        fs::write(&input, "1 100 10").unwrap();
        let error = compare_binaries(
            reference,
            adapter(Path::new(env!("CARGO_BIN_EXE_skirmish-probe"))),
            &input,
            &directory.path().join("run"),
            Duration::from_millis(100),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(directory.path().join("run/invocation.json").is_file());
        assert!(!directory.path().join("run/report.json").exists());
    }
}

#[test]
fn adapters_cannot_silently_change_the_shared_scenario() {
    let directory = tempfile::tempdir().unwrap();
    let mut reference = reference(directory.path(), "-DMUTATE_INPUT");
    let input = directory.path().join("scenario");
    let run = directory.path().join("run");
    fs::write(&input, "1 10 10").unwrap();
    reference.env.insert(
        "SKIRMISH_TEST_INPUT".into(),
        run.join("input.bin").to_str().unwrap().into(),
    );
    let error = compare_binaries(
        reference,
        adapter(Path::new(env!("CARGO_BIN_EXE_skirmish-probe"))),
        &input,
        &run,
        Duration::from_secs(10),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("modified shared scenario input"), "{error}");
    assert!(run.join("invocation.json").is_file());
    assert!(!run.join("candidate.jsonl").exists());
}
