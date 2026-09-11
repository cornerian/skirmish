//! Fox/Falco down special (Reflector): resource shape, validation and the
//! pieces of its behaviour genuinely specific to this move (its five-phase
//! Start/Loop/Turn/Hit/End state machine, the mid-move turn, jump cancel and
//! platform drop, the `reflecting` bit). Everything reusable across moves --
//! gravity-delayed fall, ground/air conversion keeping the frame, the exit
//! to Wait/Fall -- comes from `game::specials::helpers` instead of being
//! re-derived here. Gated purely on resource presence, like the shared
//! neutral shell and the side special.
//!
//! Pinned decomp `src/melee/ft/kinds/ftFox/ftfoxspeciallw.c` (rev 0bac93a5).
//! See `docs/fox-down-special.md` for the full citation list.
//!
//! The Hit phase (`ftFx_SpecialLwHit_Enter`) is reachable in the source only
//! through the projectile-reflect callback (`fp->reflect_hit_cb`,
//! `ftColl_CreateReflectHit`), fired when a reflected projectile actually
//! exists. Skirmish has no projectiles, so nothing in this module ever
//! transitions a fighter into `Action::SpecialLwHit`/`SpecialAirLwHit`; the
//! phase's own per-frame bookkeeping and exit are still implemented and
//! exercised by tests that set the action directly, matching the batch's
//! "reachable through a test hook, unreachable in play" brief.

use crate::{
    fighter::{Movement, characters::fox as math},
    game::{
        Action, BUTTON_B, Controller, Error, Fighter,
        data::{Attack, FighterData, Rules as MatchRules, StageGeometry},
        simulation,
        specials::{SpecialMove, helpers},
        validation::validate_animation_pose,
    },
};
use serde::{Deserialize, Serialize};

pub(crate) use super::side::Phase;

/// `Fighter::down_special` (`fp->mv.fx.SpecialLw`/`ftFox_DatAttrs`'s
/// `x98..xB0` slice).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownSpecial {
    pub start: Phase,
    pub loop_phase: Phase,
    pub turn: Phase,
    pub hit: Phase,
    pub end: Phase,
    pub attributes: Attributes,
    pub reflect: Reflect,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// `x98_FOX_REFLECTOR_RELEASE_LAG`: frames B must be released before the
    /// held pose ends, counted down from the Start entry across every
    /// later phase (Loop/Turn/Hit never reset it).
    pub release_lag: f32,
    /// `x9C_FOX_REFLECTOR_TURN_FRAMES`: the Turn phase's own countdown,
    /// freshly assigned on every Turn entry, and the (always-true once
    /// reached) turnFrames<=x9C guard alongside `cmd_vars[0]` that flips
    /// facing on the first Turn step.
    pub turn_frames: f32,
    /// `xA4_FOX_REFLECTOR_GRAVITY_DELAY`: assigned once at Start entry, never
    /// reset by any later phase; only the air Phys callbacks ever count it
    /// down (see the module doc's "unmodeled" note: unlike the side
    /// special, none of this move's *grounded* Phys callbacks touch it).
    pub gravity_delay: f32,
    /// `xA8_FOX_REFLECTOR_MOMENTUM_PRESERVE_X`: divides `self_vel.x` on a
    /// fresh *aerial* entry only (`ftFx_SpecialAirLw_Enter`); the grounded
    /// entry does not touch `ground_velocity` at all.
    pub air_momentum_div: f32,
    /// `xAC_FOX_REFLECTOR_FALL_ACCEL`: the single fall acceleration every
    /// air phase's `ftCommon_Fall` call uses (unlike the side special,
    /// which has a distinct value per phase).
    pub fall_accel: f32,
}

