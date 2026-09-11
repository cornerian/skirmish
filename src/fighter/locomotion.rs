//! Scalar jump, walk, running-turn, and multijump physics from ftCo_Jump.c,
//! ftwalkcommon.c, ftCo_TurnRun.c, ftCo_JumpAerial.c, and
//! ftCo_JumpAerialF1.c.
//!
//! Callers own motion transitions, jump state/timers, and sound events. The jump
//! helper translates the velocity result of ftCo_800CB110. The walk helper includes
//! ftCommon_ApplyGroundMovement with the material friction multiplier supplied by
//! the caller; it does not perform a platform lookup or integrate ground velocity.

use super::Movement;

// Keep the source literal: the C oracle uses this exact single-precision
// degrees-to-radians factor.
#[allow(clippy::excessive_precision)]
const DEGREES_TO_RADIANS: f32 = 0.017_453_292_52_f32;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JumpAttributes {
    pub momentum_multiplier: f32,
    pub horizontal_initial_velocity: f32,
    pub horizontal_max_velocity: f32,
    pub full_hop_velocity: f32,
    pub short_hop_velocity: f32,
}

/// Velocity component of ftCo_800CB110. The overwritten old-Y multiplication is
/// omitted; the original always assigns the selected hop speed and clears Z.
/// Its motion-state flag reset, multiplier storage, and tilt timer assignment
/// (0xFE) remain the caller's responsibility, as do optional sound events.
pub fn jump_velocity(
    old: [f32; 3],
    stick_x: f32,
    short_hop: bool,
    jump_mul: f32,
    attributes: &JumpAttributes,
) -> [f32; 3] {
    let momentum = old[0] * (attributes.momentum_multiplier * jump_mul);
    let initial = stick_x * attributes.horizontal_initial_velocity;
    let mut horizontal = momentum + jump_mul * initial;
    let vertical = if short_hop {
        attributes.short_hop_velocity * jump_mul
    } else {
        attributes.full_hop_velocity * jump_mul
    };
    let maximum = attributes.horizontal_max_velocity * jump_mul;
    if horizontal.abs() > maximum {
        horizontal = if horizontal < 0.0 { -maximum } else { maximum };
    }
    [horizontal, vertical, 0.0]
}

/// `ft_800CB6EC`: advance a multijump's root-bone turn and flip facing at the
/// source's integer halfway point. Signed wrapping matches the host C oracle's
/// PowerPC-style integer behavior for arbitrary differential inputs.
pub fn multi_jump_turn(remaining: &mut i32, facing: &mut f32, yaw: &mut f32, total: i32) {
    if *remaining != 0 {
        *remaining = remaining.wrapping_sub(1);
        *yaw += -((180.0_f32 / total as f32) * DEGREES_TO_RADIANS);
        if *remaining == total / 2 {
            *facing = -*facing;
        }
    }
}

/// Input predicate composed by `ftCo_8009A080` and `ftCo_80099F1C` for an
/// immediate platform drop from shield. The caller owns the Pass transition,
/// input-age consumption and collision-line skip.
pub fn shield_drop_request(
    shield_held: bool,
    stick_y: f32,
    tilt_age: u8,
    threshold: f32,
    window: u8,
    on_platform: bool,
) -> bool {
    shield_held && stick_y <= -threshold && tilt_age < window && on_platform
}

/// Complete `ftCo_800DF910` C-stick jump predicate. Unlike tap jump, this
/// extended shield-action input does not require a fresh-stick age.
pub fn cstick_jump(stick_y: f32, threshold: f32) -> bool {
    stick_y >= threshold
}

/// Ground/aerial jump direction test shared by `ftCo_Jump_Enter`
/// (`ftCo_Jump.c:157-161`) and `ftCo_JumpAerial_Enter_Basic`
/// (`ftCo_JumpAerial.c:169-171`, `190-192`): `true` selects the backward
/// motion (JumpB/JumpAerialB). The source picks forward with a strict `>`,
/// so this is its exact negation, not an independent `<=`; the two differ
/// for NaN, where every comparison is false and the source's ternary falls
/// to its `false` branch (backward).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // The negation, not `<=`, preserves NaN.
pub fn jump_backward(stick_x: f32, facing: f32, threshold: f32) -> bool {
    !(stick_x * facing > -threshold)
}

