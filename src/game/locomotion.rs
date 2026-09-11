//! Ordinary locomotion callbacks from ftCo_{Dash,Run,TurnRun,RunBrake,Turn,Squat,
//! SquatWait,SquatRv,Jump,KneeBend,JumpAerial,Pass}.c and fighter.c input history.
//! Parameters are supplied resources, not character presets. Animation lengths
//! and script events are explicit; animation poses and other character-specific
//! interrupt chains remain absent.
use super::{Action, BUTTON_X, BUTTON_Y, Controller, Error, Fighter, data::FighterData};
use crate::fighter::{Movement, locomotion as math};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub horizontal_smash_deadzone: f32,
    pub vertical_smash_deadzone: f32,
    pub walk_threshold: f32,
    pub dash_threshold: f32,
    pub dash_window: u8,
    pub dash_initial_velocity: f32,
    pub dash_acceleration_mul: f32,
    pub dash_acceleration_base: f32,
    pub dash_max_velocity: f32,
    pub dash_animation_frames: u32,
    pub dash_run_frame: u32,
    pub run_threshold: f32,
    pub run_accel_taper_gain: f32,
    pub run_friction_multiplier: f32,
    pub run_turn_animation_frames: u32,
    pub run_turn_flip_frame: u32,
    pub run_brake_animation_frames: u32,
    pub run_brake_turn_frame: u32,
    pub run_brake_max_frames: f32,
    /// `ftCommonData.x430`: the turn-run lockout (`mv.co.run.x0`) applied by
    /// `fn_800CA644` (`ftCo_TurnRun.c:74`, the turn-run-to-run re-entry) and
    /// counted down every `ftCo_Run_Anim` frame (`ftCo_Run.c:96-98`); while
    /// positive, `ftCo_Run_IASA` (`ftCo_Run.c:125-126`) skips both the
    /// TurnRun and RunBrake entry checks. `None` keeps no lockout (a plain
    /// Dash-to-Run entry never sets it either, `fn_800CA5F0`'s `arg0 = 0.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_turn_lockout_frames: Option<f32>,
    /// The RunBrake animation pose whose script sets `cmd_vars[1]`
    /// (`ftCo_RunBrake.c:53-66`), paired with `run_brake_freeze_speed`
    /// (`ftCommonData.x42C`). Both `None` or both `Some`; `None` keeps no
    /// freeze (this codebase's pre-batch behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_brake_marker_frame: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_brake_freeze_speed: Option<f32>,
    pub turn_threshold: f32,
    pub standing_turn_frames: f32,
    pub turn_animation_frames: u32,
    pub crouch_enter_threshold: f32,
    pub crouch_release_threshold: f32,
    pub crouch_animation_frames: u32,
    pub crouch_reverse_frames: u32,
    pub tap_jump_threshold: f32,
    pub relaxed_tap_jump_threshold: f32,
    pub tap_jump_release_threshold: f32,
    pub tap_jump_window: u8,
    /// `ftCommonData.x78`: the ground/aerial jump direction deadzone. `None`
    /// keeps every jump JumpF/JumpAerialF, matching data that never modeled
    /// the backward variants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_backward_threshold: Option<f32>,
    pub max_jumps: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_jump: Option<MultiJump>,
    pub air_jump_horizontal_multiplier: f32,
    pub air_jump_vertical_multiplier: f32,
    pub air_jump_animation_frames: u32,
    pub pass_stick_threshold: f32,
    pub pass_window: u8,
    pub pass_delay: f32,
    pub pass_velocity: f32,
    pub pass_animation_frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiJump {
    pub turn_frames: u32,
    pub backward_turn_threshold: f32,
    pub horizontal_velocity: f32,
    pub air_drift_threshold: f32,
    pub air_drift_acceleration_multiplier: f32,
    pub air_drift_max_velocity_multiplier: f32,
    pub vertical_velocities: [f32; 5],
    pub animation_frames: [u32; 5],
    pub repeat_input_frames: [u32; 5],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpInput {
    #[default]
    Buttons,
    Stick,
    CStick,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub tilt_x_age: u8,
    pub tilt_y_age: u8,
    /// Fighter x67F: age of a rising logical shoulder input, including analog.
    pub trigger_age: u8,
    /// Fighter x680/x684: current and previous physical L/R press ages.
    pub tech_press_age: u8,
    pub previous_tech_press_age: u8,
    /// Fighter x67E: age of the most recent fresh physical X/Y press.
    pub jump_press_age: u8,
    /// Fighter x67C/x67D: separate ages of fresh physical A/B presses.
    pub attack_a_age: u8,
    pub attack_b_age: u8,
    /// Fighter x688 (`fighter.c:1735-1739`): age of a fresh B press with the
    /// stick past the side-special threshold (`ftCo_SpecialS_HasInput`).
    /// Distinct from `attack_b_age`/x67D, which has no stick condition.
    pub side_special_b_age: u8,
    pub jumps_used: u8,
    pub jump_input: JumpInput,
    pub turn_frames: f32,
    pub turn_has_turned: bool,
    pub turn_smash: bool,
    pub dash_initial_delta: f32,
    /// `mv.co.dash.x4`: true when this Dash was entered by `try_dash`/a
    /// late re-dash, false when Turn's smash completion entered it.
    pub dash_from_input: bool,
    /// Facing captured by `ftCo_TurnRun_Enter` for its physics branch.
    pub run_turn_facing: f32,
    /// The animation event fired and is waiting for velocity to cross x0.01.
    pub run_turn_waiting: bool,
    pub run_brake_frames: f32,
    /// `mv.co.run.x0` (`ftCo_Run.c:72,96-98`): the turn-run lockout
    /// countdown, set by `enter_run` from a TurnRun-to-Run re-entry (else
    /// left at 0.0 for an ordinary Dash-to-Run entry) and counted down by
    /// 1.0 per Run animation frame while positive. Gates RunTurn/RunBrake
    /// entry from Run (`ftCo_Run_IASA`, `ftCo_Run.c:125-126`).
    pub run_lockout: f32,
    /// `mv.co.runbrake.x0` (`ftCo_RunBrake.c:55-65`): true while the
    /// velocity-gated marker freeze (`run_brake_marker_frame`/
    /// `run_brake_freeze_speed`) currently holds `action_frame` at rate 0,
    /// cleared once ground velocity crosses back under the freeze speed.
    pub run_brake_frozen: bool,
    pub pass_delay: Option<f32>,
    /// Root-joint turn state from `ft_800CB6EC`; yaw affects bone physics.
    pub multi_jump_turn_remaining: i32,
    pub multi_jump_yaw: f32,
    /// Distinguishes JumpB/JumpAerialB from JumpF/JumpAerialF for Slippi's
    /// reported motion id; the source keeps no such field on `Fighter`, since
    /// it stores the chosen state id directly. Set by the ground/aerial jump
    /// launch, cleared by every other `simulation::enter`.
    pub jump_backward: bool,
    /// Distinguishes the aerial-jump variant of Fall (`ftCo_FallAerial_Enter`)
    /// from an ordinary Fall entry. Set only when `JumpAerial`'s own
    /// animation end enters Fall, cleared by every other `simulation::enter`.
    pub fall_aerial: bool,
    /// `mv.co.walk`: the current walk animation kind and float animation
    /// frame, tracked only while `MovementData.walk_animation` is supplied
    /// (kept at its default otherwise, which reports Slippi 15/animation 7
    /// unconditionally, matching the pre-batch behavior).
    pub walk: WalkState,
    /// `mv.co.run`'s float animation frame, tracked only while
    /// `MovementData.run_animation` is supplied. See `RunState`.
    pub run: RunState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tilt_x_age: 254,
            tilt_y_age: 254,
            trigger_age: 255,
            tech_press_age: 255,
            previous_tech_press_age: 255,
            jump_press_age: 255,
            attack_a_age: 255,
            attack_b_age: 255,
            side_special_b_age: 255,
            jumps_used: 0,
            jump_input: JumpInput::Buttons,
            turn_frames: 0.0,
            turn_has_turned: false,
            turn_smash: false,
            dash_initial_delta: 0.0,
            dash_from_input: true,
            run_turn_facing: 0.0,
            run_turn_waiting: false,
            run_brake_frames: 0.0,
            run_lockout: 0.0,
            run_brake_frozen: false,
            pass_delay: None,
            multi_jump_turn_remaining: 0,
            multi_jump_yaw: 0.0,
            jump_backward: false,
            fall_aerial: false,
            walk: WalkState::default(),
            run: RunState::default(),
        }
    }
}

