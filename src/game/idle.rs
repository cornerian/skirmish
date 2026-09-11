//! Idle animation cycling from `ftCo_Wait_Anim` (`ftCo_Wait.c:34-42`) and
//! `ftCo_8008A7A8` (`ftwaitanim.c:62-105`). See `docs/idle.md`.
use super::{Action, Error, Fighter, data::FighterData};
use crate::random::HsdRng;
use serde::{Deserialize, Serialize};

/// `fighter.idle`: the character's Wait1 restart length plus its idle table
/// (`WaitStruct`, `ftwaitanim.h:6-17`, terminated by `sub_motion == -1` in
/// the source; the terminator is implicit here as the end of `entries`).
/// `entries` may be empty: only the restart-at-`wait1_length` rule then
/// applies (no `HSD_Randi` draw), the same behavior the source gives a
/// `NULL` `WaitStruct*` (`ftwaitanim.c:66`). Absent entirely (`FighterData
/// .idle: None`) keeps Skirmish's pre-batch behavior: `Action::Wait` never
/// advances or restarts (`action_frame` grows without bound) and the
/// reported animation index stays 2
/// (`crates/skirmish-replay/src/observation::animation_index`'s state-14
/// mapping, `idle::State`'s own default).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdleAnimations {
    pub wait1_length: f32,
    #[serde(default)]
    pub entries: Vec<IdleEntry>,
}

/// One `WaitStruct` row. `animation` is the sub-motion id (`ftCo_Submotion`,
/// `kinds/ftCommon/forward.h:637-641`: Wait1_0 = 2, Wait2 = 3, Unk004 = 4,
/// Unk005 = 5, Wait1_1 = 6, plus character-specific ids beyond this common
/// table); `weight` is its `HSD_Randi(100) + 1` share of the walk
/// (`ftwaitanim.c:50-59`); `length` is that idle's own figatree frame
/// count, consulted only while it is the *current* animation (Wait1_0's own
/// length is always `wait1_length`, even if a row also lists animation 2 --
/// see `update_animation`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdleEntry {
    pub animation: u32,
    pub weight: i32,
    pub length: f32,
}

pub fn validate(idle: &IdleAnimations) -> Result<(), Error> {
    let finite_positive = |value: f32| value.is_finite() && value > 0.0 && value <= 1_000_000.0;
    let valid = finite_positive(idle.wait1_length)
        && idle
            .entries
            .iter()
            .all(|entry| entry.weight > 0 && finite_positive(entry.length))
        // getAnimID's walk asserts once `max` (1..=100) exceeds the
        // accumulated weight (ftwaitanim.c:50-59); reject tables that can
        // fall off the end before a match is ever constructed. An empty
        // table is exempt: it never draws (see IdleAnimations above).
        && (idle.entries.is_empty()
            || idle.entries.iter().map(|entry| entry.weight).sum::<i32>() >= 100);
    if valid {
        Ok(())
    } else {
        Err(Error::Data("invalid idle animation resource".into()))
    }
}

/// `Fighter.idle`: `animation` mirrors `fp->anim_id` while `Action::Wait` is
/// current, `frame` mirrors its animation-frame progress toward that
/// sub-motion's own length (integer rate 1, so a plain `f32` counter
/// suffices; kept distinct from `action_frame` because both are reset
/// together on every restart/pick -- see `update_animation`). Reset to
/// `{ animation: 2, frame: 0.0 }` on every Wait entry (`simulation::enter`,
/// unconditionally on every action transition like `dash::State`/
/// `smash::State`, since it is read only while `Action::Wait` is current).
/// `Fighter_ChangeMotionState` sets `fp->anim_id` from the destination
/// motion state's own table entry (`fp->anim_id = new_motion_state->
/// anim_id`, `fighter.c:1216`); this codebase assumes that entry is 2
/// (Wait1_0) for every character's `ftCo_MS_Wait` row, since the
/// per-character `x1C_actionStateList` table itself is compiled character
/// data, not part of the decomp source available here (see `docs/idle.md`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub animation: u32,
    pub frame: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            animation: 2,
            frame: 0.0,
        }
    }
}

/// `ftCo_Wait_Anim` -> `ftCo_8008A7A8(gobj, ft_data->x24)` (`ftCo_Wait.c:
/// 34-42`, `ftwaitanim.c:62-105`), every Wait frame. Called unconditionally
/// each frame (like `dash::update_animation`); returns immediately unless
/// `Action::Wait` is current. Skipped entirely -- no advance, no restart, no
/// RNG draw, matching Skirmish's pre-batch unbounded `action_frame` -- when
/// `data.idle` is absent. `rng` is the match's shared `HsdRng`, drawn from
/// in player order by `simulation::advance`'s per-player animation-phase
/// loop, ahead of the same frame's blast-zone death draw (see
/// `simulation::advance`).
pub(crate) fn update_animation(f: &mut Fighter, data: &FighterData, rng: &mut HsdRng) {
    if f.action != Action::Wait {
        return;
    }
    let Some(idle) = &data.idle else {
        return;
    };
    // ftAnim_IsFramesRemaining false: this port compares the tracked frame
    // against the current animation's own length rather than querying joint
    // animation state (see docs/idle.md).
    f.idle.frame += 1.0;
    let length = current_length(idle, f.idle.animation);
    if f.idle.frame < length {
        return;
    }
    if idle.entries.is_empty() {
        // ftCo_8008A6D8(gobj, fp->anim_id): restart the current animation
        // at frame 0, no RNG draw.
        restart(f);
        return;
    }
    let current = f.idle.animation;
    let (animation, _draws) = crate::fighter::idle::pick(
        idle.entries
            .iter()
            .map(|entry| (entry.animation, entry.weight)),
        || rng.randi(100),
        current,
    );
    f.idle.animation = animation;
    restart(f);
}

fn current_length(idle: &IdleAnimations, animation: u32) -> f32 {
    if animation == 2 {
        idle.wait1_length
    } else {
        idle.entries
            .iter()
            .find(|entry| entry.animation == animation)
            .map_or(idle.wait1_length, |entry| entry.length)
    }
}

/// `ftAnim_8006EBE8(gobj, 0.0, 1.0, blend)` restarts the figatree at frame
/// 0 (`ftCo_8008A6D8`/`ftCo_8008A7A8`'s manually-inlined duplicate,
/// `ftwaitanim.c:32`/`97`); `action_frame` is reset alongside it so the
/// Slippi-reported age (`action_state`) restarts too, matching a real
/// Slippi file's `state_age` for Wait.
fn restart(f: &mut Fighter) {
    f.idle.frame = 0.0;
    f.action_frame = 0;
}
