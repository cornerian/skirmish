//! Original-C comparison for wall-tech jump selection.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::wall_tech_jumps;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_wall_tech_jumps(age: u8, stick_y: f32, window: f32, threshold: f32) -> i32;
}

fn compare(age: u8, stick_y: f32, window: f32, threshold: f32) {
    // SAFETY: scalar arguments have the adapter's exact declared C widths.
    let expected = unsafe { oracle_wall_tech_jumps(age, stick_y, window, threshold) } != 0;
    assert_eq!(wall_tech_jumps(age, stick_y, window, threshold), expected);
}

#[test]
fn strict_age_inclusive_stick_and_exceptional_values_match() {
    for values in [
        (2, 0.0, 3.0, 0.8),
        (3, 0.8, 3.0, 0.8),
        (3, 0.799, 3.0, 0.8),
        (255, -0.0, 255.0, 0.0),
        (0, f32::NAN, f32::NAN, f32::NAN),
        (255, f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY),
    ] {
        compare(values.0, values.1, values.2, values.3);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_inputs_match(
        age in any::<u8>(),
        stick_y in any::<u32>(),
        window in any::<u32>(),
        threshold in any::<u32>(),
    ) {
        compare(
            age,
            f32::from_bits(stick_y),
            f32::from_bits(window),
            f32::from_bits(threshold),
        );
    }
}
