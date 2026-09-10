//! Resource-driven neutral-special action pair for headless matches.

use super::{Action, Controller, Fighter, data::Attack};
use crate::fighter::special::neutral_input;
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

pub(crate) fn update_animation(fighter: &mut Fighter, parameters: Option<&Parameters>) {
    let Some(parameters) = parameters else { return };
    let Some(attack) = attack(fighter.action, parameters) else {
        return;
    };
    if fighter.action_frame as usize >= attack.frames.len() {
        super::simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
    }
}

pub(crate) fn update_actions(
    fighter: &mut Fighter,
    parameters: Option<&Parameters>,
    input: Controller,
) -> bool {
    let Some(parameters) = parameters else {
        return false;
    };
    if owns_action(fighter.action) {
        return true;
    }
    let ground = fighter.grounded
        && matches!(
            fighter.action,
            Action::Wait
                | Action::Walk
                | Action::Dash
                | Action::Run
                | Action::RunBrake
                | Action::Turn
                | Action::Squat
                | Action::SquatWait
                | Action::SquatRv
        );
    let air = !fighter.grounded
        && (super::damage::wall_tech_interruptible(fighter)
            || matches!(
                fighter.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || super::aerial::interruptible(fighter));
    let pressed = input.buttons & !fighter.previous_input.buttons;
    if (ground || air) && neutral_input(pressed, input.stick, parameters.neutral_thresholds) {
        super::simulation::enter(
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

pub(crate) fn transfer_ground_air(fighter: &mut Fighter, grounded: bool) -> bool {
    let destination = match (fighter.action, grounded) {
        (Action::SpecialN, false) => Action::SpecialAirN,
        (Action::SpecialAirN, true) => Action::SpecialN,
        _ => return false,
    };
    let frame = fighter.action_frame;
    super::simulation::enter(fighter, destination);
    fighter.action_frame = frame;
    true
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::SpecialN | Action::SpecialAirN)
}