/// `fp->x2DC/x2E0/x2E4` (`Fighter_Create_Inline2`, `fighter.c:838-840`): the
/// WalkSlow/WalkMiddle/WalkFast figatrees' frame counts, and
/// `slow_walk_max`/`mid_walk_point`/`fast_walk_min` (`types.h:690-692`):
/// the matching per-kind animation-rate divisors. Paired with `Rules.walk`;
/// absent keeps the pre-batch single Walk with an integer `action_frame`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkAnimation {
    pub lengths: [f32; 3],
    pub rates: [f32; 3],
}

/// `walk_middle_animation_stick_threshold`/`walk_fast_stick_threshold`
/// (`types.h:64-65`): fractions of `walk_max_velocity` that select the
/// Middle/Fast walk kind. Paired with `MovementData.walk_animation`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkRules {
    pub middle_threshold: f32,
    pub fast_threshold: f32,
}

pub fn validate_walk(animation: &WalkAnimation, rules: &WalkRules) -> Result<(), Error> {
    let valid = animation
        .lengths
        .iter()
        .all(|x| x.is_finite() && *x > 0.0 && *x <= 1_000_000.0)
        && animation
            .rates
            .iter()
            .all(|x| x.is_finite() && *x > 0.0 && *x <= 1_000_000.0)
        && rules.middle_threshold.is_finite()
        && rules.fast_threshold.is_finite()
        && (0.0..=1_000_000.0).contains(&rules.middle_threshold)
        && (0.0..=1_000_000.0).contains(&rules.fast_threshold)
        && rules.middle_threshold <= rules.fast_threshold;
    if valid {
        Ok(())
    } else {
        Err(Error::Data(
            "invalid walk animation lengths/rates or middle/fast thresholds".into(),
        ))
    }
}

pub use math::WalkKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkState {
    pub kind: WalkKind,
    pub frame: f32,
    /// `ftAnim_SetAnimRate` takes effect on the *next* animation update:
    /// this is the rate computed on the previous Walk animation frame,
    /// applied to advance `frame` on this one.
    pub last_rate: f32,
}

/// The Run figatree's frame count and `run_animation_scaling`
/// (`types.h:698`, `co_attrs+0x2C`). Unlike Walk's three cached figatree
/// lengths (`fp->x2DC/x2E0/x2E4`, `fighter.c:838-840`), the source has no
/// dedicated `Fighter` field caching the single Run figatree's length --
/// `Fighter_Create_Inline2` (`fighter.c:829-841`) only caches sub-motions
/// 7/8/9/0x23/0x25 (Walk kinds, Landing, GuardOn), none of which is Run --
/// so `length` here is supplied resource data standing in for a runtime
/// animation-length query, the same convention this codebase already uses
/// for other per-motion frame counts (`dash_animation_frames`,
/// `run_brake_animation_frames`, ...). Absent keeps the pre-batch integer
/// `action_frame` and rate 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAnimation {
    pub length: f32,
    pub scaling: f32,
}

pub fn validate_run(animation: &RunAnimation) -> Result<(), Error> {
    let valid = animation.length.is_finite()
        && animation.length > 0.0
        && animation.length <= 1_000_000.0
        && animation.scaling.is_finite()
        && animation.scaling > 0.0
        && animation.scaling <= 1_000_000.0;
    if valid {
        Ok(())
    } else {
        Err(Error::Data(
            "invalid run animation length or scaling".into(),
        ))
    }
}

