//! Shield-platform-drop input checked against complete pinned C predicates.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::locomotion::shield_drop_request;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_shield_drop_request(
        shield_held: i32,
        stick_y: f32,
        tilt_age: u8,
        threshold: f32,
        window: u8,
        on_platform: i32,
        did_enter: *mut i32,
    ) -> i32;
}

fn compare(
    shield_held: bool,
    stick_y: f32,
    tilt_age: u8,
    threshold: f32,
    window: u8,
    on_platform: bool,
) {
    let actual = shield_drop_request(
        shield_held,
        stick_y,
        tilt_age,
        threshold,
        window,
        on_platform,
    );
    let mut entered = 0;
    // SAFETY: the adapter owns all C state and `entered` is a live writable integer.
    let expected = unsafe {
        oracle_shield_drop_request(
            i32::from(shield_held),
            stick_y,
            tilt_age,
            threshold,
            window,
            i32::from(on_platform),
            &mut entered,
        )
    };
    assert_eq!(actual, expected != 0);
    assert_eq!(actual, entered != 0);
}

proptest! {
    #[test]
    fn arbitrary_inputs_match_original(
        shield_held in any::<bool>(),
        stick_bits in any::<u32>(),
        tilt_age in any::<u8>(),
        threshold_bits in any::<u32>(),
        window in any::<u8>(),
        on_platform in any::<bool>(),
    ) {
        compare(
            shield_held,
            f32::from_bits(stick_bits),
            tilt_age,
            f32::from_bits(threshold_bits),
            window,
            on_platform,
        );
    }
}

#[test]
fn adapter_retains_both_complete_source_predicates_and_boundaries() {
    let source = include_str!("oracle/original/pass.c");
    let adapter = include_str!("oracle/pass.c");
    assert!(source.contains("bool ftCo_80099F1C(Fighter_GObj* gobj)"));
    assert!(source.contains("bool ftCo_8009A080(Fighter_GObj* gobj)"));
    assert!(adapter.contains("#include \"pass_original.inc\""));
    for (stick, age, expected) in [(-0.7, 2, true), (-0.699, 2, false), (-0.7, 3, false)] {
        let mut entered = 0;
        let original =
            unsafe { oracle_shield_drop_request(1, stick, age, 0.7, 3, 1, &mut entered) };
        assert_eq!(original != 0, expected);
        assert_eq!(entered != 0, expected);
    }
}
