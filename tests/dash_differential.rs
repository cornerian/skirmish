//! Dash-phase and AttackDash arithmetic checked against the pinned
//! `ftCo_Dash.c` and `ftCo_AttackDash.c` C, plus the already-pinned
//! `ftCommon_ApplyFrictionGround` (shared with the `locomotion_common` and
//! `physics` adapters) that `apply_friction` mirrors.
//!
//! `oracle_dash_frame` traces `ftCo_Dash_IASA`'s complete dispatch order: the
//! pinned catch (`ftCo_800D8A38`), `ftCo_Dash_CheckInput` and guard
//! (`ftCo_80091AD8`/`ftCo_80091A4C`/`ftCo_80091B9C`) bodies are compiled
//! under renamed symbols and reintroduced through logging wrappers (see
//! `tests/oracle/dash.c`); every other callee (SpecialS, the forward-smash/
//! roll/taunt/jump/run checks, and `ftCo_AttackDash_CheckInput`, which is
//! wrapped the same way for a uniform pattern) is a scripted-answer stub.
//! `expected_calls` mirrors that exact order in Rust from
//! `docs/dash-attack.md`'s description; `dash_frame_matches_call_order`
//! compares both the trace and the observable outcome (motion/guard_action/
//! gr_vel) over random phases, boundary frames and answer combinations.
//! With no taunt (`ftCo_800DE9D8`, unmodeled and scripted false everywhere
//! else), `block_42` always returns without touching `gr_vel`, so an idle
//! frame leaves it unchanged; a taunt is the only way to reach the friction
//! tail. The full per-branch dispatch order is also covered end-to-end
//! against the Rust implementation in `tests/game_dash.rs`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::dash::{apply_friction, attack_dash_grab, dash_forward_smash};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_dash_check_input(
        rules: *const f32,
        stick_x: f32,
        facing: f32,
        tilt_x_age: u32,
        gr_vel: f32,
        motion: *mut i32,
        dash_x4: *mut i32,
    ) -> i32;
    fn oracle_dash_frame(
        rules: *const f32,
        state: *const f32,
        pressed: u32,
        held: u32,
        scripted: u32,
        motion: *mut i32,
        guard_action: *mut i32,
        gr_vel_after: *mut f32,
        dash_x4_after: *mut i32,
        out_calls: *mut u8,
        out_count: *mut u8,
        out_fired: *mut u8,
    ) -> i32;
    fn oracle_attack_dash_frame(
        lr_held: u32,
        buffer: f32,
        allow_interrupt: i32,
        buffer_after: *mut f32,
        wait_opened: *mut i32,
    ) -> i32;
    fn oracle_physics_step(state: *mut f32, operation: u32, args: *const f32);
}

const MS_DASH: i32 = 20;
const TURN_SENTINEL: i32 = -1;

fn compare_check_input(stick_x: f32, facing: f32, tilt_x_age: u8, threshold: f32, window: u8) {
    let rules = [threshold, f32::from(window)];
    let (mut motion, mut dash_x4) = (0, 0);
    // SAFETY: the adapter owns all C state; every out-pointer is live.
    let fired = unsafe {
        oracle_dash_check_input(
            rules.as_ptr(),
            stick_x,
            facing,
            u32::from(tilt_x_age),
            0.0,
            &mut motion,
            &mut dash_x4,
        )
    };
    let fresh = stick_x.abs() >= threshold && u32::from(tilt_x_age) < u32::from(window);
    if !fresh {
        assert_eq!(fired, 0);
        assert_eq!(motion, 0);
        return;
    }
    assert_eq!(fired, 1);
    if stick_x * facing < 0.0 {
        assert_eq!(motion, TURN_SENTINEL);
        assert_eq!(dash_x4, 0);
    } else {
        assert_eq!(motion, MS_DASH);
        assert_eq!(dash_x4, 1);
    }
}

