//! Ordinary aerial callbacks composed with explicit sampled physics resources.
//! Item throws, character overrides and the complete interrupt chain are separate.
use super::{
    Action, BUTTON_A, Controller, Error, Fighter,
    data::{Attack, Bone, FighterData},
};
use crate::fighter::aerial::{self as math, Direction, SelectionRules};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub selection: SelectionRules,
    pub l_cancel_window: i32,
    pub l_cancel_divisor: f32,
    /// Neutral, forward, back, up, down. No implicit fallback between actions.
    pub moves: [Move; 5],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Move {
    pub attack: Attack,
    /// One decoded command-state sample per attack pose, including recovery.
    pub flags: Vec<FrameFlags>,
    pub landing_lag: f32,
    pub landing_animation_end: f32,
    /// Integer animation-frame physics poses, covering the declared end frame.
    pub landing_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameFlags {
    pub landing_lag: bool,
    pub allow_interrupt: bool,
    /// A decoded throwB3 event, consumed once even when hitlag freezes this frame.
    pub reverse_facing: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub applied_frame: Option<u32>,
    pub landing_lag_enabled: bool,
    pub allow_interrupt: bool,
    pub landing_elapsed: f32,
    pub landing_rate: f32,
}

const ATTACKS: [Action; 5] = [
    Action::AttackAirN,
    Action::AttackAirF,
    Action::AttackAirB,
    Action::AttackAirHi,
    Action::AttackAirLw,
];
const LANDINGS: [Action; 5] = [
    Action::LandingAirN,
    Action::LandingAirF,
    Action::LandingAirB,
    Action::LandingAirHi,
    Action::LandingAirLw,
];

pub(crate) fn attack_index(action: Action) -> Option<usize> {
    ATTACKS.iter().position(|&a| a == action)
}
pub(crate) fn landing_index(action: Action) -> Option<usize> {
    LANDINGS.iter().position(|&a| a == action)
}

pub(crate) fn interruptible(fighter: &Fighter) -> bool {
    attack_index(fighter.action).is_some() && fighter.aerial.allow_interrupt
}

fn commands(f: &mut Fighter, movement: &Move) {
    if f.aerial.applied_frame == Some(f.action_frame) {
        return;
    }
    let flags = movement.flags[f.action_frame as usize];
    f.aerial.applied_frame = Some(f.action_frame);
    f.aerial.landing_lag_enabled = flags.landing_lag;
    f.aerial.allow_interrupt = flags.allow_interrupt;
    if flags.reverse_facing {
        f.facing = -f.facing;
    }
}

/// Install the destination action before any of its input callbacks run.
pub(crate) fn update_animation(f: &mut Fighter, data: &FighterData) {
    let Some(p) = &data.aerials else {
        return;
    };
    if let Some(index) = landing_index(f.action) {
        f.aerial.landing_elapsed += f.aerial.landing_rate;
        if f.aerial.landing_elapsed < p.moves[index].landing_animation_end {
            return;
        }
        super::simulation::enter(f, Action::Wait);
        return;
    }
    if let Some(index) = attack_index(f.action) {
        if f.action_frame as usize >= p.moves[index].attack.frames.len() {
            super::simulation::enter(f, Action::Fall);
        } else {
            commands(f, &p.moves[index]);
        }
    }
}

/// Returns true while this callback owns action dispatch. Selection precedes
/// ordinary aerial jump input in the supported source interrupt branches.
pub(crate) fn update(f: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let Some(p) = &data.aerials else {
        return false;
    };
    if landing_index(f.action).is_some()
        || (attack_index(f.action).is_some() && !f.aerial.allow_interrupt)
    {
        return true;
    }
    if f.grounded
        || !(matches!(
            f.action,
            Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
        ) || attack_index(f.action).is_some())
    {
        return false;
    }
    if input.buttons & !f.previous_input.buttons & BUTTON_A != 0
        || math::fresh_cstick(
            f.previous_input.cstick,
            input.cstick,
            p.selection.thresholds,
        )
    {
        let direction = math::select(
            input.stick,
            input.cstick,
            f.previous_input.cstick,
            f.facing,
            &p.selection,
        );
        let index = match direction {
            Direction::Neutral => 0,
            Direction::Forward => 1,
            Direction::Back => 2,
            Direction::Up => 3,
            Direction::Down => 4,
        };
        super::simulation::enter(f, ATTACKS[index]);
        commands(f, &p.moves[index]);
        return true;
    }
    if attack_index(f.action).is_some() {
        super::locomotion::try_aerial_jump(f, data, input);
        return true;
    }
    false
}

/// Called by the floor response before changing the aerial action or flags.
pub(crate) fn land(f: &mut Fighter, data: &FighterData) -> Result<bool, Error> {
    let Some(index) = attack_index(f.action) else {
        return Ok(false);
    };
    let p = data
        .aerials
        .as_ref()
        .ok_or_else(|| Error::Data("missing aerial resources".into()))?;
    if !f.aerial.landing_lag_enabled {
        return Ok(false);
    }
    let movement = &p.moves[index];
    let lag = math::landing_lag(
        movement.landing_lag,
        f.locomotion.trigger_age,
        p.l_cancel_window,
        p.l_cancel_divisor,
    )
    .map_err(|e| Error::Physics(e.to_string()))?;
    super::simulation::enter(f, LANDINGS[index]);
    f.aerial.landing_rate = math::landing_animation_rate(movement.landing_animation_end, lag);
    Ok(true)
}

pub(crate) fn landing_pose<'a>(f: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    let index = landing_index(f.action)?;
    data.aerials.as_ref()?.moves[index]
        .landing_poses
        .get(f.aerial.landing_elapsed as usize)
}
