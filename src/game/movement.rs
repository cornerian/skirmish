//! Per-frame physics poses for movement (non-attack) actions
//! (`docs/movement-poses.md`, `data::MovementPoses`).
//!
//! Every other pose source (attacks, landings, grabs, ledges, taunts, ...)
//! already supplies its own per-frame bones; before this module, every
//! remaining action fell back to the static rest pose (`FighterData.bones`)
//! for its entire duration, so the environmental collision box
//! (`CollisionBox::Bones`, `ecb::load_joints`) never changed shape while
//! Waiting/Walking/Dashing/Running/Jumping/Falling/Landing/Squatting/....
//! Melee poses every state from its figatree every frame (`ftAnim_8006EBA4`)
//! and reads the ECB's six bones from that same pose (`ft_081B.c:36-68`);
//! this module supplies the missing per-frame bones for the sub-motions
//! this codebase already models the timing of.
//!
//! `simulation::pose` calls this after every other pose source and before
//! the final rest-pose fallback, so an absent `MovementPoses` (or an absent
//! field within it) is exactly the pre-batch behavior for that action.
//!
//! # Frame selection
//! `simulation::advance` calls `simulation::pose` (which calls this) during
//! each frame's own collision phase, strictly before the shared end-of-frame
//! `fighter.action_frame += 1` (the increment lives in the later per-player
//! loop that decrements invincibility/hitstun, well after every
//! `collision::sample` call this frame). So the `action_frame` this module
//! reads is already the value Melee's own `cur_anim_frame` holds at the same
//! point in the frame -- `crates/skirmish-replay/src/observation.rs`'s
//! `action_age` has to reconstruct that same pre-increment value from the
//! *post*-increment `action_frame` it only sees after `advance` returns
//! (hence its general `action_frame.saturating_sub(1)`); this module needs
//! no such correction for that general case, since it runs before the
//! increment ever happens.
//!
//! Dash and Turn read `action_frame` unadjusted too, the same as every other
//! action here: `game::locomotion::start_dash`/`start_turn` model `ftCo_
//! Dash_Enter`'s and `ftCo_Turn_Enter`/`ftCo_Turn_Enter_Smash`'s extra,
//! immediate `ftAnim_8006EBA4(gobj)` call (`ftCo_Dash.c:48-63`, `ftCo_
//! Turn.c:49-62,173-188`) at the source, by setting `action_frame` to `1`
//! (not `0`) at entry, so it already matches Melee's own `cur_anim_frame`
//! at every point this module (and every other Dash/Turn frame-count gate)
//! reads it -- entry included, not just afterward. `observation.rs` no
//! longer needs a Dash/Turn exception either, for the same reason.
//!
//! Walk/Run/Wait read their existing continuous frame counters
//! (`fighter.locomotion.walk.frame`/`run.frame`, `fighter.idle.frame`)
//! directly instead of `action_frame`: `game::locomotion::update_animation`
//! and `game::idle::update_animation` already advance and wrap these once
//! per frame, earlier in `simulation::advance` than any collision phase, so
//! they are equally already-current and already-wrapped by the time this
//! module reads them (the same fields `observation.rs` reports).
//!
//! # Loop vs. hold
//! A sub-motion whose action can persist indefinitely (Fall, FallSpecial,
//! SquatWait, OttottoWait; Wait/Walk/Run are handled above) must wrap its
//! pose index at the figatree length, matching the game's `ftAnim` loop
//! flag, or this module would eventually index past the supplied samples.
//! See [`loop_period`] for the exact wrap length (one less than the sample
//! count -- replay-confirmed for Fall). Every other mapped action here is
//! bounded by an existing, already-
//! enforced frame threshold (`dash_animation_frames`, `turn_animation_
//! frames`, `run_turn_animation_frames`, `run_brake_animation_frames`,
//! `jump_startup_frames`, `air_jump_animation_frames`/`MultiJump::
//! animation_frames`, `landing_frames`, `crouch_animation_frames`,
//! `crouch_reverse_frames`, `pass_animation_frames`) that transitions the
//! action away before its own `action_frame` can exceed the matching pose
//! array (`game::locomotion::update_actions` runs, and applies any such
//! transition, earlier in `simulation::advance` than the collision phase
//! that calls this module) -- so holding at the last supplied frame is only
//! ever a defensive clamp for those, except `EntryStart`, which genuinely
//! holds its own figatree's last frame for the remainder of its 30-frame
//! action duration (`crates/skirmish-replay/src/observation.rs`'s
//! `action_age` branch for `EntryStart` documents the same cap).
use super::{
    Action, Fighter,
    data::{Bone, FighterData},
    locomotion::WalkKind,
};

