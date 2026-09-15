//! Registration-time validation for canonical class move destinations.

use super::definition::cached_program;
use crate::{
    fighter::aerial,
    game::{Error, data::FighterData},
};

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
