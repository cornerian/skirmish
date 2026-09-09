#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::special::neutral_input;

unsafe extern "C" {
    fn oracle_neutral_special_input(
        pressed_buttons: u32,
        stick_x: f32,
        stick_y: f32,
        horizontal_threshold: f32,
        vertical_threshold: f32,
    ) -> i32;
}

proptest! {
    #[test]
    fn neutral_special_region_matches_original_c(
        pressed_buttons in any::<u16>(),
        stick in any::<[f32; 2]>(),
        thresholds in any::<[f32; 2]>(),
    ) {
        let original = unsafe {
            oracle_neutral_special_input(
                pressed_buttons.into(),
                stick[0],
                stick[1],
                thresholds[0],
                thresholds[1],
            )
        } != 0;
        prop_assert_eq!(neutral_input(pressed_buttons, stick, thresholds), original);
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/special_input.c");
    assert!(original.contains("bool ftCo_800D67C4(Fighter* fp)\n{"));
    let adapter = include_str!("oracle/special_input.c");
    assert!(adapter.contains("#include \"special_input_original.inc\""));
}
