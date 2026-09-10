//! Ordinary grounded shield evasions from `ftCo_Escape.c`: EscapeF/EscapeB
//! rolls and the EscapeN spot dodge. Motions are supplied physics samples with
//! per-frame TransN root translation, bone poses and the scripted fighter-wide
//! hurtbox collision state (`Fighter::x1988`). The Samus/Yoshi entry branches,
//! item-throw interrupts and the unread `x324` copy are not modeled.
use super::{
    Action, Controller, Error, Fighter,
    data::{BodyState, Bone, FighterData},
};
use crate::fighter::escape::{self as math, RollDirection};
use serde::{Deserialize, Serialize};

/// Common escape input data (`ftCommonData` x314..x320).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `x31C`: inclusive horizontal magnitude for main-stick and C-stick rolls.
    pub roll_stick_threshold: f32,
    /// `x320`: exclusive shared tilt-age limit for a fresh main-stick roll.
    pub roll_window: u8,
    /// `x314`: inclusive downward stick value; the native value is negative.
    pub spot_dodge_stick_threshold: f32,
    /// `x318`: exclusive shared tilt-age limit for a fresh main-stick dodge.
    pub spot_dodge_window: u8,
}

/// Per-fighter escape motions. Frame `n` is the sample used while
/// `action_frame == n`; the action returns to Wait once every sample is spent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub forward: RollMotion,
    pub backward: RollMotion,
    pub spot_dodge: SpotDodgeMotion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollMotion {
    pub frames: Vec<RollFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollFrame {
    pub bones: Vec<Bone>,
    /// Source TransN delta along local forward, converted by `ft_80085030`
    /// into the exact ground velocity for this frame.
    pub root_translation: f32,
    /// Scripted fighter-wide hurtbox collision state for this frame.
    pub body_state: BodyState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotDodgeMotion {
    pub frames: Vec<SpotDodgeFrame>,
}

/// `ftCo_EscapeN_Phys` ignores TransN motion, so spot-dodge samples carry no
/// root translation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotDodgeFrame {
    pub bones: Vec<Bone>,
    pub body_state: BodyState,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
) -> Result<(), Error> {
    if !(rules.roll_stick_threshold > 0.0 && rules.roll_stick_threshold <= 1.0)
        || !(-1.0..0.0).contains(&rules.spot_dodge_stick_threshold)
        || ![rules.roll_window, rules.spot_dodge_window]
            .into_iter()
            .all(|window| (1..254).contains(&window))
    {
        return Err(Error::Data("invalid ordinary escape rules".into()));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "escape motions require locomotion parameters for shared stick ages".into(),
        ));
    }
    for motion in [&parameters.forward, &parameters.backward] {
        if motion.frames.is_empty() || motion.frames.len() > 4096 {
            return Err(Error::Data(
                "escape rolls require 1..4096 physics samples".into(),
            ));
        }
        for frame in &motion.frames {
            if !frame.root_translation.is_finite() || frame.root_translation.abs() > 1_000_000.0 {
                return Err(Error::Data("invalid escape root translation".into()));
            }
            super::validation::validate_animation_pose(&frame.bones, fighter)?;
        }
    }
    let spot = &parameters.spot_dodge;
    if spot.frames.is_empty() || spot.frames.len() > 4096 {
        return Err(Error::Data(
            "spot dodge requires 1..4096 physics samples".into(),
        ));
    }
    for frame in &spot.frames {
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::EscapeF | Action::EscapeB | Action::EscapeN)
}

fn roll<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a RollFrame> {
    let parameters = data.escape.as_ref()?;
    let motion = match fighter.action {
        Action::EscapeF => &parameters.forward,
        Action::EscapeB => &parameters.backward,
        _ => return None,
    };
    motion.frames.get(fighter.action_frame as usize)
}

fn spot_dodge<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a SpotDodgeFrame> {
    if fighter.action != Action::EscapeN {
        return None;
    }
    data.escape
        .as_ref()?
        .spot_dodge
        .frames
        .get(fighter.action_frame as usize)
}

