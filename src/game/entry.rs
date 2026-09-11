//! Match-start warp-in: Entry (Slippi 322), EntryStart (323), EntryEnd
//! (324). See `docs/match-start.md` for the replay-verified frame table
//! this module reproduces and the resource shape it uses.
//!
//! `melee/ft/ft_0C31.c` (whole file, cited per function below):
//! `ftCo_800C61B0` (24-48, the true spawn/init callback -- not itself a
//! per-frame Anim/Phys/Coll -- called once from `spawn`, not from
//! `advance`), `ftCo_Entry_Anim` (50-57, empty `_IASA`/`_Phys`/`_Coll` at
//! 59-73), `ftCo_800C6408` (75-135, EntryStart's own entry/init),
//! `ftCo_EntryStart_Anim`/`_Phys`/`_Coll` (137-201), `ftCo_800C6B6C`
//! (246-265, EntryEnd's own entry/init), `ftCo_EntryEnd_Anim`/`_Phys`/
//! `_Coll` (267-322). `ftcommon.c:596-604` (`ftCommon_8007D92C`, the
//! EntryEnd exit: airborne -> Fall, grounded -> Wait).
//!
//! Documented simplification: every callback above branches on
//! `Fighter::x221F_b4` (a secondary-entity/"follow the leader" path used by
//! multi-entity-per-port fighters, e.g. Ice Climbers' partner) before
//! touching position; only the `!x221F_b4` branch is ported, since
//! Skirmish models exactly one fighter per port. The warp-star accessory
//! scale/rotation/translation, the root-JObj squish scale (`x14`,
//! `ftCommonData.x6C4`), the spawned effect and the sound cue are visual
//! and are not ported; `EntryRules::scale_y` is kept only so the resource
//! shape has a place for `x6C4` if a future renderer wants it.
use super::{Action, Error, Fighter, data::FighterData, simulation};
use crate::fighter::entry as math;
use serde::{Deserialize, Serialize};

/// `Rules.entry`. `ft/types.h:473-476`: `x6BC`/`x6C0`/`x6C4`/`x6C8`, all
/// declared `int` (`x6C4` is the sole exception: `f32`, the squish-scale
/// target). `invincibility_frames == 0` keeps the `Player_GetFlagsBit4`
/// branch off (see `exit` below); the exporter/pack author should leave it
/// `0` for ordinary versus play.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryRules {
    /// `x6BC`: EntryStart's frame count, and EntryEnd's Phys divisor.
    pub start_frames: u32,
    /// `x6C0`: EntryEnd's frame count.
    pub end_frames: u32,
    /// `x6C4`: the root-JObj Y-scale target (visual only; not read by any
    /// gameplay arithmetic ported here). Kept for resource completeness.
    pub scale_y: f32,
    /// `x6C8`: post-EntryEnd invincibility, applied only when nonzero.
    pub invincibility_frames: u32,
}

/// `FighterData.entry`. Slippi's `state_age` for EntryStart is the tracked
/// *animation* frame (the character's own `ftCo_SM_EntryStart` figatree),
/// not the 30-frame action duration `EntryRules.start_frames` (`x6BC`)
/// counts down: the two are unrelated counters that happen to share a name.
/// The figatree is much shorter than the action (Fox: 11 frames, confirmed
/// directly against `fox-fd.slp`, whose recorded `state_age` advances
/// 0..10 then holds at 10 for the remainder of EntryStart, while position
/// keeps changing correctly under `x6BC`'s own, unrelated formula). `None`
/// leaves the reported age uncapped (today's `action_frame`-based
/// approximation), since no figatree-length data is available for every
/// character without this field.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryAnimation {
    /// The EntryStart figatree's own frame count (its last valid index is
    /// `start_frames - 1`, where the reported age holds).
    pub start_frames: u32,
}

/// `Fighter::mv.co.entry` (the union block shared by all three states).
/// `timer` is deliberately one field reused verbatim across the whole
/// sequence, exactly as the source's own union does: `ftCo_Entry_Anim`'s
/// unconditional trailing decrement (see `update_animation` below) relies
/// on it still referring to EntryStart's freshly assigned value on the
/// transition frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub timer: u32,
    /// `x4`: the spawn-frame Y position, fixed for the whole sequence.
    pub y0: f32,
    /// `x24`: `x34_scale.y * trophy_scale`, set once at the EntryStart entry.
    pub scale: f32,
    /// `x20`: the fixed target amplitude (`1.497345 * x24`).
    pub amplitude: f32,
    /// `x28`: the current interpolated offset, recomputed every Phys frame.
    pub offset: f32,
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::Entry | Action::EntryStart | Action::EntryEnd
    )
}

/// `ftCo_800C61B0:32-41` (position term only). `slot` is the 0-indexed port
/// (P1=0..P4=3, `crate::fighter::entry::entry_delay`).
pub(crate) fn enter(fighter: &mut Fighter, slot: u32) {
    fighter.entry = State {
        timer: math::entry_delay(slot),
        y0: fighter.position[1],
        ..State::default()
    };
    simulation::enter(fighter, Action::Entry);
}

/// `ftCo_800C6408:96-119` (position/timer terms only).
fn enter_start(fighter: &mut Fighter, rules: &EntryRules, trophy_scale: f32) {
    fighter.entry.timer = rules.start_frames;
    fighter.entry.scale = trophy_scale;
    let amplitude = math::amplitude(trophy_scale);
    fighter.entry.amplitude = amplitude;
    fighter.entry.offset = amplitude;
    simulation::enter(fighter, Action::EntryStart);
}

