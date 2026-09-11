//! `ftCo_RunBrake_Anim`/`_IASA` (`tests/oracle/runbrake.c`,
//! `runbrake.functions.json`) checked against a Rust mirror of the
//! freeze/resume/end decision -- the mechanism `docs/run.md`'s RunBrake
//! freeze correction (`Parameters::run_brake_marker_frame`/
//! `run_brake_freeze_speed`, `State::run_brake_frozen`) depends on.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[repr(C)]
struct RunBrakeAnimResult {
    set_rate_called: i32,
    rate: f32,
    cmd_vars1_after: i32,
    x0_after: i32,
    frames_after: f32,
    wait_called: i32,
}

#[repr(C)]
struct RunBrakeIasaResult {
    jump_called: i32,
    turn_called: i32,
    squat_called: i32,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_run_brake_anim(
        cmd_vars1: i32,
        runbrake_x0: i32,
        gr_vel: f32,
        freeze_speed: f32,
        runbrake_frames: f32,
        is_frames_remaining: i32,
    ) -> RunBrakeAnimResult;
    fn oracle_run_brake_iasa(
        jump_result: i32,
        cmd_vars0: i32,
        turn_result: i32,
        squat_result: i32,
    ) -> RunBrakeIasaResult;
}

fn exact(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} != NaN");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

/// Rust mirror of `ftCo_RunBrake_Anim` (`ftCo_RunBrake.c:49-77`): while the
/// marker (`cmd_vars1`) is set and not yet frozen (`runbrake_x0 == 0`),
/// freeze (rate 0, `x0 = 1`) once `|gr_vel| >= freeze_speed`; once frozen,
/// resume (rate 1, `cmd_vars1 = 0`) once `|gr_vel| <= freeze_speed`. The
/// `frames` countdown decrements unconditionally by 1.0 while nonzero,
/// clamped at 0.0; the exit test is
/// `!(is_frames_remaining && frames != 0.0)`.
struct AnimMirror {
    set_rate_called: bool,
    rate: f32,
    cmd_vars1_after: i32,
    x0_after: i32,
    frames_after: f32,
    wait_called: bool,
}

fn mirror_run_brake_anim(
    cmd_vars1: i32,
    runbrake_x0: i32,
    gr_vel: f32,
    freeze_speed: f32,
    runbrake_frames: f32,
    is_frames_remaining: bool,
) -> AnimMirror {
    let mut cmd_vars1 = cmd_vars1;
    let mut x0 = runbrake_x0;
    let mut set_rate_called = false;
    let mut rate = 0.0f32;
    if cmd_vars1 != 0 {
        if x0 == 0 {
            if gr_vel.abs() >= freeze_speed {
                rate = 0.0;
                set_rate_called = true;
                x0 = 1;
            }
        } else if gr_vel.abs() <= freeze_speed {
            rate = 1.0;
            set_rate_called = true;
            cmd_vars1 = 0;
        }
    }
    let mut frames = runbrake_frames;
    if frames != 0.0 {
        frames -= 1.0;
        if frames < 0.0 {
            frames = 0.0;
        }
    }
    let wait_called = !(is_frames_remaining && frames != 0.0);
    AnimMirror {
        set_rate_called,
        rate,
        cmd_vars1_after: cmd_vars1,
        x0_after: x0,
        frames_after: frames,
        wait_called,
    }
}

fn compare_anim(
    cmd_vars1: i32,
    runbrake_x0: i32,
    gr_vel: f32,
    freeze_speed: f32,
    runbrake_frames: f32,
    is_frames_remaining: bool,
) -> AnimMirror {
    let expected = mirror_run_brake_anim(
        cmd_vars1,
        runbrake_x0,
        gr_vel,
        freeze_speed,
        runbrake_frames,
        is_frames_remaining,
    );
    // SAFETY: six scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe {
        oracle_run_brake_anim(
            cmd_vars1,
            runbrake_x0,
            gr_vel,
            freeze_speed,
            runbrake_frames,
            i32::from(is_frames_remaining),
        )
    };
    assert_eq!(actual.set_rate_called != 0, expected.set_rate_called);
    if expected.set_rate_called {
        exact(actual.rate, expected.rate);
    }
    assert_eq!(actual.cmd_vars1_after, expected.cmd_vars1_after);
    assert_eq!(actual.x0_after, expected.x0_after);
    exact(actual.frames_after, expected.frames_after);
    assert_eq!(actual.wait_called != 0, expected.wait_called);
    expected
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Single-frame coverage over arbitrary binary32 `gr_vel`/`freeze_speed`
    /// (NaN-safe: every comparison with NaN is false, matching the source's
    /// own `>=`/`<=`), plus a bounded, realistic `frames` range.
    #[test]
    fn arbitrary_run_brake_anim(
        cmd_vars1 in prop_oneof![Just(0i32), Just(1i32)],
        runbrake_x0 in prop_oneof![Just(0i32), Just(1i32)],
        gr_vel in any::<u32>().prop_map(f32::from_bits),
        freeze_speed in any::<u32>().prop_map(f32::from_bits),
        runbrake_frames in prop_oneof![Just(0.0f32), (0.0f32..40.0f32), any::<u32>().prop_map(f32::from_bits)],
        is_frames_remaining in any::<bool>(),
    ) {
        compare_anim(cmd_vars1, runbrake_x0, gr_vel, freeze_speed, runbrake_frames, is_frames_remaining);
    }

    /// A generated, physically realistic (monotonically non-increasing
    /// magnitude, matching `ftCo_RunBrake_Phys`'s own friction-only
    /// deceleration) velocity sequence, threading each call's
    /// `cmd_vars1_after`/`x0_after`/`frames_after` into the next -- the
    /// same state `game::locomotion::update_animation`'s `Action::RunBrake`
    /// arm threads through `Fighter.locomotion` across real frames.
    #[test]
    fn generated_velocity_sequences_match_c(
        start_speed in 0.0f32..4.0f32,
        decay in 0.05f32..1.0f32,
        freeze_speed in 0.0f32..4.0f32,
        frames in 4.0f32..20.0f32,
    ) {
        let mut cmd_vars1 = 1;
        let mut x0 = 0;
        let mut remaining_frames = frames;
        let mut speed = start_speed;
        for _ in 0..12 {
            let result = compare_anim(cmd_vars1, x0, speed, freeze_speed, remaining_frames, true);
            cmd_vars1 = result.cmd_vars1_after;
            x0 = result.x0_after;
            remaining_frames = result.frames_after;
            speed = (speed - decay).max(0.0);
        }
    }
}