/// `ReflectDesc` (`lb/types.h:115-126`), assigned by `ftColl_CreateReflectHit`
/// alongside the `reflecting` bit this batch reports (piggybacked on the
/// existing `fighter.shield.reflecting` field, since the source itself
/// stores both the powershield's and this move's reflect state in the same
/// single `fp->reflecting` bit). The bubble geometry below has no effect in
/// this engine -- there are no projectiles to reflect -- and is kept only
/// for resource-shape completeness/validation, per the design note.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reflect {
    pub bone: u32,
    pub max_damage: i32,
    pub offset: [f32; 3],
    pub size: f32,
    pub damage_mul: f32,
    pub speed_mul: f32,
    pub behavior: u8,
}

pub(crate) fn validate(
    rules: &super::side::Rules,
    parameters: &DownSpecial,
    fighter: &FighterData,
) -> Result<(), Error> {
    // `rules.specials` (side::Rules) supplies this move's own
    // `vertical_threshold` (x21C); validate it here too, since a fighter
    // could enable the down special without the side special ever pairing
    // this same `rules` value against `validate_rules` itself.
    super::side::validate_rules(rules)?;
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    let a = &parameters.attributes;
    if !finite(a.release_lag)
        || a.release_lag < 0.0
        || !finite(a.turn_frames)
        || a.turn_frames <= 0.0
        || !finite(a.gravity_delay)
        || a.gravity_delay < 0.0
        || !finite(a.air_momentum_div)
        || a.air_momentum_div == 0.0
        || !finite(a.fall_accel)
    {
        return Err(Error::Data("invalid down-special attributes".into()));
    }
    let r = &parameters.reflect;
    if !finite(r.max_damage as f32)
        || !r.offset.iter().copied().all(finite)
        || !finite(r.size)
        || !finite(r.damage_mul)
        || !finite(r.speed_mul)
    {
        return Err(Error::Data("invalid down-special reflect geometry".into()));
    }
    for attack in [
        &parameters.start.ground,
        &parameters.start.air,
        &parameters.loop_phase.ground,
        &parameters.loop_phase.air,
        &parameters.turn.ground,
        &parameters.turn.air,
        &parameters.hit.ground,
        &parameters.hit.air,
        &parameters.end.ground,
        &parameters.end.air,
    ] {
        if attack.frames.is_empty() || attack.frames.len() > 4096 {
            return Err(Error::Data(
                "down special requires 1..4096 physics samples per phase".into(),
            ));
        }
        for frame in &attack.frames {
            validate_animation_pose(&frame.bones, fighter)?;
        }
    }
    Ok(())
}

/// Persistent per-fighter state (`mv.fx.SpecialLw`). `release_lag`/
/// `is_release`/`gravity_delay` live for the whole move (every internal
/// transition preserves them explicitly, since `simulation::enter` resets
/// this struct to `default()` like every other move's own state); `turn_
/// frames`/`turned` (`cmd_vars[0]`) are Turn-phase-local, freshly assigned
/// on every Turn entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub release_lag: f32,
    pub is_release: bool,
    pub gravity_delay: f32,
    pub turn_frames: f32,
    pub turned: bool,
    /// `Ft_MF_KeepGfx`'s own visual loop has no fixed length; Loop's pose
    /// resource does, so once `action_frame` reaches its last sample this
    /// holds there instead of running off the end (`locomotion::
    /// hold_action_frame`), the same mechanism RunTurn/RunBrake's own wait
    /// flags already use for an analogous reason.
    pub looping: bool,
}

/// Physics/hurtbox pose for the current phase, mirroring the side special's
/// own `attack`.
pub(crate) fn attack(action: Action, parameters: &DownSpecial) -> Option<&Attack> {
    match action {
        Action::SpecialLwStart => Some(&parameters.start.ground),
        Action::SpecialAirLwStart => Some(&parameters.start.air),
        Action::SpecialLw => Some(&parameters.loop_phase.ground),
        Action::SpecialAirLw => Some(&parameters.loop_phase.air),
        Action::SpecialLwTurn => Some(&parameters.turn.ground),
        Action::SpecialAirLwTurn => Some(&parameters.turn.air),
        Action::SpecialLwHit => Some(&parameters.hit.ground),
        Action::SpecialAirLwHit => Some(&parameters.hit.air),
        Action::SpecialLwEnd => Some(&parameters.end.ground),
        Action::SpecialAirLwEnd => Some(&parameters.end.air),
        _ => None,
    }
}

