use skirmish::game::{Controller, Phase, data::MatchData};
use skirmish_equivalence::{match_trace, trace};
use std::{io::Cursor, process::Command};

#[test]
fn stdin_adapter_matches_the_scripted_demo_and_rejects_malformed_input() {
    use std::{io::Write, process::Stdio};
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/game/integration-match.json"
    );
    let mut game = skirmish::game::Match::new(data(), 0).unwrap();
    let mut inputs = vec![];
    while let Some(input) = match_trace::demo_input(game.state()) {
        serde_json::to_writer(&mut inputs, &input).unwrap();
        inputs.push(b'\n');
        game.step(input).unwrap();
    }
    let run = |input: &[u8]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_skirmish"))
            .args(["run-match", "--data", fixture])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    };
    let actual = run(&inputs);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    let mut expected = vec![];
    match_trace::run(
        data(),
        0,
        |state| Ok(match_trace::demo_input(state)),
        &mut expected,
    )
    .unwrap();
    assert_eq!(
        trace::compare(Cursor::new(expected), Cursor::new(actual.stdout))
            .unwrap()
            .frames,
        game.state().next_frame as u64
    );
    let bad = run(b"[]\n");
    assert!(!bad.status.success());
    assert!(
        !String::from_utf8(bad.stdout)
            .unwrap()
            .contains("\"kind\":\"end\"")
    );
}

fn data() -> MatchData {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap()
}

#[test]
fn native_executable_finishes_and_emits_complete_bitwise_traces() {
    let run = || {
        let result = Command::new(env!("CARGO_BIN_EXE_skirmish"))
            .args(["demo-match", "--seed", "42"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    };
    let (a, b) = (run(), run());
    let report = trace::compare(Cursor::new(&a), Cursor::new(&b)).unwrap();
    assert!(report.frames > 10);
    let records: Vec<trace::Record> = String::from_utf8(a)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let trace::Record::Frame { state, .. } = &records[records.len() - 2] else {
        panic!("last observation missing")
    };
    assert_eq!(state["match"]["phase"]["kind"], "finished");
    assert!(records.iter().any(|r| matches!(r, trace::Record::Frame { events, .. } if events.iter().any(|e| e["kind"] == "hit"))));
}

#[test]
fn adapter_refuses_empty_inputs_and_input_after_completion() {
    let mut output = vec![];
    assert!(match_trace::run(data(), 0, |_| Ok(None), &mut output).is_err());
    assert!(
        !String::from_utf8(output)
            .unwrap()
            .contains("\"kind\":\"end\"")
    );
    let mut short = data();
    short.rules.countdown_frames = 0;
    short.rules.time_limit_frames = 1;
    let mut output = vec![];
    assert!(
        match_trace::run(
            short,
            0,
            |_| Ok(Some([Controller::default(); 2])),
            &mut output
        )
        .is_err()
    );
    assert!(
        !String::from_utf8(output)
            .unwrap()
            .contains("\"kind\":\"end\"")
    );
}

#[test]
fn traces_identify_changed_resources_and_preserve_float_bits() {
    let run = |data| {
        let mut out = vec![];
        let state = match_trace::run(
            data,
            0,
            |state| Ok(match_trace::demo_input(state)),
            &mut out,
        )
        .unwrap();
        assert!(matches!(state.phase, Phase::Finished { .. }));
        out
    };
    let a = run(data());
    let mut changed = data();
    changed.fighters[0].weight += 1.0;
    let b = run(changed);
    let error = trace::compare(Cursor::new(a), Cursor::new(b))
        .unwrap_err()
        .to_string();
    assert!(error.contains("resources_sha256"), "{error}");
    let value = match_trace::float_bits(&[0.0_f32, -0.0, f32::from_bits(1)]).unwrap();
    assert_eq!(
        value,
        serde_json::json!(["f32:00000000", "f32:80000000", "f32:00000001"])
    );
}
