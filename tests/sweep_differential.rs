//! Native original-C comparisons. Finite results compare every float bit;
//! nonfinite edge cases compare NaN classification without payload assumptions.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::{collision::sweep::*, fighter::combat::Capsule};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_sweep_capsules(values: *const f32, closest: *mut f32) -> i32;
    fn oracle_sweep_point(values: *const f32, xy: i32, result: *mut f32);
}

fn same(actual: f32, expected: f32, values: &[f32]) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} != NaN; {values:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}; {values:?}"
        );
    }
}

fn pair(values: [f32; 14]) {
    let a = Capsule {
        start: values[..3].try_into().unwrap(),
        end: values[3..6].try_into().unwrap(),
        radius: values[12],
    };
    let b = Capsule {
        start: values[6..9].try_into().unwrap(),
        end: values[9..12].try_into().unwrap(),
        radius: values[13],
    };
    let mut closest = ClosestPair {
        first: [31.0, -0.0, 33.0],
        second: [-34.0, 35.0, -36.0],
    };
    let mut expected = [31.0, -0.0, 33.0, -34.0, 35.0, -36.0];
    let actual = capsule_capsule(&a, &b, &mut closest);
    // SAFETY: adapter reads 14 floats and copies/writes six initialized floats;
    // source-selected functions use no global state or other pointers.
    let hit = unsafe { oracle_sweep_capsules(values.as_ptr(), expected.as_mut_ptr()) };
    assert_eq!(actual, hit != 0, "{values:?}; {closest:?} != {expected:?}");
    for (actual, expected) in closest
        .first
        .into_iter()
        .chain(closest.second)
        .zip(expected)
    {
        same(actual, expected, &values);
    }
}

fn point(values: [f32; 9]) {
    for xy in [false, true] {
        let start = values[..3].try_into().unwrap();
        let end = values[3..6].try_into().unwrap();
        let point = values[6..9].try_into().unwrap();
        let actual = if xy {
            point_segment_xy(start, end, point)
        } else {
            point_segment(start, end, point)
        };
        let mut expected = [0.0; 2];
        // SAFETY: adapter reads nine initialized floats and writes two results.
        unsafe { oracle_sweep_point(values.as_ptr(), i32::from(xy), expected.as_mut_ptr()) };
        same(actual.squared_distance, expected[0], &values);
        same(actual.parameter, expected[1], &values);
    }
}

#[test]
fn degenerate_parallel_near_parallel_and_touching_capsules_match_c() {
    for length in [0.0, -0.0, 0.001, 0.003, 0.0032, 0.01, 1.0, 10.0] {
        for offset in [-10.0, -1.0, -0.0, 0.0, 0.001, 1.0, 10.0] {
            for radius in [-1.0, -0.0, 0.0, 0.5, 10.0] {
                for bend in [0.0, 0.001, 0.003, 0.0032, 1.0] {
                    let mut values = [
                        0.0,
                        0.0,
                        0.0,
                        length,
                        0.0,
                        0.0,
                        offset,
                        radius,
                        0.0,
                        length + offset,
                        radius + bend,
                        0.0,
                        radius,
                        0.0,
                    ];
                    pair(values);
                    // Both orientation and capsule ordering affect source ties.
                    values.swap(6, 9);
                    values.swap(7, 10);
                    pair(values);
                    for i in 0..6 {
                        values.swap(i, i + 6);
                    }
                    values.swap(12, 13);
                    pair(values);
                }
            }
        }
    }
    for coordinate in [-0.0, 0.0, f32::from_bits(1), 0.001, 0.0032, 1.0] {
        point([
            coordinate, coordinate, coordinate, coordinate, coordinate, coordinate, 1.0, 1.0, 1.0,
        ]);
    }
}

#[test]
fn source_nonfinite_and_signed_zero_behavior_matches_c() {
    for signs in 0_u16..(1 << 12) {
        let mut values = [0.0; 14];
        for (i, value) in values[..12].iter_mut().enumerate() {
            *value = if signs & (1 << i) == 0 { 0.0 } else { -0.0 };
        }
        pair(values);
    }
    for special in [
        -0.0,
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        for index in 0..14 {
            let mut values = [
                -1.0, -2.0, -3.0, 4.0, 5.0, 6.0, -4.0, 2.0, 1.0, 3.0, -2.0, -1.0, 10.0, 10.0,
            ];
            values[index] = special;
            pair(values);
        }
        for index in 0..9 {
            let mut values = [-1.0, -2.0, -3.0, 4.0, 5.0, 6.0, 2.0, 1.0, -1.0];
            values[index] = special;
            point(values);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn arbitrary_segments_match_c(coordinates in prop::array::uniform12(-100_f32..100_f32), radii in prop::array::uniform2(0_f32..20_f32)) {
        let mut values = [0.0; 14];
        values[..12].copy_from_slice(&coordinates);
        values[12..].copy_from_slice(&radii);
        pair(values);
        // Disable AABB rejection to exercise closest-point writes on every case.
        values[12] = 1000.0;
        pair(values);
    }

    #[test]
    fn point_projections_match_c(values in prop::array::uniform9(-100_f32..100_f32)) {
        point(values);
    }
}
