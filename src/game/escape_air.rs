//! Air dodge from `ftCo_EscapeAir.c` with its FallSpecial and
//! LandingFallSpecial continuations. The dodge motion is a supplied sample set
//! with bone poses, the scripted `x1988` collision state and the script's
//! skip-decay flag; FallSpecial uses the supplied bind pose like Fall, and
//! LandingFallSpecial plays supplied integer-frame poses at the source rate.
//! Item throws, the `x334` item timer, tether catches and the parasol are not
//! modeled.
use super::{
    Action, Controller, Error, Fighter,
    data::{BodyState, Bone, FighterData},
};
use crate::fighter::{aerial as landing_math, escape_air as math};
use serde::{Deserialize, Serialize};

/// Common air-dodge data (`ftCommonData` x25C and x32C..x344).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `escapeair_deadzone` (x32C/x330): strict per-axis stick deadzone.
    pub deadzone: [f32; 2],
    /// `escapeair_force` (x338): launch speed along the stick angle.
    pub force: f32,
    /// `escapeair_decay` (x33C): per-frame velocity multiplier before the
    /// script's skip-decay flag.
    pub decay: f32,
    /// `x344`: landing lag of the FallSpecial/LandingFallSpecial continuation.
    pub landing_lag: f32,
    /// `x25C`: FallSpecial lands on one-way platforms only above this stick Y.
    pub platform_stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    /// One EscapeAir physics sample per action frame.
    pub frames: Vec<AirDodgeFrame>,
    /// `Fighter::x2EC`: end frame of the special-landing animation.
    pub landing_animation_end: f32,
    /// Integer animation-frame LandingFallSpecial poses covering that end.
    pub landing_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirDodgeFrame {
    pub bones: Vec<Bone>,
    /// Scripted fighter-wide hurtbox collision state for this frame.
    pub body_state: BodyState,
    /// `cmd_vars[cmd_skip_decay]` after this frame's script commands: ordinary
    /// gravity and drift replace the velocity decay.
    pub skip_decay: bool,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
) -> Result<(), Error> {
    let finite = |value: f32| value.is_finite() && value.abs() <= 1_000_000.0;
    if !rules.deadzone.into_iter().all(|v| (0.0..=1.0).contains(&v))
        || !finite(rules.force)
        || rules.force < 0.0
        || !finite(rules.decay)
        || rules.decay < 0.0
        || !finite(rules.landing_lag)
        || rules.landing_lag <= 0.0
        || !(-1.0..=1.0).contains(&rules.platform_stick_threshold)
    {
        return Err(Error::Data("invalid ordinary air-dodge rules".into()));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "air-dodge motions require locomotion parameters for jump counts".into(),
        ));
    }
    if parameters.frames.is_empty() || parameters.frames.len() > 4096 {
        return Err(Error::Data(
            "air dodge requires 1..4096 physics samples".into(),
        ));
    }
    for frame in &parameters.frames {
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    let end = parameters.landing_animation_end;
    if !(0.0..=4095.0).contains(&end)
        || parameters.landing_poses.is_empty()
        || parameters.landing_poses.len() > 4096
        || (end as usize) >= parameters.landing_poses.len()
    {
        return Err(Error::Data(
            "special landing requires complete poses covering its end frame".into(),
        ));
    }
    for pose in &parameters.landing_poses {
        super::validation::validate_animation_pose(pose, fighter)?;
    }
    let rate = landing_math::landing_animation_rate(end, rules.landing_lag);
    if !rate.is_finite() || rate <= 0.0 {
        return Err(Error::Data(
            "special landing animation rate must be finite and positive".into(),
        ));
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::EscapeAir | Action::FallSpecial | Action::LandingFallSpecial
    )
}

fn frame<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a AirDodgeFrame> {
    if fighter.action != Action::EscapeAir {
        return None;
    }
    // `fighter.action_frame` matches Melee's own `cur_anim_frame` (`ftCo_
    // 80099A9C`'s extra `ftAnim_8006EBA4` advance is modeled at the source
    // in `try_air_dodge`, `action_frame = 1` at entry, the same as `game::
    // locomotion::start_dash`/`start_turn`), which is 1 on the entry frame
    // that runs sample 0's script; `frames` is 0-indexed by sample, so this
    // reads one behind `action_frame`.
    data.escape_air
        .as_ref()?
        .frames
        .get(fighter.action_frame.saturating_sub(1) as usize)
}

/// Physics bones for the current EscapeAir or LandingFallSpecial sample.
pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    if let Some(frame) = frame(fighter, data) {
        return Some(&frame.bones);
    }
    if fighter.action != Action::LandingFallSpecial {
        return None;
    }
    data.escape_air
        .as_ref()?
        .landing_poses
        .get(fighter.aerial.landing_elapsed as usize)
        .map(Vec::as_slice)
}

/// `Some(skip)` while EscapeAir owns the physics callback.
pub(crate) fn skip_decay(fighter: &Fighter, data: &FighterData) -> Option<bool> {
    frame(fighter, data).map(|frame| frame.skip_decay)
}

