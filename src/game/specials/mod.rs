//! Shared framework for B-move ("special") actions.
//!
//! A special move is a small state machine: it owns a set of `Action`
//! variants, decides when a fresh input starts it, drives its own animation
//! and physics while active, and hands control back to the ordinary
//! locomotion/aerial state machine when it ends. Every move shares the same
//! surrounding rules -- which grounded/aerial chains may start a special this
//! frame, in what priority, and how a mid-move ground/air transition keeps
//! the current animation frame -- so that shared shell lives once, here,
//! instead of being copied into each move.
//!
//! Adding a new move (a queued Fox up/down special, or eventually another
//! character) means: write its own file implementing [`SpecialMove`], list
//! it in its character's registry entry in `game::characters`, and add its
//! Slippi state/animation ids to the observation table. Nothing in this
//! module, `simulation`, `collision`, `edge` or `ledge` needs to change.

pub mod helpers;
pub mod neutral;

use super::{
    Action, Controller, Error, Fighter, characters,
    data::{Attack, FighterData, Rules},
    tilt,
};
use crate::fighter::{Movement, edge::Mode};

/// Every phase hook a special move can implement. Callers precompute the
/// shared eligibility booleans once per frame (`grounded_chain_open`/
/// `aerial_chain_open` below) and hand them to each move in a fixed
/// priority order, matching the source's own before-the-neutral-branch
/// dispatch for character specials.
///
/// Every method defaults to "this move has nothing to say about that";
/// a move only overrides the hooks its phases actually use.
pub(crate) trait SpecialMove {
    /// True while `action` is one of this move's own phases.
    fn owns(&self, action: Action) -> bool {
        let _ = action;
        false
    }