/// `ftCo_800C6B6C:249-262` (position/timer terms only).
fn enter_end(fighter: &mut Fighter, rules: &EntryRules) {
    fighter.entry.timer = rules.end_frames;
    fighter.position[1] = fighter.entry.y0 + fighter.entry.amplitude;
    simulation::enter(fighter, Action::EntryEnd);
}

/// `ftCommon_8007D92C`. `invincibility_frames == 0` keeps the `Player_
/// GetFlagsBit4` branch off, matching ordinary versus play (the design
/// note's citation: the flag is never set outside custom Player configs
/// this codebase has no model for).
fn exit(fighter: &mut Fighter, rules: &EntryRules) {
    if rules.invincibility_frames > 0 {
        fighter.invincibility = rules.invincibility_frames;
    }
    if fighter.grounded {
        simulation::enter(fighter, Action::Wait);
    } else {
        simulation::enter(fighter, Action::Fall);
    }
}

/// The Anim-phase timer/transition logic for all three states, in the
/// per-player animation-phase pass (parallel to `rebirth::update_animation`).
/// A frame that transitions also updates the *destination* state's own
/// timer field this same call (see the per-state comments), matching the
/// source's own control flow exactly rather than a generic FSM.
pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&EntryRules>,
) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        return Ok(());
    }
    let rules = rules.ok_or_else(|| Error::Data("entry action requires explicit rules".into()))?;
    match fighter.action {
        Action::Entry => {
            // ftCo_Entry_Anim:53-56. The transition (if any) runs first and
            // overwrites the *shared* timer field with EntryStart's fresh
            // value before this same call's unconditional decrement below;
            // on a transition frame that decrements EntryStart's own timer
            // (30 -> 29), not Entry's -- exactly reproducing the observed
            // first-EntryStart-frame Y offset in docs/match-start.md's table.
            if fighter.entry.timer == 0 {
                let trophy_scale = data.trophy_scale.unwrap_or(0.0);
                enter_start(fighter, rules, trophy_scale);
            }
            fighter.entry.timer -= 1;
        }
        Action::EntryStart => {
            // ftCo_EntryStart_Anim:140-143: decrement precedes the check,
            // and nothing runs after a transition within this same call.
            fighter.entry.timer -= 1;
            if fighter.entry.timer == 0 {
                enter_end(fighter, rules);
            }
        }
        Action::EntryEnd => {
            // ftCo_EntryEnd_Anim:270-276 (the invincibility/exit tail).
            fighter.entry.timer -= 1;
            if fighter.entry.timer == 0 {
                exit(fighter, rules);
            }
        }
        _ => unreachable!("owns_action guards this match"),
    }
    Ok(())
}

/// The Phys-phase position write, called from `simulation::move_fighter`'s
/// own early return for entry-owned actions. Runs after `update_animation`
/// within the same simulated frame, using whatever action is current *after*
/// that call's own transition -- matching the source's per-frame Anim-then-
/// Phys callback order, including a same-frame Phys call for a state that
/// only began this same frame.
pub(crate) fn move_fighter(fighter: &mut Fighter, rules: Option<&EntryRules>) {
    let Some(rules) = rules else { return };
    match fighter.action {
        // ftCo_Entry_Phys is empty: position does not move during Entry.
        Action::Entry => {}
        Action::EntryStart => {
            let t = math::start_progress(fighter.entry.timer, rules.start_frames);
            fighter.entry.offset = fighter.entry.amplitude * t;
            fighter.position[1] = fighter.entry.y0 + fighter.entry.offset;
        }
        Action::EntryEnd => {
            // ftCo_EntryEnd_Phys:291: the x6BC (start_frames) divisor, not
            // x6C0 -- kept exactly as the source, not "fixed" to end_frames.
            let t = math::end_progress(fighter.entry.timer, rules.start_frames);
            fighter.entry.offset = fighter.entry.amplitude * t;
            fighter.position[1] = fighter.entry.y0 + fighter.entry.offset;
        }
        _ => {}
    }
}

// `ftCo_EntryStart_Coll`/`ftCo_EntryEnd_Coll`'s airborne branch
// (`ft_80083E64`, callback `ftCommon_8007D7FC` via `fn_800C63BC`) is folded
// into the ordinary per-frame `collision::sample`/`collision::resolve`
// pipeline instead of a bespoke box sweep: both states write `position[1] =
// y0 + offset` and set the box bottom to `-offset` (`x2C.bottom = -x28`),
// so the box's world-space bottom is always exactly `y0` for the whole
// sequence -- the ordinary sampled ECB (from the fighter's default rest
// pose, since Entry has no bone animation of its own) already tracks
// `position`, so letting entry-owned fighters fall through to the standard
// `simulation::advance` collision call (unlike `rebirth`'s bypass, which
// skips it) is behaviorally equivalent and reuses the existing landing
// dispatch (`collision::land`'s generic `Action::Landing` fallthrough, the
// same one `ftCommon_8007D7FC` reaches). The grounded variant
// (`ft_800846B0`, losing the floor) is not separately modeled: no fixture
// spawns a fighter already grounded into Entry.
