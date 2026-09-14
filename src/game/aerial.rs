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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub applied_frame: Option<u32>,
    pub landing_lag_enabled: bool,
    pub allow_interrupt: bool,
    pub landing_elapsed: f32,
    pub landing_rate: f32,
    /// `mv.co.fallspecial.mobility` is `ca->air_drift_max * mobility`
    /// (`ftCo_FallSpecial.c:55-59`); this field stores the multiplier, so
    /// the ordinary air-drift maximum is recovered unscaled by default.
    /// Every currently modeled `FallSpecial` entry but the Fox/Falco side
    /// special's own End phase passes the source's literal `mobility == 1`.
    pub mobility: f32,
    /// `mv.co.fallspecial.landing_lag`: `ftCo_80096900`/`ftCo_800969D8`'s own
    /// `landing_lag` argument, stored per `FallSpecial` instance and read
    /// back by its own eventual landing (`ftCo_FallSpecial_Coll` ->
    /// `ftCo_LandingFallSpecial_Enter(gobj, ..., fp->mv.co.fallspecial.
    /// landing_lag)`), exactly like `mobility` above -- not a single
    /// match-wide constant. `None` (the default, matching every entry that
    /// does not thread its own value through `helpers::enter_fall_special`)
    /// keeps this crate's prior behavior of reading `escape_air::Rules`'
    /// own `landing_lag` instead, at the landing site.
    pub landing_lag: Option<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            applied_frame: None,
            landing_lag_enabled: false,
            allow_interrupt: false,
            landing_elapsed: 0.0,
            landing_rate: 0.0,
            mobility: 1.0,
            landing_lag: None,
        }
    }
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
    // `attack_sample_index` compensates for the entry-time advance
    // (`update`'s own `action_frame = 1`): this move's own `flags` array is
    // indexed by elapsed action frames since entry, not decomp's raw
    // `cur_anim_frame`.
    let flags = movement.flags[attack_sample_index(f.action_frame) as usize];
    f.aerial.applied_frame = Some(f.action_frame);
    f.aerial.landing_lag_enabled = flags.landing_lag;
    f.aerial.allow_interrupt = flags.allow_interrupt;
    if flags.reverse_facing {
        f.facing = -f.facing;
    }
}

/// `ftCo_AttackAir_EnterFromMsid`/`_EnterFromCStick` (`ftCo_AttackAir.c`)
/// both call `Fighter_ChangeMotionState` then `ftAnim_8006EBA4(gobj)`
/// explicitly, the same extra advance `ftCo_Dash_Enter`/`ftCo_Turn_Enter`
/// make (`locomotion::start_dash`'s own comment, `docs/validation.md`'s
/// entry-advance table). `update`'s own aerial-attack entry models that at
/// the source (`action_frame = 1`, not `0`, from the entry frame on), but
/// unlike Dash/Turn -- whose own duration thresholds are themselves read
/// directly from decomp's `cur_anim_frame` and so need no adjustment --
/// this move's own supplied `Move.attack.frames`/`Move.flags` sample
/// arrays are indexed by elapsed action frames since entry (confirmed by
/// the existing hitbox/autocancel/interrupt tests, which regress under raw
/// `action_frame` indexing): both this module's own consumers and
/// `simulation::attack_frame`'s shared lookup subtract 1 to compensate.
pub(crate) fn attack_sample_index(action_frame: u32) -> u32 {
    action_frame.saturating_sub(1)
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
        if attack_sample_index(f.action_frame) as usize >= p.moves[index].attack.frames.len() {
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
    let damage_air = super::damage::damage_air_interruptible(f);
    let wall_tech = super::damage::wall_tech_interruptible(f);
    if f.grounded
        || !(damage_air
            || wall_tech
            || matches!(
                f.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || attack_index(f.action).is_some())
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
        // `ftCo_AttackAir_EnterFromMsid`/`_EnterFromCStick` (`ftCo_AttackAir.c`)
        // both call `Fighter_ChangeMotionState` then `ftAnim_8006EBA4(gobj)`
        // explicitly -- the same extra advance already fixed at the source
        // for Dash/Turn/Squat's own entries (`locomotion::start_dash`'s own
        // comment, `docs/validation.md`'s entry-advance table): `action_frame`
        // is 1 (not 0) from this frame on, so `attack_frame`'s own raw-
        // indexed `Attack.frames` lookup (shared with jab/tilt/smash, whose
        // own entries already assume this) and this move's own `flags`
        // sample below both already read decomp's own `cur_anim_frame`
        // unadjusted, needing no compensating shift.
        f.action_frame = 1;
        commands(f, &p.moves[index]);
        return true;
    }
    if attack_index(f.action).is_some() || wall_tech || damage_air {
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
    let l_cancelled = i32::from(f.locomotion.trigger_age) < p.l_cancel_window;
    let lag = math::landing_lag(
        movement.landing_lag,
        f.locomotion.trigger_age,
        p.l_cancel_window,
        p.l_cancel_divisor,
    )
    .map_err(|e| Error::Physics(e.to_string()))?;
    super::simulation::enter(f, LANDINGS[index]);
    f.l_cancel_status = if l_cancelled { 1 } else { 2 };
    f.aerial.landing_rate = math::landing_animation_rate(movement.landing_animation_end, lag);
    Ok(true)
}

pub(crate) fn landing_pose<'a>(f: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    let index = landing_index(f.action)?;
    data.aerials.as_ref()?.moves[index]
        .landing_poses
        .get(f.aerial.landing_elapsed as usize)
}