fn owned(action: Action) -> bool {
    matches!(
        action,
        Action::SpecialLwStart
            | Action::SpecialLw
            | Action::SpecialLwTurn
            | Action::SpecialLwHit
            | Action::SpecialLwEnd
            | Action::SpecialAirLwStart
            | Action::SpecialAirLw
            | Action::SpecialAirLwTurn
            | Action::SpecialAirLwHit
            | Action::SpecialAirLwEnd
    )
}

/// Carry the whole-move fields across an internal `simulation::enter`, which
/// resets `fighter.down_special` to `default()` like every other move's own
/// state (`Fighter_ChangeMotionState` clears `mv.fx.SpecialLw` too, but none
/// of this move's own transition functions ever reassign these three except
/// the fresh Start entry).
fn transition(fighter: &mut Fighter, destination: Action) {
    let carry = (
        fighter.down_special.release_lag,
        fighter.down_special.is_release,
        fighter.down_special.gravity_delay,
    );
    simulation::enter(fighter, destination);
    fighter.down_special.release_lag = carry.0;
    fighter.down_special.is_release = carry.1;
    fighter.down_special.gravity_delay = carry.2;
}

fn enter_start(fighter: &mut Fighter, parameters: &DownSpecial, ground: bool) {
    if !ground {
        // `ftFx_SpecialAirLw_Enter`: the grounded entry touches neither
        // velocity component.
        fighter.velocity[1] = 0.0;
        fighter.velocity[0] /= parameters.attributes.air_momentum_div;
    }
    simulation::enter(
        fighter,
        if ground {
            Action::SpecialLwStart
        } else {
            Action::SpecialAirLwStart
        },
    );
    fighter.down_special.release_lag = parameters.attributes.release_lag;
    fighter.down_special.is_release = false;
    fighter.down_special.gravity_delay = parameters.attributes.gravity_delay;
}

/// `ftFx_SpecialLw_CreateReflectHit`/`ftColl_CreateReflectHit`: only the
/// `reflecting` bit has any effect in this engine (see `Reflect`'s doc).
fn enter_loop(fighter: &mut Fighter) {
    transition(
        fighter,
        if fighter.grounded {
            Action::SpecialLw
        } else {
            Action::SpecialAirLw
        },
    );
    fighter.shield.reflecting = true;
}

/// `ftFx_SpecialLwEnd_Enter`: plain `ChangeMotionState`, nothing re-sets
/// `reflecting` afterward, so it stays cleared.
fn enter_end(fighter: &mut Fighter) {
    transition(
        fighter,
        if fighter.grounded {
            Action::SpecialLwEnd
        } else {
            Action::SpecialAirLwEnd
        },
    );
}

/// `ftFox_SpecialLwTurn_SetVarAll` + one `ftFx_SpecialLw_Turn` step, run
/// synchronously by the source's own Turn entry (not deferred to the next
/// frame's Anim callback the way the ongoing per-frame step below is).
fn enter_turn(fighter: &mut Fighter, parameters: &DownSpecial) {
    transition(
        fighter,
        if fighter.grounded {
            Action::SpecialLwTurn
        } else {
            Action::SpecialAirLwTurn
        },
    );
    fighter.shield.reflecting = true;
    fighter.down_special.turned = false;
    fighter.down_special.turn_frames = parameters.attributes.turn_frames;
    turn_step(fighter, parameters);
}

/// `ftFox_SpecialLw_Turn_Inline`: the model-rotation write is purely visual
/// and stays unmodeled; the facing flip on the first step is the only
/// gameplay effect.
fn turn_step(fighter: &mut Fighter, parameters: &DownSpecial) {
    fighter.down_special.turn_frames -= 1.0;
    if !fighter.down_special.turned
        && fighter.down_special.turn_frames <= parameters.attributes.turn_frames
    {
        fighter.down_special.turned = true;
        fighter.facing = -fighter.facing;
    }
}

