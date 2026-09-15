//! Native Pon execution proof for the fighter API's float32 arithmetic seam.
//!
//! The source is loaded from the checked-in fighter API package, so this
//! exercises Pon's importer and compiler rather than CPython or a retyped
//! fixture.

use std::sync::Mutex;

use skirmish_pon_runtime::{Program, SourceBundle, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn pon_f32_rounds_each_intermediate_and_preserves_ieee_classes() {
    let _guard = TEST_LOCK.lock().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/api");
    let bundle = SourceBundle::from_directory("fighter-api-f32-v1", root).expect("source bundle");
    let bundle = bundle
        .materialize(std::env::temp_dir().join("skirmish-pon-f32"))
        .expect("materialize source bundle");
    let source = r#"
from fighter.compat import f32

def probe():
    rounded = f32(16777216) + f32(1) - f32(16777216)
    positive_overflow = f32(1e100)
    signed_zero = f32(-0.0)
    quotient = f32(1) / f32(0)
    invalid = f32(0) / f32(0)
    return [float(rounded), float(positive_overflow), float(signed_zero), float(quotient), float(invalid)]
"#;
    let mut program = Program::new(source, "f32_arithmetic.py", ["probe"])
        .prepare_for_thread_in_bundle(&bundle)
        .expect("fighter compat must compile in Pon");
    let Value::List(values) = program.invoke("probe", &[]).expect("probe") else {
        panic!("probe did not return a list")
    };
    let bits = |value: &Value| match value {
        Value::F32(value) => value.to_bits(),
        other => panic!("expected native float, got {other:?}"),
    };
    assert_eq!(bits(&values[0]), 0.0_f32.to_bits());
    assert_eq!(bits(&values[1]), f32::INFINITY.to_bits());
    assert_eq!(bits(&values[2]), (-0.0_f32).to_bits());
    assert_eq!(bits(&values[3]), f32::INFINITY.to_bits());
    assert!(bits(&values[4]) & 0x7f80_0000 == 0x7f80_0000);
    assert!(bits(&values[4]) & 0x007f_ffff != 0);
}

#[test]
fn pon_f32_scalar_predicates_and_unary_operations_match_binary32() {
    let _guard = TEST_LOCK.lock().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/api");
    let bundle = SourceBundle::from_directory("fighter-api-f32-v1", root).expect("source bundle");
    let bundle = bundle
        .materialize(std::env::temp_dir().join("skirmish-pon-f32-scalars"))
        .expect("materialize source bundle");
    let source = r#"
from fighter.compat import f32

def probe():
    nan = f32(0) / f32(0)
    class Parent:
        def __abs__(self):
            return f32(2.5)
    class Child(Parent):
        pass
    class InstanceOnly:
        pass
    class Raises:
        def __abs__(self):
            raise ValueError("custom abs")
    instance_only = InstanceOnly()
    instance_only.__abs__ = lambda: f32(9)
    invalid = False
    try:
        abs("not numeric")
    except TypeError:
        invalid = True
    instance_ignored = False
    try:
        abs(instance_only)
    except TypeError:
        instance_ignored = True
    raised = False
    try:
        abs(Raises())
    except ValueError:
        raised = True
    return [
        float(-f32(1.5)), float(abs(-f32(1.5))),
        f32(-1) < 0, f32(1) >= 1,
        f32(0) == f32(-0.0),
        nan == nan, nan != nan,
        float(f32(1) / f32(-0.0)),
        float(abs(Child())), invalid, instance_ignored, raised,
        abs(-7) == 7, abs(7) == 7,
    ]
"#;
    let mut program = Program::new(source, "f32_scalars.py", ["probe"])
        .prepare_for_thread_in_bundle(&bundle)
        .expect("fighter compat must compile in Pon");
    let Value::List(values) = program.invoke("probe", &[]).expect("probe") else {
        panic!("probe did not return a list")
    };
    let f32_bits = |value: &Value| match value {
        Value::F32(value) => value.to_bits(),
        other => panic!("expected native float, got {other:?}"),
    };
    let bool_value = |value: &Value| match value {
        Value::Bool(value) => *value,
        other => panic!("expected native bool, got {other:?}"),
    };
    assert_eq!(f32_bits(&values[0]), (-1.5_f32).to_bits());
    assert_eq!(f32_bits(&values[1]), 1.5_f32.to_bits());
    assert!(bool_value(&values[2]));
    assert!(bool_value(&values[3]));
    assert!(bool_value(&values[4]));
    assert!(!bool_value(&values[5]));
    assert!(bool_value(&values[6]));
    assert_eq!(f32_bits(&values[7]), (-f32::INFINITY).to_bits());
    assert_eq!(f32_bits(&values[8]), 2.5_f32.to_bits());
    assert!(bool_value(&values[9]));
    assert!(bool_value(&values[10]));
    assert!(bool_value(&values[11]));
    assert!(bool_value(&values[12]));
    assert!(bool_value(&values[13]));
}
