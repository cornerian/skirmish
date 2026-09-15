//! Smash-attack input predicates from `ftCo_AttackS4.c`, `ftCo_AttackHi4.c`
//! and `ftCo_AttackLw4.c`, the fresh C-stick predicates from `ft_0DF1.c`,
//! and the smash charge arithmetic from `ft_0DF0.c`.

/// `Fighter::smash_attrs.state`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeState {
    #[default]
    None,
    PreCharge,
    Charging,
    Release,
}

/// `checkLStick` of `ftCo_AttackS4_CheckInput`: a fresh A press with the
/// stick at or beyond the dash-smash magnitude inside its window.
pub fn forward_smash_input(
    a_pressed: bool,
    stick_x: f32,
    tilt_x_age: u8,
    stick_threshold: f32,
    window: u8,
) -> bool {
    a_pressed && stick_x.abs() >= stick_threshold && tilt_x_age < window
}

/// Complete `ftCo_800DF1C8`: the C-stick crossed the dash-smash magnitude on
/// either side this frame.
pub fn fresh_cstick_smash_x(previous_x: f32, current_x: f32, stick_threshold: f32) -> bool {
    previous_x.abs() < stick_threshold && current_x.abs() >= stick_threshold
}

/// The facing the smash adopts: `x >= 0 ? +1 : -1`, so zero and negative
/// zero face right and NaN faces left.
pub fn stick_sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// `checkLStick` of `ftCo_AttackHi4_CheckInput`: inclusive stick Y with the
/// byte age strictly below the float window `xD0`. `checkLStickNoD0`
/// (KneeBend) drops the window.
pub fn up_smash_input(
    a_pressed: bool,
    stick_y: f32,
    tilt_y_age: u8,
    threshold: f32,
    window: f32,
) -> bool {
    a_pressed && stick_y >= threshold && f32::from(tilt_y_age) < window
}

/// `checkLStick` of `ftCo_AttackLw4_CheckInput` with the float window `xD8`.
pub fn down_smash_input(
    a_pressed: bool,
    stick_y: f32,
    tilt_y_age: u8,
    threshold: f32,
    window: f32,
) -> bool {
    a_pressed && stick_y <= threshold && f32::from(tilt_y_age) < window
}

/// `ftCo_800DEEB8`: released charges scale damage by the charged fraction of
/// the extra multiplier; every other state leaves it untouched.
pub fn charge_damage(
    damage: f32,
    state: ChargeState,
    frames: f32,
    hold_frames: f32,
    multiplier: f32,
) -> f32 {
    if state != ChargeState::Release {
        return damage;
    }
    damage * ((multiplier - 1.0) * (frames / hold_frames) + 1.0)
}

/// `ftCo_800DF0D0`: the pre-charge frame commits to charging only while the
/// logical A is held, and releasing it ends a charge. Returns the new state.
pub fn charge_input(state: ChargeState, a_held: bool) -> ChargeState {
    match state {
        ChargeState::PreCharge => {
            if a_held {
                ChargeState::Charging
            } else {
                ChargeState::None
            }
        }
        ChargeState::Charging if !a_held => ChargeState::Release,
        other => other,
    }
}

/// `ftCo_800DEF38`'s charging branch: count a frame and release at the hold
/// limit, clamping the count to it. Returns the new state.
pub fn charge_tick(state: ChargeState, frames: &mut f32, hold_frames: f32) -> ChargeState {
    if state != ChargeState::Charging {
        return state;
    }
    *frames += 1.0;
    if *frames >= hold_frames {
        if *frames > hold_frames {
            *frames = hold_frames;
        }
        return ChargeState::Release;
    }
    ChargeState::Charging
}

