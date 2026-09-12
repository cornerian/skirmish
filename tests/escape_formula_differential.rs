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

/// See `tests/oracle/escape_formula.c`'s own comment (and `docs/math.md`)
/// for why this test tolerates a small *relative* difference instead of
/// requiring an exact match: two of `ftCo_800DA824`'s statements are each
/// mirrored with `f32::mul_add`, matching real Gekko `fmadds` instructions,
/// but the uncontracted oracle can't be coaxed into fusing (or, with
/// contraction enabled, fuses a different, hardware-mismatching pair, or
/// reaches across a statement boundary to fuse something the real
/// PowerPC compiler never fuses at all) without touching pinned source. A
/// relative bound (rather than a raw ULP count) is used because proptest's
/// full-range generators also reach subnormal magnitudes, where a fixed ULP
/// count is a misleadingly large-looking relative difference; 1e-4 was
/// chosen with margin above the worst of 20,000 generated cases' relative
/// error (under 2e-6).
fn relative_difference(actual: f32, expected: f32) -> f64 {
    ((actual - expected).abs() / expected.abs().max(actual.abs())) as f64
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
        if !expected.is_finite() {
            // A fused multiply-add evaluates its product at full precision
            // before the single final rounding, so unlike an unfused
            // `a * b` alone, it can never overflow partway through: real
            // Gekko hardware (like `actual` here) shares that with any
            // other genuine FMA instruction, but this file's
            // intentionally-uncontracted oracle (see its own comment)
            // computes the two roundings separately, so at the extreme
            // magnitudes proptest's full `any::<f32>()` range reaches, it
            // can go non-finite in cases the fused, no-intermediate-
            // overflow computation does not (confirmed both directions:
            // an overflowed intermediate product can itself become the
            // oracle's whole non-finite result where the fused result
            // stays finite, or combine with an opposite-signed infinity
            // elsewhere into NaN where the fused result is merely
            // infinite). There is no fixed relationship between the two
            // sides left to assert here; this is the expected shape of the
            // disagreement, not a bug.
        } else if actual != expected {
            let relative = relative_difference(actual, expected);
            prop_assert!(
                relative <= 1e-4,
                "{actual:?} != {expected:?}, {relative:e} relative difference, further than \
                 the documented tolerance (empirically calibrated against 20,000 generated \
                 cases, whose worst observed relative difference was under 2e-6)"
            );
        }
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/capture_pulled.c");
    assert!(original.contains("float ftCo_800DA824(Fighter* fp)\n{"));
    let adapter = include_str!("oracle/escape_formula.c");
    assert!(adapter.contains("#include \"escape_formula_original.inc\""));
}
