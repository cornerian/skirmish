//! Grounded tilts from `ftCo_AttackS3.c`, `ftCo_AttackHi3.c` and
//! `ftCo_AttackLw3.c`: forward tilt with its four angle variants, up tilt and
//! the down tilt with its buffered repeat. Attacks are supplied physics
//! samples with per-frame script flags. Smashes, dash attacks, item branches
//! and the Game & Watch down-tilt override are not modeled.
use super::{
    Action, Controller, Error, Fighter,
    data::{Attack, FighterData},
};
use crate::fighter::{aerial::stick_angle, tilt as math, tilt::ForwardVariant};
use serde::{Deserialize, Serialize};

/// Common tilt input data (`ftCommonData` x20 and x98..xB0).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `x98`: inclusive facing-relative stick X for a forward tilt.
    pub forward_stick_threshold: f32,
    /// `x20_radians`: forward tilts need the folded angle strictly inside it;
    /// up and down tilts need the angle strictly beyond it.
    pub angle_limit: f32,
    /// `x9C_radians`, `xA0_radians`, `xA4_radians`, `xA8_radians`.
    pub forward_high: f32,
    pub forward_high_slight: f32,
    pub forward_low_slight: f32,
    pub forward_low: f32,
    /// `attackhi3_stick_threshold_y`: inclusive stick Y for an up tilt.
    pub up_stick_threshold: f32,
    /// `xB0`: inclusive (negative) stick Y for a down tilt.
    pub down_stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub forward: ForwardTilts,
    pub up: GroundAttack,
    pub down: GroundAttack,
}

/// Optional variants stand in for the source's figatree availability checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardTilts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<GroundAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_slight: Option<GroundAttack>,
    pub straight: GroundAttack,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_slight: Option<GroundAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<GroundAttack>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundAttack {
    pub attack: Attack,
    /// One decoded command-state sample per attack pose.
    pub flags: Vec<GroundFrameFlags>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundFrameFlags {
    /// `Fighter::allow_interrupt` after this frame's script commands.
    pub allow_interrupt: bool,
    /// Down tilt only: `cmd_vars[cmd_unk0_bool]` after this frame's commands.
    #[serde(default)]
    pub repeat_ready: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    /// `attacklw3.x0`: a fresh A press buffered before the repeat flag.
    pub repeat_buffered: bool,
}

/// Input chain an interruptible tilt hands its frame to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Chain {
    /// `ftCo_Wait_IASA`: attacks, grab, shield, jump, dash, squat, turn, walk.
    Wait,
    /// `ftCo_AttackLw3_IASA`: attacks, jump, dash, squat, turn and walk only.
    DownTilt,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
    staling: bool,
) -> Result<(), Error> {
    let finite = |value: f32| value.is_finite() && value.abs() <= core::f32::consts::PI;
    if !(0.0..=1.0).contains(&rules.forward_stick_threshold)
        || !(0.0..=1.0).contains(&rules.up_stick_threshold)
        || !(-1.0..=0.0).contains(&rules.down_stick_threshold)
        || !finite(rules.angle_limit)
        || rules.angle_limit <= 0.0
        || ![
            rules.forward_high,
            rules.forward_high_slight,
            rules.forward_low_slight,
            rules.forward_low,
        ]
        .into_iter()
        .all(finite)
    {
        return Err(Error::Data("invalid ordinary tilt rules".into()));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "tilt attacks require locomotion parameters for their input chains".into(),
        ));
    }
    let forward = &parameters.forward;
    let optional = [
        forward.high.as_ref(),
        forward.high_slight.as_ref(),
        forward.low_slight.as_ref(),
        forward.low.as_ref(),
    ];
    for attack in optional
        .into_iter()
        .flatten()
        .chain([&forward.straight, &parameters.up])
    {
        validate_attack(attack, false)?;
    }
    validate_attack(&parameters.down, true)?;
    if staling {
        let ids: Vec<_> = optional
            .into_iter()
            .flatten()
            .chain([&forward.straight])
            .map(|attack| attack.attack.move_id)
            .collect();
        if ids.iter().any(|id| *id != forward.straight.attack.move_id) {
            return Err(Error::Data(
                "forward-tilt variants share one native move identity".into(),
            ));
        }
    }
    Ok(())
}

