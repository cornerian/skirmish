//! Grounded shield-evasion input checked against the complete pinned C
//! dispatchers (`ftCo_8009917C`, `ftCo_8009980C`) and C-stick predicates
//! (`ftCo_800DF8B0`, `ftCo_800DF8E8`).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::escape::{
    RollDirection, cstick_roll, cstick_spot_dodge, roll_request, spot_dodge_request,
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_cstick_roll(cstick_x: f32, threshold: f32) -> i32;
    fn oracle_cstick_spot_dodge(cstick_y: f32, threshold: f32) -> i32;
    fn oracle_escape_roll(
        stick_x: f32,
        tilt_x_age: u8,
        cstick_x: f32,
        facing: f32,
        threshold: f32,
        window: i32,
        flag: i32,
        motion: *mut i32,
        captured_flag: *mut i32,
    ) -> i32;
    fn oracle_escape_spot_dodge(
        stick_y: f32,
        tilt_y_age: u8,
        cstick_y: f32,
        threshold: f32,
        window: i32,
        motion: *mut i32,
    ) -> i32;
}

fn compare_cstick(cstick_x: f32, cstick_y: f32, threshold: f32) {
    // SAFETY: the adapters accept scalar binary32 values and own their state.
    let (roll, dodge) = unsafe {
        (
            oracle_cstick_roll(cstick_x, threshold),
            oracle_cstick_spot_dodge(cstick_y, threshold),
        )
    };
    assert_eq!(cstick_roll(cstick_x, threshold), roll != 0);
    assert_eq!(cstick_spot_dodge(cstick_y, threshold), dodge != 0);
}

fn compare_roll(
    stick_x: f32,
    tilt_x_age: u8,
    cstick_x: f32,
    facing: f32,
    threshold: f32,
    window: u8,
) {
    let actual = roll_request(stick_x, tilt_x_age, cstick_x, facing, threshold, window);
    let (mut motion, mut flag) = (0, 0);
    // SAFETY: the adapter owns all C state; both out-pointers are live integers.
    let expected = unsafe {
        oracle_escape_roll(
            stick_x,
            tilt_x_age,
            cstick_x,
            facing,
            threshold,
            i32::from(window),
            7,
            &mut motion,
            &mut flag,
        )
    };
    assert_eq!(actual.is_some(), expected != 0);
    let expected_motion = match actual {
        Some(RollDirection::Forward) => 233,
        Some(RollDirection::Backward) => 234,
        None => 0,
    };
    assert_eq!(motion, expected_motion);
    // The x324 copy reaches the entry callback unchanged whenever a roll starts.
    assert_eq!(flag, if actual.is_some() { 1 } else { -1 });
}

fn compare_spot_dodge(stick_y: f32, tilt_y_age: u8, cstick_y: f32, threshold: f32, window: u8) {
    let actual = spot_dodge_request(stick_y, tilt_y_age, cstick_y, threshold, window);
    let mut motion = 0;
    // SAFETY: the adapter owns all C state; the out-pointer is a live integer.
    let expected = unsafe {
        oracle_escape_spot_dodge(
            stick_y,
            tilt_y_age,
            cstick_y,
            threshold,
            i32::from(window),
            &mut motion,
        )
    };
    assert_eq!(actual, expected != 0);
    assert_eq!(motion, if actual { 235 } else { 0 });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_cstick_bits_match(x in any::<u32>(), y in any::<u32>(), threshold in any::<u32>()) {
        compare_cstick(f32::from_bits(x), f32::from_bits(y), f32::from_bits(threshold));
    }

    #[test]
    fn arbitrary_roll_inputs_match_the_complete_dispatcher(
        stick in any::<u32>(),
        age in any::<u8>(),
        cstick in any::<u32>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32), any::<u32>().prop_map(f32::from_bits)],
        threshold in any::<u32>(),
        window in any::<u8>(),
    ) {
        compare_roll(f32::from_bits(stick), age, f32::from_bits(cstick), facing, f32::from_bits(threshold), window);
    }

    #[test]
    fn arbitrary_spot_dodge_inputs_match_the_complete_dispatcher(
        stick in any::<u32>(),
        age in any::<u8>(),
        cstick in any::<u32>(),
        threshold in any::<u32>(),
        window in any::<u8>(),
    ) {
        compare_spot_dodge(f32::from_bits(stick), age, f32::from_bits(cstick), f32::from_bits(threshold), window);
    }
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let escape = include_str!("oracle/original/escape.c");
    let cstick = include_str!("oracle/original/aerial_input.c");
    let adapter = include_str!("oracle/escape.c");
    for header in [
        "static inline bool inlineA1(Fighter* fp)",
        "bool ftCo_8009917C(Fighter_GObj* gobj)",
        "static inline bool inlineB0(Fighter* fp)",
        "bool ftCo_8009980C(Fighter_GObj* gobj)",
    ] {
        assert!(escape.contains(header), "{header}");
    }
    assert!(cstick.contains("bool ftCo_800DF8B0(Fighter* fp)"));
    assert!(cstick.contains("bool ftCo_800DF8E8(Fighter* fp)"));
    assert!(adapter.contains("#include \"escape_cstick_original.inc\""));
    assert!(adapter.contains("#include \"escape_original.inc\""));

    for (stick, age, cstick, facing) in [
        (0.7, 3, 0.0, 1.0),
        (0.699, 0, 0.0, 1.0),
        (1.0, 4, 0.0, 1.0),
        (1.0, 4, -0.7, 1.0),
        (-1.0, 0, 1.0, -1.0),
        (0.0, 0, -0.0, -1.0),
        (f32::NAN, 0, f32::NAN, 1.0),
        (f32::INFINITY, 0, 0.0, f32::NEG_INFINITY),
    ] {
        compare_roll(stick, age, cstick, facing, 0.7, 4);
    }
    for (stick, age, cstick) in [
        (-0.7, 3, 0.0),
        (-0.699, 0, 0.0),
        (-1.0, 4, 0.0),
        (-1.0, 4, -0.7),
        (0.0, 0, -0.699),
        (f32::NAN, 0, f32::NAN),
        (f32::NEG_INFINITY, 255, 0.0),
    ] {
        compare_spot_dodge(stick, age, cstick, -0.7, 4);
    }
    compare_cstick(-0.0, -0.0, 0.0);
    compare_cstick(f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY);
}
