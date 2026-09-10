//! Grounded tilt input predicates and the forward-tilt angle selection checked
//! against the complete pinned C bodies.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::aerial::stick_angle;
use skirmish::fighter::tilt::{
    ForwardVariant, down_tilt, down_tilt_repeat, forward_tilt, forward_variant, up_tilt,
};

const HSD_PAD_A: u32 = 0x100;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_forward_tilt(
        values: *const f32,
        rules: *const f32,
        pressed: u32,
        available: u32,
        motion: *mut i32,
    ) -> i32;
    fn oracle_up_tilt(
        stick: *const f32,
        threshold: f32,
        angle_limit: f32,
        pressed: u32,
        motion: *mut i32,
    ) -> i32;
    fn oracle_down_tilt(
        stick: *const f32,
        threshold: f32,
        angle_limit: f32,
        pressed: u32,
        motion: *mut i32,
        flags: *mut i32,
    ) -> i32;
    fn oracle_down_tilt_repeat(
        pressed: u32,
        repeat_ready: i32,
        buffered: i32,
        buffer_after: *mut i32,
        motion: *mut i32,
    ) -> i32;
}

fn compare_forward(stick: [f32; 2], facing: f32, rules: [f32; 6], pressed: u32, available: u32) {
    let values = [stick[0], stick[1], facing];
    let mut motion = 0;
    // SAFETY: the adapter reads three and six floats and writes one integer.
    let expected = unsafe {
        oracle_forward_tilt(
            values.as_ptr(),
            rules.as_ptr(),
            pressed,
            available,
            &mut motion,
        )
    };
    let angle = stick_angle(stick);
    let actual = forward_tilt(
        pressed & HSD_PAD_A != 0,
        stick[0],
        facing,
        angle,
        rules[0],
        rules[1],
    );
    assert_eq!(actual, expected != 0, "{stick:?} {facing} {rules:?}");
    let expected_motion = if actual {
        match forward_variant(
            angle,
            [rules[2], rules[3], rules[4], rules[5]],
            [
                available & 1 != 0,
                available & 2 != 0,
                available & 4 != 0,
                available & 8 != 0,
            ],
        ) {
            ForwardVariant::High => 51,
            ForwardVariant::HighSlight => 52,
            ForwardVariant::Straight => 53,
            ForwardVariant::LowSlight => 54,
            ForwardVariant::Low => 55,
        }
    } else {
        0
    };
    assert_eq!(motion, expected_motion, "{stick:?} {rules:?} {available}");
}

fn compare_vertical(stick: [f32; 2], threshold: f32, limit: f32, pressed: u32) {
    let (mut up_motion, mut down_motion, mut flags) = (0, 0, 0);
    // SAFETY: the adapters read two floats and write integers.
    let (up, down) = unsafe {
        (
            oracle_up_tilt(stick.as_ptr(), threshold, limit, pressed, &mut up_motion),
            oracle_down_tilt(
                stick.as_ptr(),
                -threshold,
                limit,
                pressed,
                &mut down_motion,
                &mut flags,
            ),
        )
    };
    let angle = stick_angle(stick);
    let a = pressed & HSD_PAD_A != 0;
    assert_eq!(up_tilt(a, stick[1], angle, threshold, limit), up != 0);
    assert_eq!(up_motion, if up != 0 { 56 } else { 0 });
    assert_eq!(down_tilt(a, stick[1], angle, -threshold, limit), down != 0);
    assert_eq!(down_motion, if down != 0 { 57 } else { 0 });
    if down != 0 {
        assert_eq!(flags, 0x400, "down tilt enters with Ft_MF_SkipAttackCount");
    }
}

fn compare_repeat(pressed: u32, ready: bool, buffered: bool) {
    let mut buffer = buffered;
    let actual = down_tilt_repeat(pressed & HSD_PAD_A != 0, ready, &mut buffer);
    let (mut after, mut motion) = (0, 0);
    // SAFETY: the adapter writes two integers.
    let expected = unsafe {
        oracle_down_tilt_repeat(
            pressed,
            i32::from(ready),
            i32::from(buffered),
            &mut after,
            &mut motion,
        )
    };
    assert_eq!(actual, expected != 0);
    if actual {
        // doEnter clears the buffer; the native scheduler resets tilt state.
        assert_eq!(after, 0);
    } else {
        assert_eq!(buffer, after != 0);
    }
    assert_eq!(motion, if actual { 57 } else { 0 });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_forward_inputs_match(
        stick in prop::array::uniform2(-1.0_f32..=1.0),
        facing in prop_oneof![Just(1.0_f32), Just(-1.0_f32)],
        rules in prop::array::uniform6(-1.6_f32..=1.6),
        pressed in any::<u32>(),
        available in 0_u32..16,
    ) {
        compare_forward(stick, facing, rules, pressed, available);
    }

    #[test]
    fn arbitrary_vertical_inputs_match(
        stick in prop::array::uniform2(-1.0_f32..=1.0),
        threshold in 0.0_f32..=1.0,
        limit in 0.0_f32..=1.6,
        pressed in any::<u32>(),
    ) {
        compare_vertical(stick, threshold, limit, pressed);
    }

    #[test]
    fn arbitrary_repeat_inputs_match(pressed in any::<u32>(), ready in any::<bool>(), buffered in any::<bool>()) {
        compare_repeat(pressed, ready, buffered);
    }
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let s3 = include_str!("oracle/original/attack_s3.c");
    let hi3 = include_str!("oracle/original/attack_hi3.c");
    let lw3 = include_str!("oracle/original/attack_lw3.c");
    assert!(s3.contains("bool ftCo_AttackS3_CheckInput(Fighter_GObj* gobj)"));
    assert!(s3.contains("static void decideAngle(Fighter_GObj* gobj)"));
    assert!(hi3.contains("bool ftCo_AttackHi3_CheckInput(Fighter_GObj* gobj)"));
    assert!(lw3.contains("bool ftCo_AttackLw3_CheckInput(Fighter_GObj* gobj)"));
    assert!(lw3.contains("static bool checkPadA(Fighter_GObj* gobj)"));
    let rules = [0.5, 0.5, 0.4, 0.2, -0.2, -0.4];
    for (stick, facing) in [
        ([0.5, 0.0], 1.0),
        ([0.49, 0.0], 1.0),
        ([-0.5, 0.0], -1.0),
        ([1.0, 0.5], 1.0),
        ([1.0, 0.3], 1.0),
        ([1.0, -0.3], 1.0),
        ([1.0, -0.5], 1.0),
        ([0.6, 0.6], 1.0),
        ([-0.0, 0.0], 1.0),
        ([f32::NAN, 0.0], 1.0),
    ] {
        for available in [15, 0, 14, 7] {
            compare_forward(stick, facing, rules, HSD_PAD_A, available);
        }
    }
    compare_forward([1.0, 0.0], 1.0, rules, 0, 15);
    for stick in [
        [0.0, 0.5],
        [0.0, 0.49],
        [0.0, -0.5],
        [0.0, -0.49],
        [0.3, 0.9],
        [0.9, 0.3],
        [f32::NAN, f32::NAN],
    ] {
        compare_vertical(stick, 0.5, 0.5, HSD_PAD_A);
        compare_vertical(stick, 0.5, 0.5, 0);
    }
    compare_repeat(HSD_PAD_A, false, false);
    compare_repeat(HSD_PAD_A, true, false);
    compare_repeat(0, true, true);
}
