//! Execute the exact selected ftcommon.c bodies, checking every retained state
//! field after each operation. No mock gameplay functions participate.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use melee_physics::{Attributes, Movement, decrement_toward_zero};
use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_physics_step(state: *mut f32, operation: u32, args: *const f32);
    fn ftCommon_8007CD6C(value: f32, decrement: f32) -> f32;
}

const OPERATIONS: u32 = 24;

// Host adapter layout: self XYZ, animation XYZ, ground velocity, acceleration,
// ground knockback, shield knockback, stick X, floor normal XYZ, then attributes
// in their Rust declaration order. It is not the original Fighter memory ABI.
fn unpack(state: [f32; 23]) -> Movement {
    Movement {
        self_velocity: [state[0], state[1], state[2]],
        animation_velocity: [state[3], state[4], state[5]],
        ground_velocity: state[6],
        ground_acceleration: state[7],
        ground_knockback: state[8],
        shield_knockback: state[9],
        stick_x: state[10],
        floor_normal: [state[11], state[12], state[13]],
        attributes: Attributes {
            ground_max_horizontal_velocity: state[14],
            air_max_horizontal_velocity: state[15],
            air_drift_stick_mul: state[16],
            aerial_drift_base: state[17],
            air_drift_max: state[18],
            aerial_friction: state[19],
            gravity: state[20],
            terminal_velocity: state[21],
            fast_fall_velocity: state[22],
        },
    }
}

fn pack(m: &Movement) -> [f32; 23] {
    [
        m.self_velocity[0],
        m.self_velocity[1],
        m.self_velocity[2],
        m.animation_velocity[0],
        m.animation_velocity[1],
        m.animation_velocity[2],
        m.ground_velocity,
        m.ground_acceleration,
        m.ground_knockback,
        m.shield_knockback,
        m.stick_x,
        m.floor_normal[0],
        m.floor_normal[1],
        m.floor_normal[2],
        m.attributes.ground_max_horizontal_velocity,
        m.attributes.air_max_horizontal_velocity,
        m.attributes.air_drift_stick_mul,
        m.attributes.aerial_drift_base,
        m.attributes.air_drift_max,
        m.attributes.aerial_friction,
        m.attributes.gravity,
        m.attributes.terminal_velocity,
        m.attributes.fast_fall_velocity,
    ]
}

fn apply(m: &mut Movement, operation: u32, [a, b, c, d]: [f32; 4]) {
    match operation {
        0 => m.friction_ground(a),
        1 => m.accelerate_ground(a, b, c),
        2 => m.accelerate_ground_direct(a, b),
        3 => m.control_ground_direct(a, b, c),
        4 => m.project_ground(),
        5 => m.clamp_ground_velocity(a),
        6 => m.decay_ground_knockback(a),
        7 => m.decay_shield_knockback(a),
        8 => m.friction_air(a),
        9 => m.friction_air_basic(),
        10 => m.accelerate_air(a, b, c),
        11 => m.accelerate_air_from(a, b, c, d),
        12 => m.drift_air(),
        13 => m.drift_air_from(a),
        14 => m.accelerate_air_direct(a, b),
        15 => m.control_air(a, b, c),
        16 => m.control_air_direct(a, b, c),
        17 => m.clamp_air_velocity(a),
        18 => m.clamp_air_drift(),
        19 => m.fall(a, b),
        20 => m.fall_basic(),
        21 => m.fall_fast(),
        22 => m.clamp_fall_speed(a),
        23 => m.ascend(a, b),
        _ => unreachable!(),
    }
}

fn same(actual: f32, expected: f32, context: &str) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{context}: expected NaN, got {actual:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{context}: {actual:?} != {expected:?}"
        );
    }
}

fn compare(movement: &mut Movement, state: &mut [f32; 23], operation: u32, args: [f32; 4]) {
    apply(movement, operation, args);
    // SAFETY: the host adapter reads/writes exactly 23 f32 state values and reads
    // four argument values. Both arrays remain live and operation is in range.
    unsafe { oracle_physics_step(state.as_mut_ptr(), operation, args.as_ptr()) };
    for (i, (actual, expected)) in pack(movement).into_iter().zip(*state).enumerate() {
        same(
            actual,
            expected,
            &format!("operation {operation}, field {i}"),
        );
    }
}

fn compare_single(mut state: [f32; 23], operation: u32, args: [f32; 4]) {
    compare(&mut unpack(state), &mut state, operation, args);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_movement_helper_matches_c(
        initial in prop::array::uniform23(-1000.0_f32..1000.0),
        args in prop::array::uniform4(-1000.0_f32..1000.0),
    ) {
        for operation in 0..OPERATIONS {
            compare_single(initial, operation, args);
        }
    }

    #[test]
    fn scalar_decrement_matches_c(value in any::<f32>(), decrement in any::<f32>()) {
        // SAFETY: this is a scalar function without domain restrictions.
        let expected = unsafe { ftCommon_8007CD6C(value, decrement) };
        same(decrement_toward_zero(value, decrement), expected, "decrement");
    }

    #[test]
    fn movement_sequences_match_c_after_every_operation(
        mut state in prop::array::uniform23(-100.0_f32..100.0),
        commands in prop::collection::vec((0..OPERATIONS,
            prop::array::uniform4(-10.0_f32..10.0)), 1..=64),
    ) {
        let mut movement = unpack(state);
        for (operation, args) in commands {
            compare(&mut movement, &mut state, operation, args);
        }
    }
}

#[test]
fn comparison_boundaries_signed_zeros_and_nonfinite_inputs_match_c() {
    let special = [
        -0.0,
        0.0,
        -1.0,
        1.0,
        f32::from_bits(1),
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for value in special {
        for other in special {
            for operation in 0..OPERATIONS {
                let initial = [value; 23];
                compare_single(initial, operation, [other; 4]);
            }
        }
    }
}

#[test]
fn threshold_adjacent_values_and_zero_targets_match_c() {
    for value in [-3.0_f32, -0.25, 0.0, 0.25, 3.0] {
        for stick in [value.next_down(), value, value.next_up()] {
            let mut initial = [1.0; 23];
            initial[0] = -value;
            initial[6] = value;
            initial[10] = stick;
            for operation in [0, 1, 2, 3, 8, 10, 11, 14, 15, 16] {
                for target in [value, -0.0, 0.0] {
                    compare_single(initial, operation, [value.abs(), target, 0.25, 0.125]);
                }
            }
        }
    }
}

#[test]
fn airborne_zero_target_uses_actual_velocity_instead_of_supplied_velocity() {
    let mut movement = Movement {
        self_velocity: [0.5, 1.0, 2.0],
        ..Movement::default()
    };
    let mut state = pack(&movement);
    compare(&mut movement, &mut state, 11, [-10.0, 1.0, 0.0, 0.2]);
    assert_eq!(movement.animation_velocity[0], -0.2);
}
