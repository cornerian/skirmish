// Grounded shield-evasion input predicates and motion callbacks from
// `ftCo_Escape.c` and the C-stick helpers in `ft_0DF1.c`. The per-character
// Samus/Yoshi entry branches and unread `x324` flag copy remain caller-owned.

use crate::game::{
    Action, Controller, Error, Fighter,
    data::{BodyState, Bone, FighterData},
};
use serde::{Deserialize, Serialize};

/// Roll direction relative to the fighter's facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollDirection {
    Forward,
    Backward,
}

/// `inlineA1`: a fresh main-stick horizontal excursion at or beyond `x31C`
/// whose shared tilt age is inside the `x320` window.
pub fn main_stick_roll(stick_x: f32, tilt_x_age: u8, threshold: f32, window: u8) -> bool {
    stick_x.abs() >= threshold && tilt_x_age < window
}

/// Complete `ftCo_800DF8B0`: held horizontal C-stick at or beyond `x31C`,
/// without any freshness requirement.
pub fn cstick_roll(cstick_x: f32, threshold: f32) -> bool {
    cstick_x.abs() >= threshold
}

/// `stick_x * facing_dir >= 0` selects EscapeF; the exact source comparison
/// keeps zero products (including signed zero and `-0.0` facing) forward.
pub fn roll_direction(stick_x: f32, facing: f32) -> RollDirection {
    if stick_x * facing >= 0.0 {
        RollDirection::Forward
    } else {
        RollDirection::Backward
    }
}

/// Complete `ftCo_8009917C` selection: the fresh main stick has priority, the
/// held C-stick is the fallback, and the chosen axis value picks the direction.
pub fn roll_request(
    stick_x: f32,
    tilt_x_age: u8,
    cstick_x: f32,
    facing: f32,
    threshold: f32,
    window: u8,
) -> Option<RollDirection> {
    let source = if main_stick_roll(stick_x, tilt_x_age, threshold, window) {
        stick_x
    } else if cstick_roll(cstick_x, threshold) {
        cstick_x
    } else {
        return None;
    };
    Some(roll_direction(source, facing))
}

/// `inlineB0`: a fresh downward main-stick excursion at or below `x314`
/// (a negative native value) inside the `x318` window.
pub fn main_stick_spot_dodge(stick_y: f32, tilt_y_age: u8, threshold: f32, window: u8) -> bool {
    stick_y <= threshold && tilt_y_age < window
}

/// Complete `ftCo_800DF8E8`: held downward C-stick at or below `x314`.
pub fn cstick_spot_dodge(cstick_y: f32, threshold: f32) -> bool {
    cstick_y <= threshold
}

/// Complete `ftCo_8009980C` selection without its transition side effect.
pub fn spot_dodge_request(
    stick_y: f32,
    tilt_y_age: u8,
    cstick_y: f32,
    threshold: f32,
    window: u8,
) -> bool {
    main_stick_spot_dodge(stick_y, tilt_y_age, threshold, window)
        || cstick_spot_dodge(cstick_y, threshold)
}

// Ordinary grounded shield evasions from `ftCo_Escape.c`: EscapeF/EscapeB
// rolls and the EscapeN spot dodge. Motions are supplied physics samples with
// per-frame TransN root translation, bone poses and the scripted fighter-wide
// hurtbox collision state (`Fighter::x1988`). The Samus/Yoshi entry branches,
// item-throw interrupts and the unread `x324` copy are not modeled.
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
            crate::game::validation::validate_animation_pose(&frame.bones, fighter)?;
        }
    }
    let spot = &parameters.spot_dodge;
    if spot.frames.is_empty() || spot.frames.len() > 4096 {
        return Err(Error::Data(
            "spot dodge requires 1..4096 physics samples".into(),
        ));
    }
    for frame in &spot.frames {
        crate::game::validation::validate_animation_pose(&frame.bones, fighter)?;
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
        crate::game::simulation::enter(fighter, Action::Wait);
        return Ok(false);
    }
    // The subaction script applies this frame's collision state before contacts.
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(true)
}

fn start(fighter: &mut Fighter, data: &FighterData, action: Action) -> Result<(), Error> {
    crate::game::simulation::enter(fighter, action);
    finish_start(fighter, data)
}

fn finish_start(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    crate::fighter::shield::leave_guard(fighter);
    // Fighter_ChangeMotionState also resets x1988, and ftAnim_8006EBA4
    // immediately runs the first script frame, so sample 0's state protects
    // the entry frame.
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(())
}

fn start_registered(
    fighter: &mut Fighter,
    data: &FighterData,
    action: Action,
) -> Result<bool, Error> {
    let slot = match action {
        Action::EscapeF => crate::game::script::move_registry::MoveSlot::RollForward,
        Action::EscapeB => crate::game::script::move_registry::MoveSlot::RollBack,
        Action::EscapeN => crate::game::script::move_registry::MoveSlot::SpotDodge,
        _ => {
            start(fighter, data, action)?;
            return Ok(true);
        }
    };
    match crate::game::script::move_selection::select_native_move(
        fighter,
        data,
        crate::game::script::move_registry::MoveGroup::Defense,
        slot,
        action,
    ) {
        crate::game::script::move_selection::NativeMoveSelection::Entered(_)
        | crate::game::script::move_selection::NativeMoveSelection::Unbound(_) => {
            if owns_action(fighter.action) {
                finish_start(fighter, data)?;
            }
            Ok(true)
        }
        crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. } => Ok(true),
    }
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
    if !self::spot_dodge_request(
        input.stick[1],
        fighter.locomotion.tilt_y_age,
        input.cstick[1],
        rules.spot_dodge_stick_threshold,
        rules.spot_dodge_window,
    ) {
        return Ok(false);
    }
    start_registered(fighter, data, Action::EscapeN)
}