/// `ftFx_SpecialLwHit_Check`: End when the release lag has already elapsed
/// and B has been released, else back to Loop (with a fresh reflect hit).
/// Shared by Turn's `turnFrames <= 0` exit and Hit's own clip end.
fn hit_check(fighter: &mut Fighter) {
    if fighter.down_special.release_lag <= 0.0 && fighter.down_special.is_release {
        enter_end(fighter);
    } else {
        enter_loop(fighter);
    }
}

fn update_release(fighter: &mut Fighter, held_b: bool) {
    if !held_b {
        fighter.down_special.is_release = true;
    }
}

fn tick_release_lag(fighter: &mut Fighter) {
    if fighter.down_special.release_lag > 0.0 {
        fighter.down_special.release_lag -= 1.0;
    }
}

fn release_lag_elapsed(fighter: &Fighter) -> bool {
    fighter.down_special.release_lag <= 0.0 && fighter.down_special.is_release
}

/// `ftFx_SpecialLwLoop_CheckPass`/`ftFx_SpecialLwStart_CheckPass`
/// (`ftCo_80099F1C` + `ftCo_8009A184` + a fresh reflect hit): the platform
/// drop keeps this move's own Start/Loop phase, converting straight to the
/// aerial variant at the current frame instead of the shared `Pass` action.
/// Called from `simulation::advance` alongside the generic
/// `locomotion::pass_request_after_actions`, since both need the geometry-
/// aware `on_platform` query that `SpecialMove::update_actions` does not
/// receive. Ground only (`ftFx_SpecialAirLwStart_IASA` is a no-op, and Loop's
/// aerial IASA has no platform check of its own).
pub(crate) fn platform_drop(
    fighter: &mut Fighter,
    data: &FighterData,
    geometry: &StageGeometry,
    input: Controller,
) -> bool {
    if !matches!(fighter.action, Action::SpecialLwStart | Action::SpecialLw) {
        return false;
    }
    let Some(p) = data.locomotion.as_ref() else {
        return false;
    };
    if !(crate::game::collision::on_platform(fighter, geometry)
        && input.stick[1] <= -p.pass_stick_threshold
        && fighter.locomotion.tilt_y_age < p.pass_window)
    {
        return false;
    }
    let destination = if fighter.action == Action::SpecialLwStart {
        Action::SpecialAirLwStart
    } else {
        Action::SpecialAirLw
    };
    if crate::game::collision::begin_pass_as(
        fighter,
        data,
        geometry,
        p.pass_velocity,
        destination,
        true,
    ) {
        fighter.shield.reflecting = true;
        true
    } else {
        false
    }
}

/// This move's handle in Fox's registry (`MOVES` in `characters::fox`).
pub(crate) struct Move;

pub(crate) const MOVE: Move = Move;