/// The current movement pose sample, if `data.movement_poses` supplies the
/// field this action reads and that field is nonempty. `None` for any
/// action not listed here, or for a listed action whose own field is
/// absent, leaves `simulation::pose`'s rest-pose fallback in place.
pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let poses = data.movement_poses.as_ref()?;
    let (frames, raw, looping): (&Vec<Vec<Bone>>, usize, bool) = match fighter.action {
        Action::Wait => {
            // Wait1_0 = 2 (`kinds/ftCommon/forward.h`); every other idle
            // sub-motion `game::idle` can cycle to has no track here.
            if fighter.idle.animation != 2 {
                return None;
            }
            (poses.wait.as_ref()?, fighter.idle.frame as usize, true)
        }
        Action::Walk => {
            let field = match fighter.locomotion.walk.kind {
                WalkKind::Slow => &poses.walk_slow,
                WalkKind::Middle => &poses.walk_middle,
                WalkKind::Fast => &poses.walk_fast,
            };
            (
                field.as_ref()?,
                fighter.locomotion.walk.frame as usize,
                true,
            )
        }
        Action::Run => (
            poses.run.as_ref()?,
            fighter.locomotion.run.frame as usize,
            true,
        ),
        Action::Turn => (poses.turn.as_ref()?, fighter.action_frame as usize, false),
        Action::RunTurn => (
            poses.turn_run.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Dash => (poses.dash.as_ref()?, fighter.action_frame as usize, false),
        Action::RunBrake => (
            poses.run_brake.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::JumpSquat => (
            poses.knee_bend.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Jump if fighter.locomotion.jump_backward => {
            (poses.jump_b.as_ref()?, fighter.action_frame as usize, false)
        }
        Action::Jump => (poses.jump_f.as_ref()?, fighter.action_frame as usize, false),
        Action::JumpAerial if fighter.locomotion.jump_backward => (
            poses.jump_aerial_b.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::JumpAerial => (
            poses.jump_aerial_f.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        // `ftCo_Fall_Anim_Inner`'s continuous air-drift blend between the
        // neutral/forward/backward figatrees (`ftCo_Fall.c:110-172`) is not
        // modeled here: only the neutral track is ever selected, matching
        // `docs/movement-poses.md`. `fall_aerial` distinguishes
        // `ftCo_FallAerial_Enter` from an ordinary Fall entry
        // (`game::locomotion::State::fall_aerial`).
        Action::Fall if fighter.locomotion.fall_aerial => (
            poses.fall_aerial.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::Fall => (poses.fall.as_ref()?, fighter.action_frame as usize, true),
        Action::FallSpecial => (
            poses.fall_special.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::Landing => (
            poses.landing.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::LandingFallSpecial => (
            poses.landing_fall_special.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Squat => (poses.squat.as_ref()?, fighter.action_frame as usize, false),
        Action::SquatWait => (
            poses.squat_wait.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::SquatRv => (
            poses.squat_rv.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Pass => (poses.pass.as_ref()?, fighter.action_frame as usize, false),
        Action::Ottotto => (
            poses.ottotto.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::OttottoWait => (
            poses.ottotto_wait.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::EntryStart => (
            poses.entry_start.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        _ => return None,
    };
    if frames.is_empty() {
        return None;
    }
    let index = if looping {
        raw % loop_period(frames.len())
    } else {
        raw.min(frames.len() - 1)
    };
    frames.get(index).map(Vec::as_slice)
}

/// A looping sub-motion's exported samples close the loop: the last frame
/// re-samples the same point in the figatree as frame 0 (confirmed against
/// `fox-fd.slp`: Fall's own 9-sample `movement_poses.fall` and the
/// replay-recorded state age both cycle over exactly 8 values, 0..=7, not
/// 9 -- see `crates/skirmish-replay/src/observation.rs`'s Fall/FallAerial/
/// FallSpecial/SquatWait/OttottoWait `action_age` branch, which wraps the
/// same way for the same reason). So the wrap length is one less than the
/// sample count, not the sample count itself.
/// `pub`, not `pub(crate)`: `crates/skirmish-replay/src/observation.rs`
/// reuses this exact wrap length so the reported Slippi `action_age` matches
/// the frame this module actually poses (see both call sites).
pub fn loop_period(frame_count: usize) -> usize {
    frame_count.saturating_sub(1).max(1)
}