/// `mv.co.run`'s float animation-frame bookkeeping, tracked only while
/// `MovementData.run_animation` is supplied (kept at its default otherwise,
/// which reports Slippi state 21/animation 13 unconditionally with an
/// integer `action_frame`, matching the pre-batch behavior). `run.x0` (the
/// turn-run lockout countdown, `ftCo_Run.c:72,96-98`) and `run.x4` (the
/// low-friction-stage velocity, `ftCo_Run.c:73,83-87`, dead in this
/// codebase -- see `run_animation_rate`) are not part of this state: no
/// existing Rust code models the `run.x0` IASA lockout gate
/// (`ftCo_Run_IASA`, `ftCo_Run.c:125-126`) either, only `run.x0`'s
/// *absence* of effect (RunTurn is reachable from Run unconditionally in
/// `game::locomotion::update_actions`'s `Action::Run` arm); this batch's
/// resource/state shape does not add it, so it is reported as a pre-existing
/// gap rather than silently introduced here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunState {
    pub frame: f32,
    /// `ftAnim_SetAnimRate` takes effect on the *next* animation update:
    /// this is the rate computed on the previous Run animation frame,
    /// applied to advance `frame` on this one.
    pub last_rate: f32,
}

pub fn validate(p: &Parameters) -> Result<(), Error> {
    let thresholds = [
        p.horizontal_smash_deadzone,
        p.vertical_smash_deadzone,
        p.walk_threshold,
        p.dash_threshold,
        p.run_threshold,
        p.crouch_enter_threshold,
        p.crouch_release_threshold,
        p.tap_jump_threshold,
        p.relaxed_tap_jump_threshold,
        p.tap_jump_release_threshold,
        p.pass_stick_threshold,
    ];
    let nonnegative = [
        p.dash_initial_velocity,
        p.dash_acceleration_mul,
        p.dash_acceleration_base,
        p.dash_max_velocity,
        p.run_accel_taper_gain,
        p.run_friction_multiplier,
        p.run_brake_max_frames,
        p.standing_turn_frames,
        p.air_jump_horizontal_multiplier,
        p.air_jump_vertical_multiplier,
        p.pass_delay,
    ];
    let durations = [
        p.dash_animation_frames,
        p.run_turn_animation_frames,
        p.run_brake_animation_frames,
        p.turn_animation_frames,
        p.crouch_animation_frames,
        p.crouch_reverse_frames,
        p.air_jump_animation_frames,
        p.pass_animation_frames,
    ];
    if !thresholds.into_iter().all(|x| x > 0.0 && x <= 1.0)
        || !(-1.0..0.0).contains(&p.turn_threshold)
        || !nonnegative
            .into_iter()
            .all(|x| (0.0..=1_000_000.0).contains(&x))
        || !(-1_000_000.0..=0.0).contains(&p.pass_velocity)
        || !durations.into_iter().all(|n| (1..=1_000_000).contains(&n))
        || p.dash_run_frame >= p.dash_animation_frames
        || p.run_turn_flip_frame >= p.run_turn_animation_frames
        || p.run_brake_turn_frame >= p.run_brake_animation_frames
        || p.run_turn_lockout_frames
            .is_some_and(|frames| !frames.is_finite() || !(0.0..=1_000_000.0).contains(&frames))
        || p.run_brake_marker_frame.is_some() != p.run_brake_freeze_speed.is_some()
        || p.run_brake_marker_frame
            .is_some_and(|frame| frame >= p.run_brake_animation_frames)
        || p.run_brake_freeze_speed
            .is_some_and(|speed| !speed.is_finite() || !(0.0..=1_000_000.0).contains(&speed))
        || ![p.dash_window, p.tap_jump_window, p.pass_window]
            .into_iter()
            .all(|n| (1..254).contains(&n))
        || p.pass_delay.fract() != 0.0
        || p.crouch_release_threshold > p.crouch_enter_threshold
        || p.jump_backward_threshold
            .is_some_and(|threshold| !threshold.is_finite() || threshold < 0.0)
    {
        return Err(Error::Data("invalid ordinary locomotion parameters".into()));
    }
    match &p.multi_jump {
        None if !(1..=2).contains(&p.max_jumps) => {
            return Err(Error::Data(
                "more than two jumps require multijump parameters".into(),
            ));
        }
        Some(m)
            if !(3..=6).contains(&p.max_jumps)
                || m.turn_frames == 0
                || m.turn_frames > i32::MAX as u32
                || !(0.0..=1.0).contains(&m.backward_turn_threshold)
                || !(0.0..=1.0).contains(&m.air_drift_threshold)
                || [
                    m.horizontal_velocity,
                    m.air_drift_acceleration_multiplier,
                    m.air_drift_max_velocity_multiplier,
                ]
                .into_iter()
                .any(|x| !(0.0..=1_000_000.0).contains(&x))
                || m.vertical_velocities
                    .iter()
                    .any(|x| !x.is_finite() || *x <= 0.0 || *x > 1_000_000.0)
                || m.animation_frames.iter().any(|n| *n == 0 || *n > 1_000_000)
                || m.repeat_input_frames
                    .iter()
                    .zip(&m.animation_frames)
                    .any(|(repeat, length)| repeat >= length) =>
        {
            return Err(Error::Data("invalid multijump parameters".into()));
        }
        Some(_) => {}
        None => {}
    }
    Ok(())
}

pub fn landed(f: &mut Fighter) {
    f.locomotion.jumps_used = 0;
    f.locomotion.pass_delay = None;
    f.locomotion.multi_jump_turn_remaining = 0;
    f.locomotion.multi_jump_yaw = 0.0;
}

pub(crate) fn enter(f: &mut Fighter, action: Action) {
    if !matches!(action, Action::Squat | Action::SquatWait) {
        f.locomotion.pass_delay = None;
    }
    if action != Action::JumpAerial {
        f.locomotion.multi_jump_turn_remaining = 0;
        f.locomotion.multi_jump_yaw = 0.0;
    }
    super::simulation::enter(f, action);
}

fn advance_multi_jump_turn(f: &mut Fighter, total: u32) {
    math::multi_jump_turn(
        &mut f.locomotion.multi_jump_turn_remaining,
        &mut f.facing,
        &mut f.locomotion.multi_jump_yaw,
        total as i32,
    );
}