// Smash attacks from `ftCo_AttackS4.c`, `ftCo_AttackHi4.c` and
// `ftCo_AttackLw4.c` with the `ft_0DF0.c` charge state machine. Attacks are
// supplied physics samples with per-pose interrupt flags, an optional
// charge command and, for forward smashes, per-pose TransN root motion.
// Item branches, character overrides, Link's second hit, the forward smash
// out of an early Dash and the charge shake's visual offset are not modeled.
use crate::fighter::tilt::GroundFrameFlags;
use crate::fighter::{
    aerial::stick_angle,
    tilt::{ForwardVariant, forward_variant},
};
use crate::game::grab::{fresh_down, fresh_up};
use crate::game::{
    Action, Controller, Error, Fighter,
    data::{Attack, FighterData},
};
use serde::{Deserialize, Serialize};

/// Common smash input data (`ftCommonData` xB8..xD8 and x7C4). The forward
/// stick magnitude and window are the locomotion dash-smash values.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `xB8_radians`, `xBC_radians`, `xC0_radians`, `xC4_radians`.
    pub forward_high: f32,
    pub forward_high_slight: f32,
    pub forward_low_slight: f32,
    pub forward_low: f32,
    /// `xCC` / `xD0`: inclusive stick Y and the float window the byte stick
    /// age must stay strictly below for an up smash; the C-stick crossing
    /// uses the same threshold.
    pub up_stick_threshold: f32,
    pub up_window: f32,
    /// `xD4` / `xD8`: inclusive (negative) stick Y and window for a down smash.
    pub down_stick_threshold: f32,
    pub down_window: f32,
    /// `kb_smashcharge_mul`: knockback multiplier on a victim that is charging.
    pub charging_knockback_multiplier: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub forward: ForwardSmashes,
    pub up: SmashAttack,
    pub down: SmashAttack,
}

/// Optional variants stand in for the source's figatree availability checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardSmashes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<SmashAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_slight: Option<SmashAttack>,
    pub straight: SmashAttack,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_slight: Option<SmashAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<SmashAttack>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmashAttack {
    pub attack: Attack,
    /// One decoded command-state sample per attack pose.
    pub flags: Vec<GroundFrameFlags>,
    /// The script's smash-charge command, if the animation issues one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge: Option<ChargeCommand>,
    /// Forward smashes only: per-pose TransN delta consumed by `ft_80084FA8`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_translations: Option<Vec<f32>>,
}

/// `ftAction_80073008`: the pose whose script issues the command, its hold
/// frame count and the damage multiplier the producer decoded from the
/// command's charge rate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeCommand {
    pub frame: u32,
    pub hold_frames: f32,
    pub damage_multiplier: f32,
}

/// `Fighter::smash_attrs`, reset by every motion change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub charge: ChargeState,
    pub frames: f32,
    pub hold_frames: f32,
    pub damage_multiplier: f32,
    /// The script command already fired for this action instance, so a
    /// repeated animation callback after hitlag cannot arm it again.
    pub armed: bool,
    /// This frame's animation step ran at rate zero, so its TransN delta is
    /// zero and `ft_80085030` drives the ground speed to zero.
    pub frozen: bool,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
    staling: bool,
) -> Result<(), Error> {
    let radians = |value: f32| value.is_finite() && value.abs() <= core::f32::consts::PI;
    if ![
        rules.forward_high,
        rules.forward_high_slight,
        rules.forward_low_slight,
        rules.forward_low,
    ]
    .into_iter()
    .all(radians)
        || !(0.0..=1.0).contains(&rules.up_stick_threshold)
        || !(-1.0..=0.0).contains(&rules.down_stick_threshold)
        || ![rules.up_window, rules.down_window]
            .into_iter()
            .all(|window| window.is_finite() && window > 0.0 && window <= 255.0)
        || !rules.charging_knockback_multiplier.is_finite()
        || rules.charging_knockback_multiplier < 0.0
        || rules.charging_knockback_multiplier > 1_000_000.0
    {
        return Err(Error::Data("invalid ordinary smash rules".into()));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "smash attacks require locomotion parameters for the dash-smash window".into(),
        ));
    }
    for (attack, forward) in attacks(parameters) {
        validate_attack(attack, forward)?;
    }
    let forward = &parameters.forward;
    if staling
        && [
            forward.high.as_ref(),
            forward.high_slight.as_ref(),
            forward.low_slight.as_ref(),
            forward.low.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|attack| attack.attack.move_id != forward.straight.attack.move_id)
    {
        return Err(Error::Data(
            "forward-smash variants share one native move identity".into(),
        ));
    }
    Ok(())
}