fn validate_attack(attack: &GroundAttack, down: bool) -> Result<(), Error> {
    if attack.flags.len() != attack.attack.frames.len() {
        return Err(Error::Data(
            "tilt command flags must cover every attack pose".into(),
        ));
    }
    if !down && attack.flags.iter().any(|flags| flags.repeat_ready) {
        return Err(Error::Data(
            "only the down tilt consumes a repeat flag".into(),
        ));
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::AttackS3Hi
            | Action::AttackS3HiS
            | Action::AttackS3S
            | Action::AttackS3LwS
            | Action::AttackS3Lw
            | Action::AttackHi3
            | Action::AttackLw3
    )
}

/// Every supplied tilt attack, for validation and staling identity checks.
pub(crate) fn ground_attack(parameters: &Parameters, action: Action) -> Option<&GroundAttack> {
    let forward = &parameters.forward;
    match action {
        Action::AttackS3Hi => forward.high.as_ref(),
        Action::AttackS3HiS => forward.high_slight.as_ref(),
        Action::AttackS3S => Some(&forward.straight),
        Action::AttackS3LwS => forward.low_slight.as_ref(),
        Action::AttackS3Lw => forward.low.as_ref(),
        Action::AttackHi3 => Some(&parameters.up),
        Action::AttackLw3 => Some(&parameters.down),
        _ => None,
    }
}

pub(crate) fn attack(parameters: &Parameters, action: Action) -> Option<&Attack> {
    ground_attack(parameters, action).map(|attack| &attack.attack)
}

fn flags(fighter: &Fighter, data: &FighterData) -> Option<GroundFrameFlags> {
    ground_attack(data.tilts.as_ref()?, fighter.action)?
        .flags
        .get(fighter.action_frame as usize)
        .copied()
}

/// The chain an interruptible tilt exposes on this frame, if any.
pub(crate) fn interrupt_chain(fighter: &Fighter, data: &FighterData) -> Option<Chain> {
    if !fighter.grounded || !owns_action(fighter.action) {
        return None;
    }
    let flags = flags(fighter, data)?;
    if !flags.allow_interrupt {
        return None;
    }
    Some(if fighter.action == Action::AttackLw3 {
        Chain::DownTilt
    } else {
        Chain::Wait
    })
}

/// Grounded states whose input chains reach the tilt and jab dispatchers.
pub(crate) fn attack_state(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.grounded
        && (matches!(
            fighter.action,
            Action::Wait
                | Action::Walk
                | Action::Turn
                | Action::Squat
                | Action::SquatWait
                | Action::SquatRv
                | Action::AttackLw3
        ) || interrupt_chain(fighter, data).is_some())
}

fn start(fighter: &mut Fighter, action: Action) {
    super::simulation::enter(fighter, action);
    fighter.tilt = State::default();
}

