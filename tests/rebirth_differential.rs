#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::rebirth::{approach_velocity, wait_approach_velocity};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct OracleRebirthVelocity {
    x: f32,
    y: f32,
}

unsafe extern "C" {
    fn oracle_rebirth_velocity(
        current_x: f32,
        current_y: f32,
        target_x: f32,
        target_y: f32,
        remaining: i32,
    ) -> OracleRebirthVelocity;
    fn oracle_rebirth_wait_velocity(
        current_x: f32,
        current_y: f32,
        target_x: f32,
        target_y: f32,
        remaining: i32,
    ) -> OracleRebirthVelocity;
}

proptest! {
    #[test]
    fn remaining_frame_velocity_matches_the_original_callback(
        current in any::<[f32; 2]>(),
        target in any::<[f32; 2]>(),
        remaining in 1_i32..=i32::MAX,
    ) {
        let original = unsafe {
            oracle_rebirth_velocity(
                current[0],
                current[1],
                target[0],
                target[1],
                remaining,
            )
        };
        let rust = approach_velocity(current, target, remaining as u32);
        prop_assert_eq!(rust[0].to_bits(), original.x.to_bits());
        prop_assert_eq!(rust[1].to_bits(), original.y.to_bits());
    }


    #[test]
    fn wait_velocity_operand_order_matches_the_original_callback(
        current in any::<[f32; 2]>(),
        target in any::<[f32; 2]>(),
        remaining in 1_i32..=i32::MAX,
    ) {
        let original = unsafe {
            oracle_rebirth_wait_velocity(
                current[0],
                current[1],
                target[0],
                target[1],
                remaining,
            )
        };
        let rust = wait_approach_velocity(current, target, remaining as u32);
        prop_assert_eq!(rust[0].to_bits(), original.x.to_bits());
        prop_assert_eq!(rust[1].to_bits(), original.y.to_bits());
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/rebirth.c");
    assert!(original.contains("void ftCo_Rebirth_Phys(Fighter_GObj* gobj)\n{"));
    assert!(original.contains("void ftCo_RebirthWait_Phys(Fighter_GObj* gobj)\n{"));
    let adapter = include_str!("oracle/rebirth.c");
    assert!(adapter.contains("#include \"rebirth_original.inc\""));
}
