//! Native comparisons with unchanged jump and walk C bodies. Material friction
//! is an explicit environment input. The original jump sound flag is disabled;
//! velocity, original startup flag, and tilt-timer effects are checked separately.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::{
    Attributes, Movement,
    locomotion::{JumpAttributes, WalkParameters, jump_velocity, turn_run, walk},
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_locomotion_jump(
        old: *const f32,
        stick: f32,
        short_hop: i32,
        jump_mul: f32,
        attributes: *const f32,
        out: *mut f32,
    );
    fn oracle_locomotion_walk(state: *mut f32, parameters: *const f32) -> f32;
    fn oracle_turn_run(state: *mut f32, parameters: *const f32);
    fn oracle_multi_jump_turn(remaining: *mut i32, facing: *mut f32, yaw: *mut f32, total: i32);
}

fn same(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn jump_attributes(
    [
        momentum_multiplier,
        horizontal_initial_velocity,
        horizontal_max_velocity,
        full_hop_velocity,
        short_hop_velocity,
    ]: [f32; 5],
) -> JumpAttributes {
    JumpAttributes {
        momentum_multiplier,
        horizontal_initial_velocity,
        horizontal_max_velocity,
        full_hop_velocity,
        short_hop_velocity,
    }
}

fn compare_jump(old: [f32; 3], stick: f32, short_hop: bool, jump_mul: f32, attributes: [f32; 5]) {
    let actual = jump_velocity(
        old,
        stick,
        short_hop,
        jump_mul,
        &jump_attributes(attributes),
    );
    let mut expected = [0.0; 3];
    // SAFETY: every array contains the stated live scalar elements; all f32
    // values are accepted by the source. C sound callbacks cannot be reached.
    unsafe {
        oracle_locomotion_jump(
            old.as_ptr(),
            stick,
            i32::from(short_hop),
            jump_mul,
            attributes.as_ptr(),
            expected.as_mut_ptr(),
        )
    };
    for (a, b) in actual.into_iter().zip(expected) {
        same(a, b);
    }
}

fn movement(state: [f32; 13]) -> Movement {
    Movement {
        self_velocity: [state[0], state[1], state[2]],
        animation_velocity: [state[3], state[4], state[5]],
        ground_velocity: state[6],
        ground_acceleration: state[7],
        stick_x: state[8],
        floor_normal: [state[9], state[10], state[11]],
        attributes: Attributes {
            ground_max_horizontal_velocity: state[12],
            ..Attributes::default()
        },
        ground_knockback: -7.0,
        shield_knockback: 9.0,
    }
}

fn walk_parameters(
    [
        accel_mul,
        acceleration_mul,
        acceleration_base,
        max_velocity,
        ground_friction,
        taper_gain,
        ground_friction_multiplier,
        animation_speed_multiplier,
    ]: [f32; 8],
) -> WalkParameters {
    WalkParameters {
        accel_mul,
        acceleration_mul,
        acceleration_base,
        max_velocity,
        ground_friction,
        taper_gain,
        ground_friction_multiplier,
        animation_speed_multiplier,
    }
}

fn compare_walk(initial: [f32; 13], parameters: [f32; 8]) {
    let mut actual = movement(initial);
    let mut expected = initial;
    let animation_target = walk(&mut actual, &walk_parameters(parameters));
    // SAFETY: state has thirteen live floats and parameters has eight. The C
    // adapter uses thread-local common data, so parallel test cases cannot race.
    let expected_target =
        unsafe { oracle_locomotion_walk(expected.as_mut_ptr(), parameters.as_ptr()) };
    same(animation_target, expected_target);
    let observed = [
        actual.self_velocity[0],
        actual.self_velocity[1],
        actual.self_velocity[2],
        actual.animation_velocity[0],
        actual.animation_velocity[1],
        actual.animation_velocity[2],
        actual.ground_velocity,
        actual.ground_acceleration,
        actual.stick_x,
        actual.floor_normal[0],
        actual.floor_normal[1],
        actual.floor_normal[2],
        actual.attributes.ground_max_horizontal_velocity,
    ];
    for (a, b) in observed.into_iter().zip(expected) {
        same(a, b);
    }
    assert_eq!(actual.ground_knockback, -7.0);
    assert_eq!(actual.shield_knockback, 9.0);
}

fn compare_turn_run(initial: [f32; 8], parameters: [f32; 5]) {
    let mut actual = Movement {
        ground_velocity: initial[0],
        ground_acceleration: initial[1],
        self_velocity: [initial[2], initial[3], 5.0],
        animation_velocity: [initial[4], initial[5], 7.0],
        floor_normal: [initial[6], initial[7], 0.0],
        ..Movement::default()
    };
    turn_run(
        &mut actual,
        parameters[0],
        parameters[1],
        parameters[2],
        parameters[3],
        parameters[4],
    );
    let mut expected = initial;
    // SAFETY: both arrays contain the stated live scalar elements and the C
    // adapter isolates supplied callback values in thread-local storage.
    unsafe { oracle_turn_run(expected.as_mut_ptr(), parameters.as_ptr()) };
    for (actual, expected) in [
        actual.ground_velocity,
        actual.ground_acceleration,
        actual.self_velocity[0],
        actual.self_velocity[1],
        actual.animation_velocity[0],
        actual.animation_velocity[1],
        actual.floor_normal[0],
        actual.floor_normal[1],
    ]
    .into_iter()
    .zip(expected)
    {
        same(actual, expected);
    }
    assert_eq!(actual.self_velocity[2], 0.0);
    assert_eq!(actual.animation_velocity[2], 0.0);
}

fn compare_multi_jump_turn(mut remaining: i32, mut facing: f32, mut yaw: f32, total: i32) {
    let (mut expected_remaining, mut expected_facing, mut expected_yaw) = (remaining, facing, yaw);
    skirmish::fighter::locomotion::multi_jump_turn(&mut remaining, &mut facing, &mut yaw, total);
    // SAFETY: all pointers address live scalars; the selected original callback
    // touches only the adapter's stack-owned fighter and root joint.
    unsafe {
        oracle_multi_jump_turn(
            &mut expected_remaining,
            &mut expected_facing,
            &mut expected_yaw,
            total,
        )
    };
    assert_eq!(remaining, expected_remaining);
    same(facing, expected_facing);
    same(yaw, expected_yaw);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn jump_launch_matches_c_bitwise(
        old in prop::array::uniform3(-100.0_f32..100.0),
        stick in -2.0_f32..2.0,
        multiplier in -10.0_f32..10.0,
        attributes in prop::array::uniform5(-10.0_f32..10.0),
    ) {
        for short_hop in [false, true] { compare_jump(old, stick, short_hop, multiplier, attributes); }
    }

    #[test]
    fn walk_acceleration_projection_and_target_match_c_bitwise(
        initial in prop::array::uniform13(-100.0_f32..100.0),
        parameters in prop::array::uniform8(-10.0_f32..10.0),
    ) {
        compare_walk(initial, parameters);
    }

    #[test]
    fn running_turn_physics_matches_c_over_arbitrary_binary32_inputs(
        initial in any::<[f32; 8]>(),
        parameters in any::<[f32; 5]>(),
    ) {
        compare_turn_run(initial, parameters);
    }


    #[test]
    fn multijump_root_turn_matches_c_over_arbitrary_binary32_inputs(
        remaining in any::<i32>(),
        facing in any::<f32>(),
        yaw in any::<f32>(),
        total in any::<i32>(),
    ) {
        compare_multi_jump_turn(remaining, facing, yaw, total);
    }
}

#[test]
fn running_turn_adapter_uses_the_complete_pinned_callback() {
    let original = include_str!("oracle/original/turn_run.c");
    assert!(original.contains("void ftCo_TurnRun_Phys(Fighter_GObj* gobj)\n{"));
    assert!(include_str!("oracle/turn_run.c").contains("#include \"turn_run_original.inc\""));
}

#[test]
fn multijump_adapter_uses_the_complete_pinned_callback() {
    let original = include_str!("oracle/original/multi_jump.c");
    assert!(original.contains("void ft_800CB6EC(Fighter* fp, s32 arg1)\n{"));
    assert!(include_str!("oracle/multi_jump.c").contains("#include \"multi_jump_original.inc\""));
}

#[test]
fn jump_zero_limits_signed_zero_and_exceptional_values_match_c() {
    let special = [
        -0.0,
        0.0,
        -1.0,
        1.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for old in special {
        for coefficient in special {
            for short_hop in [false, true] {
                compare_jump(
                    [old; 3],
                    coefficient,
                    short_hop,
                    coefficient,
                    [coefficient; 5],
                );
            }
        }
    }
}

#[test]
fn walk_ratio_and_material_boundaries_match_c() {
    for ratio in [-0.0_f32, 0.0, 0.5, 1.0] {
        for velocity in [ratio.next_down(), ratio, ratio.next_up()] {
            for material in [0.0, 0.5, 1.0_f32.next_down(), 1.0, 1.0_f32.next_up(), 2.0] {
                let mut initial = [0.0; 13];
                initial[6] = velocity;
                initial[8] = 1.0;
                initial[10] = 1.0;
                initial[12] = 2.0;
                compare_walk(initial, [1.0, 0.1, 0.2, 1.0, 0.05, 0.8, material, 1.0]);
            }
        }
    }
}

#[test]
fn walk_zero_and_nonfinite_values_match_c_classes() {
    let special = [
        -0.0,
        0.0,
        -1.0,
        1.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for value in special {
        for coefficient in special {
            compare_walk([value; 13], [coefficient; 8]);
        }
    }
}
