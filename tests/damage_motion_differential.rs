//! Ordinary damage motion selection checked against the pinned source table.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::{HurtHeight, damage_motion, damage_motion_id};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_damage_motion(
        knockback: f32,
        scale: f32,
        thresholds: *const f32,
        airborne: i32,
        height: i32,
    ) -> i32;
}

fn compare(knockback: f32, scale: f32, thresholds: [f32; 3], airborne: bool, height: HurtHeight) {
    let actual = damage_motion_id(damage_motion(
        knockback, scale, thresholds, airborne, height,
    ));
    // SAFETY: thresholds provides three readable floats; scalar indices are in range.
    let expected = unsafe {
        oracle_damage_motion(
            knockback,
            scale,
            thresholds.as_ptr(),
            i32::from(airborne),
            height.index() as i32,
        )
    };
    assert_eq!(i32::from(actual), expected);
}

#[test]
fn table_is_byte_for_byte_present_in_the_pinned_upstream_snapshot() {
    let adapter = include_str!("oracle/damage_motion.c");
    let source = include_str!("oracle/original/combat_hitstun.c");
    for (begin, end) in [
        (
            "/* BEGIN VERBATIM MOTION TABLE */\n",
            "/* END VERBATIM MOTION TABLE */",
        ),
        (
            "/* BEGIN VERBATIM KNOCKBACK SCALE */\n",
            "/* END VERBATIM KNOCKBACK SCALE */",
        ),
        (
            "/* BEGIN VERBATIM LEVEL SELECTION */\n",
            "/* END VERBATIM LEVEL SELECTION */",
        ),
    ] {
        let block = adapter
            .split(begin)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap();
        assert!(source.contains(block));
    }
}

#[test]
fn strict_thresholds_multiplication_and_nonfinite_edges_match_source() {
    for knockback in [
        -0.0,
        0.0,
        9.999_999,
        10.0,
        20.0,
        30.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        for scale in [-0.0, 0.0, 0.5, 1.0, f32::NAN, f32::INFINITY] {
            for thresholds in [
                [10.0, 20.0, 30.0],
                [-0.0, 0.0, f32::INFINITY],
                [f32::NAN, 20.0, 30.0],
            ] {
                for airborne in [false, true] {
                    for height in [HurtHeight::Low, HurtHeight::Middle, HurtHeight::High] {
                        compare(knockback, scale, thresholds, airborne, height);
                    }
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_bit_patterns_match(
        knockback in any::<u32>(),
        scale in any::<u32>(),
        thresholds in any::<[u32; 3]>(),
        airborne in any::<bool>(),
        height in 0usize..3,
    ) {
        let heights = [HurtHeight::Low, HurtHeight::Middle, HurtHeight::High];
        compare(
            f32::from_bits(knockback),
            f32::from_bits(scale),
            thresholds.map(f32::from_bits),
            airborne,
            heights[height],
        );
    }
}
