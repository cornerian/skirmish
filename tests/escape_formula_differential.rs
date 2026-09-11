#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::grab::escape_timer;

unsafe extern "C" {
    fn oracle_escape_formula(
        base: f32,
        handicap_scale: f32,
        handicap_max: f32,
        rank_scale: f32,
        rank_max: f32,
        percent_scale: f32,
        percent: f32,
        standing: i32,
        handicap: i32,
    ) -> f32;
}

proptest! {
    #[test]
    fn escape_timer_matches_ftco_800da824(
        base in any::<f32>(),
        handicap_scale in any::<f32>(),
        handicap_max in any::<f32>(),
        rank_scale in any::<f32>(),
        rank_max in any::<f32>(),
        percent_scale in any::<f32>(),
        percent in 0.0f32..999.0,
        standing in 0u8..=3,
        handicap in 1u8..=9,
    ) {
        let expected = unsafe {
            oracle_escape_formula(
                base,
                handicap_scale,
                handicap_max,
                rank_scale,
                rank_max,
                percent_scale,
                percent,
                i32::from(standing),
                i32::from(handicap),
            )
        };
        let actual = escape_timer(
            base,
            handicap_scale,
            handicap_max,
            rank_scale,
            rank_max,
            percent_scale,
            percent,
            standing,
            handicap,
        );
        prop_assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/capture_pulled.c");
    assert!(original.contains("float ftCo_800DA824(Fighter* fp)\n{"));
    let adapter = include_str!("oracle/escape_formula.c");
    assert!(adapter.contains("#include \"escape_formula_original.inc\""));
}
