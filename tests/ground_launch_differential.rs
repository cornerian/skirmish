//! Ground launch and floor-angle arithmetic checked against pinned source.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::{GroundLaunchRules, ground_launch, vector_angle};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_ground_launch(
        knockback: *const f32,
        floor_normal: *const f32,
        fly: i32,
        bounce_angle: f32,
        bounce_multiplier: f32,
        output: *mut f32,
        flags: *mut i32,
    );
}

fn same_float(label: &str, actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

fn compare(
    knockback: [f32; 2],
    floor_normal: [f32; 2],
    fly: bool,
    bounce_angle: f32,
    bounce_multiplier: f32,
) {
    let mut output = [0.0; 4];
    let mut flags = [0; 2];
    // SAFETY: each pointer addresses the fixed-size readable/writable array declared above.
    unsafe {
        oracle_ground_launch(
            knockback.as_ptr(),
            floor_normal.as_ptr(),
            i32::from(fly),
            bounce_angle,
            bounce_multiplier,
            output.as_mut_ptr(),
            flags.as_mut_ptr(),
        );
    }
    let actual = ground_launch(
        knockback,
        floor_normal,
        fly,
        &GroundLaunchRules {
            fly_bounce_angle_radians: bounce_angle,
            fly_bounce_vertical_multiplier: bounce_multiplier,
        },
    );
    let case = format!("{knockback:?} {floor_normal:?} fly={fly}");
    same_float(
        &format!("{case} knockback.x"),
        actual.knockback[0],
        output[0],
    );
    same_float(
        &format!("{case} knockback.y"),
        actual.knockback[1],
        output[1],
    );
    same_float(
        &format!("{case} ground_knockback"),
        actual.ground_knockback,
        output[2],
    );
    assert_eq!(actual.airborne, flags[0] != 0);
    assert_eq!(actual.bounced, flags[1] != 0);

    let angle = vector_angle(floor_normal, knockback);
    if output[3].is_nan() {
        assert!(angle.is_nan());
    } else {
        let ulps = angle.to_bits().abs_diff(output[3].to_bits());
        assert!(
            ulps <= 2,
            "angle {angle:?} != {:?} ({ulps} ulps)",
            output[3]
        );
    }
}

#[test]
fn adapter_retains_the_complete_source_branch_and_vector_angle_function() {
    let adapter = include_str!("oracle/ground_launch.c");
    let damage = include_str!("oracle/original/combat_hitstun.c");
    let vector = include_str!("oracle/original/lbvector.c");
    let block = adapter
        .split("/* BEGIN VERBATIM GROUNDED LAUNCH */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM GROUNDED LAUNCH */")
        .next()
        .unwrap();
    assert!(damage.contains(block));
    assert!(vector.contains("float lbVector_Angle(Vec3* a, Vec3* b)"));
}

#[test]
fn flat_sloped_fly_tiny_and_exceptional_cases_match() {
    for (knockback, normal, fly, angle, multiplier) in [
        ([4.0, 0.0], [0.0, 1.0], false, 0.2, 0.8),
        ([4.0, 0.0], [-0.6, 0.8], false, 0.2, 0.8),
        ([4.0, 1.0], [0.0, 1.0], false, 0.2, 0.8),
        ([4.0, 0.0], [0.0, 1.0], true, 0.2, 0.8),
        ([4.0, -2.0], [0.0, 1.0], true, 0.2, 0.8),
        ([1.0e-12, 0.0], [0.0, 1.0], false, 0.2, 0.8),
        ([f32::NAN, 1.0], [0.0, 1.0], false, 0.2, 0.8),
        ([f32::INFINITY, 1.0], [0.0, 1.0], true, 0.2, 0.8),
    ] {
        compare(knockback, normal, fly, angle, multiplier);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_ground_launch_matches(
        values in prop::array::uniform6(any::<u32>()),
        fly in any::<bool>(),
    ) {
        compare(
            [f32::from_bits(values[0]), f32::from_bits(values[1])],
            [f32::from_bits(values[2]), f32::from_bits(values[3])],
            fly,
            f32::from_bits(values[4]),
            f32::from_bits(values[5]),
        );
    }
}