pub(crate) fn jump_input(
    f: &Fighter,
    p: &Parameters,
    input: Controller,
    relaxed: bool,
) -> Option<JumpInput> {
    let threshold = if relaxed {
        p.relaxed_tap_jump_threshold
    } else {
        p.tap_jump_threshold
    };
    if input.stick[1] >= threshold && f.locomotion.tilt_y_age < p.tap_jump_window {
        Some(JumpInput::Stick)
    } else if input.buttons & !f.previous_input.buttons & (BUTTON_X | BUTTON_Y) != 0 {
        Some(JumpInput::Buttons)
    } else {
        None
    }
}

pub(crate) fn shield_jump_input(
    f: &Fighter,
    p: &Parameters,
    input: Controller,
) -> Option<JumpInput> {
    jump_input(f, p, input, false).or_else(|| {
        math::cstick_jump(input.cstick[1], p.tap_jump_threshold).then_some(JumpInput::CStick)
    })
}

fn start_dash(f: &mut Fighter, p: &Parameters, from_input: bool) {
    enter(f, Action::Dash);
    f.locomotion.tilt_x_age = 254;
    let initial = f.facing * p.dash_initial_velocity;
    f.locomotion.dash_initial_delta = if f.ground_velocity * f.facing < 0.0 {
        initial
    } else {
        initial - f.ground_velocity
    };
    f.locomotion.dash_from_input = from_input;
}

fn start_turn(f: &mut Fighter, p: &Parameters, smash: bool) {
    enter(f, Action::Turn);
    f.locomotion.turn_frames = if smash { 0.0 } else { p.standing_turn_frames };
    f.locomotion.turn_has_turned = false;
    f.locomotion.turn_smash = smash;
}

pub(crate) fn start_run_turn(f: &mut Fighter, frame: u32) {
    let facing = f.facing;
    enter(f, Action::RunTurn);
    f.action_frame = frame;
    f.locomotion.run_turn_facing = facing;
    f.locomotion.run_turn_waiting = false;
    f.locomotion.turn_has_turned = false;
}

/// `ftCo_Dash_CheckInput`: a fresh dash magnitude inside the shared window
/// restarts Dash in the same direction (`dash_from_input = true`) or enters
/// a smash Turn in the opposite one. Reused by Dash's own late-phase re-dash
/// and, guarded by the caller's direction check, its middle-phase dash-back.
pub(crate) fn try_dash(f: &mut Fighter, p: &Parameters, input: Controller) -> bool {
    if input.stick[0].abs() < p.dash_threshold || f.locomotion.tilt_x_age >= p.dash_window {
        return false;
    }
    if input.stick[0] * f.facing < 0.0 {
        start_turn(f, p, true);
    } else {
        start_dash(f, p, true);
    }
    true
}

fn ground_jump(f: &mut Fighter, data: &FighterData, p: &Parameters, input: Controller) {
    let a = &data.movement;
    let v = math::jump_velocity(
        [f.velocity[0], f.velocity[1], 0.0],
        input.stick[0],
        f.short_hop,
        1.0,
        &math::JumpAttributes {
            momentum_multiplier: a.jump_momentum_multiplier,
            horizontal_initial_velocity: a.jump_horizontal_velocity,
            horizontal_max_velocity: a.jump_horizontal_max,
            full_hop_velocity: a.jump_vertical_velocity,
            short_hop_velocity: a.short_hop_vertical_velocity,
        },
    );
    f.velocity = [v[0], v[1]];
    f.ground_velocity = 0.0;
    f.grounded = false;
    f.ground_line = None;
    f.fast_fall = false;
    f.ecb_lock = 10;
    f.ecb.bottom_locked = true;
    f.locomotion.jumps_used = 1;
    f.locomotion.tilt_y_age = 254;
    // ftCo_Jump.c:157-161: the direction test runs at the launch frame's
    // current stick, before the motion change.
    let backward = p
        .jump_backward_threshold
        .is_some_and(|threshold| math::jump_backward(input.stick[0], f.facing, threshold));
    enter(f, Action::Jump);
    f.locomotion.jump_backward = backward;
}

