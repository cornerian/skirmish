//! Fox/Falco neutral special (Blaster) checked against the pinned C
//! (`ftfoxspecialn.c`): the Start/Loop/End state machine's own Enter reset,
//! the turnaround-latch predicate (`ftFox_SpecialN_CheckLoopInput`), the
//! Start->Loop transition, Loop's own repeat-vs-end decision and same-frame
//! fire check, End's Wait/Fall/FallSpecial dispatch, and
//! `PrepareBlasterShot`/`FireBlasterShot` with the item spawn's own
//! angle/speed/kind captured. Each comparison pins the extracted callback's
//! exact behaviour with an inline Rust formula, matching
//! `fox_down_special_differential.rs`'s own style -- `tests/
//! game_fox_neutral_special.rs` and `game_fox_neutral_special_reflect.rs`
//! separately exercise the Rust engine's own mirror end to end.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_neutral_enter(
        ground: bool,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        self_vel_z_in: f32,
        out_msid: *mut i32,
        out_cmd_vars0: *mut i32,
        out_cmd_vars1: *mut i32,
        out_cmd_vars2: *mut i32,
        out_cmd_vars3: *mut i32,
        out_is_blaster_loop: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_self_vel_z: *mut f32,
    );
    fn oracle_neutral_check_loop_input(cmd0_in: i32, pressed_b: bool, is_loop_in: bool) -> bool;
    fn oracle_neutral_start_anim(ground: bool, frames_remaining: bool, out_msid: *mut i32) -> i32;
    fn oracle_neutral_loop_anim(
        ground: bool,
        frames_remaining: bool,
        is_blaster_loop_in: i32,
        cmd_vars2_in: i32,
        facing_dir_in: f32,
        angle_attr_in: f32,
        vel_attr_in: f32,
        kind_in: i32,
        out_msid: *mut i32,
        out_is_blaster_loop: *mut i32,
        out_fired: *mut i32,
        out_angle: *mut f64,
        out_speed: *mut f32,
        out_kind: *mut i32,
    );
    fn oracle_neutral_end_anim_ground(
        frames_remaining: bool,
        out_wait_calls: *mut i32,
        out_change_motion_state_calls: *mut i32,
    );
    fn oracle_neutral_end_anim_air(
        frames_remaining: bool,
        landing_lag_in: f32,
        out_fall_calls: *mut i32,
        out_fall_special_calls: *mut i32,
        out_fall_special_lag: *mut f32,
    );
    fn oracle_neutral_create_blaster_shot(
        cmd_vars2_in: i32,
        facing_dir_in: f32,
        angle_attr_in: f32,
        vel_attr_in: f32,
        kind_in: i32,
        out_fired: *mut i32,
        out_angle: *mut f64,
        out_speed: *mut f32,
        out_kind: *mut i32,
    );
}

const MS_START_GROUND: i32 = 3000;
const MS_LOOP_GROUND: i32 = 3001;
const MS_END_GROUND: i32 = 3002;
const MS_START_AIR: i32 = 3003;
const MS_LOOP_AIR: i32 = 3004;
#[allow(dead_code)]
const MS_END_AIR: i32 = 3005;