/// `ftCo_80096CC8` decides whether a supporting one-way platform may catch
/// a FallSpecial fighter on this frame.
pub(crate) fn platforms_land(fighter: &Fighter, rules: Option<&Rules>, input: Controller) -> bool {
    let Some(rules) = rules else { return true };
    fighter.action != Action::FallSpecial
        || math::platform_landing(true, true, input.stick[1], rules.platform_stick_threshold)
}

/// Airborne states whose input chains reach `ftCo_80099A58`.
fn eligible(fighter: &Fighter) -> bool {
    !fighter.grounded
        && (matches!(
            fighter.action,
            Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
        ) || super::damage::damage_air_interruptible(fighter)
            || super::damage::wall_tech_interruptible(fighter))
}

/// `ftCo_80099A58` then `ftCo_80099A9C`: a fresh physical L or R press
/// launches along the stick angle through an ordinary motion change.
pub(crate) fn try_air_dodge(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Result<bool, Error> {
    let (Some(rules), Some(parameters)) = (rules, data.escape_air.as_ref()) else {
        return Ok(false);
    };
    if !eligible(fighter)
        || input.buttons & !fighter.previous_input.buttons & (super::BUTTON_L | super::BUTTON_R)
            == 0
    {
        return Ok(false);
    }
    fighter.velocity = math::launch_velocity(input.stick, rules.deadzone, rules.force);
    super::simulation::enter(fighter, Action::EscapeAir);
    // `ftCo_80099A9C` calls `ftAnim_8006EBA4(gobj)` immediately after
    // `Fighter_ChangeMotionState`, the same extra advance `ftCo_Dash_Enter`/
    // `ftCo_Turn_Enter` make; modeled at the source like `game::locomotion::
    // start_dash`/`start_turn` (`action_frame = 1`, not `0`, at entry) rather
    // than as an observation-layer exception. This same call runs sample 0's
    // script.
    fighter.action_frame = 1;
    // Ft_MF_None clears fast fall.
    fighter.fast_fall = false;
    fighter.body_state = parameters
        .frames
        .first()
        .ok_or_else(|| Error::Data("air dodge requires samples".into()))?
        .body_state;
    Ok(true)
}

/// `ftCo_EscapeAir_Anim`, `ftCo_FallSpecial_Anim` and the shared
/// `ftCo_Landing_Anim`. Returns whether these actions own the frame.
pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
) -> Result<bool, Error> {
    if !owns_action(fighter.action) {
        return Ok(false);
    }
    let (Some(_), Some(parameters)) = (rules, data.escape_air.as_ref()) else {
        return Err(Error::Data(
            "air-dodge actions require explicit resources".into(),
        ));
    };
    match fighter.action {
        Action::EscapeAir => {
            // See `frame`'s own comment: `action_frame` is one ahead of the
            // 0-indexed sample it selects.
            let sample = fighter.action_frame.saturating_sub(1) as usize;
            if sample >= parameters.frames.len() {
                // ftCo_80096900(1, 1, false, x340, x344) with Ft_MF_KeepFastFall
                // and ftCommon_UseAllJumps.
                let fast_fall = fighter.fast_fall;
                super::simulation::enter(fighter, Action::FallSpecial);
                fighter.fast_fall = fast_fall;
                fighter.locomotion.jumps_used = data
                    .locomotion
                    .as_ref()
                    .map_or(fighter.locomotion.jumps_used, |p| p.max_jumps);
            } else {
                fighter.body_state = parameters.frames[sample].body_state;
            }
        }
        Action::LandingFallSpecial => {
            fighter.aerial.landing_elapsed += fighter.aerial.landing_rate;
            if fighter.aerial.landing_elapsed >= parameters.landing_animation_end {
                super::simulation::enter(fighter, Action::Wait);
                return Ok(false);
            }
        }
        _ => {}
    }
    Ok(true)
}

/// `ftCo_80099D70` and `ftCo_80096D28` with `x10` set: every EscapeAir or
/// FallSpecial landing enters LandingFallSpecial at `(0.1 + x2EC) / x344`.
pub(crate) fn land(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
) -> Result<bool, Error> {
    if !matches!(fighter.action, Action::EscapeAir | Action::FallSpecial) {
        return Ok(false);
    }
    let (Some(rules), Some(parameters)) = (rules, data.escape_air.as_ref()) else {
        return Err(Error::Data(
            "air-dodge landing requires explicit resources".into(),
        ));
    };
    // `fp->mv.co.fallspecial.landing_lag`: read before `simulation::enter`
    // resets `aerial::State` back to its own default (`None`, meaning no
    // per-instance override was threaded in, so this landing keeps using
    // the shared `escape_air::Rules`' own rate as before).
    let landing_lag = fighter.aerial.landing_lag.unwrap_or(rules.landing_lag);
    super::simulation::enter(fighter, Action::LandingFallSpecial);
    fighter.aerial.allow_interrupt = false;
    fighter.aerial.landing_rate =
        landing_math::landing_animation_rate(parameters.landing_animation_end, landing_lag);
    Ok(true)
}