impl SpecialMove for Move {
    fn owns(&self, action: Action) -> bool {
        owned(action)
    }

    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        attack(action, data.specials.as_ref()?.fox_down()?)
    }

    /// The grounded/aerial dispatch chains check this move's own fresh entry
    /// after the side special and the shared neutral shell (`characters::
    /// fox::MOVES`'s own order encodes that -- the source's grounded chain
    /// checks SpecialS, SpecialHi(unmodeled), SpecialN, then SpecialLw in
    /// that fixed order; the aerial dispatcher partitions purely by stick
    /// direction instead, so this move's own zone check already yields to
    /// Up/Side/Neutral without needing a particular iteration order there).
    /// Once owned, this also carries every phase's own per-frame bookkeeping
    /// and mid-move transitions (Turn/jump-cancel IASA, the release-lag
    /// countdown, Turn's own countdown, Hit's clip end); the platform drop
    /// is handled separately (`platform_drop`, geometry-aware).
    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_down()) else {
            return false;
        };
        let held_b = input.buttons & BUTTON_B != 0;
        let fresh_b = input.buttons & !fighter.previous_input.buttons & BUTTON_B != 0;

        match fighter.action {
            Action::SpecialLwStart | Action::SpecialAirLwStart => {
                // A fresh Anim-callback frame skips the entry frame itself:
                // the source's own Enter never invokes Start's Anim
                // callback synchronously (unlike Turn's explicit one-time
                // step below), so its first per-frame bookkeeping happens
                // on the next real frame.
                if fighter.action_frame != 0 {
                    update_release(fighter, held_b);
                }
                true
            }
            Action::SpecialLw | Action::SpecialAirLw => {
                if fighter.action_frame != 0 {
                    update_release(fighter, held_b);
                    tick_release_lag(fighter);
                    if release_lag_elapsed(fighter) {
                        enter_end(fighter);
                        return true;
                    }
                }
                let Some(p) = data.locomotion.as_ref() else {
                    return true;
                };
                let turning =
                    math::should_turn_mid_move(input.stick[0], fighter.facing, p.turn_threshold);
                if turning {
                    enter_turn(fighter, parameters);
                    return true;
                }
                if fighter.grounded {
                    if let Some(source) =
                        crate::game::locomotion::jump_input(fighter, p, input, false)
                    {
                        fighter.short_hop = false;
                        fighter.locomotion.jump_input = source;
                        simulation::enter(fighter, Action::JumpSquat);
                        return true;
                    }
                    // The platform drop is handled by `platform_drop` at the
                    // geometry-aware call site in `simulation::advance`.
                } else {
                    crate::game::locomotion::try_aerial_jump(fighter, data, input);
                }
                true
            }
            Action::SpecialLwTurn | Action::SpecialAirLwTurn => {
                if fighter.action_frame != 0 {
                    update_release(fighter, held_b);
                    tick_release_lag(fighter);
                    turn_step(fighter, parameters);
                    if fighter.down_special.turn_frames <= 0.0 {
                        hit_check(fighter);
                    }
                }
                true
            }
            Action::SpecialLwHit | Action::SpecialAirLwHit => {
                if fighter.action_frame != 0 {
                    update_release(fighter, held_b);
                    tick_release_lag(fighter);
                    let pose = attack(fighter.action, parameters);
                    if pose.is_none_or(|pose| fighter.action_frame as usize >= pose.frames.len()) {
                        hit_check(fighter);
                    }
                }
                true
            }
            Action::SpecialLwEnd | Action::SpecialAirLwEnd => true,
            _ => {
                if !(ground || air) || !fresh_b {
                    return false;
                }
                let Some(rules) = rules.specials.as_ref() else {
                    return false;
                };
                let in_zone = if ground {
                    input.stick[1] < -rules.vertical_threshold
                } else {
                    input.stick[1] <= -rules.vertical_threshold
                };
                if !in_zone {
                    return false;
                }
                enter_start(fighter, parameters, ground);
                true
            }
        }
    }

    fn update_animation(&self, fighter: &mut Fighter, data: &FighterData) {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_down()) else {
            return;
        };
        match fighter.action {
            Action::SpecialLwStart
                if fighter.action_frame as usize >= parameters.start.ground.frames.len() =>
            {
                enter_loop(fighter);
            }
            Action::SpecialAirLwStart
                if fighter.action_frame as usize >= parameters.start.air.frames.len() =>
            {
                enter_loop(fighter);
            }
            Action::SpecialLwEnd
                if fighter.action_frame as usize >= parameters.end.ground.frames.len() =>
            {
                helpers::exit_to_wait_or_fall(fighter);
            }
            Action::SpecialAirLwEnd
                if fighter.action_frame as usize >= parameters.end.air.frames.len() =>
            {
                helpers::exit_to_wait_or_fall(fighter);
            }
            // Loop has no clip-length exit of its own (`Ft_MF_KeepGfx`
            // loops the animation indefinitely; the exit is releaseLag/
            // isRelease-driven, in `update_actions`). Hold at the pose's
            // last sample once reached instead of running past it.
            Action::SpecialLw
                if fighter.action_frame as usize + 1
                    >= parameters.loop_phase.ground.frames.len() =>
            {
                fighter.down_special.looping = true;
            }
            Action::SpecialAirLw
                if fighter.action_frame as usize + 1 >= parameters.loop_phase.air.frames.len() =>
            {
                fighter.down_special.looping = true;
            }
            _ => {}
        }
    }

    fn air_physics(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        movement: &mut Movement,
    ) -> bool {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_down()) else {
            return false;
        };
        if !owned(fighter.action) || fighter.grounded {
            return false;
        }
        let Some(over_drift_step) = rules.specials.as_ref().map(|r| r.air_drift_recovery_step)
        else {
            return false;
        };
        let terminal_velocity = movement.attributes.terminal_velocity;
        // `ftFox_SpecialLw_InlinePhys`/every other air Phys: gravity delay
        // then fall, then the common `ftCommon_8007CF58` drift-or-friction
        // call (`helpers::drift_or_friction_air`), shared by Start/Loop/
        // Turn/Hit/End alike (a single `xAC` fall accel for the whole move,
        // unlike the side special's per-phase values).
        helpers::gravity_delayed_fall_with_drift(
            &mut fighter.down_special.gravity_delay,
            movement,
            parameters.attributes.fall_accel,
            terminal_velocity,
            over_drift_step,
        );
        true
    }

    fn transfer_ground_air(&self, fighter: &mut Fighter, grounded: bool) -> bool {
        let destination = match (fighter.action, grounded) {
            (Action::SpecialLwStart, false) => Action::SpecialAirLwStart,
            (Action::SpecialAirLwStart, true) => Action::SpecialLwStart,
            (Action::SpecialLw, false) => Action::SpecialAirLw,
            (Action::SpecialAirLw, true) => Action::SpecialLw,
            (Action::SpecialLwTurn, false) => Action::SpecialAirLwTurn,
            (Action::SpecialAirLwTurn, true) => Action::SpecialLwTurn,
            (Action::SpecialLwHit, false) => Action::SpecialAirLwHit,
            (Action::SpecialAirLwHit, true) => Action::SpecialLwHit,
            (Action::SpecialLwEnd, false) => Action::SpecialAirLwEnd,
            (Action::SpecialAirLwEnd, true) => Action::SpecialLwEnd,
            _ => return false,
        };
        let carry = (
            fighter.down_special.release_lag,
            fighter.down_special.is_release,
            fighter.down_special.gravity_delay,
            fighter.down_special.turn_frames,
            fighter.down_special.turned,
            fighter.down_special.looping,
        );
        let reflecting = fighter.shield.reflecting;
        helpers::transfer_frame(fighter, destination);
        fighter.down_special.release_lag = carry.0;
        fighter.down_special.is_release = carry.1;
        fighter.down_special.gravity_delay = carry.2;
        fighter.down_special.turn_frames = carry.3;
        fighter.down_special.turned = carry.4;
        fighter.down_special.looping = carry.5;
        // Loop/Turn/Hit's own ground<->air conversions all re-set
        // `reflecting` (`ftFox_SpecialLw_SetReflectVars`/`ftFx_SpecialLwHit_
        // SetCall`/a fresh `CreateReflectHit`); Start/End's conversions
        // never touch it, so restoring the pre-conversion value (already
        // false there) is equivalent and shorter than a phase match.
        if matches!(
            destination,
            Action::SpecialLw
                | Action::SpecialAirLw
                | Action::SpecialLwTurn
                | Action::SpecialAirLwTurn
                | Action::SpecialLwHit
                | Action::SpecialAirLwHit
        ) {
            fighter.shield.reflecting = reflecting;
        }
        true
    }
}
