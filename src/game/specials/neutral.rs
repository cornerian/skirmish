//! The shared neutral-special ("B") action shell: a resource-driven ground/
//! air pose pair any character can supply, common to every fighter that has
//! one, unlike a character-specific move.

use super::{SpecialMove, helpers};
use crate::{
    fighter::special::neutral_input,
    game::{
        Action, Controller, Fighter,
        data::{Attack, FighterData},
        simulation,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    /// Ground neutral-B X/Y bounds from common data.
    pub neutral_thresholds: [f32; 2],
    pub ground: Attack,
    pub air: Attack,
}

pub(crate) fn attack(action: Action, parameters: &Parameters) -> Option<&Attack> {
    match action {
        Action::SpecialN => Some(&parameters.ground),
        Action::SpecialAirN => Some(&parameters.air),
        _ => None,
    }
}

/// The neutral shell's move handle. Any character whose data supplies a
/// `neutral` resource gets this behaviour for free by listing `&Move` in
/// its registry entry; there is nothing Fox-specific about it.
pub(crate) struct Move;

/// The registry-facing handle characters list in their `MOVES` slice.
pub(crate) const MOVE: Move = Move;

impl SpecialMove for Move {
    fn owns(&self, action: Action) -> bool {
        matches!(action, Action::SpecialN | Action::SpecialAirN)
    }

    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        attack(action, data.specials.as_ref()?.neutral()?)
    }

    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &crate::game::data::Rules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let _ = rules;
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.neutral()) else {
            return false;
        };
        if self.owns(fighter.action) {
            return true;
        }
        let pressed = input.buttons & !fighter.previous_input.buttons;
        if (ground || air) && neutral_input(pressed, input.stick, parameters.neutral_thresholds) {
            simulation::enter(
                fighter,
                if ground {
                    Action::SpecialN
                } else {
                    Action::SpecialAirN
                },
            );
            return true;
        }
        false
    }

    fn update_animation(&self, fighter: &mut Fighter, data: &FighterData) {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.neutral()) else {
            return;
        };
        let Some(pose) = attack(fighter.action, parameters) else {
            return;
        };
        if fighter.action_frame as usize >= pose.frames.len() {
            helpers::exit_to_wait_or_fall(fighter);
        }
    }

    fn transfer_ground_air(&self, fighter: &mut Fighter, grounded: bool) -> bool {
        let destination = match (fighter.action, grounded) {
            (Action::SpecialN, false) => Action::SpecialAirN,
            (Action::SpecialAirN, true) => Action::SpecialN,
            _ => return false,
        };
        // The neutral shell keeps no per-move state beyond the action frame
        // `transfer_frame` already preserves.
        helpers::transfer_frame(fighter, destination);
        true
    }
}
