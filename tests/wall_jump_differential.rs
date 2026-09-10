//! Arbitrary-data comparison against the complete original wall-jump interrupt.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::game::{Controller, wall_jump};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct OracleResult {
    wall_side_bits: u32,
    entered_side_bits: u32,
    entered_timer: i32,
    entered_exponent: i32,
    input_timer: u8,
    used: u8,
    triggered: u8,
    padding: u8,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_wall_jump(
        can_walljump: i32,
        contact: i32,
        speed_valid: i32,
        input_timer: u8,
        wall_side: f32,
        used: u8,
        position_delta_x: f32,
        wall_speed_x: f32,
        minimum_approach_speed: f32,
        stick_x: f32,
        tilt_x_age: u8,
        input_window: f32,
        stick_threshold: f32,
        tilt_window: f32,
        startup_frames: i32,
    ) -> OracleResult;
}

#[allow(clippy::too_many_arguments)]
fn compare(
    can_walljump: bool,
    contact: u8,
    speed_valid: bool,
    input_timer: u8,
    wall_side: f32,
    used: u8,
    position_delta_x: f32,
    wall_speed_x: f32,
    minimum_approach_speed: f32,
    stick_x: f32,
    tilt_x_age: u8,
    input_window: f32,
    stick_threshold: f32,
    tilt_window: f32,
    startup_frames: i32,
) {
    // SAFETY: scalar arguments and the repr(C) return value match the adapter.
    let c = unsafe {
        oracle_wall_jump(
            can_walljump.into(),
            contact.into(),
            speed_valid.into(),
            input_timer,
            wall_side,
            used,
            position_delta_x,
            wall_speed_x,
            minimum_approach_speed,
            stick_x,
            tilt_x_age,
            input_window,
            stick_threshold,
            tilt_window,
            startup_frames,
        )
    };
    let mut state = wall_jump::State {
        input_timer,
        wall_side,
        used,
        ..wall_jump::State::default()
    };
    let rules = wall_jump::Rules {
        tilt_deadzone: 0.1,
        input_window,
        stick_threshold,
        tilt_window,
        startup_frames: startup_frames as u32,
        vertical_velocity_base: 1.0,
    };
    let rust = wall_jump::interrupt(
        &mut state,
        can_walljump,
        match contact {
            1 => Some(wall_jump::Contact {
                wall_side: -1.0,
                wall_velocity_x: speed_valid.then_some(wall_speed_x),
                position_delta_x,
            }),
            2 => Some(wall_jump::Contact {
                wall_side: 1.0,
                wall_velocity_x: speed_valid.then_some(wall_speed_x),
                position_delta_x,
            }),
            _ => None,
        },
        minimum_approach_speed,
        Controller {
            stick: [stick_x, 0.0],
            ..Controller::default()
        },
        tilt_x_age,
        &rules,
    );
    assert_eq!(state.wall_side.to_bits(), c.wall_side_bits);
    assert_eq!(state.input_timer, c.input_timer);
    assert_eq!(state.used, c.used);
    assert_eq!(rust.is_some(), c.triggered != 0);
    if let Some(trigger) = rust {
        assert_eq!(state.wall_side.to_bits(), c.entered_side_bits);
        assert_eq!(startup_frames, c.entered_timer);
        assert_eq!(i32::from(trigger.vertical_exponent), c.entered_exponent);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(768))]
    #[test]
    fn arbitrary_binary32_inputs_match_complete_original_callback(
        can_walljump in any::<bool>(), contact in 0_u8..=2,
        speed_valid in any::<bool>(), input_timer in any::<u8>(),
        wall_side in any::<u32>(), used in any::<u8>(),
        position_delta_x in any::<u32>(), wall_speed_x in any::<u32>(),
        minimum_approach_speed in any::<u32>(), stick_x in any::<u32>(),
        tilt_x_age in any::<u8>(), input_window in any::<u32>(),
        stick_threshold in any::<u32>(), tilt_window in any::<u32>(),
        startup_frames in any::<i32>(),
    ) {
        compare(
            can_walljump, contact, speed_valid, input_timer,
            f32::from_bits(wall_side), used,
            f32::from_bits(position_delta_x), f32::from_bits(wall_speed_x),
            f32::from_bits(minimum_approach_speed), f32::from_bits(stick_x),
            tilt_x_age, f32::from_bits(input_window),
            f32::from_bits(stick_threshold), f32::from_bits(tilt_window),
            startup_frames,
        );
    }
}

#[test]
fn adapter_selects_the_complete_pinned_definition() {
    assert!(
        include_str!("oracle/original/wall_jump.c")
            .contains("bool ftWallJump_8008169C(HSD_GObj* gobj)\n{")
    );
    assert!(include_str!("oracle/wall_jump.c").contains("#include \"wall_jump_original.inc\""));
}
