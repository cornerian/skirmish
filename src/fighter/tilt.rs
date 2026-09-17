//! Grounded tilt input predicates from `ftCo_AttackS3.c`, `ftCo_AttackHi3.c`
//! and `ftCo_AttackLw3.c`. The stick angle is `ftCo_GetLStickAngle`
//! (`atan2f(y, |x|)`), supplied by callers; item branches are not modeled.

/// Forward-tilt angle variants selected by `decideAngle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardVariant {
    High,
    HighSlight,
    Straight,
    LowSlight,
    Low,
}

/// `ftCo_AttackS3_CheckInput` after its item branches: a fresh A press with
/// the facing-relative stick at or beyond `x98` and the folded stick angle
/// strictly inside `x20_radians`.
pub fn forward_tilt(
    a_pressed: bool,
    stick_x: f32,
    facing: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_x * facing >= stick_threshold && angle.abs() < angle_limit
}

/// Complete `decideAngle` selection. `thresholds` are `x9C`, `xA0`, `xA4`
/// and `xA8` in radians; `available` says whether the High, HighSlight,
/// LowSlight and Low animations exist (the source tests their figatree
/// entries, which the decomp labels with the motion ids of neighboring
/// states).
pub fn forward_variant(angle: f32, thresholds: [f32; 4], available: [bool; 4]) -> ForwardVariant {
    let [high, high_slight, low_slight, low] = thresholds;
    let [has_high, has_high_slight, has_low_slight, has_low] = available;
    if angle > high && has_high {
        ForwardVariant::High
    } else if angle > high_slight && has_high_slight {
        ForwardVariant::HighSlight
    } else if angle < low && has_low {
        ForwardVariant::Low
    } else if angle < low_slight && has_low_slight {
        ForwardVariant::LowSlight
    } else {
        ForwardVariant::Straight
    }
}

/// `ftCo_AttackHi3_CheckInput` after its item branch.
pub fn up_tilt(
    a_pressed: bool,
    stick_y: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_y >= stick_threshold && angle > angle_limit
}

/// `ftCo_AttackLw3_CheckInput` and its in-action `checkItemThrowInput`
/// after their item branches.
pub fn down_tilt(
    a_pressed: bool,
    stick_y: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_y <= stick_threshold && angle < -angle_limit
}

/// `checkPadA`: a fresh A press re-enters the down tilt once the script has
/// raised its repeat flag, and otherwise arms the `attacklw3.x0` buffer.
/// Returns whether the tilt restarts now.
pub fn down_tilt_repeat(a_pressed: bool, repeat_ready: bool, buffer: &mut bool) -> bool {
    if !a_pressed {
        return false;
    }
    if repeat_ready {
        return true;
    }
    *buffer = true;
    false
}

// Grounded tilts from `ftCo_AttackS3.c`, `ftCo_AttackHi3.c` and
// `ftCo_AttackLw3.c`: forward tilt with its four angle variants, up tilt and
// the down tilt with its buffered repeat. Attacks are supplied physics
// samples with per-frame script flags. Smashes, dash attacks, item branches
// and the Game & Watch down-tilt override are not modeled.
use crate::fighter::aerial::stick_angle;
use crate::game::{
    Action, Controller, Error, Fighter,
    data::{Attack, FighterData},
};
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
    /// `ftCo_Attack11_IASA` / `ftCo_Attack12_IASA`: smashes and tilts, the
    /// rapid and follow-up checks, then jump, dash, squat, turn and walk.
    Jab,
    /// `ftCo_AppealS_IASA`: `ftCo_Wait_IASA`'s specials/catch/attacks/spot-
    /// dodge/shield segment exactly, without its jump/dash/squat/turn/walk
    /// tail.
    Taunt,
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

/// Grounded tilt resources are zero-based, while every ordinary tilt entry
/// calls `ftAnim_8006EBA4` immediately after changing motion. The native
/// action clock therefore reports age one for resource sample zero.
pub(crate) fn sample(action: Action, action_frame: u32) -> u32 {
    if owns_action(action) {
        action_frame.saturating_sub(1)
    } else {
        action_frame
    }
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
        .get(sample(fighter.action, fighter.action_frame) as usize)
        .copied()
}