pub(crate) fn update_animation(f: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let Some(p) = data.locomotion.as_ref() else {
        return false;
    };
    if !f.grounded && f.locomotion.jumps_used == 0 {
        f.locomotion.jumps_used = 1;
    }

    let mut just_turned = false;
    // Anim callbacks precede IASA. A release on the launch frame is too late to
    // change a ground jump's stored short-hop choice.
    match f.action {
        Action::JumpSquat if f.action_frame >= data.movement.jump_startup_frames => {
            ground_jump(f, data, p, input)
        }
        Action::Dash if f.action_frame >= p.dash_animation_frames => enter(f, Action::Wait),
        // ftCo_RunBrake_Anim (ftCo_RunBrake.c:49-77): while the script's
        // cmd_vars[1] marker is set (modeled as action_frame >=
        // run_brake_marker_frame, the same level-condition convention
        // RunTurn's own marker already uses) and the freeze has not yet
        // happened, freeze (rate 0, held via hold_action_frame) once
        // |gr_vel| >= run_brake_freeze_speed; once frozen, resume (rate 1)
        // once |gr_vel| <= run_brake_freeze_speed. The frames countdown
        // continues regardless and ends the brake into Wait on either the
        // countdown or the animation running out, independent of the
        // freeze. Ground velocity only ever loses magnitude under
        // RunBrake's own friction (ftCo_RunBrake_Phys/game::locomotion::
        // ground_motion), so a single "currently frozen" bit -- reset false
        // at RunBrake entry, mirroring runbrake.x0's own false at Enter --
        // cannot be re-armed by a later RunBrake frame in this codebase.
        Action::RunBrake => {
            if let (Some(marker), Some(freeze_speed)) =
                (p.run_brake_marker_frame, p.run_brake_freeze_speed)
                && f.action_frame >= marker
            {
                if !f.locomotion.run_brake_frozen {
                    if f.ground_velocity.abs() >= freeze_speed {
                        f.locomotion.run_brake_frozen = true;
                    }
                } else if f.ground_velocity.abs() <= freeze_speed {
                    f.locomotion.run_brake_frozen = false;
                }
            }
            if f.locomotion.run_brake_frames != 0.0 {
                f.locomotion.run_brake_frames = (f.locomotion.run_brake_frames - 1.0).max(0.0);
            }
            if f.action_frame >= p.run_brake_animation_frames
                || f.locomotion.run_brake_frames == 0.0
            {
                enter(f, Action::Wait);
            }
        }
        // ftCo_TurnRun_Anim (ftCo_TurnRun.c:57-77): while its own marker is
        // set, first freeze (rate 0), then resume at rate 1 and flip facing
        // once facing_at_entry * gr_vel <= 0.01 -- the per-entry facing
        // ftCo_TurnRun_Enter stored at entry (turnrun.accel_mul, read back
        // through the middle_anim_frame union alias), already captured here
        // as run_turn_facing, not a fixed per-match resource constant.
        Action::RunTurn => {
            if !f.locomotion.turn_has_turned && f.action_frame >= p.run_turn_flip_frame {
                f.locomotion.run_turn_waiting = true;
                if f.locomotion.run_turn_facing * f.ground_velocity <= 0.01 {
                    f.locomotion.run_turn_waiting = false;
                    f.locomotion.turn_has_turned = true;
                    f.facing = -f.facing;
                }
            }
            if f.locomotion.turn_has_turned && f.action_frame >= p.run_turn_animation_frames {
                if input.stick[0] * f.facing >= p.run_threshold {
                    enter_run(f, p.run_turn_lockout_frames.unwrap_or(0.0));
                } else {
                    enter(f, Action::Wait);
                }
            }
        }
        Action::Turn => {
            if f.locomotion.turn_frames > 0.0 {
                f.locomotion.turn_frames -= 1.0;
            } else if !f.locomotion.turn_has_turned {
                f.locomotion.turn_has_turned = true;
                f.facing = -f.facing;
                just_turned = true;
            }
            if f.action == Action::Turn && f.action_frame >= p.turn_animation_frames {
                enter(f, Action::Wait);
            }
        }
        Action::Squat if f.action_frame >= p.crouch_animation_frames => enter(f, Action::SquatWait),
        Action::SquatRv if f.action_frame >= p.crouch_reverse_frames => enter(f, Action::Wait),
        Action::JumpAerial => {
            if let Some(multi) = &p.multi_jump {
                advance_multi_jump_turn(f, multi.turn_frames);
                let index = usize::from(f.locomotion.jumps_used.saturating_sub(2));
                if multi
                    .animation_frames
                    .get(index)
                    .is_some_and(|length| f.action_frame >= *length)
                {
                    // ftCo_JumpAerial_Anim (ftCo_JumpAerial.c:272-277) ends
                    // into ftCo_FallAerial_Enter, not the ordinary Fall.
                    enter(f, Action::Fall);
                    f.locomotion.fall_aerial = true;
                }
            } else if f.action_frame >= p.air_jump_animation_frames {
                enter(f, Action::Fall);
                f.locomotion.fall_aerial = true;
            }
        }
        Action::Pass if f.action_frame >= p.pass_animation_frames => enter(f, Action::Fall),
        // ftCo_Walk_Anim (ftWalkCommon_800DFDDC): advance by the rate
        // computed on the previous frame (SetAnimRate's one-frame delay),
        // wrap at the current kind's figatree length, then store this
        // frame's rate for the next call.
        Action::Walk => {
            if let Some(animation) = &data.movement.walk_animation {
                advance_walk_animation(f, animation);
            }
        }
        // ftCo_Run_Anim (ftCo_Run.c:76-99): advance by the rate computed on
        // the previous frame (SetAnimRate's one-frame delay), wrap at the
        // Run figatree's length, then store this frame's rate for the next
        // call. Same wrap rule as Walk's own animation phase.
        Action::Run => {
            // ftCo_Run.c:96-98: run.x0 counts down by 1.0 per frame while
            // positive (never otherwise clamped), independent of whether
            // the float animation-frame resource below is supplied.
            if f.locomotion.run_lockout > 0.0 {
                f.locomotion.run_lockout -= 1.0;
            }
            if let Some(animation) = &data.movement.run_animation {
                advance_run_animation(f, animation);
            }
        }
        _ => {}
    }
    just_turned
}

/// `ftCo_Run_Enter_Full` (`ftCo_Run.c:66-74`) with `anim_start = 0.0`, the
/// value both of this codebase's reachable Run entries use: Dash's own
/// `ftCo_Dash_IASA` transition (`fn_800CA5F0`, arg0 = 0.0) and RunTurn's own
/// Anim-phase re-entry (`fn_800CA644`, arg0 = `x430`) both call
/// `ftCo_Run_Enter`, which always supplies `anim_start = 0.0F`,
/// `anim_speed = 1.0F` to `Enter_Full` -- only `fn_800CA698`
/// (`ftCo_RunDirect.c`, a distinct, unreached motion state in this
/// codebase) calls `Enter_Full` directly with `fp->cur_anim_frame`/
/// `fp->frame_speed_mul`, and is not modeled. `last_rate` is initialized to
/// 1.0 (`Fighter_ChangeMotionState`'s own `rate = 1` argument), applied on
/// the first Run animation update, exactly as the walk batch modeled
/// Walk's own entry rate.
/// `lockout` is `mv.co.run.x0` (`ftCo_Run.c:72`): `fn_800CA5F0` (Dash's own
/// IASA transition) always supplies `0.0`; `fn_800CA644` (RunTurn's own
/// Anim-phase re-entry once its flip has finished) supplies
/// `Parameters::run_turn_lockout_frames` (`ftCommonData.x430`), or `0.0`
/// when that resource is absent (keeping this codebase's pre-batch
/// behavior of no lockout).
pub(crate) fn enter_run(f: &mut Fighter, lockout: f32) {
    enter(f, Action::Run);
    f.locomotion.run = RunState {
        frame: 0.0,
        last_rate: 1.0,
    };
    f.locomotion.run_lockout = lockout;
}