fn same_bits(label: &str, actual: f32, expected: f32) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{label}: {actual} (0x{:08x}) != {expected} (0x{:08x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

/// `ftFox_SpecialN_PrepareBlasterShot`'s own launch-angle formula, inline:
/// `fp->facing_dir == 1.0F ? x10 : M_PI - x10`.
fn expected_launch_angle(facing_dir: f32, angle_attr: f64) -> f64 {
    if facing_dir == 1.0 {
        angle_attr
    } else {
        core::f64::consts::PI - angle_attr
    }
}

fn compare_enter(ground: bool, gr_vel_in: f32, svx: f32, svy: f32, svz: f32) {
    let (
        mut msid,
        mut c0,
        mut c1,
        mut c2,
        mut c3,
        mut is_loop,
        mut gr_vel,
        mut out_svx,
        mut out_svy,
        mut out_svz,
    ) = (0, 0, 0, 0, 0, 0, 0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_neutral_enter(
            ground,
            gr_vel_in,
            svx,
            svy,
            svz,
            &mut msid,
            &mut c0,
            &mut c1,
            &mut c2,
            &mut c3,
            &mut is_loop,
            &mut gr_vel,
            &mut out_svx,
            &mut out_svy,
            &mut out_svz,
        );
    }
    assert_eq!(msid, if ground { MS_START_GROUND } else { MS_START_AIR });
    assert_eq!((c0, c1, c2, c3), (0, 0, 0, 0));
    assert_eq!(is_loop, 0);
    if ground {
        // `ftFx_SpecialN_Enter`'s own explicit zero, unconditionally, after
        // `ftCommon_8007D7FC` (which this adapter captures rather than
        // reimplements -- see its own header note).
        same_bits("gr_vel", gr_vel, 0.0);
        same_bits("self_vel_x", out_svx, 0.0);
        same_bits("self_vel_y", out_svy, 0.0);
        same_bits("self_vel_z", out_svz, 0.0);
    } else {
        // `ftFx_SpecialAirN_Enter` touches neither `gr_vel` nor `self_vel`.
        same_bits("gr_vel", gr_vel, gr_vel_in);
        same_bits("self_vel_x", out_svx, svx);
        same_bits("self_vel_y", out_svy, svy);
        same_bits("self_vel_z", out_svz, svz);
    }
    let _ = (svx, svy, svz);
}

fn compare_check_loop_input(cmd0: i32, pressed_b: bool, is_loop_in: bool) {
    let expected = is_loop_in || (cmd0 != 0 && pressed_b);
    let actual = unsafe { oracle_neutral_check_loop_input(cmd0, pressed_b, is_loop_in) };
    assert_eq!(actual, expected);
}

fn compare_start_anim(ground: bool, frames_remaining: bool) {
    let mut msid = 0;
    let transitions = unsafe { oracle_neutral_start_anim(ground, frames_remaining, &mut msid) };
    if frames_remaining {
        assert_eq!(transitions, 0);
    } else {
        assert_eq!(transitions, 1);
        assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_loop_anim(
    ground: bool,
    frames_remaining: bool,
    is_blaster_loop_in: bool,
    cmd_vars2_in: i32,
    facing_dir_in: f32,
    angle_attr: f32,
    vel_attr: f32,
    kind: i32,
) {
    let (mut msid, mut is_loop_out, mut fired, mut angle, mut speed, mut out_kind) =
        (0, 0, 0, 0.0, 0.0, 0);
    unsafe {
        oracle_neutral_loop_anim(
            ground,
            frames_remaining,
            is_blaster_loop_in as i32,
            cmd_vars2_in,
            facing_dir_in,
            angle_attr,
            vel_attr,
            kind,
            &mut msid,
            &mut is_loop_out,
            &mut fired,
            &mut angle,
            &mut speed,
            &mut out_kind,
        );
    }
    if frames_remaining {
        assert_eq!(msid, 0, "no transition mid-clip");
        assert_eq!(is_loop_out, is_blaster_loop_in as i32, "isBlasterLoop untouched mid-clip");
    } else if is_blaster_loop_in {
        assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
        assert_eq!(is_loop_out, 0, "a repeat cycle resets isBlasterLoop for the next one");
    } else {
        assert_eq!(msid, if ground { MS_END_GROUND } else { MS_END_AIR });
    }
    // The same-frame fire check (`cmd_vars[2] != 0`) is independent of the
    // loop/end transition decision above -- both branches re-derive it.
    if cmd_vars2_in != 0 {
        assert_eq!(fired, 1);
        same_bits(
            "angle",
            angle as f32,
            expected_launch_angle(facing_dir_in, angle_attr as f64) as f32,
        );
        same_bits("speed", speed, vel_attr);
        assert_eq!(out_kind, kind);
    } else {
        assert_eq!(fired, 0);
    }
}

fn compare_end_anim_ground(frames_remaining: bool) {
    let (mut wait_calls, mut change_calls) = (0, 0);
    unsafe {
        oracle_neutral_end_anim_ground(frames_remaining, &mut wait_calls, &mut change_calls);
    }
    if frames_remaining {
        assert_eq!(wait_calls, 0);
    } else {
        // Direct `Wait` re-entry, no landing lag, no further motion-state
        // change of its own.
        assert_eq!(wait_calls, 1);
        assert_eq!(change_calls, 0);
    }
}

fn compare_end_anim_air(frames_remaining: bool, landing_lag: f32) {
    let (mut fall_calls, mut fall_special_calls, mut fall_special_lag) = (0, 0, 0.0);
    unsafe {
        oracle_neutral_end_anim_air(
            frames_remaining,
            landing_lag,
            &mut fall_calls,
            &mut fall_special_calls,
            &mut fall_special_lag,
        );
    }
    if frames_remaining {
        assert_eq!((fall_calls, fall_special_calls), (0, 0));
        return;
    }
    if landing_lag == 0.0 {
        assert_eq!((fall_calls, fall_special_calls), (1, 0));
    } else {
        assert_eq!((fall_calls, fall_special_calls), (0, 1));
        same_bits("fall_special_lag", fall_special_lag, landing_lag);
    }
}

fn compare_create_blaster_shot(
    cmd_vars2_in: i32,
    facing_dir_in: f32,
    angle_attr: f32,
    vel_attr: f32,
    kind: i32,
) {
    let (mut fired, mut angle, mut speed, mut out_kind) = (0, 0.0, 0.0, 0);
    unsafe {
        oracle_neutral_create_blaster_shot(
            cmd_vars2_in,
            facing_dir_in,
            angle_attr,
            vel_attr,
            kind,
            &mut fired,
            &mut angle,
            &mut speed,
            &mut out_kind,
        );
    }
    if cmd_vars2_in != 0 {
        assert_eq!(fired, 1);
        same_bits(
            "angle",
            angle as f32,
            expected_launch_angle(facing_dir_in, angle_attr as f64) as f32,
        );
        same_bits("speed", speed, vel_attr);
        assert_eq!(out_kind, kind);
    } else {
        assert_eq!(fired, 0);
    }
}

#[test]
fn adapter_statements_are_verbatim_in_the_pinned_sources() {
    let adapter = include_str!("oracle/fox_neutral_special.c");
    let source = include_str!("oracle/original/ftfox_inlines.h");
    let block = adapter
        .split("/* BEGIN VERBATIM CHECK LOOP INPUT */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM CHECK LOOP INPUT */")
        .next()
        .unwrap();
    assert!(source.contains(block));
}

#[test]
fn known_values_match() {
    compare_enter(true, 3.0, 1.0, 2.0, 0.0);
    compare_enter(false, 3.0, 1.0, 2.0, 0.0);
    compare_start_anim(true, true);
    compare_start_anim(true, false);
    compare_start_anim(false, false);
    compare_loop_anim(true, false, true, 1, 1.0, 0.0, 7.0, 54);
    compare_loop_anim(true, false, false, 0, -1.0, 0.0, 7.0, 54);
    compare_end_anim_ground(false);
    compare_end_anim_air(false, 0.0);
    compare_end_anim_air(false, 12.0);
    compare_create_blaster_shot(1, 1.0, 0.0, 7.0, 54);
    compare_create_blaster_shot(0, 1.0, 0.0, 7.0, 54);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn enter_matches_arbitrary_velocities(
        ground in any::<bool>(),
        gr_vel in -50.0f32..50.0,
        svx in -50.0f32..50.0,
        svy in -50.0f32..50.0,
        svz in -50.0f32..50.0,
    ) {
        compare_enter(ground, gr_vel, svx, svy, svz);
    }

    #[test]
    fn check_loop_input_matches_arbitrary_inputs(
        cmd0 in any::<i32>(),
        pressed_b in any::<bool>(),
        is_loop in any::<bool>(),
    ) {
        compare_check_loop_input(cmd0, pressed_b, is_loop);
    }

    #[test]
    fn start_anim_matches(ground in any::<bool>(), frames_remaining in any::<bool>()) {
        compare_start_anim(ground, frames_remaining);
    }

    #[test]
    #[allow(clippy::too_many_arguments)]
    fn loop_anim_matches(
        ground in any::<bool>(),
        frames_remaining in any::<bool>(),
        is_blaster_loop in any::<bool>(),
        fire in any::<bool>(),
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        angle_attr in -10.0f32..10.0,
        vel_attr in 0.0f32..20.0,
        kind in any::<i32>(),
    ) {
        compare_loop_anim(
            ground,
            frames_remaining,
            is_blaster_loop,
            if fire { 1 } else { 0 },
            facing_dir,
            angle_attr,
            vel_attr,
            kind,
        );
    }

    #[test]
    fn end_anim_ground_matches(frames_remaining in any::<bool>()) {
        compare_end_anim_ground(frames_remaining);
    }

    #[test]
    fn end_anim_air_matches(frames_remaining in any::<bool>(), landing_lag in 0.0f32..40.0) {
        compare_end_anim_air(frames_remaining, landing_lag);
    }

    #[test]
    fn create_blaster_shot_matches(
        fire in any::<bool>(),
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        angle_attr in -10.0f32..10.0,
        vel_attr in 0.0f32..20.0,
        kind in any::<i32>(),
    ) {
        compare_create_blaster_shot(if fire { 1 } else { 0 }, facing_dir, angle_attr, vel_attr, kind);
    }
}
