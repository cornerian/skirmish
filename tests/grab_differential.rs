#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::grab::{fresh_down, fresh_horizontal, fresh_up};

unsafe extern "C" {
    fn oracle_throw_horizontal(current: f32, previous: f32, threshold: f32) -> i32;
    fn oracle_throw_up(current: f32, previous: f32, threshold: f32) -> i32;
    fn oracle_throw_down(current: f32, previous: f32, threshold: f32) -> i32;
}

proptest! {
    #[test]
    fn throw_stick_predicates_match_original(
        current in any::<f32>(),
        previous in any::<f32>(),
        threshold in any::<f32>(),
    ) {
        let horizontal = unsafe { oracle_throw_horizontal(current, previous, threshold) != 0 };
        let up = unsafe { oracle_throw_up(current, previous, threshold) != 0 };
        let down = unsafe { oracle_throw_down(current, previous, threshold) != 0 };
        prop_assert_eq!(fresh_horizontal(current, previous, threshold), horizontal);
        prop_assert_eq!(fresh_up(current, previous, threshold), up);
        prop_assert_eq!(fresh_down(current, previous, threshold), down);
    }
}

#[test]
fn adapter_uses_complete_pinned_definitions() {
    let original = include_str!("oracle/original/throw_input.c");
    for definition in [
        "static inline bool ftCo_800DD1E4_inline1(Fighter* fp)\n{",
        "static inline bool ftCo_800DD1E4_inline2(Fighter* fp)\n{",
        "static inline bool ftCo_800DD1E4_inline3(Fighter* fp)\n{",
    ] {
        assert!(original.contains(definition));
    }
    let adapter = include_str!("oracle/throw_input.c");
    assert!(adapter.contains("#include \"throw_input_original.inc\""));
}
