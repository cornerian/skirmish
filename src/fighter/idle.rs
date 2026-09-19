//! Idle animation cycling from `ftCo_Wait_Anim` (`ftCo_Wait.c:34-42`) and
//! `ftCo_8008A7A8` (`ftwaitanim.c:62-105`). See `docs/idle.md`.
use crate::compat::math::random::HsdRng;
use crate::game::{Action, Error, Fighter, data::FighterData};
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
    let (animation, _draws) = pick(
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
    // Idle animation cycling is a genuine same-action animation wrap, just
    // like Attack100Loop.  The simulation consumes this marker after the
    // animation phase to restart the action-relative script clock, canceling
    // deadlines from the completed idle cycle before they can be redelivered
    // against the reset action frame.
    f.script_events.pending_action_clock_restart = true;
}

// Pure fighter arithmetic and predicates.
/// `getAnimID`'s walk plus `ftCo_8008A7A8`'s re-draw loop:
///
/// ```c
/// int max = HSD_Randi(100) + 1;
/// int count = 0;
/// while (wait_data->u.i.x != -1) {
///     count += wait_data->u.i.y;
///     if (max <= count) {
///         return (enum_t) wait_data->u.p.x;
///     }
///     wait_data += 1;
/// }
/// HSD_ASSERTREPORT(86, 0, "wait anim data illegal!!\n", max);
/// ```
///
/// and:
///
/// ```c
/// do {
///     temp = anim_id = getAnimID(arg1);
/// } while (!inlineA0(fp) && fp->anim_id == temp);
/// ```
///
/// `entries` yields `(sub_motion, weight)` pairs in `WaitStruct` order (the
/// `-1` terminator is implicit as the end of the sequence, not a sentinel
/// value); it is walked again on every re-draw, so it must be cheaply
/// `Clone` (an iterator over a slice, not a one-shot generator). `draws`
/// supplies each call's raw `HSD_Randi(100)` result (`0..100`); this
/// function adds the source's own `+ 1`. `current` is `fp->anim_id` at
/// entry (the animation about to be replaced). Returns the picked
/// sub-motion and the number of `HSD_Randi` calls consumed (at least 1;
/// more only while the pick keeps repeating a non-Wait1/-31 `current`).
///
/// Panics if a draw's accumulated weight never reaches `max` (the table
/// falls off the end, `HSD_ASSERTREPORT`'s condition in the source):
/// callers that need to observe this instead of aborting (the C oracle)
/// walk the table themselves; `fighter::idle::validate` rejects tables that
/// can trigger it before a match is ever constructed.
pub fn pick(
    entries: impl Iterator<Item = (u32, i32)> + Clone,
    mut draws: impl FnMut() -> i32,
    current: u32,
) -> (u32, u32) {
    let mut used = 0u32;
    loop {
        let max = draws() + 1;
        used += 1;
        let mut count = 0;
        let mut picked = None;
        for (animation, weight) in entries.clone() {
            count += weight;
            if max <= count {
                picked = Some(animation);
                break;
            }
        }
        let picked = picked.expect("idle animation weights must sum to at least `max`");
        // inlineA0: accepted unconditionally from Wait1_0 (2) or 31; from
        // any other current animation, re-draw while the pick repeats it.
        if current == 2 || current == 31 || picked != current {
            return (picked, used);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<(u32, i32)> {
        vec![(2, 60), (3, 40)]
    }

    #[test]
    fn draw_at_or_below_the_first_weight_selects_the_first_entry() {
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
        let mut draws = [59].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
    }

    #[test]
    fn draw_above_the_first_weight_selects_the_second_entry() {
        let mut draws = [60].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (3, 1)
        );
        let mut draws = [99].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (3, 1)
        );
    }

    #[test]
    fn a_repeated_pick_from_wait1_or_31_is_accepted_without_a_redraw() {
        let mut draws = [0, 0, 0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (2, 1)
        );
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 31),
            (2, 1)
        );
    }

    #[test]
    fn a_repeated_pick_from_a_non_wait1_animation_redraws() {
        // current = 3 (Wait2); the first draw (0) picks 2, which differs
        // from 3, so it is accepted on the first draw despite current != 2.
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
        // current = 2 is the accepted-unconditionally case; force a repeat
        // of a non-Wait1 current instead: current = 3, first draw picks 3
        // again (>= 60), second draw picks 2.
        let mut draws = [60, 0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 2)
        );
    }

    #[test]
    #[should_panic(expected = "weights must sum")]
    fn a_draw_past_the_accumulated_weight_panics() {
        let short = vec![(2, 50)];
        pick(short.into_iter(), || 99, 2);
    }
}
