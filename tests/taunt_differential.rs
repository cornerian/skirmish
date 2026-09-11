//! Taunt entry selection and IASA chain order checked against the complete
//! pinned C dispatchers (`ftCo_800DE9D8`/`ftCo_800DEAE8`, `ftCo_AppealS_
//! IASA`), and the Wait-chain spot dodge predicate (`ftCo_80099794`)
//! extended onto the pinned escape adapter.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::{
    escape::main_stick_spot_dodge,
    taunt::{Side, pressed, select_side},
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_taunt_enter(
        pressed: bool,
        facing: f32,
        left_available: bool,
        out_msid: *mut i32,
        out_allow_interrupt: *mut bool,
    ) -> i32;
    fn oracle_taunt_iasa(
        allow_interrupt: bool,
        answers: u32,
        out_calls: *mut i32,
        out_count: *mut i32,
    ) -> i32;
    fn oracle_wait_spot_dodge(
        held: i32,
        stick_y: f32,
        tilt_y_age: u8,
        threshold: f32,
        window: i32,
        motion: *mut i32,
    ) -> i32;
}

const APPEAL_SR: i32 = 264;
const APPEAL_SL: i32 = 265;

fn compare_entry(pressed_dpad: bool, facing: f32, left_available: bool) {
    let (mut msid, mut allow_interrupt) = (0, true);
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    let fired = unsafe {
        oracle_taunt_enter(
            pressed_dpad,
            facing,
            left_available,
            &mut msid,
            &mut allow_interrupt,
        )
    };
    let expected_fired = pressed(u16::from(pressed_dpad) << 3, 0x8);
    assert_eq!(
        fired != 0,
        expected_fired,
        "facing {facing} left {left_available}"
    );
    if !expected_fired {
        assert_eq!(msid, 0);
        return;
    }
    let expected_side = select_side(facing, left_available);
    assert_eq!(
        msid,
        match expected_side {
            Side::Right => APPEAL_SR,
            Side::Left => APPEAL_SL,
        }
    );
    // `ftCo_800DEAE8` unconditionally sets `fp->allow_interrupt = false`
    // before the `Fighter_ChangeMotionState` call this adapter captures it
    // at.
    assert!(!allow_interrupt);
}

/// `ftCo_AppealS_IASA`'s own RETURN_IF chain, in its pinned source order
/// (`appeal.c`'s `CHECK_STUB` indices mirror this exactly).
const IASA_ORDER: [u32; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

/// Pure Rust mirror of `ftCo_AppealS_IASA`: `allow_interrupt` gates the
/// whole chain; otherwise every check in `IASA_ORDER` runs in order until
/// one answers true (its bit set in `answers`), which stops the chain.
/// Returns whether any check fired and the ordered list of checks reached.
fn mirror_iasa(allow_interrupt: bool, answers: u32) -> (bool, Vec<u32>) {
    if !allow_interrupt {
        return (false, Vec::new());
    }
    let mut calls = Vec::new();
    for check in IASA_ORDER {
        calls.push(check);
        if answers & (1 << check) != 0 {
            return (true, calls);
        }
    }
    (false, calls)
}

fn compare_iasa(allow_interrupt: bool, answers: u32) {
    let (expected_fired, expected_calls) = mirror_iasa(allow_interrupt, answers);
    let mut calls = [0i32; 16];
    let mut count = 0;
    // SAFETY: the adapter owns all C state; `calls` has the adapter's own
    // documented capacity (16), well past the 14 checks it can log.
    let fired =
        unsafe { oracle_taunt_iasa(allow_interrupt, answers, calls.as_mut_ptr(), &mut count) };
    assert_eq!(
        fired != 0,
        expected_fired,
        "allow_interrupt {allow_interrupt} answers {answers:#x}"
    );
    let actual_calls: Vec<u32> = calls[..count as usize].iter().map(|&c| c as u32).collect();
    assert_eq!(
        actual_calls, expected_calls,
        "allow_interrupt {allow_interrupt} answers {answers:#x}"
    );
}

fn compare_wait_spot_dodge(held: bool, stick_y: f32, tilt_y_age: u8, threshold: f32, window: u8) {
    let actual = held && main_stick_spot_dodge(stick_y, tilt_y_age, threshold, window);
    let mut motion = 0;
    // SAFETY: the adapter owns all C state; the out-pointer is a live integer.
    let expected = unsafe {
        oracle_wait_spot_dodge(
            i32::from(held),
            stick_y,
            tilt_y_age,
            threshold,
            i32::from(window),
            &mut motion,
        )
    };
    assert_eq!(
        actual,
        expected != 0,
        "held {held} stick_y {stick_y} age {tilt_y_age}"
    );
    assert_eq!(motion, if actual { 235 } else { 0 });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_entry_inputs_match_the_complete_dispatcher(
        pressed_dpad in any::<bool>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32), any::<u32>().prop_map(f32::from_bits)],
        left_available in any::<bool>(),
    ) {
        compare_entry(pressed_dpad, facing, left_available);
    }

    #[test]
    fn arbitrary_iasa_answer_masks_match_the_complete_dispatcher(
        allow_interrupt in any::<bool>(),
        answers in any::<u32>(),
    ) {
        compare_iasa(allow_interrupt, answers);
    }

    #[test]
    fn arbitrary_wait_spot_dodge_inputs_match_the_complete_dispatcher(
        held in any::<bool>(),
        stick in any::<u32>(),
        age in any::<u8>(),
        threshold in any::<u32>(),
        window in any::<u8>(),
    ) {
        compare_wait_spot_dodge(held, f32::from_bits(stick), age, f32::from_bits(threshold), window);
    }
}

