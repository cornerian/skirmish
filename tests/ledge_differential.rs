#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::ledge::{StickOption, stick_option};

unsafe extern "C" {
    fn oracle_ledge_option(
        main_stick: i32,
        input_ready: i32,
        stick_x: f32,
        angle: f32,
        facing: f32,
        angle_threshold: f32,
        initial_cooldown: i32,
        drop_cooldown: i32,
    ) -> u64;
}

proptest! {
    #[test]
    fn ledge_stick_region_matches_original_callback(
        main_stick in any::<bool>(),
        input_ready in any::<bool>(),
        stick_x in any::<f32>(),
        angle in any::<f32>(),
        facing in any::<f32>(),
        angle_threshold in any::<f32>(),
        initial_cooldown in any::<i32>(),
        drop_cooldown in any::<i32>(),
    ) {
        let packed = unsafe {
            oracle_ledge_option(
                main_stick.into(),
                input_ready.into(),
                stick_x,
                angle,
                facing,
                angle_threshold,
                initial_cooldown,
                drop_cooldown,
            )
        };
        let original_action = packed as u32;
        let original_cooldown = (packed >> 32) as u32 as i32;
        let rust = stick_option(
            main_stick,
            input_ready,
            stick_x,
            angle,
            facing,
            angle_threshold,
        );
        let rust_action = match rust {
            None => 0,
            Some(StickOption::Climb) => 1,
            Some(StickOption::Drop) => 2,
        };
        let rust_cooldown = if rust == Some(StickOption::Drop) {
            drop_cooldown
        } else {
            initial_cooldown
        };
        prop_assert_eq!(rust_action, original_action);
        prop_assert_eq!(rust_cooldown, original_cooldown);
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/ledge_option.c");
    assert!(original.contains(
        "bool ftCo_8009AAFC(Fighter_GObj* gobj, bool arg1, float stick_x, float angle)\n{"
    ));
    let adapter = include_str!("oracle/ledge_option.c");
    assert!(adapter.contains("#include \"ledge_option_original.inc\""));
}
