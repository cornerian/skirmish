//! Typed ABI round-trip coverage for the native Pon facade.

use std::{collections::BTreeMap, sync::Mutex};

use skirmish_pon_runtime::{Program, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn nested_values_round_trip_without_repr_parsing() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut program = Program::new(
        "import gc\ndef echo(value):\n    gc.collect()\n    return value\necho = echo\n",
        "typed-roundtrip.py",
        ["echo"],
    )
    .prepare_for_thread()
    .expect("compile");

    let value = Value::Dict(BTreeMap::from([
        ("none_text".into(), Value::String("None".into())),
        ("true_text".into(), Value::String("True".into())),
        ("none".into(), Value::None),
        ("bool".into(), Value::Bool(true)),
        (
            "nested".into(),
            Value::List(vec![
                Value::Int(1),
                Value::F32(2.5),
                Value::Dict(BTreeMap::from([(
                    "message".into(),
                    Value::String("still a string".into()),
                )])),
            ]),
        ),
    ]));

    assert_eq!(
        program
            .invoke("echo", std::slice::from_ref(&value))
            .unwrap(),
        value
    );
}