/// `ftCo_AttackS3_CheckInput`, `ftCo_AttackHi3_CheckInput` and
/// `ftCo_AttackLw3_CheckInput` in their shared order, then the jab.
/// `facing` is the value the chain evaluates (Turn applies its flip first).
fn attacks(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: &Rules,
    input: Controller,
    facing: f32,
    down_allowed: bool,
) -> Option<Action> {
    let parameters = data.tilts.as_ref()?;
    let pressed = input.buttons & !fighter.previous_input.buttons & super::BUTTON_A != 0;
    if !pressed {
        return None;
    }
    let angle = stick_angle(input.stick);
    if math::forward_tilt(
        pressed,
        input.stick[0],
        facing,
        angle,
        rules.forward_stick_threshold,
        rules.angle_limit,
    ) {
        let forward = &parameters.forward;
        return Some(
            match math::forward_variant(
                angle,
                [
                    rules.forward_high,
                    rules.forward_high_slight,
                    rules.forward_low_slight,
                    rules.forward_low,
                ],
                [
                    forward.high.is_some(),
                    forward.high_slight.is_some(),
                    forward.low_slight.is_some(),
                    forward.low.is_some(),
                ],
            ) {
                ForwardVariant::High => Action::AttackS3Hi,
                ForwardVariant::HighSlight => Action::AttackS3HiS,
                ForwardVariant::Straight => Action::AttackS3S,
                ForwardVariant::LowSlight => Action::AttackS3LwS,
                ForwardVariant::Low => Action::AttackS3Lw,
            },
        );
    }
    if math::up_tilt(
        pressed,
        input.stick[1],
        angle,
        rules.up_stick_threshold,
        rules.angle_limit,
    ) {
        return Some(Action::AttackHi3);
    }
    if down_allowed
        && math::down_tilt(
            pressed,
            input.stick[1],
            angle,
            rules.down_stick_threshold,
            rules.angle_limit,
        )
    {
        return Some(Action::AttackLw3);
    }
    None
}

/// Grounded A-attack dispatch shared by Wait-family states and interruptible
/// tilts. Returns whether an attack started.
pub(crate) fn update_ground_attacks(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> bool {
    if !attack_state(fighter, data) {
        return false;
    }
    let pressed = input.buttons & !fighter.previous_input.buttons & super::BUTTON_A != 0;
    // Turn_IASA evaluates the whole chain with the post-turn facing.
    let facing = if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
        -fighter.facing
    } else {
        fighter.facing
    };
    let chain = interrupt_chain(fighter, data);
    if fighter.action == Action::AttackLw3 {
        // ftCo_AttackLw3_IASA: forward and up tilts, checkPadA, then the down
        // tilt itself and the jab.
        if chain.is_some()
            && let Some(rules) = rules
            && let Some(action) = attacks(fighter, data, rules, input, facing, false)
        {
            start(fighter, action);
            return true;
        }
        let ready = flags(fighter, data).is_some_and(|flags| flags.repeat_ready);
        if math::down_tilt_repeat(pressed, ready, &mut fighter.tilt.repeat_buffered) {
            start(fighter, Action::AttackLw3);
            return true;
        }
        if chain.is_none() {
            return false;
        }
        if let Some(rules) = rules
            && let Some(action) = attacks(fighter, data, rules, input, facing, true)
        {
            start(fighter, action);
            return true;
        }
    } else if let Some(rules) = rules
        && let Some(action) = attacks(fighter, data, rules, input, facing, true)
    {
        if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
            fighter.facing = -fighter.facing;
        }
        start(fighter, action);
        return true;
    }
    if pressed {
        if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
            fighter.facing = -fighter.facing;
        }
        super::simulation::enter(fighter, Action::Jab);
        return true;
    }
    false
}

/// `ftCo_AttackS3_Anim`, `ftCo_AttackHi3_Anim` and `ftCo_AttackLw3_Anim`.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        return Ok(());
    }
    let parameters = data
        .tilts
        .as_ref()
        .ok_or_else(|| Error::Data("tilt actions require tilt resources".into()))?;
    let attack = ground_attack(parameters, fighter.action)
        .ok_or_else(|| Error::Data("tilt action without its supplied variant".into()))?;
    if fighter.action == Action::AttackLw3
        && fighter.tilt.repeat_buffered
        && attack
            .flags
            .get(fighter.action_frame as usize)
            .is_some_and(|flags| flags.repeat_ready)
    {
        start(fighter, Action::AttackLw3);
        return Ok(());
    }
    if fighter.action_frame as usize >= attack.attack.frames.len() {
        let next = if !fighter.grounded {
            Action::Fall
        } else if fighter.action == Action::AttackLw3 {
            Action::SquatWait
        } else {
            Action::Wait
        };
        super::simulation::enter(fighter, next);
    }
    Ok(())
}
