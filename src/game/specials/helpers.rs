//! Shared phase behaviours reused across special-move phases: the pieces of
//! Fox's side special that are not specific to Fox, and every queued move
//! is expected to need again. A move's own file calls these instead of
//! re-deriving the same gravity, friction, conversion and exit arithmetic.

use crate::{
    fighter::{Movement, aerial as landing_math},
    game::{Action, Fighter, data::FighterData, simulation},
};

/// A countdown before gravity starts pulling a phase down, ticked every
/// frame it is nonzero (`ftFx_SpecialSStart_Phys`'s pattern): count down,
/// and only once it reaches zero does this frame's fall apply. Air friction
/// is applied every frame regardless, matching every side-special air phase
/// that has one.
pub(crate) fn gravity_delayed_fall(
    delay: &mut f32,
    movement: &mut Movement,
    fall_accel: f32,
    terminal_velocity: f32,
    air_friction: f32,
) {
    if *delay > 0.0 {
        *delay -= 1.0;
    } else {
        movement.fall(fall_accel, terminal_velocity);
    }
    movement.friction_air(air_friction);
}

/// `ftCommon_8007CF58` (`ftcommon.c:283-306`): the ordinary aerial drift a
/// phase with no custom air friction of its own uses -- decelerate toward
/// `movement.attributes.air_drift_max` using the common over-drift step
/// (`Rules.specials.air_drift_recovery_step`, `ftCommonData.x1FC`) once
/// already past it, else apply the fighter's ordinary aerial friction
/// toward zero. Matches the source's own bool result (true: was over the
/// maximum); no caller in this codebase currently reads it.
pub(crate) fn drift_or_friction_air(movement: &mut Movement, over_drift_step: f32) -> bool {
    let velocity = movement.self_velocity[0];
    let drift_max = movement.attributes.air_drift_max;
    if velocity.abs() > drift_max {
        let mut accel = over_drift_step;
        if accel.abs() >= velocity.abs() {
            accel = -velocity;
        } else if velocity > 0.0 {
            accel = -accel;
        }
        movement.animation_velocity[0] = accel;
        true
    } else {
        movement.friction_air_basic();
        false
    }
}

/// The gravity-delay countdown then fall exactly like [`gravity_delayed_fall`],
/// followed by the common drift-or-friction call above instead of a fixed
/// custom friction coefficient (`ftFox_SpecialLw_InlinePhys`'s pattern,
/// shared by every Reflector air phase).
pub(crate) fn gravity_delayed_fall_with_drift(
    delay: &mut f32,
    movement: &mut Movement,
    fall_accel: f32,
    terminal_velocity: f32,
    over_drift_step: f32,
) {
    if *delay > 0.0 {
        *delay -= 1.0;
    } else {
        movement.fall(fall_accel, terminal_velocity);
    }
    drift_or_friction_air(movement, over_drift_step);
}

/// The ground-side half of the same countdown: some phases keep ticking it
/// down while grounded even though gravity is never read there, so that a
/// mid-phase ground/air conversion observes the same countdown the air
/// phase would have reached instead of resetting it.
pub(crate) fn tick_ground_delay(delay: &mut f32) {
    if *delay > 0.0 {
        *delay -= 1.0;
    }
}

/// Pose-driven ground velocity, gated per pose: `None` at a pose with no
/// recorded root motion falls through to the caller's ordinary ground
/// friction instead of forcing a velocity.
pub(crate) fn ground_pose_velocity(
    samples: &[Option<f32>],
    frame: u32,
    facing: f32,
) -> Option<f32> {
    (*samples.get(frame as usize)?).map(|z| z * facing)
}

/// The aerial counterpart, applied unconditionally: every pose in an air
/// dash supplies both axes directly, with no per-pose root-motion gate.
pub(crate) fn air_pose_velocity(samples: &[[f32; 2]], frame: u32) -> Option<[f32; 2]> {
    samples.get(frame as usize).copied()
}

/// Move a phase between its grounded and aerial variant at the same
/// animation frame and facing: `enter` otherwise resets `action_frame` like
/// every other per-move field. A move with its own persistent state (a
/// gravity-delay countdown, for example) saves and restores it around this
/// call itself, since `enter` would reset that too.
pub(crate) fn transfer_frame(fighter: &mut Fighter, destination: Action) {
    let frame = fighter.action_frame;
    simulation::enter(fighter, destination);
    fighter.action_frame = frame;
}

/// Restore every jump, matching a fresh aerial special's entry and a
/// `FallSpecial` exit alike.
pub(crate) fn max_out_jumps(fighter: &mut Fighter, data: &FighterData) {
    fighter.locomotion.jumps_used = data
        .locomotion
        .as_ref()
        .map_or(fighter.locomotion.jumps_used, |p| p.max_jumps);
}

/// Exit a phase's natural in-air completion into `FallSpecial`: an
/// interruptible free fall with a scaled drift multiplier and every jump
/// restored. `landing_lag` is this call's own per-instance
/// `ftCo_80096900`/`ftCo_800969D8` argument (`aerial::State::landing_lag`'s
/// own doc); `None` keeps this move's prior behavior of landing at the
/// shared `escape_air::Rules`' own rate.
pub(crate) fn enter_fall_special(
    fighter: &mut Fighter,
    data: &FighterData,
    mobility: f32,
    landing_lag: Option<f32>,
) {
    simulation::enter(fighter, Action::FallSpecial);
    fighter.aerial.allow_interrupt = true;
    fighter.aerial.mobility = mobility;
    fighter.aerial.landing_lag = landing_lag;
    max_out_jumps(fighter, data);
}

/// Land directly into the shared uninterruptible special-landing action,
/// with a rate derived from the fighter's common landing-animation-end
/// frame and this move's own landing lag.
pub(crate) fn enter_landing_fall_special(
    fighter: &mut Fighter,
    landing_animation_end: f32,
    landing_lag: f32,
) {
    simulation::enter(fighter, Action::LandingFallSpecial);
    fighter.aerial.allow_interrupt = false;
    fighter.aerial.landing_rate =
        landing_math::landing_animation_rate(landing_animation_end, landing_lag);
}

/// The ordinary end-of-animation exit for a ground/air pose pair with no
/// move-specific destination: Wait on the ground, Fall in the air.
pub(crate) fn exit_to_wait_or_fall(fighter: &mut Fighter) {
    simulation::enter(
        fighter,
        if fighter.grounded {
            Action::Wait
        } else {
            Action::Fall
        },
    );
}

// Ledge catching during an aerial phase is not a separate helper: it is the
// same shared per-frame ledge scan every other aerial action already uses
// (`ledge::catchable`/the collision pipeline's own scan). A move opts in by
// implementing `SpecialMove::ledge_catchable`, not by calling anything here.
