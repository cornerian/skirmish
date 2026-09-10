//! Original-C comparison for directional floor-tech selection.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::{TechRoll, tech_roll_direction};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_floor_tech_roll(eligible: bool, stick_x: f32, facing: f32, threshold: f32) -> i32;
}

fn compare(eligible: bool, stick_x: f32, facing: f32, threshold: f32) {
    // SAFETY: scalar arguments have the adapter's exact declared C widths.
    let expected = unsafe { oracle_floor_tech_roll(eligible, stick_x, facing, threshold) };
    let actual = eligible
        .then(|| tech_roll_direction(stick_x, facing, threshold))
        .flatten()
        .map_or(0, |direction| match direction {
            TechRoll::Forward => 1,
            TechRoll::Backward => 2,
        });
    assert_eq!(actual, expected);
}

#[test]
fn eligibility_threshold_facing_and_exceptional_values_match() {
    for values in [
        (false, 1.0, 1.0, 0.7),
        (true, 0.699, 1.0, 0.7),
        (true, 0.7, 1.0, 0.7),
        (true, 0.7, -1.0, 0.7),
        (true, -0.7, -1.0, 0.7),
        (true, -0.0, -0.0, -0.0),
        (true, f32::NAN, 1.0, f32::NAN),
        (true, f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY),
    ] {
        compare(values.0, values.1, values.2, values.3);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_inputs_match(
        eligible in any::<bool>(),
        stick_x in any::<u32>(),
        facing in any::<u32>(),
        threshold in any::<u32>(),
    ) {
        compare(
            eligible,
            f32::from_bits(stick_x),
            f32::from_bits(facing),
            f32::from_bits(threshold),
        );
    }
}
