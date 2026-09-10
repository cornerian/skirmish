use serde_json::json;
use skirmish_equivalence::trace::{compare, f32_bits};
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

#[test]
fn decimal_signed_zero_is_rejected_on_either_side_and_when_identical() {
    let mut negative = trace();
    negative[1]["state"]["fighter"]["x"] = json!(-0.0);
    let mut positive = trace();
    positive[1]["state"]["fighter"]["x"] = json!(0.0);
    for (left, right) in [
        (&negative, &positive),
        (&negative, &negative),
        (&positive, &positive),
        (&trace(), &positive),
        (&negative, &trace()),
    ] {
        let error = compare(encode(left), encode(right)).unwrap_err();
        let diagnostic = format!("{error:#}");
        assert!(
            diagnostic.contains("frame 0/state/fighter/x"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains("decimal floats are forbidden"),
            "{diagnostic}"
        );
    }
}

#[test]
fn decimal_numbers_are_rejected_recursively_in_every_observation_section() {
    for (record, field) in [
        (0, "initial_state"),
        (1, "inputs"),
        (1, "state"),
        (1, "events"),
    ] {
        for number in [json!(1.0), json!(-0.5), json!(1e100)] {
            let mut invalid = trace();
            let nested = json!({"outer":[{"inner":number}]});
            invalid[record][field] = if field == "events" {
                json!([nested])
            } else {
                nested
            };
            let error = compare(encode(&invalid), encode(&invalid)).unwrap_err();
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains(field), "{diagnostic}");
            assert!(diagnostic.contains("outer/0/inner"), "{diagnostic}");
            assert!(
                diagnostic.contains("decimal floats are forbidden"),
                "{diagnostic}"
            );
        }
    }
}

#[test]
fn malformed_float_bit_strings_are_rejected_even_when_identical() {
    for bits in [
        "f32:",
        "f32:0000000",
        "f32:000000000",
        "f32:0x000000",
        "f32:gggggggg",
        "f64:",
        "f64:000000000000000",
        "f64:00000000000000000",
        "f64:000000000000000g",
        "f32:００",
        "f64:000000000000000\n",
    ] {
        let mut invalid = trace();
        invalid[1]["events"] = json!([{"nested":[bits]}]);
        let error = compare(encode(&invalid), encode(&invalid)).unwrap_err();
        let diagnostic = format!("{error:#}");
        assert!(diagnostic.contains("events/0/nested/0"), "{diagnostic}");
        assert!(diagnostic.contains("hexadecimal digits"), "{diagnostic}");
    }
}

#[test]
fn exact_integer_limits_and_float_special_values_preserve_their_bits() {
    let mut original = trace();
    original[0]["initial_state"] = json!({
        "signed":i64::MIN, "unsigned":u64::MAX, "zero":0,
        "values":["f32:00000000", "f32:80000000", "f32:7f800000", "f32:ff800000",
                  "f32:7fc00001", "f32:7f800001", "f64:7ff0000000000000",
                  "f64:fff0000000000000", "f64:7ff8000000000001", "ordinary string"],
    });
    assert!(compare(encode(&original), encode(&original)).is_ok());
    for (index, changed_bits) in [(4, "f32:7fc00002"), (8, "f64:7ff8000000000002")] {
        let mut changed = original.clone();
        changed[0]["initial_state"]["values"][index] = json!(changed_bits);
        let error = compare(encode(&original), encode(&changed))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("header/initial_state/values/{index}")),
            "{error}"
        );
    }
}
