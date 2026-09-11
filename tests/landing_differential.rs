//! `ftCo_Landing_IASA`'s complete dispatch order and the entry family's
//! motion/allow/rate arithmetic, checked against the pinned C bodies. This is
//! a Rust mirror of the exact per-call protocol `oracle_landing_iasa` drives
//! (see `tests/oracle/landing.c`), independent of `skirmish::game::landing`
//! (which only implements the two frame gates the Rust port actually needs;
//! this file checks the complete chain, including every attack/movement
//! callee this batch does not otherwise exercise).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_landing_iasa(
        frame: f32,
        rate: f32,
        lag: f32,
        allow: i32,
        answers: u32,
        out_calls: *mut u8,
        out_count: *mut u8,
    ) -> i32;
    fn oracle_landing_enter(
        kind_of_entry: i32,
        allow: i32,
        lag_for_fallspecial: f32,
        out_msid: *mut i32,
        out_allow: *mut i32,
        out_rate: *mut f32,
    ) -> i32;
}

// Call-order trace codes, mirroring `tests/oracle/landing.c`.
const SPECIAL_S: u8 = 1;
const ATTACK100: u8 = 2;
const CALL_800D6824: u8 = 3;
const CALL_800D68C0: u8 = 4;
const CATCH: u8 = 5;
const ATTACK_S4: u8 = 6;
const ATTACK_HI4: u8 = 7;
const ATTACK_LW4: u8 = 8;
const ATTACK_S3: u8 = 9;
const ATTACK_HI3: u8 = 10;
const ATTACK_LW3: u8 = 11;
const JAB: u8 = 12;
const SHIELD: u8 = 13;
const TAUNT: u8 = 14;
const JUMP: u8 = 15;
const DASH: u8 = 16;
const SQUAT: u8 = 17;
const TURN: u8 = 18;
const WALK: u8 = 19;

// Sentinel motion IDs, mirroring `tests/oracle/landing.c`; not claimed to be
// the authentic FtMotionId values.
const MS_LANDING: i32 = 42;
const MS_LANDING_FALL_SPECIAL: i32 = 43;

// Mirrors `tests/oracle/landing.c`'s `LANDING_X2EC`.
const LANDING_X2EC: f32 = 0.37;

fn answer(answers: u32, code: u8) -> bool {
    answers & (1u32 << code) != 0
}

/// Rust mirror of `ftCo_Landing_IASA`'s exact dispatch order
/// (`ftCo_Landing.c:122-150`): the lag and allow_interrupt gates, then
/// SpecialS, Attack100, 800D6824, 800D68C0, catch, S4, Hi4, Lw4, S3, Hi3,
/// Lw3, jab, shield, taunt, jump, dash, the frame-gated squat check, turn
/// and walk. Returns the ordered call trace and which code (0 = none) made
/// the chain return.
fn mirror_iasa(frame: f32, rate: f32, lag: f32, allow: bool, answers: u32) -> (Vec<u8>, u8) {
    let mut calls = Vec::new();
    if frame < lag {
        return (calls, 0);
    }
    if !allow {
        return (calls, 0);
    }
    let mut check = |code: u8| -> bool {
        calls.push(code);
        answer(answers, code)
    };
    for code in [
        SPECIAL_S,
        ATTACK100,
        CALL_800D6824,
        CALL_800D68C0,
        CATCH,
        ATTACK_S4,
        ATTACK_HI4,
        ATTACK_LW4,
        ATTACK_S3,
        ATTACK_HI3,
        ATTACK_LW3,
        JAB,
        SHIELD,
        TAUNT,
        JUMP,
        DASH,
    ] {
        if check(code) {
            return (calls, code);
        }
    }
    if frame < rate + lag && check(SQUAT) {
        return (calls, SQUAT);
    }
    for code in [TURN, WALK] {
        if check(code) {
            return (calls, code);
        }
    }
    (calls, 0)
}

