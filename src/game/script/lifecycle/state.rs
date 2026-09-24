//! Validation and transactional commit for the native lifecycle state.
//!
//! The Starlark backend exposes custom safe values which write into a private
//! fighter clone. This module is deliberately unaware of that runtime: it
//! validates the completed clone and performs the final all-or-nothing commit.
//! Keeping validation here prevents a callback error or a malformed nested
//! value from partially changing simulation state.

use super::{Error, LocalState, MAX_LOCAL_KEY_BYTES, MAX_LOCAL_STRING_BYTES};
use crate::game::{self, Fighter};
use serde_json::Value;

pub(crate) const STATE_LIMIT: f32 = 1_000_000.0;
const MAX_STATE_VALUE_NODES: usize = 4096;

/// Validate a native fighter after all pending host writes and helper calls.
/// Serialization is also useful here: it traverses every serde-visible field,
/// including nested locomotion, shield, aerial and ECB state, without giving
/// the scripting runtime ownership of those structures.
pub(crate) fn validate(fighter: &Fighter) -> Result<(), Error> {
    validate_controller(fighter.previous_input)?;
    // serde_json represents non-finite Rust floats as JSON null.  That makes
    // a serialize-then-walk check unable to distinguish NaN/Inf from a real
    // optional field, so validate the hot top-level motion scalars before
    // serialization.  The nested state is still covered by the structural
    // serializer walk below.
    validate_native_scalars(fighter)?;
    let value = serde_json::to_value(fighter)
        .map_err(|error| Error::Invalid(format!("fighter state is not serializable: {error}")))?;
    validate_numbers(&value, "fighter", 0)
}

fn validate_native_scalars(fighter: &Fighter) -> Result<(), Error> {
    bounded("fighter.position", &fighter.position)?;
    bounded("fighter.deferred_position", &fighter.deferred_position)?;
    bounded("fighter.nudge", &fighter.nudge)?;
    bounded("fighter.velocity", &fighter.velocity)?;
    bounded("fighter.knockback", &fighter.knockback)?;
    bounded("fighter.floor_normal", &fighter.floor_normal)?;
    validate_nested_scalars(
        fighter.shield.health,
        fighter.aerial.mobility,
        fighter.aerial.landing_lag,
    )?;
    for (name, value) in [
        ("fighter.depth", fighter.depth),
        ("fighter.ground_knockback", fighter.ground_knockback),
        ("fighter.ground_velocity", fighter.ground_velocity),
        ("fighter.facing", fighter.facing),
        ("fighter.percent", fighter.percent),
        ("fighter.hitlag", fighter.hitlag),
    ] {
        validate_f32(name, value)?;
    }
    Ok(())
}

fn validate_nested_scalars(
    shield_health: f32,
    aerial_mobility: f32,
    landing_lag: Option<f32>,
) -> Result<(), Error> {
    validate_f32("fighter.shield.health", shield_health)?;
    validate_f32("fighter.aerial.mobility", aerial_mobility)?;
    if let Some(landing_lag) = landing_lag {
        validate_f32("fighter.aerial.landing_lag", landing_lag)?;
    }
    Ok(())
}

/// Commit a validated clone. The source remains untouched when validation
/// fails, so callers can use this directly for lifecycle transactions.
pub(crate) fn commit(candidate: &Fighter, destination: &mut Fighter) -> Result<(), Error> {
    validate(candidate)?;
    *destination = candidate.clone();
    Ok(())
}

pub(crate) fn validate_controller(controller: game::Controller) -> Result<(), Error> {
    bounded("input.stick", &controller.stick)?;
    bounded("input.cstick", &controller.cstick)?;
    if controller
        .stick
        .into_iter()
        .chain(controller.cstick)
        .any(|value| !(-1.0..=1.0).contains(&value))
        || !controller.trigger.is_finite()
        || !(0.0..=1.0).contains(&controller.trigger)
    {
        return Err(Error::Invalid("input analog values out of range".into()));
    }
    Ok(())
}