/// Walking coefficients, including the environment query result used by the
/// original ground projection. No character or material defaults are assumed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkParameters {
    pub accel_mul: f32,
    pub acceleration_mul: f32,
    pub acceleration_base: f32,
    pub max_velocity: f32,
    pub ground_friction: f32,
    pub taper_gain: f32,
    pub ground_friction_multiplier: f32,
    pub animation_speed_multiplier: f32,
}

/// ftWalkCommon_800E0060 and getWalkAccel, returning upstream walk.x0 (the
/// animation target). Ground acceleration, animation velocity, and self velocity
/// are updated in the original order. Ground velocity itself is not integrated.
pub fn walk(movement: &mut Movement, parameters: &WalkParameters) -> f32 {
    let stick = movement.stick_x;
    let mut acceleration = stick * parameters.acceleration_mul * parameters.accel_mul;
    let base = if stick > 0.0 {
        parameters.acceleration_base
    } else {
        -parameters.acceleration_base
    };
    acceleration += parameters.accel_mul * base;
    let target = stick * parameters.max_velocity * parameters.accel_mul;
    if target != 0.0 {
        let ratio = movement.ground_velocity / target;
        if ratio > 0.0 && ratio < 1.0 {
            acceleration *= (1.0 - ratio) * parameters.taper_gain;
        }
    }
    let animation_target = target * parameters.animation_speed_multiplier;
    movement.accelerate_ground(acceleration, target, parameters.ground_friction);
    if parameters.ground_friction_multiplier < 1.0 {
        movement.ground_acceleration *= parameters.ground_friction_multiplier;
    }
    movement.project_ground();
    animation_target
}

