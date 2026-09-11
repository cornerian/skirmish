#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::game::wall_jump::launch_velocity;

#[repr(C)]
struct OracleVelocity {
    x_bits: u32,
    y_bits: u32,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_passive_wall_jump_launch(
        facing: f32,
        horizontal: f32,
        vertical: f32,
        base: f32,
        used: u8,
        exponent: u8,
    ) -> OracleVelocity;
}

/// NaN payload propagation through arithmetic is unspecified (IEEE 754
/// leaves which input NaN's payload survives, if any, up to the
/// implementation), so two NaN results are equivalent for parity purposes
/// even when their bit patterns differ; a non-NaN result must still match
/// exactly.
fn same_bits(label: &str, actual: u32, expected: u32) {
    let (a, e) = (f32::from_bits(actual), f32::from_bits(expected));
    if e.is_nan() {
        assert!(
            a.is_nan(),
            "{label}: {a:?} ({actual:#x}) != {e:?} ({expected:#x})"
        );
    } else {
        assert_eq!(actual, expected, "{label}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]
    #[test]
    fn launch_arithmetic_matches_full_original_animation_callback(
        facing in any::<u32>(), horizontal in any::<u32>(), vertical in any::<u32>(),
        base in any::<u32>(), used in any::<u8>(), exponent in any::<u8>(),
    ) {
        let values = [facing, horizontal, vertical, base].map(f32::from_bits);
        // SAFETY: scalar parameters and repr(C) return fields match the adapter.
        let c = unsafe {
            oracle_passive_wall_jump_launch(
                values[0], values[1], values[2], values[3], used, exponent,
            )
        };
        let rust = launch_velocity(values[0], values[1], values[2], values[3], used, exponent);
        same_bits("x", rust[0].to_bits(), c.x_bits);
        same_bits("y", rust[1].to_bits(), c.y_bits);
    }
}

#[test]
fn adapter_selects_the_complete_pinned_animation_callback() {
    assert!(
        include_str!("oracle/original/passive_wall.c")
            .contains("void ftCo_PassiveWall_Anim(Fighter_GObj* gobj)\n{")
    );
    assert!(
        include_str!("oracle/passive_wall_launch.c")
            .contains("#include \"passive_wall_launch_original.inc\"")
    );
}

#[test]
fn host_powf_rounding_case_stays_bit_exact() {
    let values = [0, 0, 928_641_269, 850_651_768].map(f32::from_bits);
    // SAFETY: scalar parameters and repr(C) return fields match the adapter.
    let c = unsafe {
        oracle_passive_wall_jump_launch(values[0], values[1], values[2], values[3], 1, 3)
    };
    let rust = launch_velocity(values[0], values[1], values[2], values[3], 1, 3);
    assert_eq!(rust.map(f32::to_bits), [c.x_bits, c.y_bits]);
}
