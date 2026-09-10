//! Ordinary locomotion callbacks from ftCo_{Dash,Run,TurnRun,RunBrake,Turn,Squat,
//! SquatWait,SquatRv,Jump,KneeBend,JumpAerial,Pass}.c and fighter.c input history.
//! Parameters are supplied resources, not character presets. Animation lengths
//! and script events are explicit; animation poses and other character-specific
//! interrupt chains remain absent.
use super::{Action, BUTTON_A, BUTTON_X, BUTTON_Y, Controller, Error, Fighter, data::FighterData};
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
    pub run_turn_velocity_scale: f32,
    pub run_brake_animation_frames: u32,
    pub run_brake_turn_frame: u32,
    pub run_brake_max_frames: f32,
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
    pub jumps_used: u8,
    pub jump_input: JumpInput,
    pub turn_frames: f32,
    pub turn_has_turned: bool,
    pub turn_smash: bool,
    pub dash_initial_delta: f32,
    /// Facing captured by `ftCo_TurnRun_Enter` for its physics branch.
    pub run_turn_facing: f32,
    /// The animation event fired and is waiting for velocity to cross x0.01.
    pub run_turn_waiting: bool,
    pub run_brake_frames: f32,
    pub pass_delay: Option<f32>,
    /// Root-joint turn state from `ft_800CB6EC`; yaw affects bone physics.
    pub multi_jump_turn_remaining: i32,
    pub multi_jump_yaw: f32,
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
            jumps_used: 0,
            jump_input: JumpInput::Buttons,
            turn_frames: 0.0,
            turn_has_turned: false,
            turn_smash: false,
            dash_initial_delta: 0.0,
            run_turn_facing: 0.0,
            run_turn_waiting: false,
            run_brake_frames: 0.0,
            pass_delay: None,
            multi_jump_turn_remaining: 0,
            multi_jump_yaw: 0.0,
        }
    }
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
        p.run_turn_velocity_scale,
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
        || p.run_turn_velocity_scale == 0.0
        || ![p.dash_window, p.tap_jump_window, p.pass_window]
            .into_iter()
            .all(|n| (1..254).contains(&n))
        || p.pass_delay.fract() != 0.0
        || p.crouch_release_threshold > p.crouch_enter_threshold
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

fn enter(f: &mut Fighter, action: Action) {
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

fn jump_input(f: &Fighter, p: &Parameters, input: Controller, relaxed: bool) -> Option<JumpInput> {
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

fn start_dash(f: &mut Fighter, p: &Parameters) {
    enter(f, Action::Dash);
    f.locomotion.tilt_x_age = 254;
    let initial = f.facing * p.dash_initial_velocity;
    f.locomotion.dash_initial_delta = if f.ground_velocity * f.facing < 0.0 {
        initial
    } else {
        initial - f.ground_velocity
    };
}

fn start_turn(f: &mut Fighter, p: &Parameters, smash: bool) {
    enter(f, Action::Turn);
    f.locomotion.turn_frames = if smash { 0.0 } else { p.standing_turn_frames };
    f.locomotion.turn_has_turned = false;
    f.locomotion.turn_smash = smash;
}

fn start_run_turn(f: &mut Fighter, frame: u32) {
    let facing = f.facing;
    enter(f, Action::RunTurn);
    f.action_frame = frame;
    f.locomotion.run_turn_facing = facing;
    f.locomotion.run_turn_waiting = false;
    f.locomotion.turn_has_turned = false;
}

fn try_dash(f: &mut Fighter, p: &Parameters, input: Controller) -> bool {
    if input.stick[0].abs() < p.dash_threshold || f.locomotion.tilt_x_age >= p.dash_window {
        return false;
    }
    if input.stick[0] * f.facing < 0.0 {
        start_turn(f, p, true);
    } else {
        start_dash(f, p);
    }
    true
}

fn ground_jump(f: &mut Fighter, data: &FighterData, input: Controller) {
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
    enter(f, Action::Jump);
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
            ground_jump(f, data, input)
        }
        Action::Dash if f.action_frame >= p.dash_animation_frames => enter(f, Action::Wait),
        Action::RunBrake => {
            if f.locomotion.run_brake_frames != 0.0 {
                f.locomotion.run_brake_frames = (f.locomotion.run_brake_frames - 1.0).max(0.0);
            }
            if f.action_frame >= p.run_brake_animation_frames
                || f.locomotion.run_brake_frames == 0.0
            {
                enter(f, Action::Wait);
            }
        }
        Action::RunTurn => {
            if !f.locomotion.turn_has_turned && f.action_frame >= p.run_turn_flip_frame {
                f.locomotion.run_turn_waiting = true;
                if p.run_turn_velocity_scale * f.ground_velocity <= 0.01 {
                    f.locomotion.run_turn_waiting = false;
                    f.locomotion.turn_has_turned = true;
                    f.facing = -f.facing;
                }
            }
            if f.locomotion.turn_has_turned && f.action_frame >= p.run_turn_animation_frames {
                if input.stick[0] * f.facing >= p.run_threshold {
                    enter(f, Action::Run);
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
                    enter(f, Action::Fall);
                }
            } else if f.action_frame >= p.air_jump_animation_frames {
                enter(f, Action::Fall)
            }
        }
        Action::Pass if f.action_frame >= p.pass_animation_frames => enter(f, Action::Fall),
        _ => {}
    }
    just_turned
}