/// `ftCo_80099794`/`inlineB0`: in the Wait and AppealS chains only (`ftCo_
/// Wait.c:58` precedes the ordinary shield check `ftCo_80091A4C` at line 59;
/// `ftCo_AppealS_IASA` checks it at the same relative position), a held
/// logical shoulder together with a fresh downward main stick (age inside
/// the `x318` window) enters EscapeN directly -- unlike `try_spot_dodge`
/// above (`ftCo_8009980C`), the held C-stick alternative (`ftCo_800DF8E8`)
/// is not part of this check, and the shoulder must be held rather than the
/// stick predicate alone being sufficient. Walk's chain has no
/// `ftCo_80099794` call, so this must not be reached from Walk.
pub(crate) fn try_wait_chain_spot_dodge(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Result<bool, Error> {
    let (Some(rules), Some(_)) = (rules, data.escape.as_ref()) else {
        return Ok(false);
    };
    if !input.shield_held()
        || !self::main_stick_spot_dodge(
            input.stick[1],
            fighter.locomotion.tilt_y_age,
            rules.spot_dodge_stick_threshold,
            rules.spot_dodge_window,
        )
    {
        return Ok(false);
    }
    start_registered(fighter, data, Action::EscapeN)
}

/// `ftCo_80099264`: a held shoulder starts the forward roll directly
/// (`ftCo_800992A8(EscapeF, false)`), skipping the ordinary stick-based
/// selection in `try_roll`. Dash's early phase only.
pub(crate) fn dash_forward_roll(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Result<bool, Error> {
    if rules.is_none() || data.escape.is_none() || !input.shield_held() {
        return Ok(false);
    }
    start_registered(fighter, data, Action::EscapeF)
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
    let Some(direction) = self::roll_request(
        input.stick[0],
        fighter.locomotion.tilt_x_age,
        input.cstick[0],
        fighter.facing,
        rules.roll_stick_threshold,
        rules.roll_window,
    ) else {
        return Ok(false);
    };
    start_registered(
        fighter,
        data,
        match direction {
            RollDirection::Forward => Action::EscapeF,
            RollDirection::Backward => Action::EscapeB,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_stick_roll_uses_inclusive_magnitude_and_strict_age() {
        assert!(main_stick_roll(0.7, 3, 0.7, 4));
        assert!(main_stick_roll(-0.7, 0, 0.7, 4));
        assert!(!main_stick_roll(0.699, 0, 0.7, 4));
        assert!(!main_stick_roll(1.0, 4, 0.7, 4));
        assert!(!main_stick_roll(f32::NAN, 0, 0.7, 4));
    }

    #[test]
    fn cstick_roll_ignores_freshness() {
        assert!(cstick_roll(0.7, 0.7));
        assert!(cstick_roll(-1.0, 0.7));
        assert!(!cstick_roll(0.69, 0.7));
        assert!(!cstick_roll(f32::NAN, 0.7));
    }

    #[test]
    fn direction_follows_the_signed_product_with_zero_forward() {
        assert_eq!(roll_direction(0.8, 1.0), RollDirection::Forward);
        assert_eq!(roll_direction(0.8, -1.0), RollDirection::Backward);
        assert_eq!(roll_direction(-0.8, -1.0), RollDirection::Forward);
        assert_eq!(roll_direction(0.0, -1.0), RollDirection::Forward);
        assert_eq!(roll_direction(-0.0, 1.0), RollDirection::Forward);
        assert_eq!(roll_direction(f32::NAN, 1.0), RollDirection::Backward);
    }

    #[test]
    fn roll_request_prefers_the_fresh_main_stick_over_the_held_cstick() {
        assert_eq!(
            roll_request(-1.0, 0, 1.0, 1.0, 0.7, 4),
            Some(RollDirection::Backward)
        );
        assert_eq!(
            roll_request(-1.0, 4, 1.0, 1.0, 0.7, 4),
            Some(RollDirection::Forward)
        );
        assert_eq!(
            roll_request(-1.0, 4, -1.0, 1.0, 0.7, 4),
            Some(RollDirection::Backward)
        );
        assert_eq!(roll_request(0.5, 0, 0.5, 1.0, 0.7, 4), None);
    }

    #[test]
    fn spot_dodge_accepts_fresh_main_stick_or_held_cstick() {
        assert!(main_stick_spot_dodge(-0.7, 3, -0.7, 4));
        assert!(!main_stick_spot_dodge(-0.69, 0, -0.7, 4));
        assert!(!main_stick_spot_dodge(-1.0, 4, -0.7, 4));
        assert!(cstick_spot_dodge(-0.7, -0.7));
        assert!(!cstick_spot_dodge(-0.69, -0.7));
        assert!(spot_dodge_request(-1.0, 0, 0.0, -0.7, 4));
        assert!(spot_dodge_request(-1.0, 200, -0.7, -0.7, 4));
        assert!(!spot_dodge_request(-1.0, 200, 0.0, -0.7, 4));
        assert!(!spot_dodge_request(f32::NAN, 0, f32::NAN, -0.7, 4));
    }
}
