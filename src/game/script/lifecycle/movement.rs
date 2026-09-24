//! Native movement support for lifecycle callbacks.
//!
//! This module intentionally contains no scripting runtime types. The
//! Starlark backend owns the safe value implementation; this module owns the
//! small, deterministic movement operations and their validation. Keeping
//! those operations here also lets the backend stage a private `Movement`
//! clone and commit it only after the callback succeeds.

use crate::fighter::Movement;

const MOVEMENT_LIMIT: f32 = 1_000_000.0;

pub(crate) fn finite(value: f32) -> bool {
    value.is_finite()
}

pub(crate) fn require_finite(value: f32, name: &str) -> Result<f32, String> {
    finite(value)
        .then_some(value)
        .ok_or_else(|| format!("non-finite {name}"))
}

/// Validate all fields exposed through the lifecycle movement host.
pub(crate) fn validate_native(movement: &Movement) -> Result<(), String> {
    for value in movement
        .self_velocity
        .into_iter()
        .chain(movement.animation_velocity)
        .chain(movement.floor_normal)
        .chain([
            movement.ground_velocity,
            movement.ground_acceleration,
            movement.ground_knockback,
            movement.shield_knockback,
            movement.ground_friction_multiplier,
            movement.stick_x,
            movement.attributes.ground_max_horizontal_velocity,
            movement.attributes.air_max_horizontal_velocity,
            movement.attributes.air_drift_stick_mul,
            movement.attributes.aerial_drift_base,
            movement.attributes.air_drift_max,
            movement.attributes.aerial_friction,
            movement.attributes.gravity,
            movement.attributes.terminal_velocity,
            movement.attributes.fast_fall_velocity,
        ])
    {
        if !finite(value) || value.abs() > MOVEMENT_LIMIT {
            return Err("non-finite movement state".into());
        }
    }
    Ok(())
}

/// Apply a callback's staged movement view after validating every exposed
/// field. Assignment is a single commit after all reads have succeeded.
pub(crate) fn commit(candidate: &Movement, destination: &mut Movement) -> Result<(), String> {
    validate_native(candidate)?;
    *destination = *candidate;
    Ok(())
}

pub(crate) fn set_velocity(movement: &mut Movement, x: f32, y: f32) -> Result<(), String> {
    require_finite(x, "velocity.x")?;
    require_finite(y, "velocity.y")?;
    movement.self_velocity[..2].copy_from_slice(&[x, y]);
    // Keep this setter on the same validation boundary as the other
    // script-facing movement commands.  In particular, a native proxy can
    // carry an invalid third component or attribute value even when the two
    // values supplied by the script are finite.
    validate_native(movement)
}

pub(crate) fn fall(movement: &mut Movement, gravity: f32, terminal: f32) -> Result<(), String> {
    require_finite(gravity, "gravity")?;
    require_finite(terminal, "terminal velocity")?;
    if gravity < 0.0 {
        return Err("gravity must be nonnegative".into());
    }
    if terminal < 0.0 {
        return Err("terminal velocity must be nonnegative".into());
    }
    movement.fall(gravity, terminal);
    validate_native(movement)
}

pub(crate) fn friction_air(movement: &mut Movement, friction: f32) -> Result<(), String> {
    require_finite(friction, "air friction")?;
    movement.friction_air(friction);
    validate_native(movement)
}

pub(crate) fn drift_clamp(
    movement: &mut Movement,
    maximum: f32,
    acceleration: f32,
) -> Result<bool, String> {
    require_finite(maximum, "drift maximum")?;
    require_finite(acceleration, "drift acceleration")?;
    let result = movement.drift_clamp(maximum, acceleration);
    validate_native(movement)?;
    Ok(result)
}

pub(crate) fn drift_or_friction(movement: &mut Movement, step: f32) -> Result<bool, String> {
    require_finite(step, "drift step")?;
    let result = crate::fighter::helpers::drift_or_friction_air(movement, step);
    validate_native(movement)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_velocity_revalidates_the_complete_native_view() {
        let mut movement = Movement::default();
        movement.self_velocity[2] = f32::NAN;

        let result = set_velocity(&mut movement, 1.25, -0.5);

        assert!(result.is_err());
        assert_eq!(movement.self_velocity[..2], [1.25, -0.5]);
    }

    #[test]
    fn fall_clamps_downward_speed_without_touching_horizontal_or_depth_velocity() {
        let mut movement = Movement {
            self_velocity: [2.0, -1.9, 0.25],
            ..Movement::default()
        };

        fall(&mut movement, 0.2, 2.0).unwrap();

        assert_eq!(movement.self_velocity, [2.0, -2.0, 0.25]);
    }

    #[test]
    fn fall_rejects_negative_gravity_and_terminal_velocity_before_mutation() {
        for (gravity, terminal) in [(-0.1, 2.0), (0.2, -2.0)] {
            let mut movement = Movement {
                self_velocity: [1.0, -0.5, 0.25],
                ..Movement::default()
            };
            let before = movement;

            assert!(fall(&mut movement, gravity, terminal).is_err());
            assert_eq!(movement, before);
        }
    }
}
