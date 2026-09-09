//! Ordinary locomotion callbacks from ftCo_{Dash,Run,RunBrake,Turn,Squat,
//! SquatWait,SquatRv,Jump,KneeBend,JumpAerial,Pass}.c and fighter.c input history.
//! Parameters are supplied resources, not character presets. Animation lengths
//! and the dash-to-run script event are explicit; animation poses, turn-run,
//! character-specific jumps, multijumps and other interrupt chains remain absent.
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
    pub run_brake_animation_frames: u32,
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
    pub air_jump_horizontal_multiplier: f32,
    pub air_jump_vertical_multiplier: f32,
    pub air_jump_animation_frames: u32,
    pub pass_stick_threshold: f32,
    pub pass_window: u8,
    pub pass_delay: f32,
    pub pass_velocity: f32,
    pub pass_animation_frames: u32,
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
    pub jumps_used: u8,
    pub jump_input: JumpInput,
    pub turn_frames: f32,
    pub turn_has_turned: bool,
    pub turn_smash: bool,
    pub dash_initial_delta: f32,
    pub run_brake_frames: f32,
    pub pass_delay: Option<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tilt_x_age: 254,
            tilt_y_age: 254,
            trigger_age: 255,
            tech_press_age: 255,
            previous_tech_press_age: 255,
            jumps_used: 0,
            jump_input: JumpInput::Buttons,
            turn_frames: 0.0,
            turn_has_turned: false,
            turn_smash: false,
            dash_initial_delta: 0.0,
            run_brake_frames: 0.0,
            pass_delay: None,
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
        p.run_brake_max_frames,
        p.standing_turn_frames,
        p.air_jump_horizontal_multiplier,
        p.air_jump_vertical_multiplier,
        p.pass_delay,
    ];
    let durations = [
        p.dash_animation_frames,
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
        || ![p.dash_window, p.tap_jump_window, p.pass_window]
            .into_iter()
            .all(|n| (1..254).contains(&n))
        || !(1..=2).contains(&p.max_jumps)
        || p.pass_delay.fract() != 0.0
        || p.crouch_release_threshold > p.crouch_enter_threshold
    {
        return Err(Error::Data(
            "invalid ordinary locomotion parameters (multijump callbacks are unsupported)".into(),
        ));
    }
    Ok(())
}

pub fn landed(f: &mut Fighter) {
    f.locomotion.jumps_used = 0;
    f.locomotion.pass_delay = None;
}

fn enter(f: &mut Fighter, action: Action) {
    if !matches!(action, Action::Squat | Action::SquatWait) {
        f.locomotion.pass_delay = None;
    }
    super::simulation::enter(f, action);
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
        Action::JumpAerial if f.action_frame >= p.air_jump_animation_frames => {
            enter(f, Action::Fall)
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
            matches!(f.action, Action::Dash | Action::Run | Action::RunBrake),
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
        Action::Run if input.stick[0].abs() < p.run_threshold => {
            enter(f, Action::RunBrake);
            f.locomotion.run_brake_frames = p.run_brake_max_frames;
        }
        Action::RunBrake if input.stick[1] < -p.crouch_enter_threshold => enter(f, Action::Squat),
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
    if f.grounded
        || f.locomotion.jumps_used >= p.max_jumps
        || jump_input(f, p, input, false).is_none()
    {
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
    true
}

pub fn pass_request(
    f: &mut Fighter,
    data: &FighterData,
    input: Controller,
    on_platform: bool,
) -> Option<f32> {
    let p = data.locomotion.as_ref()?;
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
    if !matches!(f.action, Action::Dash | Action::Run | Action::RunBrake) {
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