fn advance_run_animation(f: &mut Fighter, animation: &RunAnimation) {
    f.locomotion.run.frame += f.locomotion.run.last_rate;
    while f.locomotion.run.frame >= animation.length {
        f.locomotion.run.frame -= animation.length;
    }
    // x4/friction_multiplier: this codebase's own caller always treats the
    // stage friction multiplier as 1 (unmodeled), which always selects the
    // ground_velocity branch; x4 is unused here (see run_animation_rate).
    f.locomotion.run.last_rate =
        math::run_animation_rate(f.ground_velocity, f.facing, 0.0, animation.scaling, 1.0);
}

fn advance_walk_animation(f: &mut Fighter, animation: &WalkAnimation) {
    let length = animation.lengths[f.locomotion.walk.kind as usize];
    f.locomotion.walk.frame += f.locomotion.walk.last_rate;
    while f.locomotion.walk.frame >= length {
        f.locomotion.walk.frame -= length;
    }
    // x0/friction_multiplier: this codebase's own caller always treats the
    // stage friction multiplier as 1 (unmodeled), which always selects the
    // `ground_velocity` branch; x0 is unused here (see `walk_animation_rate`).
    f.locomotion.walk.last_rate = math::walk_animation_rate(
        f.ground_velocity,
        f.facing,
        f.locomotion.walk.kind,
        animation.rates,
        0.0,
        1.0,
    );
}

/// `ftCo_Walk_Enter`/`ftWalkCommon_800DFCA4` with `accel_mul = 1`: compute
/// the walk kind from `|ground_velocity|` and (re-)enter Walk at
/// `start_frame`. Used both by a fresh Wait/tilt -> Walk transition
/// (`start_frame = 0`) and by `retype_walk`'s mid-walk re-entry.
fn enter_walk(
    f: &mut Fighter,
    data: &FighterData,
    walk_rules: Option<&WalkRules>,
    start_frame: f32,
) {
    let kind = match (data.movement.walk_animation.as_ref(), walk_rules) {
        (Some(_), Some(rules)) => math::walk_kind(
            f.ground_velocity,
            1.0,
            rules.middle_threshold,
            rules.fast_threshold,
            data.movement.walk_max_velocity,
        ),
        _ => WalkKind::default(),
    };
    enter(f, Action::Walk);
    f.locomotion.walk = WalkState {
        kind,
        frame: start_frame,
        last_rate: 1.0,
    };
}

/// `ftWalkCommon_800DFEC8`, called at the end of every Walk IASA once the
/// earlier chain (catch/specials/smashes/tilts/jab/shield/taunt/jump/dash/
/// squat/the Wait-exit check) has not already returned: recompute the walk
/// kind from the current `ground_velocity`; if it differs from the stored
/// kind, re-enter Walk (a real `ChangeMotionState`, so `action_instance.id`
/// is unaffected -- Walk/Dash share motion identity 102) with the source's
/// truncating start-frame remap.
fn retype_walk(f: &mut Fighter, data: &FighterData, animation: &WalkAnimation, rules: &WalkRules) {
    let new_kind = math::walk_kind(
        f.ground_velocity,
        1.0,
        rules.middle_threshold,
        rules.fast_threshold,
        data.movement.walk_max_velocity,
    );
    if new_kind == f.locomotion.walk.kind {
        return;
    }
    let current_length = animation.lengths[f.locomotion.walk.kind as usize];
    let new_length = animation.lengths[new_kind as usize];
    let start_frame =
        math::walk_retype_frame(f.locomotion.walk.frame, current_length, new_length) as f32;
    // ground_velocity is unchanged within this frame, so enter_walk's own
    // fresh GetWalkType call (matching ftCo_Walk_Enter's real re-entry)
    // reproduces new_kind exactly.
    enter_walk(f, data, Some(rules), start_frame);
}

