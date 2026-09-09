#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::fighter::damage::can_tech;

unsafe extern "C" {
    fn oracle_damage_floor_tech(
        locked: u32,
        age: u8,
        previous: u8,
        window: f32,
        repeat_lockout: i32,
    ) -> i32;
}

proptest! {
    #[test]
    fn buffered_tech_gate_matches_original(
        locked in any::<bool>(),
        age in any::<u8>(),
        previous in any::<u8>(),
        window in any::<f32>(),
        repeat_lockout in any::<i32>(),
    ) {
        let original = unsafe {
            oracle_damage_floor_tech(
                u32::from(locked), age, previous, window, repeat_lockout,
            ) != 0
        };
        prop_assert_eq!(
            can_tech(locked, age, previous, window, repeat_lockout),
            original,
        );
    }
}