#[test]
fn adapter_retains_the_complete_source_functions_and_boundaries() {
    let appeal = include_str!("oracle/original/appeal.c");
    let escape = include_str!("oracle/original/escape.c");
    let adapter = include_str!("oracle/appeal.c");
    let escape_adapter = include_str!("oracle/escape.c");
    for header in [
        "bool ftCo_800DE9B8(Fighter_GObj* gobj)",
        "bool ftCo_800DE9D8(Fighter_GObj* gobj)",
        "void ftCo_800DEAE8(Fighter_GObj* gobj, FtMotionId msid0, FtMotionId msid1)",
        "void ftCo_800DEBD0(Fighter_GObj* gobj)",
        "void ftCo_AppealS_IASA(Fighter_GObj* gobj)",
    ] {
        assert!(appeal.contains(header), "{header}");
    }
    assert!(escape.contains("bool ftCo_80099794(Fighter_GObj* gobj)"));
    assert!(adapter.contains("#include \"appeal_original.inc\""));
    assert!(escape_adapter.contains("#include \"escape_original.inc\""));

    // Entry boundaries: no press never fires; a press with every
    // combination of facing/availability picks the documented side.
    compare_entry(false, 1.0, true);
    compare_entry(false, -1.0, true);
    for (facing, left_available) in [
        (1.0, true),
        (1.0, false),
        (-1.0, true),
        (-1.0, false),
        (-0.0, true),
    ] {
        compare_entry(true, facing, left_available);
    }
    compare_entry(true, f32::NAN, true);

    // IASA boundaries: locked entirely; every single check as the sole
    // winner; nothing answers; every check answers (first one wins).
    compare_iasa(false, 0xffff_ffff);
    for check in 0u32..14 {
        compare_iasa(true, 1 << check);
    }
    compare_iasa(true, 0);
    compare_iasa(true, 0xffff_ffff);

    // Wait-chain spot dodge boundaries (mirrors the escape oracle's own
    // inlineB0 boundary cases, with the added shoulder-held gate).
    for (held, stick, age) in [
        (true, -0.7, 3),
        (true, -0.699, 0),
        (true, -1.0, 4),
        (false, -1.0, 0),
    ] {
        compare_wait_spot_dodge(held, stick, age, -0.7, 4);
    }
    compare_wait_spot_dodge(true, f32::NAN, 0, -0.7, 4);
}