pub(crate) fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    attack_rules: (Option<&super::tilt::Rules>, Option<&super::smash::Rules>),
    edge_rules: Option<&super::edge::Rules>,
    walk_rules: Option<&WalkRules>,
    input: Controller,
    just_turned: bool,
) {
    let Some(p) = data.locomotion.as_ref() else {
        return;
    };
    let chain = super::tilt::interrupt_chain(f, data);
    let interruptible_tilt = chain.is_some();
    // ftCo_AppealS_IASA has no jump/dash/squat/turn/walk checks at all
    // (see taunt.rs); it still reaches the shared attack dispatch below.
    let taunt_chain = chain == Some(super::tilt::Chain::Taunt);
    let grounded_action = f.grounded
        && (matches!(
            f.action,
            Action::Wait
                | Action::Walk
                | Action::Dash
                | Action::Run
                | Action::RunTurn
                | Action::RunBrake
                | Action::Turn
                | Action::Squat
                | Action::SquatWait
                | Action::SquatRv
        ) || interruptible_tilt);
    if grounded_action {
        if super::tilt::update_ground_attacks(f, data, attack_rules, input) {
            return;
        }
        if !taunt_chain
            && let Some(source) = jump_input(
                f,
                p,
                input,
                matches!(
                    f.action,
                    Action::Dash | Action::Run | Action::RunTurn | Action::RunBrake
                ),
            )
        {
            f.short_hop = false;
            f.locomotion.jump_input = source;
            enter(f, Action::JumpSquat);
            // ftCo_Wait_IASA (and every other Wait-chain IASA) is a
            // RETURN_IF chain: `RETURN_IF(ftCo_Jump_CheckInput(gobj))`
            // returns immediately once the jump-squat entry fires, before
            // ftCo_Dash_CheckInput/ftCo_800D5FB0/ftCo_Turn_CheckInput/the
            // walk check ever run. Without this return, the second match
            // below still matches the shared Wait/Walk arm's guard (its
            // `interruptible_tilt` was computed before this entry, so it
            // does not know `f.action` just changed) and can immediately
            // overwrite the fresh JumpSquat with Dash/Squat/Turn/Walk.
            return;
        }
    } else if matches!(
        f.action,
        Action::Jump | Action::Fall | Action::JumpAerial | Action::Pass
    ) {
        try_aerial_jump(f, data, input);
    }
    match f.action {
        // Interruptible tilts hand their frame to the same dash, squat, turn
        // and walk checks as Wait; taunt's own chain has none of these.
        action
            if (matches!(action, Action::Wait | Action::Walk) || interruptible_tilt)
                && !taunt_chain =>
        {
            if try_dash(f, p, input) {
                return;
            }
            // ftCo_Landing.c:146-147: unlike Wait and Walk, an interruptible
            // Landing only opens the squat entry on the single frame
            // `cur_anim_frame < frame_speed_mul + normal_landing_lag`; past
            // it the chain still reaches Turn and Walk below.
            let landing_squat_ready =
                f.action != Action::Landing || super::landing::squat_window(f, data);
            if landing_squat_ready && input.stick[1] < -p.crouch_enter_threshold {
                f.locomotion.pass_delay = None;
                enter(f, Action::Squat);
            } else if input.stick[0] * f.facing <= p.turn_threshold {
                start_turn(f, p, false);
            } else if input.stick[0] * f.facing >= p.walk_threshold
                // ftCo_Walk_CheckInput_Ottotto: Ottotto/OttottoWait ANDs the
                // ordinary walk predicate with this extra facing-relative
                // stick gate; every other Wait-chain state has none.
                && (!super::edge::owns_action(f.action)
                    || edge_rules.is_some_and(|rules| {
                        crate::fighter::edge::teeter_walk_allowed(
                            input.stick[0],
                            f.facing,
                            rules.teeter_walk_threshold,
                        )
                    }))
            {
                if f.action != Action::Walk {
                    enter_walk(f, data, walk_rules, 0.0);
                } else if let (Some(animation), Some(rules)) =
                    (data.movement.walk_animation.as_ref(), walk_rules)
                {
                    retype_walk(f, data, animation, rules);
                }
            } else if f.action == Action::Walk {
                enter(f, Action::Wait);
            }
        }
        Action::Dash
            if f.action_frame >= p.dash_run_frame
                && input.stick[0] * f.facing >= p.run_threshold =>
        {
            enter_run(f, 0.0)
        }
        // ftCo_Run_IASA (ftCo_Run.c:125-126): while run.x0 > 0.0 (freshly
        // set by a RunTurn-to-Run re-entry), the whole rest of the IASA
        // chain -- including this RunTurn/RunBrake entry check -- is
        // skipped.
        Action::Run if f.locomotion.run_lockout <= 0.0 => {
            if input.stick[0] * f.facing <= p.turn_threshold {
                start_run_turn(f, 0);
            } else if input.stick[0].abs() < p.run_threshold {
                enter(f, Action::RunBrake);
                f.locomotion.run_brake_frames = p.run_brake_max_frames;
                f.locomotion.run_brake_frozen = false;
            }
        }
        Action::RunBrake => {
            if f.action_frame >= p.run_brake_turn_frame
                && input.stick[0] * f.facing <= p.turn_threshold
            {
                start_run_turn(f, f.action_frame);
            } else if input.stick[1] < -p.crouch_enter_threshold {
                enter(f, Action::Squat);
            }
        }
        Action::Turn => {
            let facing_after = if f.locomotion.turn_has_turned {
                f.facing
            } else {
                -f.facing
            };
            if input.stick[0] * facing_after >= p.dash_threshold
                && f.locomotion.tilt_x_age < p.dash_window
            {
                f.locomotion.turn_smash = true;
            }
            if just_turned
                && f.locomotion.turn_smash
                && input.stick[0] * facing_after >= p.dash_threshold
            {
                // ftCo_Turn.c:133: this Dash is entered from Turn's own smash
                // completion, not ftCo_Dash_CheckInput, so dash.x4 = 0.
                start_dash(f, p, false);
            }
        }
        Action::SquatWait => {
            if try_dash(f, p, input) {
                return;
            }
            if input.stick[1] > -p.crouch_release_threshold {
                enter(f, Action::SquatRv);
            }
        }
        Action::JumpSquat => {
            // ftCo_KneeBend_IASA: the windowless up smash precedes the
            // short-hop sample (the catch check ran earlier in the frame).
            if super::smash::jump_squat_up_smash(f, data, attack_rules.1, input) {
                return;
            }
            f.short_hop |= match f.locomotion.jump_input {
                JumpInput::Buttons => input.buttons & (BUTTON_X | BUTTON_Y) == 0,
                JumpInput::Stick => input.stick[1] < p.tap_jump_release_threshold,
                JumpInput::CStick => input.cstick[1] < p.tap_jump_release_threshold,
            };
        }
        _ => {}
    }
}

pub(crate) fn try_aerial_jump(f: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let Some(p) = &data.locomotion else {
        return false;
    };
    if f.grounded || f.locomotion.jumps_used >= p.max_jumps {
        return false;
    }
    let index = usize::from(f.locomotion.jumps_used.saturating_sub(1));
    let Some(multi) = &p.multi_jump else {
        if jump_input(f, p, input, false).is_none() {
            return false;
        }
        f.ground_velocity = 0.0;
        f.ecb_lock = 10;
        f.ecb.bottom_locked = true;
        f.velocity = [
            input.stick[0] * p.air_jump_horizontal_multiplier,
            data.movement.jump_vertical_velocity * p.air_jump_vertical_multiplier,
        ];
        f.fast_fall = false;
        f.locomotion.jumps_used += 1;
        f.locomotion.tilt_y_age = 254;
        // ftCo_JumpAerial_Enter_Basic (ftCo_JumpAerial.c:169-171): the
        // direction test runs at the launch frame's current stick/facing.
        let backward = p
            .jump_backward_threshold
            .is_some_and(|threshold| math::jump_backward(input.stick[0], f.facing, threshold));
        enter(f, Action::JumpAerial);
        f.locomotion.jump_backward = backward;
        return true;
    };
    let requested = if f.locomotion.jumps_used == 1 {
        jump_input(f, p, input, false).is_some()
    } else {
        let marker_ready = f.action != Action::JumpAerial
            || multi
                .repeat_input_frames
                .get(index.saturating_sub(1))
                .is_some_and(|frame| f.action_frame >= *frame);
        marker_ready
            && (input.stick[1] >= p.tap_jump_threshold
                || input.buttons & (BUTTON_X | BUTTON_Y) != 0)
    };
    if !requested {
        return false;
    }
    f.ground_velocity = 0.0;
    f.ecb_lock = 10;
    f.ecb.bottom_locked = true;
    f.velocity = [
        input.stick[0] * multi.horizontal_velocity,
        multi.vertical_velocities[index],
    ];
    f.fast_fall = false;
    f.locomotion.jumps_used += 1;
    // ftCo_JumpAerial_Enter_Basic (ftCo_JumpAerial.c:169-171): the direction
    // test runs at the launch frame's facing, before this multijump's own
    // root-bone turn can flip it below.
    let backward = p
        .jump_backward_threshold
        .is_some_and(|threshold| math::jump_backward(input.stick[0], f.facing, threshold));
    enter(f, Action::JumpAerial);
    f.locomotion.jump_backward = backward;
    f.locomotion.multi_jump_turn_remaining =
        if input.stick[0] * f.facing < -multi.backward_turn_threshold {
            multi.turn_frames as i32
        } else {
            0
        };
    f.locomotion.multi_jump_yaw = 0.0;
    advance_multi_jump_turn(f, multi.turn_frames);
    true
}