pub(crate) fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    input: Controller,
    just_turned: bool,
) {
    let Some(p) = data.locomotion.as_ref() else {
        return;
    };
    let grounded_action = f.grounded
        && matches!(
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
        );
    if grounded_action {
        if input.buttons & !f.previous_input.buttons & BUTTON_A != 0
            && matches!(
                f.action,
                Action::Wait | Action::Walk | Action::Turn | Action::Squat | Action::SquatWait
            )
        {
            if f.action == Action::Turn && !f.locomotion.turn_has_turned {
                f.facing = -f.facing;
            }
            enter(f, Action::Jab);
            return;
        }
        if let Some(source) = jump_input(
            f,
            p,
            input,
            matches!(
                f.action,
                Action::Dash | Action::Run | Action::RunTurn | Action::RunBrake
            ),
        ) {
            f.short_hop = false;
            f.locomotion.jump_input = source;
            enter(f, Action::JumpSquat);
        }
    } else if matches!(
        f.action,
        Action::Jump | Action::Fall | Action::JumpAerial | Action::Pass
    ) {
        try_aerial_jump(f, data, input);
    }
    match f.action {
        Action::Wait | Action::Walk => {
            if try_dash(f, p, input) {
                return;
            }
            if input.stick[1] < -p.crouch_enter_threshold {
                f.locomotion.pass_delay = None;
                enter(f, Action::Squat);
            } else if input.stick[0] * f.facing <= p.turn_threshold {
                start_turn(f, p, false);
            } else if input.stick[0] * f.facing >= p.walk_threshold {
                if f.action != Action::Walk {
                    enter(f, Action::Walk);
                }
            } else if f.action == Action::Walk {
                enter(f, Action::Wait);
            }
        }
        Action::Dash
            if f.action_frame >= p.dash_run_frame
                && input.stick[0] * f.facing >= p.run_threshold =>
        {
            enter(f, Action::Run)
        }
        Action::Run => {
            if input.stick[0] * f.facing <= p.turn_threshold {
                start_run_turn(f, 0);
            } else if input.stick[0].abs() < p.run_threshold {
                enter(f, Action::RunBrake);
                f.locomotion.run_brake_frames = p.run_brake_max_frames;
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
                start_dash(f, p);
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
            f.short_hop |= match f.locomotion.jump_input {
                JumpInput::Buttons => input.buttons & (BUTTON_X | BUTTON_Y) == 0,
                JumpInput::Stick => input.stick[1] < p.tap_jump_release_threshold,
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
        enter(f, Action::JumpAerial);
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
    enter(f, Action::JumpAerial);
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
    f.action == Action::RunTurn && f.locomotion.run_turn_waiting
}