/// The chain an interruptible tilt exposes on this frame, if any.
pub(crate) fn interrupt_chain(fighter: &Fighter, data: &FighterData) -> Option<Chain> {
    if crate::fighter::smash::interruptible(fighter, data)
        || crate::fighter::dash::interruptible(fighter, data)
        || crate::game::flow::landing::interruptible(fighter, data)
    {
        return Some(Chain::Wait);
    }
    if crate::fighter::edge::owns_action(fighter.action) {
        // ftCo_Ottotto_IASA / ftCo_OttottoWait_IASA: the same chain Wait
        // exposes (catch, smashes, tilts, jab, shield, jump, dash, squat,
        // turn); the walk-only override lives in locomotion's own gate.
        return Some(Chain::Wait);
    }
    if crate::fighter::jab::owns_action(fighter.action) {
        return crate::fighter::jab::interrupt_chain(fighter, data);
    }
    if crate::fighter::taunt::owns_action(fighter.action) {
        return crate::fighter::taunt::interrupt_chain(fighter, data);
    }
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
    crate::game::simulation::enter(fighter, action);
    fighter.tilt = State::default();
}

fn start_registered(fighter: &mut Fighter, data: &FighterData, action: Action) -> bool {
    let slot = match action {
        Action::AttackS3Hi
        | Action::AttackS3HiS
        | Action::AttackS3S
        | Action::AttackS3LwS
        | Action::AttackS3Lw => crate::game::script::move_registry::MoveSlot::Forward,
        Action::AttackHi3 => crate::game::script::move_registry::MoveSlot::Up,
        Action::AttackLw3 => crate::game::script::move_registry::MoveSlot::Down,
        _ => {
            start(fighter, action);
            return true;
        }
    };
    let default_entry = if matches!(slot, crate::game::script::move_registry::MoveSlot::Forward) {
        Action::AttackS3S
    } else {
        action
    };
    match crate::game::script::move_selection::select_native_move_variant(
        fighter,
        data,
        crate::game::script::move_registry::MoveGroup::Tilts,
        slot,
        default_entry,
        action,
    ) {
        crate::game::script::move_selection::NativeMoveSelection::Entered(_)
        | crate::game::script::move_selection::NativeMoveSelection::Unbound(_) => {
            fighter.tilt = State::default();
            true
        }
        crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. } => true,
    }
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
    let pressed = crate::fighter::smash::a_pressed(fighter, input);
    if !pressed {
        return None;
    }
    let angle = stick_angle(input.stick);
    if forward_tilt(
        pressed,
        input.stick[0],
        facing,
        angle,
        rules.forward_stick_threshold,
        rules.angle_limit,
    ) {
        let forward = &parameters.forward;
        return Some(
            match forward_variant(
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
    if up_tilt(
        pressed,
        input.stick[1],
        angle,
        rules.up_stick_threshold,
        rules.angle_limit,
    ) {
        return Some(Action::AttackHi3);
    }
    if down_allowed
        && down_tilt(
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
    (rules, smash_rules): (Option<&Rules>, Option<&crate::fighter::smash::Rules>),
    input: Controller,
) -> bool {
    if !attack_state(fighter, data) {
        return false;
    }
    // Fighter_procInput folds physical Z into the logical A press.
    let pressed = crate::fighter::smash::a_pressed(fighter, input);
    // Turn_IASA evaluates the whole chain with the post-turn facing.
    let facing = if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
        -fighter.facing
    } else {
        fighter.facing
    };
    let chain = interrupt_chain(fighter, data);
    // ftCo_Attack13_IASA runs the rapid check before its Wait chain.
    if fighter.action == Action::Attack13
        && crate::fighter::jab::update_actions(fighter, data, input)
    {
        return true;
    }
    // Smashes precede every tilt in the Wait chain and in the down tilt's
    // interruptible block; up and down smashes keep the chain facing.
    if (fighter.action != Action::AttackLw3 || chain.is_some())
        && let Some((action, facing)) =
            crate::fighter::smash::select(fighter, data, smash_rules, input, facing)
    {
        return crate::fighter::smash::start_registered(fighter, data, action, facing);
    }
    if fighter.action == Action::AttackLw3 {
        // ftCo_AttackLw3_IASA: forward and up tilts, checkPadA, then the down
        // tilt itself and the jab.
        if chain.is_some()
            && let Some(rules) = rules
            && let Some(action) = attacks(fighter, data, rules, input, facing, false)
        {
            return start_registered(fighter, data, action);
        }
        let ready = flags(fighter, data).is_some_and(|flags| flags.repeat_ready);
        if down_tilt_repeat(pressed, ready, &mut fighter.tilt.repeat_buffered) {
            return start_registered(fighter, data, Action::AttackLw3);
        }
        if chain.is_none() {
            return false;
        }
        if let Some(rules) = rules
            && let Some(action) = attacks(fighter, data, rules, input, facing, true)
        {
            return start_registered(fighter, data, action);
        }
    } else if let Some(rules) = rules
        && let Some(action) = attacks(fighter, data, rules, input, facing, true)
    {
        if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
            fighter.facing = -fighter.facing;
        }
        return start_registered(fighter, data, action);
    }
    // The first and second jabs run the rapid and follow-up checks after
    // their attacks and never reach the jab check itself.
    if matches!(fighter.action, Action::Jab | Action::Attack12) {
        return crate::fighter::jab::update_actions(fighter, data, input);
    }
    if pressed {
        if fighter.action == Action::Turn && !fighter.locomotion.turn_has_turned {
            fighter.facing = -fighter.facing;
        }
        return crate::fighter::jab::press(fighter, data);
    }
    crate::fighter::jab::decay(fighter);
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
            .get(sample(fighter.action, fighter.action_frame) as usize)
            .is_some_and(|flags| flags.repeat_ready)
    {
        start(fighter, Action::AttackLw3);
        return Ok(());
    }
    if sample(fighter.action, fighter.action_frame) as usize >= attack.attack.frames.len() {
        let next = if !fighter.grounded {
            Action::Fall
        } else if fighter.action == Action::AttackLw3 {
            Action::SquatWait
        } else {
            Action::Wait
        };
        crate::game::simulation::enter(fighter, next);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::data::Hitbox;

    fn fixture() -> crate::game::data::MatchData {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap()
    }

    fn attack(bones: &[crate::game::data::Bone], repeat: bool) -> GroundAttack {
        GroundAttack {
            attack: Attack {
                move_id: Some(1),
                blend_frames: 0,
                dynamics_variant: 0,
                frames: (0..7)
                    .map(|_| crate::game::data::AttackFrame {
                        bones: bones.to_vec(),
                        hitboxes: vec![],
                        hurtbox_states: vec![],
                    })
                    .collect(),
            },
            flags: (0..7)
                .map(|frame| GroundFrameFlags {
                    allow_interrupt: matches!(frame, 5 | 6),
                    repeat_ready: repeat && matches!(frame, 5 | 6),
                })
                .collect(),
        }
    }

    fn tilt_data() -> crate::game::data::MatchData {
        let mut data = fixture();
        let bones = data.fighters[0].bones.clone();
        data.fighters[0].tilts = Some(Parameters {
            forward: ForwardTilts {
                high: Some(attack(&bones, false)),
                high_slight: Some(attack(&bones, false)),
                straight: attack(&bones, false),
                low_slight: Some(attack(&bones, false)),
                low: Some(attack(&bones, false)),
            },
            up: attack(&bones, false),
            down: attack(&bones, true),
        });
        data
    }

    #[test]
    fn forward_tilt_uses_inclusive_stick_and_strict_angle_bounds() {
        assert!(forward_tilt(true, 0.5, 1.0, 0.49, 0.5, 0.5));
        assert!(forward_tilt(true, -0.5, -1.0, -0.49, 0.5, 0.5));
        assert!(!forward_tilt(true, 0.5, 1.0, 0.5, 0.5, 0.5));
        assert!(!forward_tilt(true, 0.49, 1.0, 0.0, 0.5, 0.5));
        assert!(!forward_tilt(false, 1.0, 1.0, 0.0, 0.5, 0.5));
        assert!(!forward_tilt(true, f32::NAN, 1.0, 0.0, 0.5, 0.5));
    }

    #[test]
    fn forward_variant_prefers_high_then_slight_then_low_and_honors_availability() {
        let thresholds = [0.4, 0.2, -0.2, -0.4];
        let all = [true; 4];
        assert_eq!(forward_variant(0.5, thresholds, all), ForwardVariant::High);
        assert_eq!(
            forward_variant(0.3, thresholds, all),
            ForwardVariant::HighSlight
        );
        assert_eq!(
            forward_variant(0.0, thresholds, all),
            ForwardVariant::Straight
        );
        assert_eq!(
            forward_variant(-0.3, thresholds, all),
            ForwardVariant::LowSlight
        );
        assert_eq!(forward_variant(-0.5, thresholds, all), ForwardVariant::Low);
        assert_eq!(
            forward_variant(0.5, thresholds, [false, true, true, true]),
            ForwardVariant::HighSlight
        );
        assert_eq!(
            forward_variant(-0.5, thresholds, [true, true, false, false]),
            ForwardVariant::Straight
        );
        assert_eq!(
            forward_variant(-0.5, thresholds, [true, true, true, false]),
            ForwardVariant::LowSlight
        );
        assert_eq!(
            forward_variant(f32::NAN, thresholds, all),
            ForwardVariant::Straight
        );
    }

    #[test]
    fn up_and_down_tilts_use_inclusive_stick_and_strict_angle_bounds() {
        assert!(up_tilt(true, 0.5, 0.51, 0.5, 0.5));
        assert!(!up_tilt(true, 0.5, 0.5, 0.5, 0.5));
        assert!(!up_tilt(true, 0.49, 1.0, 0.5, 0.5));
        assert!(down_tilt(true, -0.5, -0.51, -0.5, 0.5));
        assert!(!down_tilt(true, -0.5, -0.5, -0.5, 0.5));
        assert!(!down_tilt(false, -1.0, -1.0, -0.5, 0.5));
    }

    #[test]
    fn down_tilt_repeat_buffers_early_presses() {
        let mut buffer = false;
        assert!(!down_tilt_repeat(false, false, &mut buffer));
        assert!(!buffer);
        assert!(!down_tilt_repeat(true, false, &mut buffer));
        assert!(buffer);
        assert!(down_tilt_repeat(true, true, &mut buffer));
    }

    #[test]
    fn native_tilt_action_age_selects_the_zero_based_authored_frame() {
        for action in [
            Action::AttackS3S,
            Action::AttackS3Hi,
            Action::AttackS3HiS,
            Action::AttackS3LwS,
            Action::AttackS3Lw,
            Action::AttackHi3,
            Action::AttackLw3,
        ] {
            for (action_frame, authored_frame) in [(1, 0), (6, 5), (7, 6)] {
                assert_eq!(
                    sample(action, action_frame),
                    authored_frame,
                    "action frame {action_frame} for {action:?}"
                );
            }
        }
        assert_eq!(sample(Action::Wait, 1), 1);
    }

    #[test]
    fn native_tilt_frame_six_is_not_active_before_action_frame_seven() {
        let mut data = tilt_data();
        let bones = data.fighters[0].bones.clone();
        let mut up = attack(&bones, false);
        up.attack.frames[6].hitboxes.push(Hitbox {
            clank: false,
            rebound: false,
            element: Default::default(),
            group: 0,
            bone: 1,
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            damage: 1,
            shield_damage: 0,
            angle_degrees: 0.0,
            growth: 0,
            fixed: 0,
            base: 0,
        });
        data.fighters[0].tilts.as_mut().unwrap().up = up;
        let attack =
            ground_attack(data.fighters[0].tilts.as_ref().unwrap(), Action::AttackHi3).unwrap();

        assert!(attack.attack.frames[sample(Action::AttackHi3, 6) as usize]
            .hitboxes
            .is_empty());
        assert_eq!(
            attack.attack.frames[sample(Action::AttackHi3, 7) as usize]
                .hitboxes
                .len(),
            1
        );
    }

    #[test]
    fn native_age_drives_tilt_flags_and_action_end_with_the_scheduler_offset() {
        let data = tilt_data();
        let mut state = crate::game::simulation::initial_state(&data, 0, [0, 1]).unwrap();
        let fighter = &mut state.fighters[0];

        for action in [Action::AttackS3S, Action::AttackHi3, Action::AttackLw3] {
            fighter.action = action;
            fighter.action_frame = 6;
            assert!(
                flags(fighter, &data.fighters[0])
                    .expect("native tilt frame 6 flags")
                    .allow_interrupt
            );
            if action == Action::AttackLw3 {
                assert!(
                    flags(fighter, &data.fighters[0])
                        .expect("native down tilt frame 6 flags")
                        .repeat_ready
                );
            }

            fighter.action_frame = 7;
            assert!(
                flags(fighter, &data.fighters[0])
                    .expect("native tilt frame 7 flags")
                    .allow_interrupt
            );
        }

        fighter.action = Action::AttackHi3;
        fighter.action_frame = 8;
        update_animation(fighter, &data.fighters[0]).unwrap();
        assert_eq!(fighter.action, Action::Wait);
    }
}
