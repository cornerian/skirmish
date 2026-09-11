//! Taunts (AppealSR/AppealSL) from `ftCo_AppealS.c`. Motions are supplied
//! attack-like physics samples without hitboxes, reusing the escape motion's
//! pose/hurtbox-collision-state sample shape (`escape::SpotDodgeFrame`).
//! Young Link, Dr. Mario, Ganondorf, Kirby, the debug-only Peach/Zelda hooks
//! and the Fox/Falco Corneria special-taunt override (`ftFx_AppealS_
//! CheckInput`, checked before the ordinary D-pad-up press in `ftCo_Wait.c`)
//! are not modeled.
use super::{
    Action, Controller, Error, Fighter,
    data::{BodyState, Bone, FighterData},
    tilt::{Chain, GroundFrameFlags},
};
use crate::fighter::taunt as math;
use serde::{Deserialize, Serialize};

/// Per-fighter taunt motions (`Fighter::taunt`, i.e. `fp->x1C_actionStateList`
/// entries for `ftCo_MS_AppealSR`/`ftCo_MS_AppealSL`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Taunt {
    pub right: TauntAnimation,
    /// `ftCo_800DEAE8`'s `ftData_80085FD4(fp, ms->anim_id)->x8` figatree-
    /// availability check, modeled as "a left motion is supplied".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<TauntAnimation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauntAnimation {
    /// One physics/collision sample per pose. Frame 0's `body_state` is the
    /// entry frame's state (`Fighter_ChangeMotionState` resets `x1988`, and
    /// `ftAnim_8006EBA4` immediately runs the first script frame).
    pub frames: Vec<TauntFrame>,
    /// One decoded command-state sample per pose. Frame 0's `allow_interrupt`
    /// must be `false`, matching `ftCo_800DEAE8`'s own entry assignment
    /// (`fp->allow_interrupt = false`) before any script command can raise
    /// it; every later frame's own commands govern the flag from then on.
    pub flags: Vec<GroundFrameFlags>,
    /// `ft_80084FA8`'s root-motion branch (`fp->x594_b0`): per-pose TransN
    /// delta along local forward. `None` uses ordinary ground friction
    /// instead, exactly like every other root-motion-optional action here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_translations: Option<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauntFrame {
    pub bones: Vec<Bone>,
    pub body_state: BodyState,
}

pub(crate) fn validate(taunt: &Taunt, fighter: &FighterData) -> Result<(), Error> {
    validate_animation(&taunt.right, fighter)?;
    if let Some(left) = &taunt.left {
        validate_animation(left, fighter)?;
    }
    Ok(())
}

fn validate_animation(animation: &TauntAnimation, fighter: &FighterData) -> Result<(), Error> {
    if animation.frames.is_empty() || animation.frames.len() > 4096 {
        return Err(Error::Data("taunt requires 1..4096 physics samples".into()));
    }
    if animation.flags.len() != animation.frames.len() {
        return Err(Error::Data(
            "taunt command flags must cover every pose".into(),
        ));
    }
    if animation.flags.iter().any(|flags| flags.repeat_ready) {
        return Err(Error::Data(
            "taunt flags carry no down-tilt repeat state".into(),
        ));
    }
    if animation
        .flags
        .first()
        .is_some_and(|flags| flags.allow_interrupt)
    {
        return Err(Error::Data(
            "taunt entry is not interruptible before its first pose's own commands".into(),
        ));
    }
    for frame in &animation.frames {
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    if let Some(roots) = &animation.root_translations
        && (roots.len() != animation.frames.len()
            || roots
                .iter()
                .any(|root| !root.is_finite() || root.abs() > 1_000_000.0))
    {
        return Err(Error::Data(
            "taunt root motion must supply one finite delta per pose".into(),
        ));
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::AppealSR | Action::AppealSL)
}

fn current<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a TauntAnimation> {
    if !owns_action(fighter.action) {
        return None;
    }
    let taunt = data.taunt.as_ref()?;
    match fighter.action {
        Action::AppealSR => Some(&taunt.right),
        Action::AppealSL => taunt.left.as_ref(),
        _ => unreachable!(),
    }
}

/// Physics bones for the current taunt sample.
pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    current(fighter, data)?
        .frames
        .get(fighter.action_frame as usize)
        .map(|frame| frame.bones.as_slice())
}

fn flags(fighter: &Fighter, data: &FighterData) -> Option<GroundFrameFlags> {
    current(fighter, data)?
        .flags
        .get(fighter.action_frame as usize)
        .copied()
}

/// The chain an interruptible taunt exposes on this frame, if any:
/// `ftCo_AppealS_IASA`'s own chain is `ftCo_Wait_IASA`'s attack/catch/shield
/// segment (specials, catch, smashes, tilts, jab, the Wait-chain spot dodge,
/// shield) without its jump/dash/squat/turn/walk tail.
pub(crate) fn interrupt_chain(fighter: &Fighter, data: &FighterData) -> Option<Chain> {
    if !fighter.grounded {
        return None;
    }
    if flags(fighter, data)?.allow_interrupt {
        Some(Chain::Taunt)
    } else {
        None
    }
}

/// `ft_80084FA8`'s TransN target for a taunt sample, if supplied.
pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    let roots = current(fighter, data)?.root_translations.as_ref()?;
    Some(*roots.get(fighter.action_frame as usize)? * fighter.facing)
}

fn sampled_body_state(fighter: &Fighter, data: &FighterData) -> Result<BodyState, Error> {
    current(fighter, data)
        .and_then(|animation| animation.frames.get(fighter.action_frame as usize))
        .map(|frame| frame.body_state)
        .ok_or_else(|| Error::Physics("taunt sample is outside the supplied motion".into()))
}

/// `ftCo_AppealS_Anim`: end of poses returns to Wait (taunts are grounded
/// only, so there is no airborne `Fall` destination to consider).
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        return Ok(());
    }
    let length = current(fighter, data)
        .ok_or_else(|| Error::Data("taunt action requires its supplied animation".into()))?
        .frames
        .len();
    if fighter.action_frame as usize >= length {
        super::simulation::enter(fighter, Action::Wait);
        return Ok(());
    }
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(())
}

/// `ftCo_800DE9D8`/`ftCo_800DEAE8`: a fresh D-pad-up press starts AppealSL
/// when facing left and a left motion is supplied, else AppealSR.
pub(crate) fn try_taunt(
    fighter: &mut Fighter,
    data: &FighterData,
    input: Controller,
) -> Result<bool, Error> {
    let Some(taunt) = data.taunt.as_ref() else {
        return Ok(false);
    };
    let pressed_buttons = input.buttons & !fighter.previous_input.buttons;
    if !fighter.grounded || !math::pressed(pressed_buttons, super::BUTTON_DPAD_UP) {
        return Ok(false);
    }
    let action = match math::select_side(fighter.facing, taunt.left.is_some()) {
        math::Side::Left => Action::AppealSL,
        math::Side::Right => Action::AppealSR,
    };
    super::simulation::enter(fighter, action);
    fighter.body_state = sampled_body_state(fighter, data)?;
    Ok(true)
}
