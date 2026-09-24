//! Apply the typed fighter attributes exported by the class based SDK.
//!
//! The exported parameter map remains metadata except for the explicitly
//! native fields handled here.  Projection happens once while a match is
//! constructed, so gameplay never consults the script metadata per frame.

use super::definition::program_for_registration;
use crate::game::Error;
use crate::game::data::{FighterData, MatchData, MovementData};
use serde_json::Value;

pub(crate) fn apply(data: &mut MatchData) -> Result<(), Error> {
    for fighter in &mut data.fighters {
        let Some(program) =
            program_for_registration(fighter).map_err(|error| Error::Data(error.to_string()))?
        else {
            continue;
        };
        apply_fighter(fighter, &program.metadata().parameters)?;
    }
    Ok(())
}

fn apply_fighter(
    fighter: &mut FighterData,
    parameters: &std::collections::BTreeMap<String, Value>,
) -> Result<(), Error> {
    if let Some(value) = parameters.get("weight") {
        fighter.weight = number(value, "weight")?;
    }
    if let Some(value) = parameters.get("model_scaling") {
        apply_model_scaling(&mut fighter.model_scaling, value)?;
    }
    if let Some(value) = parameters.get("movement") {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("movement must be an object"))?;
        apply_movement(&mut fighter.movement, object)?;
    }
    if let Some(value) = parameters.get("can_walljump") {
        apply_can_walljump(fighter.wall_jump.as_mut(), value)?;
    }
    Ok(())
}

fn apply_movement(
    movement: &mut MovementData,
    values: &serde_json::Map<String, Value>,
) -> Result<(), Error> {
    for (name, value) in values {
        match name.as_str() {
            "ground_max_horizontal_velocity" => {
                movement.ground_max_horizontal_velocity = number(value, name)?
            }
            "air_max_horizontal_velocity" => {
                movement.air_max_horizontal_velocity = number(value, name)?
            }
            "air_drift_stick_mul" => movement.air_drift_stick_mul = number(value, name)?,
            "aerial_drift_base" => movement.aerial_drift_base = number(value, name)?,
            "air_drift_max" => movement.air_drift_max = number(value, name)?,
            "aerial_friction" => movement.aerial_friction = number(value, name)?,
            "gravity" => movement.gravity = number(value, name)?,
            "terminal_velocity" => movement.terminal_velocity = number(value, name)?,
            "fast_fall_velocity" => movement.fast_fall_velocity = number(value, name)?,
            "ground_friction" => movement.ground_friction = number(value, name)?,
            "walk_acceleration_mul" => movement.walk_acceleration_mul = number(value, name)?,
            "walk_acceleration_base" => movement.walk_acceleration_base = number(value, name)?,
            "walk_max_velocity" => movement.walk_max_velocity = number(value, name)?,
            "jump_startup_frames" => movement.jump_startup_frames = integer(value, name)?,
            "jump_vertical_velocity" => movement.jump_vertical_velocity = number(value, name)?,
            "short_hop_vertical_velocity" => {
                movement.short_hop_vertical_velocity = number(value, name)?
            }
            "jump_horizontal_velocity" => movement.jump_horizontal_velocity = number(value, name)?,
            "jump_horizontal_max" => movement.jump_horizontal_max = number(value, name)?,
            "jump_momentum_multiplier" => movement.jump_momentum_multiplier = number(value, name)?,
            "landing_frames" => movement.landing_frames = integer(value, name)?,
            "normal_landing_lag" => movement.normal_landing_lag = Some(number(value, name)?),
            _ => return Err(invalid(&format!("unknown movement attribute {name:?}"))),
        }
    }
    Ok(())
}

fn number(value: &Value, name: &str) -> Result<f32, Error> {
    let value = value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(&format!("attribute {name:?} must be a finite number")))?;
    let value = value as f32;
    value.is_finite().then_some(value).ok_or_else(|| {
        invalid(&format!(
            "attribute {name:?} is outside native number range"
        ))
    })
}

fn integer(value: &Value, name: &str) -> Result<u32, Error> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid(&format!("attribute {name:?} must be a nonnegative integer")))
}

fn boolean(value: &Value, name: &str) -> Result<bool, Error> {
    value
        .as_bool()
        .ok_or_else(|| invalid(&format!("attribute {name:?} must be a boolean")))
}

fn apply_can_walljump(
    profile: Option<&mut crate::game::wall_jump::Attributes>,
    value: &Value,
) -> Result<(), Error> {
    let can_walljump = boolean(value, "can_walljump")?;
    let profile =
        profile.ok_or_else(|| invalid("can_walljump requires a wall_jump attribute profile"))?;
    profile.can_walljump = can_walljump;
    Ok(())
}

fn apply_model_scaling(target: &mut f32, value: &Value) -> Result<(), Error> {
    *target = number(value, "model_scaling")?;
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::Data(format!("invalid fighter attributes: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::data::Bone;

    fn wall_jump_attributes(can_walljump: bool) -> crate::game::wall_jump::Attributes {
        crate::game::wall_jump::Attributes {
            can_walljump,
            minimum_approach_speed: 0.2,
            horizontal_velocity: 3.0,
            vertical_velocity: 4.0,
            blend_frames: 0,
            dynamics_variant: 0,
            frames: vec![vec![Bone {
                parent: None,
                classical_scale: false,
                translation: [0.0; 3],
                rotation: [0.0; 3],
                scale: [1.0; 3],
            }]],
        }
    }

    #[test]
    fn can_walljump_is_a_typed_fighter_attribute() {
        let mut profile = wall_jump_attributes(false);
        let value = serde_json::json!(true);
        apply_can_walljump(Some(&mut profile), &value).expect("projection");
        assert!(profile.can_walljump);
    }

    #[test]
    fn can_walljump_requires_the_source_profile() {
        let error = apply_can_walljump(None, &serde_json::json!(true))
            .expect_err("the flag cannot be projected without wall-jump data");
        assert!(
            error
                .to_string()
                .contains("can_walljump requires a wall_jump attribute profile")
        );
    }

    #[test]
    fn can_walljump_rejects_numeric_wire_values() {
        let error = boolean(&serde_json::json!(1), "can_walljump")
            .expect_err("numeric values must not coerce to booleans");
        assert!(
            error
                .to_string()
                .contains("attribute \"can_walljump\" must be a boolean")
        );
    }

    #[test]
    fn model_scaling_is_projected_as_the_collision_root_scale() {
        let mut model_scaling = 1.0;
        let value = serde_json::json!(0.75);
        apply_model_scaling(&mut model_scaling, &value).expect("finite scale");
        assert_eq!(model_scaling, 0.75);
    }
}
