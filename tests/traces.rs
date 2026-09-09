use serde_json::json;
use skirmish::trace::{compare, f32_bits};
use std::io::Cursor;

fn trace() -> Vec<serde_json::Value> {
    vec![
        json!({"kind":"header", "schema":1, "upstream_commit":"0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9", "scenario":"test", "initial_state":{"seed":1}}),
        json!({"kind":"frame", "frame":0, "inputs":{"buttons":256}, "state":{"fighter":{"x":f32_bits(-0.0),"damage":0}}, "events":[]}),
        json!({"kind":"end", "frames":1}),
    ]
}

fn encode(records: &[serde_json::Value]) -> Cursor<String> {
    Cursor::new(records.iter().map(|r| format!("{r}\n")).collect())
}

#[test]
fn complete_trace_passes() {
    let result = compare(encode(&trace()), encode(&trace())).unwrap();
    assert_eq!(result.frames, 1);
}

#[test]
fn mismatch_reports_frame_and_field_including_signed_zero() {
    let original = trace();
    let mut changed = original.clone();
    changed[1]["state"]["fighter"]["x"] = f32_bits(0.0);
    let error = compare(encode(&original), encode(&changed))
        .unwrap_err()
        .to_string();
    assert!(error.contains("frame 0/state/fighter/x"), "{error}");
}

#[test]
fn wrong_inputs_or_scenario_never_count_as_parity() {
    for (record, key) in [
        (0, "scenario"),
        (0, "upstream_commit"),
        (1, "inputs"),
        (1, "events"),
    ] {
        let mut changed = trace();
        changed[record][key] = json!("changed");
        assert!(compare(encode(&trace()), encode(&changed)).is_err());
    }
}

#[test]
fn empty_truncated_noncontiguous_and_extra_records_fail_even_when_identical() {
    let original = trace();
    let mut duplicate = original.clone();
    duplicate.insert(2, duplicate[1].clone());
    let mut missing = original.clone();
    missing[1]["frame"] = json!(1);
    let mut wrong_count = original.clone();
    wrong_count[2]["frames"] = json!(2);
    let mut extra = original.clone();
    extra.push(original[1].clone());
    let mut empty_state = original.clone();
    empty_state[1]["state"] = json!({});
    for invalid in [
        vec![],
        original[..1].to_vec(),
        original[..2].to_vec(),
        vec![original[0].clone(), json!({"kind":"end","frames":0})],
        duplicate,
        missing,
        wrong_count,
        extra,
        empty_state,
    ] {
        assert!(
            compare(encode(&invalid), encode(&invalid)).is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn unknown_fields_and_schema_are_rejected() {
    let mut future = trace();
    future[0]["schema"] = json!(2);
    assert!(compare(encode(&future), encode(&future)).is_err());
    let mut typo = trace();
    typo[1]["unobserved_state"] = json!(5);
    assert!(compare(encode(&typo), encode(&typo)).is_err());
}