#[test]
fn exact_boundaries() {
    // Marker inactive: no freeze, SetAnimRate never called.
    let r = compare_anim(0, 0, 4.0, 1.0, 8.0, true);
    assert!(!r.set_rate_called);
    // Not yet frozen, exactly at the freeze boundary (>=): freezes.
    compare_anim(1, 0, 1.0, 1.0, 8.0, true);
    compare_anim(1, 0, 1.0_f32.next_down(), 1.0, 8.0, true);
    // Frozen, exactly at the resume boundary (<=): resumes.
    compare_anim(1, 1, 1.0, 1.0, 8.0, true);
    compare_anim(1, 1, 1.0_f32.next_up(), 1.0, 8.0, true);
    // frames already 0: stays 0, never negative.
    let r = compare_anim(0, 0, 0.0, 0.0, 0.0, true);
    assert_eq!(r.frames_after, 0.0);
    assert!(r.wait_called);
    // frames exactly 1.0: decrements to 0.0 this frame and ends.
    let r = compare_anim(0, 0, 0.0, 0.0, 1.0, true);
    assert_eq!(r.frames_after, 0.0);
    assert!(r.wait_called);
    // No frames remaining even with frames still positive: ends anyway.
    let r = compare_anim(0, 0, 0.0, 0.0, 8.0, false);
    assert!(r.frames_after > 0.0);
    assert!(r.wait_called);
    // NaN ground velocity: neither freeze nor resume ever fires.
    let r = compare_anim(1, 0, f32::NAN, 1.0, 8.0, true);
    assert!(!r.set_rate_called);
    let r = compare_anim(1, 1, f32::NAN, 1.0, 8.0, true);
    assert!(!r.set_rate_called);
}

fn compare_iasa(jump: bool, cmd_vars0: bool, turn: bool, squat: bool) {
    // ftCo_RunBrake_IASA (ftCo_RunBrake.c:80-87): a RETURN_IF chain -- jump
    // first, then the turn-run entry gated by cmd_vars[0], then squat.
    let jump_called = true;
    // `cmd_vars[0] && fn_800C9CEC(gobj)` short-circuits: the call only
    // happens when cmd_vars[0] is truthy.
    let turn_called = !jump && cmd_vars0;
    let squat_called = !jump && !(cmd_vars0 && turn);
    // SAFETY: four scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe {
        oracle_run_brake_iasa(
            i32::from(jump),
            i32::from(cmd_vars0),
            i32::from(turn),
            i32::from(squat),
        )
    };
    assert_eq!(actual.jump_called != 0, jump_called);
    assert_eq!(actual.turn_called != 0, turn_called);
    assert_eq!(actual.squat_called != 0, squat_called);
}

#[test]
fn iasa_gate_order_matches_c_over_every_boolean_combination() {
    for jump in [false, true] {
        for cmd_vars0 in [false, true] {
            for turn in [false, true] {
                for squat in [false, true] {
                    compare_iasa(jump, cmd_vars0, turn, squat);
                }
            }
        }
    }
}

#[test]
fn adapter_retains_the_complete_pinned_functions() {
    let source = include_str!("oracle/original/runbrake.c");
    assert!(source.contains("void ftCo_RunBrake_Anim(Fighter_GObj* gobj)"));
    assert!(source.contains("void ftCo_RunBrake_IASA(Fighter_GObj* gobj)"));
    assert!(include_str!("oracle/runbrake.c").contains("#include \"runbrake_original.inc\""));
}
