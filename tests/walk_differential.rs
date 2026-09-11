//! The teeter walk predicate `ftCo_Walk_CheckInput_Ottotto`
//! (`tests/oracle/walk.c`, `tests/oracle/original/walk.c`) checked against
//! `fighter::edge::teeter_walk_allowed`, ANDed with a scripted ordinary
//! walk predicate answer standing in for `ftWalkCommon_800DFC70`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::edge::teeter_walk_allowed;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_walk_ottotto_predicate(
        stick_x: f32,
        facing_dir: f32,
        threshold: f32,
        ordinary_walk_ok: i32,
    ) -> i32;
}

fn compare(stick_x: f32, facing: f32, threshold: f32, ordinary_walk_ok: bool) {
    let expected = ordinary_walk_ok && teeter_walk_allowed(stick_x, facing, threshold);
    // SAFETY: the adapter owns all C state; no pointers are passed.
    let actual = unsafe {
        oracle_walk_ottotto_predicate(stick_x, facing, threshold, i32::from(ordinary_walk_ok))
    };
    assert_eq!(actual != 0, expected);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_inputs_match_the_pinned_predicate(
        stick_x in any::<u32>().prop_map(f32::from_bits),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        threshold in any::<u32>().prop_map(f32::from_bits),
        ordinary_walk_ok in any::<bool>(),
    ) {
        compare(stick_x, facing, threshold, ordinary_walk_ok);
    }
}

#[test]
fn exact_boundaries() {
    for ordinary_walk_ok in [false, true] {
        compare(0.4, 1.0, 0.4, ordinary_walk_ok);
        compare(0.399, 1.0, 0.4, ordinary_walk_ok);
        compare(-0.4, -1.0, 0.4, ordinary_walk_ok);
        compare(-0.399, -1.0, 0.4, ordinary_walk_ok);
        compare(0.4, -1.0, 0.4, ordinary_walk_ok);
        compare(f32::NAN, 1.0, 0.4, ordinary_walk_ok);
        compare(f32::INFINITY, 1.0, f32::NEG_INFINITY, ordinary_walk_ok);
    }
}

#[test]
fn adapter_retains_the_complete_pinned_function() {
    let source = include_str!("oracle/original/walk.c");
    assert!(source.contains("bool ftCo_Walk_CheckInput_Ottotto(Fighter_GObj* gobj)"));
    assert!(include_str!("oracle/walk.c").contains("#include \"walk_original.inc\""));
}