fn compare_iasa(frame: f32, rate: f32, lag: f32, allow: bool, answers: u32) {
    let (expected_calls, expected_fired) = mirror_iasa(frame, rate, lag, allow, answers);
    let mut out_calls = [0u8; 20];
    let mut out_count = 0u8;
    // SAFETY: `out_calls` is sized at least 20, matching the adapter's own bound.
    let fired = unsafe {
        oracle_landing_iasa(
            frame,
            rate,
            lag,
            i32::from(allow),
            answers,
            out_calls.as_mut_ptr(),
            &mut out_count,
        )
    };
    assert_eq!(
        fired as u8, expected_fired,
        "frame {frame} rate {rate} lag {lag} allow {allow} answers {answers:#x}"
    );
    assert_eq!(
        &out_calls[..out_count as usize],
        expected_calls.as_slice(),
        "frame {frame} rate {rate} lag {lag} allow {allow} answers {answers:#x}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn arbitrary_landing_iasa_frames_match(
        frame in 0.0_f32..8.0,
        rate in 0.5_f32..2.0,
        lag in 0.0_f32..6.0,
        allow in any::<bool>(),
        answers in any::<u32>(),
    ) {
        compare_iasa(frame, rate, lag, allow, answers);
    }
}

#[test]
fn boundary_iasa_frames_match() {
    // frame == lag: the lag gate's strict `<` no longer blocks it.
    compare_iasa(3.0, 1.0, 3.0, true, 0);
    // frame == rate + lag: the squat check's strict `<` excludes it exactly
    // at the boundary, but the chain still reaches Turn and Walk.
    compare_iasa(4.0, 1.0, 3.0, true, 0);
    compare_iasa(4.0, 1.0, 3.0, true, 1 << u32::from(SQUAT));
    // Just inside the squat window.
    compare_iasa(3.9, 1.0, 3.0, true, 1 << u32::from(SQUAT));
    // allow_interrupt == false locks out the chain even once frame >= lag.
    compare_iasa(5.0, 1.0, 3.0, false, u32::MAX);
    // frame < lag locks out the chain regardless of allow_interrupt.
    compare_iasa(2.9, 1.0, 3.0, true, u32::MAX);
    // The first callee that answers true stops the chain immediately; later
    // callees are never even consulted.
    compare_iasa(5.0, 1.0, 3.0, true, 1 << u32::from(CATCH));
    // Every callee answers false: the chain runs to Walk and reports none.
    compare_iasa(5.0, 1.0, 3.0, true, 0);
}

fn compare_enter(kind_of_entry: i32, allow: bool, lag_for_fallspecial: f32) {
    let mut msid = 0;
    let mut out_allow = 0;
    let mut rate = 0.0;
    // SAFETY: every out-parameter is a plain scalar the adapter writes once.
    unsafe {
        oracle_landing_enter(
            kind_of_entry,
            i32::from(allow),
            lag_for_fallspecial,
            &mut msid,
            &mut out_allow,
            &mut rate,
        );
    }
    match kind_of_entry {
        0 => {
            assert_eq!(msid, MS_LANDING);
            assert_eq!(out_allow, 1);
            assert_eq!(rate.to_bits(), 1.0_f32.to_bits());
        }
        1 => {
            assert_eq!(msid, MS_LANDING_FALL_SPECIAL);
            assert_eq!(out_allow, 0);
            assert_eq!(rate.to_bits(), 1.0_f32.to_bits());
        }
        _ => {
            assert_eq!(msid, MS_LANDING_FALL_SPECIAL);
            assert_eq!(out_allow, i32::from(allow));
            let expected = (0.1_f32 + LANDING_X2EC) / lag_for_fallspecial;
            assert_eq!(
                rate.to_bits(),
                expected.to_bits(),
                "lag {lag_for_fallspecial}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_landing_fall_special_entries_match(
        allow in any::<bool>(),
        lag in 0.1_f32..10.0,
    ) {
        compare_enter(2, allow, lag);
    }
}

#[test]
fn entry_boundary_cases_match() {
    compare_enter(0, true, 1.0);
    compare_enter(0, false, 1.0);
    compare_enter(1, true, 1.0);
    compare_enter(1, false, 1.0);
    compare_enter(2, true, 1.0);
    compare_enter(2, false, 0.1);
}

#[test]
fn adapter_retains_the_complete_source_functions() {
    let landing = include_str!("oracle/original/landing.c");
    assert!(landing.contains("void ftCo_Landing_Enter(Fighter_GObj* gobj, FtMotionId msid,"));
    assert!(landing.contains("void ftCo_Landing_Enter_Basic(Fighter_GObj* gobj)"));
    assert!(landing.contains("void ftCo_LandingFallSpecial_Enter_Basic(Fighter_GObj* gobj)"));
    assert!(
        landing.contains(
            "void ftCo_LandingFallSpecial_Enter(Fighter_GObj* gobj, bool allow_interrupt,"
        )
    );
    assert!(landing.contains("void ftCo_Landing_IASA(Fighter_GObj* gobj)"));
}
