//! Shared mechanics used by scripted fighter actions: hitbox validation,
//! aerial drift, and common special-action exits.

use crate::{
    collision::bones::BoneCapsule,
    fighter::{Movement, aerial as landing_math},
    game::{
        Action, Error, Fighter,
        data::{Attack, FighterData, Rules as MatchRules},
        simulation,
        validation::{validate_animation_pose, validate_shape},
    },
};

/// Every physics/hitbox frame of `attack` against `fighter`'s own skeleton
/// and hurtboxes: the pose/topology check every move kind already runs
/// (`validate_animation_pose`), plus the same per-hitbox field bounds,
/// `rules.clank`-gated clank/rebound bits, `move_id`-under-staling
/// requirement and transformed-geometry sanity check the generic
/// jab/aerial/tilt/smash/neutral-special chain applies (`validation.rs`'s
/// own hitbox loop) -- bone/group bounds, finite/nonnegative geometry,
/// damage/growth/fixed/base ranges, and an integral 0..=362 launch angle.
/// Narrower than that generic chain in one respect: `move_id` is required
/// under staling only for a phase that actually has a hitbox on some frame,
/// not unconditionally for every phase passed in (pack v6 populates
/// `move_id` on every up/down-special phase regardless, including the
/// hitbox-free ones like `bound.pose`/`fall`/`landing`, but a future pack
/// need not, and this batch is scoped to the hitboxes, not that separate
/// requirement).
pub(crate) fn validate_hitboxes(
    attack: &Attack,
    fighter: &FighterData,
    rules: &MatchRules,
) -> Result<(), Error> {
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    let has_hitboxes = attack.frames.iter().any(|frame| !frame.hitboxes.is_empty());
    if has_hitboxes && rules.staling.is_some() && !attack.move_id.is_some_and(|id| id != 0) {
        return Err(Error::Data(
            "staling requires an explicit nonzero attack move_id".into(),
        ));
    }
    for frame in &attack.frames {
        let pose = validate_animation_pose(&frame.bones, fighter)?;
        if frame.hitboxes.len() > 4 {
            return Err(Error::Data("at most four hitboxes per frame".into()));
        }
        if !(frame.hurtbox_states.is_empty()
            || frame.hurtbox_states.len() == fighter.hurtboxes.len())
        {
            return Err(Error::Data(
                "attack hurtbox state samples must be empty or complete".into(),
            ));
        }
        for hit in &frame.hitboxes {
            if rules.clank.is_none() && (hit.clank || hit.rebound) {
                return Err(Error::Data(
                    "clank/rebound flags require an explicit ordinary profile".into(),
                ));
            }
            if !(hit.bone < frame.bones.len()
                && hit.group < 16
                && hit.center.iter().copied().all(finite)
                && (0.0..=1_000_000.0).contains(&hit.radius)
                && hit.damage <= 999
                && (-1000..=1000).contains(&hit.shield_damage)
                && hit.growth <= 1000
                && hit.fixed <= 1000
                && hit.base <= 1000
                && (0.0..=362.0).contains(&hit.angle_degrees)
                && hit.angle_degrees.fract() == 0.0)
            {
                return Err(Error::Data("invalid or unsupported hitbox".into()));
            }
            validate_shape(BoneCapsule::sphere(hit.bone, hit.center, hit.radius), &pose)?;
        }
    }
    Ok(())
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

// Ledge catching during an aerial phase is not a separate helper: it is the
// same shared per-frame ledge scan every other aerial action already uses
// (`ledge::catchable`/the collision pipeline's own scan). A move opts in by
// implementing `SpecialMove::ledge_catchable`, not by calling anything here.
