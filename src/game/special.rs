//! Resource-driven neutral-special action pair for headless matches.

use super::{Action, Controller, Fighter, data::Attack, data::FighterData};
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
    data: &FighterData,
    rules: &super::data::Rules,
    input: Controller,
) -> bool {
    let ground = fighter.grounded
        && (matches!(
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
        ) || matches!(
            super::tilt::interrupt_chain(fighter, data),
            Some(super::tilt::Chain::Wait) | Some(super::tilt::Chain::Taunt)
        ));
    let air = !fighter.grounded
        && (super::damage::wall_tech_interruptible(fighter)
            || super::damage::damage_air_interruptible(fighter)
            || matches!(
                fighter.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || super::aerial::interruptible(fighter));
    // ftCo_SpecialS_CheckInput/ftCo_SpecialAir_CheckInput's side branch is
    // the first check of every chain that also reaches the neutral branch
    // below (`ftCo_SpecialS.c:15-49`, `ftCo_SpecialAir.c:11-56`).
    if super::fox_side_special::update_actions(
        fighter,
        data,
        rules.specials.as_ref(),
        ground,
        air,
        input,
    ) {
        return true;
    }
    let Some(parameters) = data.special.as_ref() else {
        return false;
    };
    if owns_action(fighter.action) {
        return true;
    }
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
        // ftFx_SpecialSStart_GroundToAir/AirToGround, ftFx_SpecialS_
        // GroundToAir/AirToGround: `Fighter_ChangeMotionState`/
        // `ftCommon_AirToGroundStateChange` at the current animation frame.
        // The End phase's own conversions are asymmetric (ground leaving
        // the floor enters ordinary Fall; air landing enters
        // LandingFallSpecial directly), so SpecialSEnd/SpecialAirSEnd are
        // deliberately absent here (`fox_side_special::land`, and the
        // ordinary `Action::Fall` fallback at this function's ground-leaving
        // call site).
        (Action::SpecialSStart, false) => Action::SpecialAirSStart,
        (Action::SpecialAirSStart, true) => Action::SpecialSStart,
        (Action::SpecialS, false) => Action::SpecialAirS,
        (Action::SpecialAirS, true) => Action::SpecialS,
        _ => return false,
    };
    let frame = fighter.action_frame;
    // mv.fx.SpecialS.gravityDelay survives this conversion untouched in the
    // source (the GroundToAir/AirToGround handlers never touch it); `enter`
    // otherwise resets it like every other per-move state.
    let gravity_delay = fighter.fox_side_special.gravity_delay;
    super::simulation::enter(fighter, destination);
    fighter.action_frame = frame;
    fighter.fox_side_special.gravity_delay = gravity_delay;
    true
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::SpecialN | Action::SpecialAirN)
}