/// Complete scalar and ground-projection behavior of `ftCo_TurnRun_Phys` after
/// `getAccelAndTarget` supplies its two stick-derived values. The stored facing
/// is the direction from TurnRun entry, even after the animation flips the
/// fighter.
pub fn turn_run(
    movement: &mut Movement,
    mut acceleration: f32,
    target: f32,
    entry_facing: f32,
    ground_friction: f32,
    friction_multiplier: f32,
) {
    let friction = ground_friction * friction_multiplier;
    if target == 0.0 {
        movement.friction_ground(friction);
    } else if entry_facing * acceleration < 0.0 {
        if acceleration > 0.0 {
            if movement.ground_velocity + acceleration > target {
                acceleration -= friction;
                if movement.ground_velocity + acceleration < target {
                    acceleration = target - movement.ground_velocity;
                }
            }
        } else if movement.ground_velocity + acceleration < target {
            acceleration += friction;
            if movement.ground_velocity + acceleration > target {
                acceleration = target - movement.ground_velocity;
            }
        }
        movement.ground_acceleration = acceleration;
    } else {
        movement.friction_ground(friction);
    }
    movement.project_ground();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_preserves_momentum_and_chooses_hop_height() {
        let attributes = JumpAttributes {
            momentum_multiplier: 0.5,
            horizontal_initial_velocity: 1.0,
            horizontal_max_velocity: 2.0,
            full_hop_velocity: 4.0,
            short_hop_velocity: 2.0,
        };
        assert_eq!(
            jump_velocity([2.0, -100.0, 9.0], 0.5, false, 1.0, &attributes),
            [1.5, 4.0, 0.0]
        );
        assert_eq!(
            jump_velocity([-9.0, 100.0, 9.0], -1.0, true, 0.5, &attributes),
            [-1.0, 1.0, 0.0]
        );
    }

    #[test]
    fn jump_backward_treats_equality_and_nan_as_backward() {
        assert!(!jump_backward(-0.29, 1.0, 0.3));
        assert!(jump_backward(-0.3, 1.0, 0.3));
        assert!(jump_backward(-0.31, 1.0, 0.3));
        // Facing negates the effective stick direction: -1 * -1 = 1 > -0.3.
        assert!(!jump_backward(-1.0, -1.0, 0.3));
        // 0.31 * -1 = -0.31, past the boundary the other way: backward,
        // even though the raw stick alone (with facing +1.0) would be forward.
        assert!(jump_backward(0.31, -1.0, 0.3));
        assert!(jump_backward(f32::NAN, 1.0, 0.3));
        assert!(jump_backward(1.0, 1.0, f32::NAN));
    }

    #[test]
    fn multijump_turn_uses_the_source_integer_halfway_point() {
        let (mut remaining, mut facing, mut yaw) = (5, 1.0, 0.0);
        multi_jump_turn(&mut remaining, &mut facing, &mut yaw, 5);
        assert_eq!(remaining, 4);
        assert_eq!(facing, 1.0);
        for _ in 0..2 {
            multi_jump_turn(&mut remaining, &mut facing, &mut yaw, 5);
        }
        assert_eq!(remaining, 2);
        assert_eq!(facing, -1.0);
        for _ in 0..2 {
            multi_jump_turn(&mut remaining, &mut facing, &mut yaw, 5);
        }
        assert_eq!(remaining, 0);
        assert_eq!(facing, -1.0);
        assert_eq!(yaw.to_bits(), (-core::f32::consts::PI).to_bits());
        let stopped = yaw;
        multi_jump_turn(&mut remaining, &mut facing, &mut yaw, 5);
        assert_eq!(yaw.to_bits(), stopped.to_bits());
    }

    #[test]
    fn shield_drop_uses_inclusive_stick_and_strict_age_boundaries() {
        assert!(shield_drop_request(true, -0.7, 2, 0.7, 3, true));
        assert!(!shield_drop_request(true, -0.699, 2, 0.7, 3, true));
        assert!(!shield_drop_request(true, -0.7, 3, 0.7, 3, true));
        assert!(!shield_drop_request(false, -1.0, 0, 0.7, 3, true));
        assert!(!shield_drop_request(true, -1.0, 0, 0.7, 3, false));
    }

    #[test]
    fn cstick_jump_is_inclusive_and_does_not_require_a_previous_sample() {
        assert!(!cstick_jump(0.799, 0.8));
        assert!(cstick_jump(0.8, 0.8));
        assert!(cstick_jump(1.0, 0.8));
    }

    #[test]
    fn walk_tapers_acceleration_then_applies_material_multiplier_and_slope() {
        let mut movement = Movement {
            stick_x: 1.0,
            ground_velocity: 2.0,
            floor_normal: [-0.6, 0.8, 0.0],
            ..Movement::default()
        };
        movement.attributes.ground_max_horizontal_velocity = 10.0;
        let parameters = WalkParameters {
            accel_mul: 1.0,
            acceleration_mul: 0.3,
            acceleration_base: 0.1,
            max_velocity: 4.0,
            ground_friction: 0.05,
            taper_gain: 1.0,
            ground_friction_multiplier: 0.5,
            animation_speed_multiplier: 2.0,
        };
        assert_eq!(walk(&mut movement, &parameters), 8.0);
        assert_eq!(movement.ground_acceleration, 0.1);
        assert_eq!(movement.self_velocity, [1.6, 1.2, 0.0]);
        assert_eq!(movement.ground_velocity, 2.0);
    }

    #[test]
    fn running_turn_uses_entry_facing_and_source_overshoot_correction() {
        let mut movement = Movement {
            ground_velocity: 2.0,
            floor_normal: [0.0, 1.0, 0.0],
            ..Movement::default()
        };
        turn_run(&mut movement, -0.5, -2.0, 1.0, 0.5, 0.5);
        assert_eq!(movement.ground_acceleration, -0.5);
        assert_eq!(movement.self_velocity, [2.0, -0.0, 0.0]);
        assert_eq!(movement.animation_velocity, [-0.5, 0.0, 0.0]);

        movement.ground_velocity = -1.9;
        turn_run(&mut movement, -0.2, -2.0, 1.0, 0.5, 0.5);
        assert_eq!(movement.ground_acceleration, -2.0_f32 - -1.9_f32);
    }
}
