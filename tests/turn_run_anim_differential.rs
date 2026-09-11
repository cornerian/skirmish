//! `ftCo_TurnRun_Enter`/`_Anim`/`fn_800C9CEC`/`fn_800C9D40`
//! (`tests/oracle/turn_run.c`, extending `turn_run.functions.json`) checked
//! against a Rust mirror of the freeze/resume/flip decision -- the exact
//! mechanism `docs/run.md`'s run-turn-flip correction depends on
//! (`Fighter.locomotion.run_turn_facing` replacing the fixed
//! `run_turn_velocity_scale` resource constant).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[repr(C)]
struct TurnRunEnterResult {
    msid: i32,
    flags: u32,
    start: f32,
    speed: f32,
    cmd_vars1_after: i32,
    accel_mul_after: f32,
    x14_after: i32,
}

#[repr(C)]
struct TurnRunAnimResult {
    set_rate_called: i32,
    rate: f32,
    cmd_vars1_after: i32,
    x14_after: i32,
    facing_after: f32,
    fn_800ca644_called: i32,
    wait_called: i32,
}

#[repr(C)]
struct TurnRunCheckResult {
    returned: i32,
    entered: i32,
    start_used: f32,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_turn_run_enter(anim_start: f32, facing_dir: f32) -> TurnRunEnterResult;
    fn oracle_turn_run_anim(
        cmd_vars1: i32,
        turnrun_x14: i32,
        entry_facing: f32,
        facing_dir: f32,
        gr_vel: f32,
        is_frames_remaining: i32,
        fn_800ca644_result: i32,
    ) -> TurnRunAnimResult;
    fn oracle_fn_800c9cec(
        lstick_x: f32,
        facing_dir: f32,
        threshold: f32,
        cur_anim_frame: f32,
    ) -> TurnRunCheckResult;
    fn oracle_fn_800c9d40(
        lstick_x: f32,
        facing_dir: f32,
        threshold: f32,
        cur_anim_frame: f32,
    ) -> TurnRunCheckResult;
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

/// Rust mirror of `ftCo_TurnRun_Anim` (`ftCo_TurnRun.c:57-77`): while the
/// marker (`cmd_vars1`) is set, first freeze (rate 0, `turnrun_x14 = 1`),
/// then on later calls resume (rate 1) and flip once `entry_facing * gr_vel
/// <= 0.01` (the union-aliased `turnrun.accel_mul`/`walk.middle_anim_frame`
/// read). Independently, the unconditional animation-end check
/// (`!is_frames_remaining && !fn_800ca644_result => wait`) is exposed via
/// scripted stand-ins for `ftAnim_IsFramesRemaining`/`fn_800CA644`, since
/// this codebase does not reimplement either.
struct AnimMirror {
    set_rate_called: bool,
    rate: f32,
    cmd_vars1_after: i32,
    x14_after: i32,
    facing_after: f32,
    fn_800ca644_called: bool,
    wait_called: bool,
}

fn mirror_turn_run_anim(
    cmd_vars1: i32,
    turnrun_x14: i32,
    entry_facing: f32,
    facing_dir: f32,
    gr_vel: f32,
    is_frames_remaining: bool,
    fn_800ca644_result: bool,
) -> AnimMirror {
    let mut cmd_vars1 = cmd_vars1;
    let mut turnrun_x14 = turnrun_x14;
    let mut facing_dir = facing_dir;
    let mut set_rate_called = false;
    let mut rate = 0.0f32;
    if cmd_vars1 != 0 {
        if turnrun_x14 == 0 {
            rate = 0.0;
            set_rate_called = true;
            turnrun_x14 = 1;
        } else if entry_facing * gr_vel <= 0.01 {
            rate = 1.0;
            set_rate_called = true;
            cmd_vars1 = 0;
            facing_dir = -facing_dir;
        }
    }
    let mut fn_800ca644_called = false;
    let mut wait_called = false;
    if !is_frames_remaining {
        fn_800ca644_called = true;
        if !fn_800ca644_result {
            wait_called = true;
        }
    }
    AnimMirror {
        set_rate_called,
        rate,
        cmd_vars1_after: cmd_vars1,
        x14_after: turnrun_x14,
        facing_after: facing_dir,
        fn_800ca644_called,
        wait_called,
    }
}

fn compare_anim(
    cmd_vars1: i32,
    turnrun_x14: i32,
    entry_facing: f32,
    facing_dir: f32,
    gr_vel: f32,
    is_frames_remaining: bool,
    fn_800ca644_result: bool,
) -> AnimMirror {
    let expected = mirror_turn_run_anim(
        cmd_vars1,
        turnrun_x14,
        entry_facing,
        facing_dir,
        gr_vel,
        is_frames_remaining,
        fn_800ca644_result,
    );
    // SAFETY: seven scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe {
        oracle_turn_run_anim(
            cmd_vars1,
            turnrun_x14,
            entry_facing,
            facing_dir,
            gr_vel,
            i32::from(is_frames_remaining),
            i32::from(fn_800ca644_result),
        )
    };
    assert_eq!(actual.set_rate_called != 0, expected.set_rate_called);
    if expected.set_rate_called {
        exact(actual.rate, expected.rate);
    }
    assert_eq!(actual.cmd_vars1_after, expected.cmd_vars1_after);
    assert_eq!(actual.x14_after, expected.x14_after);
    exact(actual.facing_after, expected.facing_after);
    assert_eq!(actual.fn_800ca644_called != 0, expected.fn_800ca644_called);
    assert_eq!(actual.wait_called != 0, expected.wait_called);
    expected
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Single-frame coverage over arbitrary binary32 `gr_vel` (NaN-safe:
    /// every comparison with NaN is false in both Rust and C, matching the
    /// source's own `<=`).
    #[test]
    fn arbitrary_turn_run_anim(
        cmd_vars1 in prop_oneof![Just(0i32), Just(1i32)],
        turnrun_x14 in prop_oneof![Just(0i32), Just(1i32)],
        entry_facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        gr_vel in any::<u32>().prop_map(f32::from_bits),
        is_frames_remaining in any::<bool>(),
        fn_800ca644_result in any::<bool>(),
    ) {
        compare_anim(
            cmd_vars1,
            turnrun_x14,
            entry_facing,
            facing_dir,
            gr_vel,
            is_frames_remaining,
            fn_800ca644_result,
        );
    }

    /// A generated multi-frame sequence per entry facing: the marker sets on
    /// frame 0 (freeze), then a generated velocity sequence drives the
    /// resume/flip decision frame by frame, feeding each call's
    /// `cmd_vars1_after`/`x14_after`/`facing_after` into the next -- the
    /// same threading `game::locomotion::update_animation`'s `Action::
    /// RunTurn` arm performs across real frames.
    #[test]
    fn generated_velocity_sequences_and_both_facings_match_c(
        entry_facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        velocities in prop::array::uniform8(-4.0f32..4.0f32),
    ) {
        let mut cmd_vars1 = 1;
        let mut x14 = 0;
        let mut facing = entry_facing;
        for &gr_vel in &velocities {
            let result = compare_anim(cmd_vars1, x14, entry_facing, facing, gr_vel, true, false);
            cmd_vars1 = result.cmd_vars1_after;
            x14 = result.x14_after;
            facing = result.facing_after;
        }
    }
}

#[test]
fn exact_boundaries() {
    // The marker frame itself: freeze regardless of velocity/facing.
    compare_anim(1, 0, 1.0, 1.0, 4.0, true, false);
    compare_anim(1, 0, -1.0, -1.0, -4.0, true, false);
    // Waiting, condition exactly at the boundary (<=): resumes and flips.
    compare_anim(1, 1, 1.0, 1.0, 0.01, true, false);
    compare_anim(1, 1, 1.0, 1.0, 0.01_f32.next_up(), true, false);
    // Negative entry facing: the sign of the check flips too.
    compare_anim(1, 1, -1.0, -1.0, -0.01, true, false);
    compare_anim(1, 1, -1.0, -1.0, -0.01_f32.next_down(), true, false);
    // Marker inactive: SetAnimRate is never called, facing untouched.
    compare_anim(0, 0, 1.0, 1.0, -4.0, true, false);
    // NaN ground velocity: the resume/flip check is always false.
    compare_anim(1, 1, 1.0, 1.0, f32::NAN, true, false);
    // Animation-end branch, independent of the marker: frames remaining
    // suppresses both fn_800CA644 and the Wait fallback.
    compare_anim(0, 0, 1.0, 1.0, 1.0, true, false);
    // No frames remaining, fn_800CA644 succeeds: no Wait fallback.
    compare_anim(0, 0, 1.0, 1.0, 1.0, false, true);
    // No frames remaining, fn_800CA644 fails: Wait fallback fires.
    compare_anim(0, 0, 1.0, 1.0, 1.0, false, false);
}

#[test]
fn turn_run_enter_reports_the_literal_field_assignments() {
    for (anim_start, facing_dir) in [
        (0.0f32, 1.0f32),
        (3.5, -1.0),
        (f32::NAN, f32::INFINITY),
        (-0.0, 0.0),
    ] {
        // SAFETY: two scalar inputs; the adapter owns all thread-local state.
        let result = unsafe { oracle_turn_run_enter(anim_start, facing_dir) };
        exact(result.start, anim_start);
        assert_eq!(result.speed, 1.0);
        assert_eq!(result.cmd_vars1_after, 0);
        exact(result.accel_mul_after, facing_dir);
        assert_eq!(result.x14_after, 0);
        // The captured motion id/flags are some fixed constants every call.
        let baseline = unsafe { oracle_turn_run_enter(0.0, 0.0) };
        assert_eq!(result.msid, baseline.msid);
        assert_eq!(result.flags, baseline.flags);
    }
}

fn compare_check(
    entering: bool,
    lstick_x: f32,
    facing_dir: f32,
    threshold: f32,
    cur_anim_frame: f32,
) {
    let expected_return = lstick_x * facing_dir <= threshold;
    // SAFETY: four scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe {
        if entering {
            oracle_fn_800c9cec(lstick_x, facing_dir, threshold, cur_anim_frame)
        } else {
            oracle_fn_800c9d40(lstick_x, facing_dir, threshold, cur_anim_frame)
        }
    };
    assert_eq!(actual.returned != 0, expected_return);
    assert_eq!(actual.entered != 0, expected_return);
    if expected_return {
        let expected_start = if entering { cur_anim_frame } else { 0.0 };
        exact(actual.start_used, expected_start);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `fn_800C9CEC`/`fn_800C9D40` (`ftCo_TurnRun.c:19-42`): identical
    /// `lstick.x * facing_dir <= x38` gate; only the entered `anim_start`
    /// differs (`cur_anim_frame` versus a fixed `0.0F`).
    #[test]
    fn arbitrary_turn_run_checks(
        lstick_x in -2.0f32..2.0f32,
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        threshold in -2.0f32..2.0f32,
        cur_anim_frame in 0.0f32..40.0f32,
    ) {
        compare_check(true, lstick_x, facing_dir, threshold, cur_anim_frame);
        compare_check(false, lstick_x, facing_dir, threshold, cur_anim_frame);
    }
}

#[test]
fn check_boundaries() {
    // Exactly at the threshold (<=): both enter.
    compare_check(true, 0.3, 1.0, 0.3, 5.0);
    compare_check(false, 0.3, 1.0, 0.3, 5.0);
    compare_check(true, 0.3_f32.next_down(), 1.0, 0.3, 5.0);
    compare_check(true, f32::NAN, 1.0, 0.3, 5.0);
}

#[test]
fn adapter_retains_the_complete_pinned_functions() {
    let source = include_str!("oracle/original/turn_run.c");
    assert!(source.contains("void ftCo_TurnRun_Enter(Fighter_GObj* gobj, float anim_start)"));
    assert!(source.contains("void ftCo_TurnRun_Anim(Fighter_GObj* gobj)"));
    assert!(source.contains("bool fn_800C9CEC(Fighter_GObj* gobj)"));
    assert!(source.contains("bool fn_800C9D40(Fighter_GObj* gobj)"));
    assert!(include_str!("oracle/turn_run.c").contains("#include \"turn_run_original.inc\""));
}