pub(crate) fn bounded(name: &str, values: &[f32]) -> Result<(), Error> {
    if values
        .iter()
        .all(|value| value.is_finite() && value.abs() <= STATE_LIMIT)
    {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "invalid {name}: non-finite or out of range"
        )))
    }
}

pub(crate) fn validate_numbers(value: &Value, name: &str, depth: usize) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::Invalid(format!("{name} is too deeply nested")));
    }
    match value {
        Value::Number(number) => {
            let number = number
                .as_f64()
                .ok_or_else(|| Error::Invalid(format!("invalid numeric value in {name}")))?;
            if !number.is_finite() || number.abs() > f64::from(STATE_LIMIT) {
                return Err(Error::Invalid(format!("invalid numeric value in {name}")));
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_numbers(value, name, depth + 1)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_numbers(value, &format!("{name}.{key}"), depth + 1)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
    Ok(())
}

/// Validate persistent and per-action script state before it is copied into
/// a checkpoint. State is intentionally scalar and bounded; arbitrary nested
/// Starlark objects never become persistent gameplay state.
pub(crate) fn validate_state(state: &LocalState) -> Result<(), Error> {
    if state.len() > super::MAX_LOCALS {
        return Err(Error::Invalid("too many script state fields".into()));
    }
    let mut budget = MAX_STATE_VALUE_NODES;
    for (key, value) in state {
        if key.is_empty() || key.len() > MAX_LOCAL_KEY_BYTES {
            return Err(Error::Invalid("script state key is too long".into()));
        }
        if !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(Error::Invalid(format!("invalid script state key {key:?}")));
        }
        // Native animation command variables are a fixed four-slot register
        // file (`cmd_vars[0..3]` in the decomp).  Keep the projected action
        // state ABI equally strict so callbacks cannot observe a short or
        // mixed-type command tuple and silently fall back to zeroes.
        if key == "command" {
            validate_command_state(value)?;
        }
        match value {
            super::LocalValue::Bool(_) => {}
            super::LocalValue::Integer(value) => {
                if (*value as f64).abs() > f64::from(STATE_LIMIT) {
                    return Err(Error::Invalid(format!(
                        "invalid integer state field {key:?}"
                    )));
                }
            }
            super::LocalValue::Number(number) => {
                if !number.is_finite() || number.abs() > f64::from(STATE_LIMIT) {
                    return Err(Error::Invalid(format!(
                        "invalid numeric state field {key:?}"
                    )));
                }
            }
            super::LocalValue::String(value) => {
                if value.len() > MAX_LOCAL_STRING_BYTES {
                    return Err(Error::Invalid(format!(
                        "script state string {key:?} is too long"
                    )));
                }
            }
            super::LocalValue::Tuple(values) => {
                if values.len() > super::MAX_LOCALS {
                    return Err(Error::Invalid(format!(
                        "script state tuple {key:?} is too long"
                    )));
                }
                for nested in values {
                    validate_local_value(nested, key, 1, &mut budget)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_command_state(value: &super::LocalValue) -> Result<(), Error> {
    let super::LocalValue::Tuple(values) = value else {
        return Err(Error::Invalid(
            "command state must be a four-slot integer tuple".into(),
        ));
    };
    if values.len() != 4
        || values
            .iter()
            .any(|value| !matches!(value, super::LocalValue::Integer(_)))
    {
        return Err(Error::Invalid(
            "command state must be a four-slot integer tuple".into(),
        ));
    }
    Ok(())
}

fn validate_local_value(
    value: &super::LocalValue,
    key: &str,
    depth: usize,
    budget: &mut usize,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::Invalid(format!(
            "script state tuple {key:?} is too deeply nested"
        )));
    }
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| Error::Invalid("script state contains too many tuple values".into()))?;
    match value {
        super::LocalValue::Bool(_) => Ok(()),
        super::LocalValue::Integer(value) if (*value as f64).abs() <= f64::from(STATE_LIMIT) => {
            Ok(())
        }
        super::LocalValue::Integer(_) => Err(Error::Invalid(format!(
            "invalid integer state field {key:?}"
        ))),
        super::LocalValue::Number(value)
            if value.is_finite() && value.abs() <= f64::from(STATE_LIMIT) =>
        {
            Ok(())
        }
        super::LocalValue::Number(_) => Err(Error::Invalid(format!(
            "invalid numeric state field {key:?}"
        ))),
        super::LocalValue::String(value) if value.len() <= MAX_LOCAL_STRING_BYTES => Ok(()),
        super::LocalValue::String(_) => Err(Error::Invalid(format!(
            "script state string {key:?} is too long"
        ))),
        super::LocalValue::Tuple(values) => {
            if values.len() > super::MAX_LOCALS {
                return Err(Error::Invalid(format!(
                    "script state tuple {key:?} is too long"
                )));
            }
            values
                .iter()
                .try_for_each(|value| validate_local_value(value, key, depth + 1, budget))
        }
    }
}

/// Check a single value before writing it through a native host field.
pub(crate) fn validate_f32(name: &str, value: f32) -> Result<f32, Error> {
    bounded(name, std::slice::from_ref(&value)).map(|()| value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_validation_rejects_nonfinite_values_before_json_projection() {
        assert!(validate_f32("fighter.position", f32::NAN).is_err());
        assert!(validate_f32("fighter.position", f32::INFINITY).is_err());
        assert!(validate_f32("fighter.position", f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn scalar_validation_preserves_signed_zero_and_state_edges() {
        assert_eq!(
            validate_f32("fighter.velocity", -0.0)
                .expect("signed zero should remain valid")
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            validate_f32("fighter.velocity", 0.0)
                .expect("positive zero should remain valid")
                .to_bits(),
            0.0_f32.to_bits()
        );
        assert_eq!(
            validate_f32("fighter.percent", STATE_LIMIT)
                .expect("positive state edge should remain valid"),
            STATE_LIMIT
        );
        assert_eq!(
            validate_f32("fighter.percent", -STATE_LIMIT)
                .expect("negative state edge should remain valid"),
            -STATE_LIMIT
        );
    }

    #[test]
    fn nested_mutable_scalars_reject_nonfinite_values_before_json_projection() {
        assert!(validate_nested_scalars(f32::NAN, 0.0, Some(0.0)).is_err());
        assert!(validate_nested_scalars(0.0, f32::INFINITY, Some(0.0)).is_err());
        assert!(validate_nested_scalars(0.0, 0.0, Some(f32::NEG_INFINITY)).is_err());
        assert!(validate_nested_scalars(-0.0, 0.0, Some(-0.0)).is_ok());
    }

    #[test]
    fn state_validation_is_transaction_safe_for_rejected_nested_values() {
        let original = LocalState::from([("stable".into(), super::super::LocalValue::Integer(7))]);
        let mut candidate = original.clone();
        candidate.insert(
            "invalid".into(),
            super::super::LocalValue::Number(f64::INFINITY),
        );
        assert!(validate_state(&candidate).is_err());
        assert_eq!(original["stable"], super::super::LocalValue::Integer(7));
        assert!(!original.contains_key("invalid"));
    }

    #[test]
    fn command_state_matches_native_four_slot_registers() {
        let valid = LocalState::from([(
            "command".into(),
            super::super::LocalValue::Tuple(vec![
                super::super::LocalValue::Integer(0),
                super::super::LocalValue::Integer(1),
                super::super::LocalValue::Integer(2),
                super::super::LocalValue::Integer(3),
            ]),
        )]);
        assert!(validate_state(&valid).is_ok());

        for value in [
            super::super::LocalValue::Tuple(vec![super::super::LocalValue::Integer(0); 3]),
            super::super::LocalValue::Tuple(vec![
                super::super::LocalValue::Integer(0),
                super::super::LocalValue::Bool(false),
                super::super::LocalValue::Integer(0),
                super::super::LocalValue::Integer(0),
            ]),
            super::super::LocalValue::Integer(0),
        ] {
            let state = LocalState::from([("command".into(), value)]);
            assert!(validate_state(&state).is_err());
        }
    }
}