fn motion_length(fighter: &Fighter, data: &FighterData) -> Option<usize> {
    let parameters = data.escape.as_ref()?;
    Some(match fighter.action {
        Action::EscapeF => parameters.forward.frames.len(),
        Action::EscapeB => parameters.backward.frames.len(),
        Action::EscapeN => parameters.spot_dodge.frames.len(),
        _ => return None,
    })
}

/// Physics bones for the current escape sample.
pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    if let Some(frame) = roll(fighter, data) {
        return Some(&frame.bones);
    }
    spot_dodge(fighter, data).map(|frame| frame.bones.as_slice())
}

/// `ft_80085004`'s target ground velocity for the current roll sample.
pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    Some(roll(fighter, data)?.root_translation * fighter.facing)
}

fn sampled_body_state(fighter: &Fighter, data: &FighterData) -> Result<BodyState, Error> {
    if let Some(frame) = roll(fighter, data) {
        return Ok(frame.body_state);
    }
    spot_dodge(fighter, data)
        .map(|frame| frame.body_state)
        .ok_or_else(|| Error::Physics("escape sample is outside the supplied motion".into()))
}

/// Priority-1 callback shared by `ftCo_Escape_Anim` and `ftCo_EscapeN_Anim`.
/// Returns whether an escape still owns this frame's action dispatch.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<bool, Error> {
    if !owns_action(fighter.action) {
        return Ok(false);
    }
    let length = motion_length(fighter, data)
        .ok_or_else(|| Error::Data("escape action requires escape motions".into()))?;
    if fighter.action_frame as usize >= length {
        // Both callbacks return to Wait; the roll additionally clears gr_vel.
        if fighter.action != Action::EscapeN {
            fighter.ground_velocity = 0.0;
        }
        super::simulation::enter(fighter, Action::Wait);
        return Ok(false);
    }
    // The subaction script applies this frame's collision state before contacts.
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(true)
}

fn start(fighter: &mut Fighter, data: &FighterData, action: Action) -> Result<(), Error> {
    super::simulation::enter(fighter, action);
    // Fighter_ChangeMotionState clears the Slippi-visible x2218 reflect bit and
    // the x221C_b3 powershield entry latch; the x221C_b2 immunity window and
    // its frozen timer persist because only guard callbacks tick them.
    fighter.shield.reflecting = false;
    fighter.shield.powershield_just_started = false;
    // It also resets x1988, and ftAnim_8006EBA4 immediately runs the first
    // script frame, so sample 0's state protects the entry frame.
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(())
}

/// `ftCo_8009980C`: fresh downward main stick or held downward C-stick.
pub(crate) fn try_spot_dodge(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Result<bool, Error> {
    let (Some(rules), Some(_)) = (rules, data.escape.as_ref()) else {
        return Ok(false);
    };
    if !math::spot_dodge_request(
        input.stick[1],
        fighter.locomotion.tilt_y_age,
        input.cstick[1],
        rules.spot_dodge_stick_threshold,
        rules.spot_dodge_window,
    ) {
        return Ok(false);
    }
    start(fighter, data, Action::EscapeN)?;
    Ok(true)
}

/// `ftCo_8009917C`: fresh main-stick roll with held-C-stick fallback, with the
/// facing-relative direction chosen from the selected axis value.
pub(crate) fn try_roll(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Result<bool, Error> {
    let (Some(rules), Some(_)) = (rules, data.escape.as_ref()) else {
        return Ok(false);
    };
    let Some(direction) = math::roll_request(
        input.stick[0],
        fighter.locomotion.tilt_x_age,
        input.cstick[0],
        fighter.facing,
        rules.roll_stick_threshold,
        rules.roll_window,
    ) else {
        return Ok(false);
    };
    start(
        fighter,
        data,
        match direction {
            RollDirection::Forward => Action::EscapeF,
            RollDirection::Backward => Action::EscapeB,
        },
    )?;
    Ok(true)
}