    /// The sampled pose/hitbox attack for `action`, if this move owns it.
    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        let _ = (action, data);
        None
    }

    /// Start or continue this move from this frame's input. `ground`/`air`
    /// report whether the fighter's current action chain is one that may
    /// open a fresh special this frame (computed once by
    /// [`grounded_chain_open`]/[`aerial_chain_open`], not per move); `rules`
    /// is the match-wide configuration (stick thresholds shared by every
    /// fighter), separate from this fighter's own `data`. Returns whether
    /// this move consumed the frame's action dispatch.
    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &Rules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let _ = (fighter, data, rules, ground, air, input);
        false
    }

    /// Advance this move's animation-driven transitions (natural phase end).
    /// `input` is this frame's already-sampled controller state (some
    /// transitions, like Fox's up-special launch angle, read the stick at
    /// the moment the source's own Anim callback fires rather than at the
    /// move's original entry); `on_platform` reports whether the fighter's
    /// current supporting line is a passable platform rather than solid
    /// ground (Fox's up special's grounded-launch-vs-leave-ground decision,
    /// `ftCo_8009A134`).
    fn update_animation(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        input: Controller,
        on_platform: bool,
    ) {
        let _ = (fighter, data, input, on_platform);
    }

    /// A ground-target velocity this move's current pose dictates directly
    /// (root-motion-driven dashes), ahead of ordinary friction.
    fn ground_target_velocity(&self, fighter: &Fighter, data: &FighterData) -> Option<f32> {
        let _ = (fighter, data);
        None
    }

    /// A ground friction value this move's current phase uses instead of
    /// the fighter's ordinary attribute.
    fn ground_friction_override(&self, fighter: &Fighter, data: &FighterData) -> Option<f32> {
        let _ = (fighter, data);
        None
    }

    /// Drive this move's airborne physics for the frame. `true` means this
    /// move owns the frame's airborne physics entirely (the caller's
    /// ordinary fast-fall/drift/damage-lock chain must not also run).
    /// `rules` is the match-wide configuration (stick thresholds and other
    /// shared common data, like `update_actions`'s own `rules` argument),
    /// separate from this fighter's own `data`.
    fn air_physics(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &Rules,
        movement: &mut Movement,
    ) -> bool {
        let _ = (fighter, data, rules, movement);
        false
    }

    /// Tick any per-frame countdown this move's ground phases keep running
    /// even though the ground never reads the counted-down value, so a
    /// mid-move ground/air conversion observes the same countdown the air
    /// phase would have reached.
    fn tick_ground_timers(&self, fighter: &mut Fighter) {
        let _ = fighter;
    }

    /// Re-derive any per-frame state this move's grounded phase keeps
    /// reading off the *current* floor contact (Fox's up special's own
    /// `rotateModel`, continuously updated from the floor normal while
    /// Travel runs along a slope, `ftFx_SpecialHi_Coll`). Called once per
    /// frame after collision resolution has finished updating
    /// `fighter.floor_normal`/`fighter.grounded` for this frame -- unlike
    /// `tick_ground_timers` (called from the physics step, before this
    /// frame's own collision resolution, so it only ever sees last frame's
    /// contact), this hook sees the fresh one.
    fn update_ground_contact(&self, fighter: &mut Fighter) {
        let _ = fighter;
    }

    /// Convert this move's current phase between its grounded and aerial
    /// variant at the same animation frame, preserving any phase-local
    /// state that would otherwise be reset by a fresh `enter`. Returns
    /// whether this move owned the conversion.
    fn transfer_ground_air(&self, fighter: &mut Fighter, grounded: bool) -> bool {
        let _ = (fighter, grounded);
        false
    }

    /// Handle this move's own landing conversion, when it differs from the
    /// generic ground/air transfer above (a phase that lands directly into
    /// a shared landing action rather than its own grounded counterpart, or
    /// needs a data- and geometry-dependent decision the plain conversion
    /// above cannot make). `on_platform` reports whether the fighter's new
    /// supporting line is a passable platform (Fox's up special's Travel
    /// landing bound-eligibility gate, `ftCo_8009A134`/`ftFox_SpecialHi_
    /// IsBound`). `pre_landing` is a snapshot of `fighter` from just before
    /// the caller's own generic landing bookkeeping ran (grounded/velocity/
    /// knockback/jumps/wall-jump state all as they were the instant the
    /// floor was actually touched) -- a move that declines the landing
    /// outright (Fox's up special's shallow-floor-angle graze, staying
    /// airborne) restores from it (`*fighter = pre_landing.clone()`) rather
    /// than hand-reverting individual fields, then applies whatever it
    /// still wants changed (facing, its own rotate-model state) on top.
    /// Returns whether this move owned the landing.
    fn land(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        on_platform: bool,
        pre_landing: &Fighter,
    ) -> Result<bool, Error> {
        let _ = (fighter, data, on_platform, pre_landing);
        Ok(false)
    }

    /// The floor-end collision mode this move's phase uses, when it
    /// overrides the ordinary per-action table in `edge::mode_for_action`.
    fn collision_mode(&self, action: Action) -> Option<Mode> {
        let _ = action;
        None
    }

    /// Whether this move's aerial phase reuses the ordinary ledge-catch
    /// scan, like the other aerial actions.
    fn ledge_catchable(&self, action: Action) -> bool {
        let _ = action;
        false
    }

    /// Whether this move's aerial phase wants its own mid-air wall/ceiling
    /// contact response instead of the ordinary auto-zero-into-wall one
    /// (Fox's up special's own Travel-air redirect, `ftFx_SpecialAirHi_
    /// Coll`'s non-ground branch) -- mirrors `damage::can_surface_tech`/
    /// `can_reflect`'s own role gating the *existing* tech/reflect
    /// response, just keyed by action alone since this needs no fighter
    /// state to decide.
    fn wants_redirect(&self, action: Action) -> bool {
        let _ = action;
        false
    }

    /// Handle a mid-air ceiling/wall contact this move made itself
    /// eligible for via `wants_redirect` (`ftFx_SpecialAirHi_Coll`'s own
    /// non-ground branch: ceiling checked first, then whichever wall).
    /// Each candidate is `Some((contact_normal, line_id))` when that
    /// surface was actually touched this step. Returns whether this move
    /// consumed the contact (suppressing the ordinary auto-zero-into-wall
    /// response this frame, like a successful tech or reflect would).
    fn air_contact(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        ceiling: Option<([f32; 3], usize)>,
        wall: Option<([f32; 3], usize)>,
    ) -> bool {
        let _ = (fighter, data, ceiling, wall);
        false
    }
}

/// Every move any registered character can play, independent of which
/// fighter's resources enable it this match. Bare `Action`-keyed queries
/// (collision mode, ledge catchability, phase ownership, ground/air
/// conversion) only need to know which move a state belongs to, not which
/// character is currently playing it, so they search this whole set rather
/// than one fighter's own registry slice.
fn all_moves() -> &'static [&'static dyn SpecialMove] {
    characters::ALL_MOVES
}

