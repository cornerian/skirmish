//! Original-C comparison for the retained wall/ceiling reflection arithmetic.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::reflect_velocity;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_damage_reflect(
        values: *const f32,
        multiplier: f32,
        lockout: u8,
        wall: i32,
        output: *mut f32,
        timer: *mut u8,
    );
}

fn exact(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} != {expected:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn compare(values: [f32; 14], multiplier: f32, lockout: u8, wall: bool) {
    let mut expected = [0.0; 7];
    let mut expected_timer = 0;
    // SAFETY: fourteen readable and seven writable floats plus one timer byte.
    // The adapter stubs effect, camera, animation and collision services.
    unsafe {
        oracle_damage_reflect(
            values.as_ptr(),
            multiplier,
            lockout,
            i32::from(wall),
            expected.as_mut_ptr(),
            &mut expected_timer,
        );
    }
    let actual = reflect_velocity(
        [values[2], values[3]],
        [values[4], values[5]],
        [values[10], values[11]],
        multiplier,
    );
    exact(actual.knockback[0], expected[0]);
    exact(actual.knockback[1], expected[1]);
    exact(actual.facing, expected[4]);
    assert_eq!(expected[2].to_bits(), 0.0_f32.to_bits());
    assert_eq!(expected[3].to_bits(), 0.0_f32.to_bits());
    assert_eq!(expected_timer, lockout);
}

#[test]
fn axes_diagonals_signed_zero_and_exceptional_values_match() {
    for (self_velocity, knockback, normal, multiplier) in [
        ([1.0, 2.0], [3.0, -1.0], [-1.0, 0.0], 0.8),
        ([-1.0, 2.0], [0.5, 4.0], [0.0, -1.0], 1.0),
        ([-0.0, 0.0], [0.0, -0.0], [1.0, 0.0], -0.0),
        ([f32::INFINITY, 1.0], [0.0, 2.0], [1.0, 0.0], 0.5),
        ([f32::NAN, 1.0], [2.0, 3.0], [0.0, 1.0], 2.0),
    ] {
        let mut values = [0.0; 14];
        values[2..4].copy_from_slice(&self_velocity);
        values[4..6].copy_from_slice(&knockback);
        values[10..12].copy_from_slice(&normal);
        compare(values, multiplier, 255, true);
        compare(values, multiplier, 0, false);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_velocity_reflections_match(
        bits in prop::array::uniform7(any::<u32>()),
        multiplier in any::<u32>(),
        lockout in any::<u8>(),
        wall in any::<bool>(),
    ) {
        let floats = bits.map(f32::from_bits);
        let mut values = [0.0; 14];
        values[2] = floats[0];
        values[3] = floats[1];
        values[4] = floats[2];
        values[5] = floats[3];
        values[10] = floats[4];
        values[11] = floats[5];
        values[12] = floats[6];
        compare(values, f32::from_bits(multiplier), lockout, wall);
    }
}

#[test]
fn adapter_selects_the_complete_upstream_entry_and_vector_helpers() {
    let source = include_str!("oracle/original/fly_reflect.c");
    assert!(source.contains("void ftCo_800C18A8("));
    assert!(source.contains("lbVector_Mirror(&vec1, normal);"));
    let vector = include_str!("oracle/original/lbvector.c");
    assert!(vector.contains("void lbVector_Mirror("));
    assert!(vector.contains("Vec3* lbVector_Add_xy("));
}