/// Horizontal portion of `ftCo_JumpAerialF1_Phys`; fall/fast-fall remains in
/// the shared scheduler because it is identical to other aerial actions.
pub(crate) fn multi_jump_drift(f: &Fighter, data: &FighterData, movement: &mut Movement) -> bool {
    let Some(multi) = data.locomotion.as_ref().and_then(|p| p.multi_jump.as_ref()) else {
        return false;
    };
    if f.action != Action::JumpAerial {
        return false;
    }
    movement.control_air(
        multi.air_drift_threshold,
        data.movement.air_drift_stick_mul * multi.air_drift_acceleration_multiplier,
        data.movement.air_drift_max * multi.air_drift_max_velocity_multiplier,
    );
    true
}

pub fn pass_request(
    f: &mut Fighter,
    data: &FighterData,
    input: Controller,
    on_platform: bool,
) -> Option<f32> {
    pass_request_after_actions(f, data, input, on_platform, false)
}

pub(crate) fn pass_request_after_actions(
    f: &mut Fighter,
    data: &FighterData,
    input: Controller,
    on_platform: bool,
    shield_owned_frame: bool,
) -> Option<f32> {
    let p = data.locomotion.as_ref()?;
    if shield_owned_frame
        && matches!(
            f.action,
            Action::GuardOn | Action::Guard | Action::GuardReflect
        )
        && math::shield_drop_request(
            input.shield_held(),
            input.stick[1],
            f.locomotion.tilt_y_age,
            p.pass_stick_threshold,
            p.pass_window,
            on_platform,
        )
    {
        f.locomotion.pass_delay = None;
        f.locomotion.tilt_y_age = 254;
        f.locomotion.jumps_used = 1;
        return Some(p.pass_velocity);
    }
    if !matches!(f.action, Action::Squat | Action::SquatWait) {
        return None;
    }
    if f.locomotion.pass_delay.is_none()
        && on_platform
        && input.stick[1] <= -p.pass_stick_threshold
        && f.locomotion.tilt_y_age < p.pass_window
    {
        f.locomotion.pass_delay = Some(p.pass_delay);
        return None;
    }
    if f.action == Action::Squat
        && let Some(delay) = &mut f.locomotion.pass_delay
        && *delay != 0.0
    {
        *delay -= 1.0;
        if *delay == 0.0 && on_platform {
            f.locomotion.pass_delay = None;
            f.locomotion.tilt_y_age = 254;
            f.locomotion.jumps_used = 1;
            return Some(p.pass_velocity);
        }
    }
    None
}

pub fn ground_motion(
    f: &mut Fighter,
    data: &FighterData,
    movement: &mut Movement,
    input: Controller,
) -> bool {
    let Some(p) = data.locomotion.as_ref() else {
        return false;
    };
    if !matches!(
        f.action,
        Action::Dash | Action::Run | Action::RunTurn | Action::RunBrake
    ) {
        return false;
    }
    let friction = data.movement.ground_friction * p.run_friction_multiplier;
    if f.action == Action::Dash && f.locomotion.dash_initial_delta != 0.0 {
        // Dash Enter writes gr_accel2; unlike gr_accel1 it is not projected into
        // self_vel before Fighter_procUpdate integrates this first frame.
        movement.project_ground();
        movement.ground_acceleration = f.locomotion.dash_initial_delta;
        f.locomotion.dash_initial_delta = 0.0;
        return true;
    }
    if f.action == Action::RunBrake {
        movement.friction_ground(friction);
    } else {
        let mut accel = input.stick[0] * p.dash_acceleration_mul;
        accel += if input.stick[0] > 0.0 {
            p.dash_acceleration_base
        } else {
            -p.dash_acceleration_base
        };
        let target = input.stick[0] * p.dash_max_velocity;
        if f.action == Action::RunTurn {
            math::turn_run(
                movement,
                accel,
                target,
                f.locomotion.run_turn_facing,
                data.movement.ground_friction,
                p.run_friction_multiplier,
            );
            return true;
        }
        if f.action == Action::Run && target != 0.0 {
            let ratio = movement.ground_velocity / target;
            if ratio > 0.0 && ratio < 1.0 {
                accel *= (1.0 - ratio) * p.run_accel_taper_gain;
            }
        }
        movement.accelerate_ground(accel, target, friction);
    }
    movement.project_ground();
    true
}

pub(crate) fn hold_action_frame(f: &Fighter) -> bool {
    (f.action == Action::RunTurn && f.locomotion.run_turn_waiting)
        || (f.action == Action::RunBrake && f.locomotion.run_brake_frozen)
        || (matches!(f.action, Action::SpecialLw | Action::SpecialAirLw) && f.down_special.looping)
}
