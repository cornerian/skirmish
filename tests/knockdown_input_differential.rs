//! Exact C comparison for the two reusable C-stick predicates in DownWait.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::{fresh_horizontal_cstick, fresh_up_cstick};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_knockdown_cstick_up(previous: f32, current: f32, threshold: f32) -> i32;
    fn oracle_knockdown_cstick_horizontal(values: *const f32, threshold: f32, angle: f32) -> i32;
}

fn compare_up(previous: f32, current: f32, threshold: f32) {
    // SAFETY: scalar arguments have the adapter's exact declared C widths.
    let expected = unsafe { oracle_knockdown_cstick_up(previous, current, threshold) } != 0;
    assert_eq!(fresh_up_cstick(previous, current, threshold), expected);
}

fn compare_horizontal(previous: [f32; 2], current: [f32; 2], threshold: f32, angle: f32) {
    let values = [previous[0], previous[1], current[0], current[1]];
    // SAFETY: the adapter reads exactly four floats from this live array.
    let expected =
        unsafe { oracle_knockdown_cstick_horizontal(values.as_ptr(), threshold, angle) } != 0;
    assert_eq!(
        fresh_horizontal_cstick(previous, current, threshold, angle),
        expected
    );
}

#[test]
fn exceptional_values_and_boundaries_match() {
    for (previous, current, threshold) in [
        (0.799, 0.8, 0.8),
        (0.8, 1.0, 0.8),
        (-0.0, 0.0, 0.0),
        (f32::NAN, 1.0, 0.5),
        (0.0, f32::INFINITY, f32::INFINITY),
    ] {
        compare_up(previous, current, threshold);
    }
    for (previous, current, threshold, angle) in [
        ([0.699, 0.0], [0.7, 0.0], 0.7, 0.8),
        ([0.7, 0.0], [1.0, 0.0], 0.7, 0.8),
        ([-0.0, 0.0], [-0.0, 0.0], -0.0, 0.0),
        ([f32::NAN; 2], [1.0, 0.0], 0.7, f32::NAN),
    ] {
        compare_horizontal(previous, current, threshold, angle);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_up_inputs_match(previous in any::<u32>(), current in any::<u32>(), threshold in any::<u32>()) {
        compare_up(f32::from_bits(previous), f32::from_bits(current), f32::from_bits(threshold));
    }

    #[test]
    fn arbitrary_binary32_horizontal_inputs_match(
        previous in prop::array::uniform2(any::<u32>()),
        current in prop::array::uniform2(any::<u32>()),
        threshold in any::<u32>(),
        angle in any::<u32>(),
    ) {
        compare_horizontal(
            previous.map(f32::from_bits),
            current.map(f32::from_bits),
            f32::from_bits(threshold),
            f32::from_bits(angle),
        );
    }
}