fn compare_attack_dash_grab(lr_held: bool, buffer: f32) {
    let mut actual_buffer = buffer;
    let actual = attack_dash_grab(lr_held, &mut actual_buffer);
    let (mut buffer_after, mut wait_opened) = (0.0, 0);
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    let expected = unsafe {
        oracle_attack_dash_frame(
            lr_held as u32,
            buffer,
            0,
            &mut buffer_after,
            &mut wait_opened,
        )
    };
    assert_eq!(actual, expected != 0);
    assert_eq!(actual_buffer.to_bits(), buffer_after.to_bits());
    // allow_interrupt is false (0) here, so the Wait chain never opens
    // regardless of the catch outcome.
    assert_eq!(wait_opened, 0);
}

fn compare_wait_chain_exposure(catch_fires: bool, allow_interrupt: bool) {
    let buffer = if catch_fires { 1.0 } else { 0.0 };
    let (mut buffer_after, mut wait_opened) = (0.0, 0);
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    unsafe {
        oracle_attack_dash_frame(
            catch_fires as u32,
            buffer,
            allow_interrupt as i32,
            &mut buffer_after,
            &mut wait_opened,
        );
    }
    assert_eq!(wait_opened != 0, !catch_fires && allow_interrupt);
}

/// NaN payload propagation through arithmetic is unspecified, so two NaN
/// results are equivalent for parity purposes even when their bit patterns
/// differ; a non-NaN result must still match exactly.
fn same_bits(label: &str, actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

fn compare_apply_friction(gr_vel: f32, amount: f32) {
    let actual = apply_friction(gr_vel, amount);
    let mut state = [0.0_f32; 23];
    state[6] = gr_vel;
    let args = [amount, 0.0, 0.0, 0.0];
    // SAFETY: the adapter owns all C state; `state` is a live 23-element array.
    unsafe { oracle_physics_step(state.as_mut_ptr(), 0, args.as_ptr()) };
    let expected = state[6] + state[7];
    same_bits(
        &format!("gr_vel {gr_vel} amount {amount}"),
        actual,
        expected,
    );
}

struct DashFrameResult {
    motion: i32,
    guard_action: i32,
    gr_vel_after: f32,
    /// Not read by any comparison here; `oracle_dash_check_input`'s own
    /// `dash_x4` output already covers `ftCo_Dash_CheckInput`'s side effect.
    #[allow(dead_code)]
    dash_x4_after: i32,
    calls: Vec<u8>,
    fired: u8,
}

#[allow(clippy::too_many_arguments)]
fn raw_oracle_dash_frame(
    rules: &[f32; 8],
    state: &[f32; 9],
    pressed: u32,
    held: u32,
    scripted: u32,
) -> DashFrameResult {
    let (mut motion, mut guard_action, mut gr_vel_after, mut dash_x4_after) = (0, 0, 0.0, 0);
    let mut out_calls = [0u8; 16];
    let (mut out_count, mut out_fired) = (0u8, 0u8);
    // SAFETY: the adapter owns all C state; every out-pointer is live, and
    // `out_calls` has the 16-entry capacity the adapter documents.
    unsafe {
        oracle_dash_frame(
            rules.as_ptr(),
            state.as_ptr(),
            pressed,
            held,
            scripted,
            &mut motion,
            &mut guard_action,
            &mut gr_vel_after,
            &mut dash_x4_after,
            out_calls.as_mut_ptr(),
            &mut out_count,
            &mut out_fired,
        );
    }
    DashFrameResult {
        motion,
        guard_action,
        gr_vel_after,
        dash_x4_after,
        calls: out_calls[..out_count as usize].to_vec(),
        fired: out_fired,
    }
}

/// Call-order trace codes, matching the enum in `tests/oracle/dash.c`.
mod call_code {
    pub const SPECIAL_S: u8 = 1;
    pub const CATCH: u8 = 2;
    pub const ATTACK_S4: u8 = 3;
    pub const ROLL: u8 = 4;
    pub const ATTACKDASH_CHECKINPUT: u8 = 5;
    pub const DASH_CHECKINPUT: u8 = 6;
    pub const SHIELD_AD8: u8 = 7;
    pub const SHIELD_A4C: u8 = 8;
    pub const SHIELD_B9C: u8 = 9;
    pub const TAUNT: u8 = 10;
    pub const JUMP_CAF78: u8 = 11;
    pub const RUN_CA5F0: u8 = 12;
    pub const FRICTION: u8 = 13;
}

fn ans(answers: u32, code: u8) -> bool {
    answers & (1u32 << code) != 0
}

/// `ftCo_Dash_IASA`'s complete dispatch order (`docs/dash-attack.md`), given
/// the same construction the C adapter uses for the pinned catch/AttackDash/
/// shield bodies: `held_lr = CATCH || SHIELD_AD8 || SHIELD_A4C` answers,
/// `pressed_a = CATCH || ATTACKDASH_CHECKINPUT` answers (both bitmask bits,
/// not raw button state), so `ftCo_800D8A38`'s real `(held&LR)&&(pressed&A)`
/// gate and `ftCo_80091AD8`/`ftCo_80091A4C`'s real `held&LR&&shield_health`
/// gate (health is always nonzero here) come out exactly as those answers
/// intend, including their natural coupling (`CATCH` alone always satisfies
/// both halves of its own gate). `ftCo_Dash_CheckInput`'s real predicate
/// (`stick_x`/`facing`/`tilt_x_age` against the shared dash-smash threshold/
/// window) decides both the middle-phase dash-back (only attempted when
/// `stick_x * facing < 0`) and the late-phase re-dash (always attempted).
#[allow(clippy::too_many_arguments)]
fn expected_calls(
    from_input: bool,
    frame: f32,
    x44: f32,
    x48: f32,
    x4c: f32,
    dash_threshold: f32,
    dash_window: u32,
    stick_x: f32,
    facing: f32,
    tilt_x_age: u32,
    cmd_vars0: i32,
    answers: u32,
) -> (Vec<u8>, u8) {
    use call_code::*;
    let want_catch = ans(answers, CATCH);
    let want_attackdash = ans(answers, ATTACKDASH_CHECKINPUT);
    let want_shield_ad8 = ans(answers, SHIELD_AD8);
    let want_shield_a4c = ans(answers, SHIELD_A4C);
    let held_lr = want_catch || want_shield_ad8 || want_shield_a4c;
    let pressed_a = want_catch || want_attackdash;
    let catch_fires = held_lr && pressed_a;
    let checkinput_fires = stick_x.abs() >= dash_threshold && tilt_x_age < dash_window;

    let mut calls = Vec::new();
    let block_42 = |calls: &mut Vec<u8>| -> u8 {
        calls.push(TAUNT);
        if ans(answers, TAUNT) {
            calls.push(FRICTION);
            return TAUNT;
        }
        calls.push(JUMP_CAF78);
        if ans(answers, JUMP_CAF78) {
            return JUMP_CAF78;
        }
        if cmd_vars0 == 0 {
            return 0;
        }
        calls.push(RUN_CA5F0);
        if ans(answers, RUN_CA5F0) {
            return RUN_CA5F0;
        }
        0
    };

    let fired = if from_input && frame <= x44 {
        // Early.
        calls.push(SPECIAL_S);
        if ans(answers, SPECIAL_S) {
            calls.push(FRICTION);
            SPECIAL_S
        } else {
            calls.push(CATCH);
            if catch_fires {
                CATCH
            } else {
                calls.push(ATTACK_S4);
                if ans(answers, ATTACK_S4) {
                    calls.push(FRICTION);
                    ATTACK_S4
                } else if frame <= x48 {
                    calls.push(ROLL);
                    if ans(answers, ROLL) {
                        calls.push(FRICTION);
                        ROLL
                    } else {
                        block_42(&mut calls)
                    }
                } else {
                    block_42(&mut calls)
                }
            }
        }
    } else if frame <= x4c {
        // Middle.
        calls.push(SPECIAL_S);
        if ans(answers, SPECIAL_S) {
            calls.push(FRICTION);
            SPECIAL_S
        } else {
            calls.push(CATCH);
            if catch_fires {
                CATCH
            } else {
                calls.push(ATTACKDASH_CHECKINPUT);
                if pressed_a {
                    ATTACKDASH_CHECKINPUT
                } else {
                    let dash_back_fired = if stick_x * facing < 0.0 {
                        calls.push(DASH_CHECKINPUT);
                        checkinput_fires
                    } else {
                        false
                    };
                    if dash_back_fired {
                        calls.push(FRICTION);
                        DASH_CHECKINPUT
                    } else {
                        calls.push(SHIELD_AD8);
                        if held_lr {
                            calls.push(FRICTION);
                            SHIELD_AD8
                        } else {
                            block_42(&mut calls)
                        }
                    }
                }
            }
        }
    } else {
        // Late.
        calls.push(CATCH);
        if catch_fires {
            CATCH
        } else {
            calls.push(DASH_CHECKINPUT);
            if checkinput_fires {
                calls.push(FRICTION);
                DASH_CHECKINPUT
            } else {
                calls.push(SHIELD_A4C);
                if held_lr {
                    calls.push(SHIELD_B9C);
                    calls.push(FRICTION);
                    SHIELD_A4C
                } else {
                    block_42(&mut calls)
                }
            }
        }
    };
    (calls, fired)
}

#[allow(clippy::too_many_arguments)]
fn compare_call_order(
    from_input: bool,
    frame: f32,
    stick_x: f32,
    facing: f32,
    tilt_x_age: u8,
    cmd_vars0: i32,
    answers: u32,
) {
    let (x44, x48, x4c, dash_threshold, dash_window) = (3.0_f32, 2.0_f32, 6.0_f32, 0.8_f32, 4u32);
    let want_catch = ans(answers, call_code::CATCH);
    let want_attackdash = ans(answers, call_code::ATTACKDASH_CHECKINPUT);
    let want_shield_ad8 = ans(answers, call_code::SHIELD_AD8);
    let want_shield_a4c = ans(answers, call_code::SHIELD_A4C);
    let held = if want_catch || want_shield_ad8 || want_shield_a4c {
        0x60
    } else {
        0
    };
    let pressed = if want_catch || want_attackdash {
        0x100
    } else {
        0
    };

    let rules = [
        x44,
        x48,
        x4c,
        0.0,
        5.0,
        dash_threshold,
        dash_window as f32,
        0.0,
    ];
    let state = [
        if from_input { 1.0 } else { 0.0 },
        frame,
        1.5, // gr_vel: arbitrary, only its bit pattern is echoed back untouched
        // unless the friction tail fires (excluded: taunt stays false).
        facing,
        stick_x,
        f32::from(tilt_x_age),
        255.0, // trigger_analog_timer: stale, so the powershield path never fires
        cmd_vars0 as f32,
        1.0, // shield_health: nonzero, so the held-shoulder path can fire
    ];
    let result = raw_oracle_dash_frame(&rules, &state, pressed, held, answers);
    let (expected, expected_fired) = expected_calls(
        from_input,
        frame,
        x44,
        x48,
        x4c,
        dash_threshold,
        dash_window,
        stick_x,
        facing,
        u32::from(tilt_x_age),
        cmd_vars0,
        answers,
    );
    assert_eq!(
        result.calls, expected,
        "from_input {from_input} frame {frame} stick_x {stick_x} facing {facing} \
         tilt_x_age {tilt_x_age} cmd_vars0 {cmd_vars0} answers {answers:#x}"
    );
    assert_eq!(result.fired, expected_fired);
    // x54 (rules[3]) is 0.0, so even when a taunt reaches the friction tail
    // the deceleration is a no-op: gr_vel is unchanged either way.
    assert_eq!(result.gr_vel_after.to_bits(), state[2].to_bits());
}

/// Boundary frames for the phase gate, run through the complete pinned
/// `ftCo_Dash_IASA` with every non-pinned callee scripted to "did not fire"
/// (SpecialS false, item disabled, the dash-back/attack-dash/roll/smash/run
/// checks all false, and the taunt check `ftCo_800DE9D8` false, since taunts
/// are unmodeled) so the only possible outcome is that nothing fires.
/// Reading `block_42` literally: whenever the taunt check is false, it
/// always returns (via one of its `RETURN_IF`s or the unconditional
/// `return` at the end of that block, since a Dash instance's `cmd_vars[0]`
/// is fixed at 0 by `ftCo_Dash_Enter`) without ever touching `gr_vel`; the
/// `x54` friction tail is reachable only past a taunt entry
/// (`ftCo_800DE9D8`/`ftCo_800DE9B8`, D-pad up, `ftCo_AppealS.c:35,43`).
fn compare_dash_frame_boundary(from_input: bool, frame: f32, gr_vel: f32) {
    let x44 = 3.0_f32;
    let x4c = 6.0_f32;
    // x44, x48, x4C, x54, x68, dash_smash_stick_threshold, dash_smash_window,
    // powershield_input_window (x54 and the last are unused on this path: no
    // L/R held below, and the friction tail is never reached).
    let rules = [x44, 2.0, x4c, 0.0, 5.0, 0.8, 4.0, 0.0];
    let state = [
        if from_input { 1.0 } else { 0.0 },
        frame,
        gr_vel,
        1.0,   // facing
        0.0,   // stick_x (neutral: no dash-back, no forward smash/roll)
        255.0, // tilt_x_age (stale)
        255.0, // trigger_analog_timer
        0.0,   // cmd_vars0
        0.0,   // shield_health (also keeps the guard branch closed)
    ];
    let result = raw_oracle_dash_frame(&rules, &state, 0, 0, 0);
    // With every callee closed (including the unmodeled taunt), nothing
    // fires and gr_vel is left exactly as it was.
    assert_eq!(result.motion, 0);
    assert_eq!(result.guard_action, 0);
    assert_eq!(result.gr_vel_after.to_bits(), gr_vel.to_bits());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn check_input_matches(
        stick_x in -1.5_f32..1.5,
        facing in prop::sample::select(vec![1.0_f32, -1.0]),
        tilt_x_age in 0u8..=255,
        threshold in 0.1_f32..1.0,
        window in 1u8..=20,
    ) {
        compare_check_input(stick_x, facing, tilt_x_age, threshold, window);
    }

    #[test]
    fn attack_dash_grab_matches(lr_held in any::<bool>(), buffer in -5.0_f32..10.0) {
        compare_attack_dash_grab(lr_held, buffer);
    }

    #[test]
    fn apply_friction_matches(gr_vel in -10.0_f32..10.0, amount in -10.0_f32..10.0) {
        compare_apply_friction(gr_vel, amount);
    }

    #[test]
    fn apply_friction_matches_arbitrary_bits(gr_vel in any::<u32>(), amount in any::<u32>()) {
        compare_apply_friction(f32::from_bits(gr_vel), f32::from_bits(amount));
    }

    #[test]
    fn dash_frame_boundary_leaves_gr_vel_unchanged(
        from_input in any::<bool>(),
        gr_vel in -8.0_f32..8.0,
    ) {
        for frame in [0.0, 1.0, 2.0, 3.0, 4.0, 6.0, 7.0, 12.0] {
            compare_dash_frame_boundary(from_input, frame, gr_vel);
        }
    }

    #[test]
    fn dash_frame_matches_call_order(
        from_input in any::<bool>(),
        frame in prop_oneof![
            0.0_f32..12.0,
            Just(0.0_f32), Just(2.0_f32), Just(3.0_f32), Just(6.0_f32), Just(12.0_f32),
        ],
        stick_x in -1.5_f32..1.5,
        facing in prop::sample::select(vec![1.0_f32, -1.0]),
        tilt_x_age in 0u8..=10,
        cmd_vars0 in 0i32..=2,
        answers in any::<u32>(),
    ) {
        compare_call_order(from_input, frame, stick_x, facing, tilt_x_age, cmd_vars0, answers);
    }
}

#[test]
fn call_order_boundary_cases() {
    let (x44, x48, x4c) = (3.0_f32, 2.0_f32, 6.0_f32);
    for from_input in [true, false] {
        for &frame in &[0.0, x48, x48 + 1.0, x44, x44 + 1.0, x4c, x4c + 1.0, 12.0] {
            for &answers in &[
                0u32,
                1 << call_code::SPECIAL_S,
                1 << call_code::CATCH,
                1 << call_code::ATTACK_S4,
                1 << call_code::ROLL,
                1 << call_code::ATTACKDASH_CHECKINPUT,
                1 << call_code::SHIELD_AD8,
                1 << call_code::SHIELD_A4C,
                1 << call_code::TAUNT,
                1 << call_code::JUMP_CAF78,
                1 << call_code::RUN_CA5F0,
                (1 << call_code::CATCH) | (1 << call_code::SHIELD_AD8),
            ] {
                for &(stick_x, facing) in &[(0.8, 1.0), (-0.8, 1.0), (0.0, 1.0), (0.8, -1.0)] {
                    for cmd_vars0 in [0, 1] {
                        compare_call_order(
                            from_input, frame, stick_x, facing, 0, cmd_vars0, answers,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn dash_forward_smash_predicate_has_no_age_window() {
    // Pure-arithmetic parity check against the `checkFacingDir` formula
    // (`ftCo_AttackS4_8008C114`), which the pinned attack_dash.c snapshot
    // below is checked to still contain.
    assert!(dash_forward_smash(true, 0.8, 1.0, 0.8));
    assert!(!dash_forward_smash(true, 0.79, 1.0, 0.8));
    assert!(!dash_forward_smash(false, 1.0, 1.0, 0.8));
}

#[test]
fn boundary_frames_and_directions() {
    let rules = [0.8_f32, 4.0];
    for (stick_x, facing, tilt_x_age) in [
        (0.8, 1.0, 0u8),
        (0.8, 1.0, 3),
        (0.8, 1.0, 4),
        (-0.8, 1.0, 0),
        (0.79, 1.0, 0),
        (1.0, -1.0, 0),
    ] {
        compare_check_input(stick_x, facing, tilt_x_age, rules[0], rules[1] as u8);
    }
    for buffer in [0.0, 1.0, -0.5, f32::NAN] {
        compare_attack_dash_grab(true, buffer);
        compare_attack_dash_grab(false, buffer);
    }
    for allow_interrupt in [true, false] {
        for catch_fires in [true, false] {
            compare_wait_chain_exposure(catch_fires, allow_interrupt);
        }
    }
    for (gr_vel, amount) in [
        (5.0, 2.0),
        (-5.0, 2.0),
        (1.0, 5.0),
        (-1.0, 5.0),
        (0.0, 0.0),
        (f32::NAN, 1.0),
        (1.0, f32::NAN),
    ] {
        compare_apply_friction(gr_vel, amount);
    }
    for from_input in [true, false] {
        for frame in [0.0, 3.0, 4.0, 6.0, 7.0] {
            compare_dash_frame_boundary(from_input, frame, 1.5);
        }
    }
}

#[test]
fn snapshots_contain_the_pinned_function_signatures() {
    let dash = include_str!("oracle/original/dash.c");
    let attack_dash = include_str!("oracle/original/attack_dash.c");
    for signature in [
        "bool ftCo_Dash_CheckInput(Fighter_GObj* gobj)",
        "void ftCo_Dash_Enter(Fighter_GObj* gobj, int arg1)",
        "void ftCo_Dash_IASA(Fighter_GObj* gobj)",
    ] {
        assert!(dash.contains(signature), "missing {signature}");
    }
    for signature in [
        "bool ftCo_AttackDash_CheckInput(HSD_GObj* gobj)",
        "static void decideFighter(Fighter_GObj* gobj)",
        "static void doEnter(Fighter_GObj* gobj)",
        "void ftCo_AttackDash_SetMv0(HSD_GObj* gobj)",
        "void ftCo_AttackDash_IASA(Fighter_GObj* gobj)",
        "void ftCo_AttackDash_Phys(HSD_GObj* gobj)",
    ] {
        assert!(attack_dash.contains(signature), "missing {signature}");
    }
    let catch = include_str!("oracle/original/catch.c");
    assert!(catch.contains("bool ftCo_800D8A38(Fighter_GObj* gobj)"));
    assert!(catch.contains("bool ftCo_800D8AE0(Fighter_GObj* gobj)"));
    let guard = include_str!("oracle/original/shield_guard.c");
    assert!(guard.contains("bool ftCo_80091AD8(Fighter_GObj* gobj, int mv_x20)"));
    assert!(guard.contains("bool ftCo_80091A4C(Fighter_GObj* gobj)"));
    assert!(guard.contains("void ftCo_80091B9C(Fighter_GObj* gobj)"));
    let adapter = include_str!("oracle/dash.c");
    assert!(adapter.contains("#include \"dash_original.inc\""));
    assert!(adapter.contains("#include \"attack_dash_original.inc\""));
    assert!(adapter.contains("#include \"dash_catch_original.inc\""));
    assert!(adapter.contains("#include \"dash_guard_original.inc\""));
}
