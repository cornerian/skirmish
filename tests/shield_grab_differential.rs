//! Shield-grab input checked against the complete pinned C dispatchers
//! (`ftCo_Catch_CheckInput` and `ftCo_800D8B9C`).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::grab::{dash_shield_grab, shield_grab};

const HSD_PAD_A: u32 = 0x100;
const HSD_PAD_LR: u32 = 0x60;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_shield_grab(held: u32, pressed: u32, motion: *mut i32) -> i32;
    fn oracle_dash_shield_grab(
        pressed: u32,
        buffer: f32,
        buffer_after: *mut f32,
        motion: *mut i32,
    ) -> i32;
}

fn compare_shield_grab(held: u32, pressed: u32) {
    let actual = shield_grab(held & HSD_PAD_LR != 0, pressed & HSD_PAD_A != 0);
    let mut motion = 0;
    // SAFETY: the adapter owns all C state; the out-pointer is a live integer.
    let expected = unsafe { oracle_shield_grab(held, pressed, &mut motion) };
    assert_eq!(actual, expected != 0);
    assert_eq!(motion, if actual { 212 } else { 0 });
}

fn compare_dash_shield_grab(pressed: u32, buffer: f32) {
    let mut actual_buffer = buffer;
    let actual = dash_shield_grab(pressed & HSD_PAD_A != 0, &mut actual_buffer);
    let (mut expected_buffer, mut motion) = (0.0_f32, 0);
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    let expected =
        unsafe { oracle_dash_shield_grab(pressed, buffer, &mut expected_buffer, &mut motion) };
    assert_eq!(actual, expected != 0);
    assert_eq!(motion, if actual { 214 } else { 0 });
    // NaN payload propagation through the buffer's `-= 1.0` arithmetic is
    // unspecified, so two NaN results are equivalent for parity purposes
    // even when their bit patterns differ.
    if expected_buffer.is_nan() {
        assert!(actual_buffer.is_nan());
    } else {
        assert_eq!(actual_buffer.to_bits(), expected_buffer.to_bits());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_button_words_match(held in any::<u32>(), pressed in any::<u32>()) {
        compare_shield_grab(held, pressed);
    }

    #[test]
    fn arbitrary_buffers_match(pressed in any::<u32>(), buffer in any::<u32>()) {
        compare_dash_shield_grab(pressed, f32::from_bits(buffer));
    }
}

#[test]
fn adapter_retains_the_complete_source_functions_and_boundaries() {
    let source = include_str!("oracle/original/catch.c");
    let adapter = include_str!("oracle/catch.c");
    assert!(source.contains("bool ftCo_Catch_CheckInput(Fighter_GObj* gobj)"));
    assert!(source.contains("bool ftCo_800D8B9C(Fighter_GObj* gobj)"));
    assert!(adapter.contains("#include \"catch_original.inc\""));
    for (held, pressed) in [
        (HSD_PAD_LR, HSD_PAD_A),
        (0x20, HSD_PAD_A),
        (0x40, HSD_PAD_A),
        (HSD_PAD_LR, 0),
        (0, HSD_PAD_A),
        (HSD_PAD_A, HSD_PAD_LR),
    ] {
        compare_shield_grab(held, pressed);
    }
    for (pressed, buffer) in [
        (HSD_PAD_A, 3.0),
        (HSD_PAD_A, 0.0),
        (HSD_PAD_A, -0.0),
        (0, 1.0),
        (0, 0.5),
        (0, -1.0),
        (HSD_PAD_A, f32::NAN),
        (0, f32::NAN),
        (0, f32::INFINITY),
    ] {
        compare_dash_shield_grab(pressed, buffer);
    }
}