/// The grounded chains that may open a fresh special this frame: ordinary
/// standing locomotion, or the Wait/Taunt chain's own interruptible frames.
fn grounded_chain_open(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.grounded
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
            tilt::interrupt_chain(fighter, data),
            Some(tilt::Chain::Wait) | Some(tilt::Chain::Taunt)
        ))
}

/// The aerial chains that may open a fresh special this frame: an ordinary
/// jump/fall, or an interruptible wall-tech/airborne-damage window.
fn aerial_chain_open(fighter: &Fighter) -> bool {
    !fighter.grounded
        && (super::damage::wall_tech_interruptible(fighter)
            || super::damage::damage_air_interruptible(fighter)
            || matches!(
                fighter.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || super::aerial::interruptible(fighter))
}

pub(crate) fn attack(action: Action, data: &FighterData) -> Option<&Attack> {
    let specials = data.specials.as_ref()?;
    characters::moves(Some(specials))
        .iter()
        .find_map(|mv| mv.attack(action, data))
}

/// Runs every registered move for this fighter's data in priority order
/// (character-specific moves ahead of the shared neutral shell, matching
/// the source's own before-the-neutral-branch dispatch), stopping at the
/// first one that consumes the frame.
pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: &Rules,
    input: Controller,
) -> bool {
    // Eligibility only gates a *fresh* entry; a move already in progress
    // (mid-dash, awaiting its IASA press) keeps dispatch regardless of
    // whether the current action is itself one of the entry chains, so
    // these booleans are passed through rather than used to short-circuit
    // the loop below.
    let ground = grounded_chain_open(fighter, data);
    let air = aerial_chain_open(fighter);
    for mv in characters::moves(data.specials.as_ref()) {
        if mv.update_actions(fighter, data, rules, ground, air, input) {
            return true;
        }
    }
    false
}

pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &FighterData,
    input: Controller,
    on_platform: bool,
) {
    for mv in characters::moves(data.specials.as_ref()) {
        mv.update_animation(fighter, data, input, on_platform);
    }
}

pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    characters::moves(data.specials.as_ref())
        .iter()
        .find_map(|mv| mv.ground_target_velocity(fighter, data))
}

pub(crate) fn ground_friction_override(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    characters::moves(data.specials.as_ref())
        .iter()
        .find_map(|mv| mv.ground_friction_override(fighter, data))
}

pub(crate) fn air_physics(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: &Rules,
    movement: &mut Movement,
) -> bool {
    for mv in characters::moves(data.specials.as_ref()) {
        if mv.air_physics(fighter, data, rules, movement) {
            return true;
        }
    }
    false
}

pub(crate) fn tick_ground_timers(fighter: &mut Fighter) {
    for mv in all_moves() {
        mv.tick_ground_timers(fighter);
    }
}

pub(crate) fn update_ground_contact(fighter: &mut Fighter) {
    for mv in all_moves() {
        mv.update_ground_contact(fighter);
    }
}

pub(crate) fn transfer_ground_air(fighter: &mut Fighter, grounded: bool) -> bool {
    all_moves()
        .iter()
        .any(|mv| mv.transfer_ground_air(fighter, grounded))
}

pub(crate) fn land(
    fighter: &mut Fighter,
    data: &FighterData,
    on_platform: bool,
    pre_landing: &Fighter,
) -> Result<bool, Error> {
    for mv in characters::moves(data.specials.as_ref()) {
        if mv.land(fighter, data, on_platform, pre_landing)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn collision_mode(action: Action) -> Option<Mode> {
    all_moves().iter().find_map(|mv| mv.collision_mode(action))
}

pub(crate) fn ledge_catchable(action: Action) -> bool {
    all_moves().iter().any(|mv| mv.ledge_catchable(action))
}

pub(crate) fn wants_redirect(action: Action) -> bool {
    all_moves().iter().any(|mv| mv.wants_redirect(action))
}

pub(crate) fn air_contact(
    fighter: &mut Fighter,
    data: &FighterData,
    ceiling: Option<([f32; 3], usize)>,
    wall: Option<([f32; 3], usize)>,
) -> bool {
    characters::moves(data.specials.as_ref())
        .iter()
        .any(|mv| mv.air_contact(fighter, data, ceiling, wall))
}
