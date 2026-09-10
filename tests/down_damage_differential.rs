#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::down_damage_face_up;

unsafe extern "C" {
    fn oracle_down_damage(
        prone_action: bool,
        face_up_wait: bool,
        forced: bool,
        pending_damage: f32,
        threshold: i32,
    ) -> i32;
}

proptest! {
    #[test]
    fn prone_damage_gate_and_orientation_match_original(
        prone_action in any::<bool>(),
        face_up_wait in any::<bool>(),
        forced in any::<bool>(),
        pending_damage in any::<f32>(),
        threshold in any::<i32>(),
    ) {
        let expected = unsafe {
            oracle_down_damage(
                prone_action,
                face_up_wait,
                forced,
                pending_damage,
                threshold,
            )
        };
        let actual = match down_damage_face_up(
            prone_action,
            face_up_wait,
            forced,
            pending_damage,
            threshold,
        ) {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        };
        prop_assert_eq!(actual, expected);
    }
}
