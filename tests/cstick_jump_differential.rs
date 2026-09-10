//! C-stick jump threshold checked against the complete pinned source predicate.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::locomotion::cstick_jump;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_cstick_jump(current: f32, threshold: f32) -> i32;
}

fn compare(current: f32, threshold: f32) {
    // SAFETY: the adapter accepts two scalar binary32 values and owns its state.
    let expected = unsafe { oracle_cstick_jump(current, threshold) } != 0;
    assert_eq!(cstick_jump(current, threshold), expected);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_inputs_match(current in any::<u32>(), threshold in any::<u32>()) {
        compare(f32::from_bits(current), f32::from_bits(threshold));
    }
}

#[test]
fn complete_predicate_and_exceptional_boundaries_match() {
    let source = include_str!("oracle/original/aerial_input.c");
    let adapter = include_str!("oracle/aerial_input.c");
    assert!(source.contains("bool ftCo_800DF910(Fighter* fp)"));
    assert!(adapter.contains("ftCo_800DF910(&fighter)"));
    for (current, threshold) in [
        (0.799, 0.8),
        (0.8, 0.8),
        (f32::NAN, 0.8),
        (f32::INFINITY, f32::INFINITY),
        (-0.0, 0.0),
    ] {
        compare(current, threshold);
    }
}
