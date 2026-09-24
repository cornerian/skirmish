//! Registration-time validation for canonical class move destinations.

use super::definition::cached_program;
use crate::{
    fighter::aerial,
    game::{Action, Error, data::FighterData},
};

fn validate_source_state_reference(action: Action, state: Option<u32>) -> Result<(), Error> {
    if action.custom_id().is_some() && state.is_none() {
        return Err(Error::Data(format!(
            "canonical source action {action:?} has no Slippi state"
        )));
    }
    Ok(())
}

fn validate_source_state_profile(
    action: Action,
    state: Option<u32>,
    profiles: Option<&[crate::game::data::MotionStateProfile]>,
) -> Result<(), Error> {
    validate_source_state_reference(action, state)?;
    let Some(state) = state else {
        return Ok(());
    };
    let Some(profiles) = profiles else {
        return Err(Error::Data(format!(
            "canonical source action {action:?} has no motion state profile"
        )));
    };
    if !profiles.iter().any(|profile| profile.state_id == state) {
        return Err(Error::Data(format!(
            "canonical source action {action:?} state {state} has no motion state profile"
        )));
    }
    Ok(())
}

/// Ensure every canonical move link has the native resource its destination
/// action consumes. Callback-only entries deliberately have no requirement.
pub(crate) fn validate(data: &FighterData) -> Result<(), Error> {
    let Some(program) = cached_program(data) else {
        return Ok(());
    };
    for (group, value) in &program.metadata().movesets {
        let Some(slots) = value.as_object() else {
            continue;
        };
        for (slot, _) in slots {
            let Some(entry) = program.moves().resolve(group, slot) else {
                continue;
            };
            let Some(action) = entry.canonical else {
                continue;
            };
            validate_source_state_profile(
                action,
                program
                    .metadata()
                    .action(action)
                    .and_then(|definition| definition.slippi_state),
                data.motion_states.as_deref(),
            )?;
            // Aerial entry actions are consumed directly by the aerial
            // dispatcher, which indexes this native five-slot array. Other
            // action families have distinct ownership/resource contracts;
            // leave those to their existing validators until a shared
            // destination resolver exists for them.
            let Some(index) = aerial::attack_index(action) else {
                continue;
            };
            let available = data
                .aerials
                .as_ref()
                .and_then(|parameters| parameters.moves.get(index))
                .is_some();
            if !available {
                return Err(Error::Data(format!(
                    "moveset {group:?}.{slot:?} canonical action {action:?} has no native resource"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_source_state_profile, validate_source_state_reference};
    use crate::game::{Action, CustomActionId, Error, data::MotionStateProfile};

    #[test]
    fn source_state_validation_only_applies_to_custom_actions() {
        assert!(matches!(
            validate_source_state_reference(Action::Wait, None),
            Ok(())
        ));
        assert!(matches!(
            validate_source_state_reference(Action::Custom(CustomActionId::new(7)), None),
            Err(Error::Data(message)) if message.contains("no Slippi state")
        ));
        assert!(matches!(
            validate_source_state_reference(Action::Custom(CustomActionId::new(7)), Some(381)),
            Ok(())
        ));
    }

    #[test]
    fn canonical_custom_state_requires_a_matching_motion_profile() {
        let action = Action::Custom(CustomActionId::new(341));
        let profiles = [MotionStateProfile {
            state_id: 340,
            animation_id: -1,
            move_id: 0,
            flags: 0,
        }];
        assert!(matches!(
            validate_source_state_profile(action, Some(341), Some(&profiles)),
            Err(Error::Data(message)) if message.contains("no motion state profile")
        ));
        assert!(matches!(
            validate_source_state_profile(action, Some(341), None),
            Err(Error::Data(message)) if message.contains("no motion state profile")
        ));
    }
}
