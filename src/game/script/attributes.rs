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
    if let Some(value) = parameters.get("movement") {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("movement must be an object"))?;
        apply_movement(&mut fighter.movement, object)?;
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

fn invalid(message: &str) -> Error {
    Error::Data(format!("invalid fighter attributes: {message}"))
}
