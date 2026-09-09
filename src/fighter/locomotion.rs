//! Scalar jump launch and walking from ftCo_Jump.c and ftwalkcommon.c.
//!
//! Callers own motion transitions, jump state/timers, and sound events. The jump
//! helper translates the velocity result of ftCo_800CB110. The walk helper includes
//! ftCommon_ApplyGroundMovement with the material friction multiplier supplied by
//! the caller; it does not perform a platform lookup or integrate ground velocity.

use super::Movement;

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
}