/// Every supplied smash with whether it is a forward variant.
pub(crate) fn attacks(parameters: &Parameters) -> impl Iterator<Item = (&SmashAttack, bool)> {
    let forward = &parameters.forward;
    [
        forward.high.as_ref(),
        forward.high_slight.as_ref(),
        Some(&forward.straight),
        forward.low_slight.as_ref(),
        forward.low.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|attack| (attack, true))
    .chain([(&parameters.up, false), (&parameters.down, false)])
}

fn validate_attack(attack: &SmashAttack, forward: bool) -> Result<(), Error> {
    let frames = attack.attack.frames.len();
    if attack.flags.len() != frames || attack.flags.iter().any(|flags| flags.repeat_ready) {
        return Err(Error::Data(
            "smash command flags must cover every pose without repeat flags".into(),
        ));
    }
    if let Some(charge) = &attack.charge {
        if charge.frame as usize >= frames
            || !charge.hold_frames.is_finite()
            || charge.hold_frames <= 0.0
            || charge.hold_frames > 1_000_000.0
            || !charge.damage_multiplier.is_finite()
            || charge.damage_multiplier < 0.0
            || charge.damage_multiplier > 1_000_000.0
        {
            return Err(Error::Data("invalid smash charge command".into()));
        }
        // ftColl_8007ABD0 prices a hitbox once at creation, so hitboxes that
        // outlive the release would keep their uncharged damage; ordinary
        // smash scripts create every hitbox after the charge pose.
        if attack.attack.frames[..=charge.frame as usize]
            .iter()
            .any(|frame| !frame.hitboxes.is_empty())
        {
            return Err(Error::Data(
                "smash hitboxes must follow the charge command pose".into(),
            ));
        }
    }
    match &attack.root_translations {
        Some(_) if !forward => {
            return Err(Error::Data(
                "only forward smashes carry TransN root motion".into(),
            ));
        }
        Some(roots)
            if roots.len() != frames
                || roots
                    .iter()
                    .any(|root| !root.is_finite() || root.abs() > 1_000_000.0) =>
        {
            return Err(Error::Data(
                "smash root motion must supply one finite delta per pose".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::AttackS4Hi
            | Action::AttackS4HiS
            | Action::AttackS4S
            | Action::AttackS4LwS
            | Action::AttackS4Lw
            | Action::AttackHi4
            | Action::AttackLw4
    )
}

pub(crate) fn smash_attack(parameters: &Parameters, action: Action) -> Option<&SmashAttack> {
    let forward = &parameters.forward;
    match action {
        Action::AttackS4Hi => forward.high.as_ref(),
        Action::AttackS4HiS => forward.high_slight.as_ref(),
        Action::AttackS4S => Some(&forward.straight),
        Action::AttackS4LwS => forward.low_slight.as_ref(),
        Action::AttackS4Lw => forward.low.as_ref(),
        Action::AttackHi4 => Some(&parameters.up),
        Action::AttackLw4 => Some(&parameters.down),
        _ => None,
    }
}

pub(crate) fn attack(parameters: &Parameters, action: Action) -> Option<&Attack> {
    smash_attack(parameters, action).map(|attack| &attack.attack)
}

fn current<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a SmashAttack> {
    smash_attack(data.smashes.as_ref()?, fighter.action)
}

/// Whether this frame's flags open the Wait chain.
pub(crate) fn interruptible(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.grounded
        && current(fighter, data)
            .and_then(|attack| attack.flags.get(fighter.action_frame as usize))
            .is_some_and(|flags| flags.allow_interrupt)
}

/// `ftAnim_SetAnimRate(gobj, 0)` while charging holds the animation frame.
pub(crate) fn charging(fighter: &Fighter) -> bool {
    fighter.smash.charge == ChargeState::Charging
}

/// `ft_80084FA8`'s TransN target for a forward smash sample, if supplied. A
/// frozen animation step yields a zero delta.
pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    let roots = current(fighter, data)?.root_translations.as_ref()?;
    if fighter.smash.frozen {
        return Some(0.0);
    }
    Some(roots.get(fighter.action_frame as usize)? * fighter.facing)
}

/// `Fighter_procInput` folds physical Z into the logical A bit.
pub(crate) fn logical_a(buttons: u16) -> bool {
    buttons & (crate::game::BUTTON_A | crate::game::BUTTON_Z) != 0
}

pub(crate) fn a_pressed(fighter: &Fighter, input: Controller) -> bool {
    logical_a(input.buttons) && !logical_a(fighter.previous_input.buttons)
}

/// The Wait-chain smash checks in their shared order. `facing` is the value
/// the chain evaluates (Turn applies its flip first) and is what the up and
/// down smashes keep; forward smashes adopt the stick sign instead.
pub(crate) fn select(
    fighter: &Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
    facing: f32,
) -> Option<(Action, f32)> {
    let (rules, parameters, locomotion) =
        (rules?, data.smashes.as_ref()?, data.locomotion.as_ref()?);
    let a_pressed = a_pressed(fighter, input);
    let selected = if forward_smash_input(
        a_pressed,
        input.stick[0],
        fighter.locomotion.tilt_x_age,
        locomotion.dash_threshold,
        locomotion.dash_window,
    ) {
        Some((stick_sign(input.stick[0]), stick_angle(input.stick)))
    } else if fresh_cstick_smash_x(
        fighter.previous_input.cstick[0],
        input.cstick[0],
        locomotion.dash_threshold,
    ) {
        Some((stick_sign(input.cstick[0]), stick_angle(input.cstick)))
    } else {
        None
    };
    if let Some((sign, angle)) = selected {
        let forward = &parameters.forward;
        let action = match forward_variant(
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
            ForwardVariant::High => Action::AttackS4Hi,
            ForwardVariant::HighSlight => Action::AttackS4HiS,
            ForwardVariant::Straight => Action::AttackS4S,
            ForwardVariant::LowSlight => Action::AttackS4LwS,
            ForwardVariant::Low => Action::AttackS4Lw,
        };
        return Some((action, sign));
    }
    if up_smash_input(
        a_pressed,
        input.stick[1],
        fighter.locomotion.tilt_y_age,
        rules.up_stick_threshold,
        rules.up_window,
    ) || fresh_up(
        input.cstick[1],
        fighter.previous_input.cstick[1],
        rules.up_stick_threshold,
    ) {
        return Some((Action::AttackHi4, facing));
    }
    if down_smash_input(
        a_pressed,
        input.stick[1],
        fighter.locomotion.tilt_y_age,
        rules.down_stick_threshold,
        rules.down_window,
    ) || fresh_down(
        input.cstick[1],
        fighter.previous_input.cstick[1],
        rules.down_stick_threshold,
    ) {
        return Some((Action::AttackLw4, facing));
    }
    None
}

/// `ftCo_AttackS4_8008C114`: the Dash early-phase forward-smash entry. The
/// facing-relative stick check has no age window and keeps the current
/// facing; the C-stick crossing behaves like the ordinary Wait-chain check.
/// The item-throw branch is not modeled.
pub(crate) fn select_dash(
    fighter: &Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> Option<(Action, f32)> {
    let (rules, parameters, locomotion) =
        (rules?, data.smashes.as_ref()?, data.locomotion.as_ref()?);
    let a_pressed = a_pressed(fighter, input);
    let selected = if crate::fighter::dash::dash_forward_smash(
        a_pressed,
        input.stick[0],
        fighter.facing,
        locomotion.dash_threshold,
    ) {
        Some((fighter.facing, stick_angle(input.stick)))
    } else if fresh_cstick_smash_x(
        fighter.previous_input.cstick[0],
        input.cstick[0],
        locomotion.dash_threshold,
    ) {
        Some((stick_sign(input.cstick[0]), stick_angle(input.cstick)))
    } else {
        None
    };
    let (sign, angle) = selected?;
    let forward = &parameters.forward;
    let action = match forward_variant(
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
        ForwardVariant::High => Action::AttackS4Hi,
        ForwardVariant::HighSlight => Action::AttackS4HiS,
        ForwardVariant::Straight => Action::AttackS4S,
        ForwardVariant::LowSlight => Action::AttackS4LwS,
        ForwardVariant::Low => Action::AttackS4Lw,
    };
    Some((action, sign))
}

/// `ftCo_AttackHi4_CheckInputNoD0` from KneeBend: the up smash without its
/// age window, or the C-stick crossing.
pub(crate) fn jump_squat_up_smash(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> bool {
    let Some(rules) = rules else { return false };
    if data.smashes.is_none() {
        return false;
    }
    if (a_pressed(fighter, input) && input.stick[1] >= rules.up_stick_threshold)
        || fresh_up(
            input.cstick[1],
            fighter.previous_input.cstick[1],
            rules.up_stick_threshold,
        )
    {
        let facing = fighter.facing;
        return start_registered(fighter, data, Action::AttackHi4, facing);
    }
    false
}

fn arm(fighter: &mut Fighter, command: &ChargeCommand) {
    // ftCo_800DEE84
    fighter.smash = State {
        charge: ChargeState::PreCharge,
        frames: 0.0,
        hold_frames: command.hold_frames,
        damage_multiplier: command.damage_multiplier,
        armed: true,
        frozen: fighter.smash.frozen,
    };
}

/// Enter a selected smash. `doEnter` runs `ftAnim_8006EBA4` immediately, so
/// a command on the entry pose arms during the entry frame.
pub(crate) fn start(fighter: &mut Fighter, data: &FighterData, action: Action, facing: f32) {
    fighter.facing = facing;
    crate::game::simulation::enter(fighter, action);
    if let Some(command) = current(fighter, data).and_then(|attack| attack.charge.as_ref())
        && command.frame == 0
    {
        arm(fighter, command);
    }
}

pub(crate) fn start_registered(
    fighter: &mut Fighter,
    data: &FighterData,
    action: Action,
    facing: f32,
) -> bool {
    let slot = match action {
        Action::AttackS4Hi
        | Action::AttackS4HiS
        | Action::AttackS4S
        | Action::AttackS4LwS
        | Action::AttackS4Lw => crate::game::script::move_registry::MoveSlot::Forward,
        Action::AttackHi4 => crate::game::script::move_registry::MoveSlot::Up,
        Action::AttackLw4 => crate::game::script::move_registry::MoveSlot::Down,
        _ => {
            start(fighter, data, action, facing);
            return true;
        }
    };
    let default_entry = if matches!(slot, crate::game::script::move_registry::MoveSlot::Forward) {
        Action::AttackS4S
    } else {
        action
    };
    match crate::game::script::move_selection::select_native_move_variant(
        fighter,
        data,
        crate::game::script::move_registry::MoveGroup::Smashes,
        slot,
        default_entry,
        action,
    ) {
        crate::game::script::move_selection::NativeMoveSelection::Entered(_) => {
            fighter.facing = facing;
            true
        }
        crate::game::script::move_selection::NativeMoveSelection::Unbound(selected) => {
            start(fighter, data, selected, facing);
            true
        }
        crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. } => true,
    }
}

/// `ftCo_800DF0D0`, run before every fighter's input callback.
pub(crate) fn update_charge_input(fighter: &mut Fighter, input: Controller) {
    fighter.smash.charge = charge_input(fighter.smash.charge, logical_a(input.buttons));
}

/// The script's charge command, `ftCo_800DEF38` and the shared end check.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        fighter.smash.frozen = false;
        return Ok(());
    }
    let attack = current(fighter, data)
        .ok_or_else(|| Error::Data("smash action without its supplied variant".into()))?;
    // The animation step precedes the script, the tick and the input phase.
    fighter.smash.frozen = charging(fighter);
    if let Some(command) = &attack.charge
        && !fighter.smash.armed
        && fighter.action_frame == command.frame
    {
        arm(fighter, command);
    }
    fighter.smash.charge = charge_tick(
        fighter.smash.charge,
        &mut fighter.smash.frames,
        fighter.smash.hold_frames,
    );
    if fighter.action_frame as usize >= attack.attack.frames.len() {
        crate::game::simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_input_needs_fresh_magnitude_inside_the_window() {
        assert!(forward_smash_input(true, -0.8, 3, 0.8, 4));
        assert!(!forward_smash_input(true, 0.79, 0, 0.8, 4));
        assert!(!forward_smash_input(true, 1.0, 4, 0.8, 4));
        assert!(!forward_smash_input(false, 1.0, 0, 0.8, 4));
    }

    #[test]
    fn cstick_crossing_uses_absolute_values_on_both_frames() {
        assert!(fresh_cstick_smash_x(0.0, 0.8, 0.8));
        assert!(fresh_cstick_smash_x(0.5, -1.0, 0.8));
        assert!(!fresh_cstick_smash_x(-0.8, 0.8, 0.8));
        assert!(!fresh_cstick_smash_x(0.0, 0.79, 0.8));
    }

    #[test]
    fn sign_treats_zero_as_forward() {
        assert_eq!(stick_sign(0.0), 1.0);
        assert_eq!(stick_sign(-0.0), 1.0);
        assert_eq!(stick_sign(-0.1), -1.0);
        assert_eq!(stick_sign(f32::NAN), -1.0);
    }

    #[test]
    fn vertical_inputs_are_inclusive_with_strict_windows() {
        assert!(up_smash_input(true, 0.7, 3, 0.7, 4.0));
        assert!(!up_smash_input(true, 0.7, 4, 0.7, 4.0));
        assert!(up_smash_input(true, 0.7, 4, 0.7, 4.5));
        assert!(down_smash_input(true, -0.7, 0, -0.7, 4.0));
        assert!(!down_smash_input(true, -0.69, 0, -0.7, 4.0));
    }

    #[test]
    fn charge_state_machine_matches_the_source_transitions() {
        assert_eq!(
            charge_input(ChargeState::PreCharge, true),
            ChargeState::Charging
        );
        assert_eq!(
            charge_input(ChargeState::PreCharge, false),
            ChargeState::None
        );
        assert_eq!(
            charge_input(ChargeState::Charging, true),
            ChargeState::Charging
        );
        assert_eq!(
            charge_input(ChargeState::Charging, false),
            ChargeState::Release
        );
        assert_eq!(
            charge_input(ChargeState::Release, true),
            ChargeState::Release
        );
        let mut frames = 58.0;
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut frames, 60.0),
            ChargeState::Charging
        );
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut frames, 60.0),
            ChargeState::Release
        );
        assert_eq!(frames, 60.0);
        let mut over = 60.5;
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut over, 60.0),
            ChargeState::Release
        );
        assert_eq!(over, 60.0);
        assert_eq!(
            charge_tick(ChargeState::Release, &mut over, 60.0),
            ChargeState::Release
        );
    }

    #[test]
    fn charge_damage_scales_only_released_charges() {
        assert_eq!(
            charge_damage(10.0, ChargeState::Charging, 30.0, 60.0, 1.4),
            10.0
        );
        assert_eq!(
            charge_damage(10.0, ChargeState::Release, 30.0, 60.0, 1.4),
            12.0
        );
        assert_eq!(
            charge_damage(10.0, ChargeState::Release, 0.0, 60.0, 1.4),
            10.0
        );
    }
}
