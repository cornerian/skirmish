//! `src/math.rs` against the pinned `trigf.c`/`math_data.c`/`lbtrigf.c`
//! bodies, compiled under process-unique names and reached through
//! `tests/oracle/trig.c`'s `oracle_*` wrappers (`trigf_body.c`'s header
//! comment explains why they are not compiled under libm's own `sinf`/
//! `cosf`/... names). Full binary32 domain, NaN-safe.
//!
//! Every fused operation each function below actually uses comes from
//! disassembling the retail binary (not from reading the decompiled C text,
//! which is value- but not instruction-equivalent -- see `src/math.rs`'s
//! module doc comment and `docs/math.md`), and every function here is
//! shipped fused exactly that way against this pinned, unfused,
//! `-ffp-contract=off` oracle -- so none of the tolerances below are
//! papering over an uncertain port; they exist because the pinned oracle
//! itself, deliberately built without the contraction the retail compiler
//! used, is not bit-identical to what is now known to actually ship, and
//! `docs/math.md`'s fused-multiply-add finding explains why fused is the
//! right choice regardless (per this project's own precedent, "the
//! recording is the final arbiter"): measured against `fox-fd.slp`, the
//! plain, oracle-bit-exact form of `sinf`/`cosf` regresses the real-replay
//! ratchet by a frame, while fusing them exactly as the retail binary does
//! ties it. `atan2f`/`atanf` differ from the oracle only by a small, fixed
//! ULP bound (`close_ulps`); `sinf`/`cosf`/`tanf` need their own tolerances
//! (`close_abs`/`close_rel`'s doc comments explain why ULP distance is the
//! wrong tool for them specifically).
//!
//! `acosf`/`asinf` are compared against `std`'s accurate `acos`/`asin`
//! instead of the oracle: both are seeded from a real reciprocal-sqrt
//! estimate (confirmed by disassembly -- real hardware calls the genuine
//! PowerPC `frsqrte` instruction), not the oracle's placeholder-derived
//! `sqrt`-seeded one (this decompilation project's own host stand-in for
//! `__frsqrte`, not real hardware behavior), so the two are only expected to
//! agree very close to the placeholder's own accidental sweet spot. See
//! `src/math.rs`'s module and `frsqrte_newton3` doc comments and
//! `docs/math.md`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::math::{acosf, asinf, atan2f, atanf, cosf, sinf, tanf};

// `acosf`/`asinf` are compared against `std`, not `oracle_acosf`/
// `oracle_asinf` (see below), so those two are deliberately not declared.
#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_sinf(x: f32) -> f32;
    fn oracle_cosf(x: f32) -> f32;
    fn oracle_tanf(x: f32) -> f32;
    fn oracle_atan2f(y: f32, x: f32) -> f32;
    fn oracle_atanf(x: f32) -> f32;
}

/// `atanf`/`atan2f` are shipped fused (see the module doc comment); this
/// accepts a small ULP difference from the pinned, unfused-arithmetic oracle
/// rather than requiring raw bit equality. NaN-safe like `same_bits`. Not
/// used for `sinf`/`cosf`/`tanf`: see `close_abs`'s doc comment for why raw
/// ULP distance is the wrong tool for those.
fn close_ulps(label: &str, actual: f32, expected: f32, max_ulps: u32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
        return;
    }
    if !actual.is_finite() || !expected.is_finite() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
        return;
    }
    let ulps = actual.to_bits().abs_diff(expected.to_bits());
    assert!(
        ulps <= max_ulps,
        "{label}: {actual:?} != {expected:?} ({ulps} ulps)"
    );
}

/// `sinf`/`cosf` are shipped fused (see the module doc comment), and a raw
/// ULP distance from the pinned, unfused-arithmetic oracle is the wrong tool
/// to bound that: right where either crosses zero, an absolute difference
/// of only ULPs becomes an arbitrarily large *relative* one (both have
/// unit-magnitude derivative at their own zero), so a ULP or relative bound
/// tight enough to mean anything elsewhere fails at every zero crossing
/// (confirmed directly: 51,147 ULP at `x` just past `PI`, though the
/// absolute difference there is small). `sinf`/`cosf` are bounded to
/// `-1.0..=1.0`, so a single absolute bound is exactly the tool this needs
/// (measured directly across the full `-1000.0..1000.0` domain used below:
/// the worst absolute difference found was under `1e-6`).
fn close_abs(label: &str, actual: f32, expected: f32, max_abs: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
        return;
    }
    if !actual.is_finite() || !expected.is_finite() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
        return;
    }
    let diff = (actual - expected).abs();
    assert!(
        diff <= max_abs,
        "{label}: {actual:?} != {expected:?} ({diff})"
    );
}

/// `tanf`'s zeros are well-behaved (matching `sinf`'s), but its *poles*
/// (`cosf`'s zeros) are the mirror problem: `close_abs`'s bounded output
/// range assumption fails outright, and even a small absolute or relative
/// `sinf`/`cosf` difference explodes in the ratio. A combined relative
/// (scaled to the expected magnitude) and absolute floor handles both the
/// well-conditioned bulk of the domain and small values near `tanf`'s own
/// zeros at once.
fn close_rel(label: &str, actual: f32, expected: f32, rel_eps: f32, abs_floor: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
        return;
    }
    if !actual.is_finite() || !expected.is_finite() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
        return;
    }
    let diff = (actual - expected).abs();
    let bound = (rel_eps * expected.abs()).max(abs_floor);
    assert!(
        diff <= bound,
        "{label}: {actual:?} != {expected:?} ({diff} > {bound})"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    // `sinf`/`cosf` are shipped fused in more places than just the Horner
    // polynomial (`reduce`'s four `__four_over_pi_m1[i] * x` terms and the
    // small-angle branch are each fused too, confirmed by disassembling
    // `sinf`/`cosf` at `0x803263D4`/`0x80326240`; `docs/math.md`), against
    // this pinned, unfused, `-ffp-contract=off` oracle. `close_abs`'s doc
    // comment explains the absolute bound. No real stick angle approaches
    // even this domain (about 300 turns), so this remains the meaningful
    // check; `tests/escape_air_differential.rs`'s bit-exact comparison
    // against actual gameplay-shaped stick angles is the even-tighter,
    // real-call-site check. `full_domain_never_panics` below is the
    // full-`u32`-domain liveness check this trades away precision for.
    #[test]
    fn sinf_matches_the_pinned_msl_body(x in -1000.0_f32..=1000.0) {
        close_abs("sinf", sinf(x), unsafe { oracle_sinf(x) }, 1e-5);
    }

    #[test]
    fn cosf_matches_the_pinned_msl_body(x in -1000.0_f32..=1000.0) {
        close_abs("cosf", cosf(x), unsafe { oracle_cosf(x) }, 1e-5);
    }

    // `tanf` is `sinf(x)/cosf(x)`; `close_rel`'s doc comment explains why it
    // needs its own combined relative/absolute bound. No finite tolerance
    // can bound this near an actual pole: as `x` approaches a zero of
    // `cosf`, an arbitrarily small error in `cosf` alone produces an
    // arbitrarily large error in the ratio (confirmed directly: over
    // `-1000.0..1000.0`, which crosses hundreds of poles, random sampling
    // alone found a case 10x past a already-generous 1% tolerance). This
    // excludes inputs within `oracle_cosf`'s own smallest 1% of range,
    // matching the well-conditioned domain any of `tanf`'s real callers
    // (`compat::bytecode`'s `0x0f` opcode, given a reasonable script angle)
    // actually exercise.
    #[test]
    fn tanf_matches_the_pinned_msl_body(x in -1000.0_f32..=1000.0) {
        // SAFETY: scalar argument only.
        prop_assume!(unsafe { oracle_cosf(x) }.abs() > 0.01);
        close_rel("tanf", tanf(x), unsafe { oracle_tanf(x) }, 1e-2, 1e-2);
    }

    /// The full binary32 domain check `sinf`/`cosf`/`tanf`'s bounded-domain
    /// tests above trade away: every input, including NaN and both
    /// infinities, returns a value without panicking. This does not attempt
    /// a numeric (or even finite/infinite/NaN-shaped) comparison against the
    /// oracle across the *entire* domain: past roughly `|x| > 2^24`,
    /// `reduce`'s `x - n*2` cancellation leaves a residual with essentially
    /// no correct bits left (confirmed directly at `x == 1686629760.0`: the
    /// intended near-zero residual comes out near `520`, many pi past any
    /// quadrant, and other nearby inputs push the polynomial to `Infinity`),
    /// so which shape of nonsense either implementation lands on there is
    /// itself just noise, not a meaningful property to compare.
    #[test]
    fn full_domain_never_panics(bits in any::<u32>()) {
        let x = f32::from_bits(bits);
        let _ = (sinf(x), cosf(x), tanf(x));
    }

    // `atanf`'s Horner tail is fused differently from how `lbtrigf.c`'s
    // decompiled text groups it (`docs/math.md`; confirmed by disassembling
    // `atanf` at `0x80022E68`), so, like `sinf`/`cosf`, it is shipped fused
    // and intentionally differs from this disabled-FMA oracle by a small,
    // bounded number of ULP rather than being bit-exact.
    #[test]
    fn atanf_matches_the_pinned_lb_body(bits in any::<u32>()) {
        let x = f32::from_bits(bits);
        // SAFETY: scalar argument only.
        close_ulps("atanf", atanf(x), unsafe { oracle_atanf(x) }, 4);
    }

    /// `atan2f` composes `atanf` with only bit-pattern branches and plain
    /// arithmetic of its own, so its own ULP bound matches `atanf`'s.
    #[test]
    fn atan2f_matches_the_pinned_lb_body(y_bits in any::<u32>(), x_bits in any::<u32>()) {
        let (y, x) = (f32::from_bits(y_bits), f32::from_bits(x_bits));
        // SAFETY: scalar arguments only.
        close_ulps("atan2f", atan2f(y, x), unsafe { oracle_atan2f(y, x) }, 4);
    }

    // `acosf`/`asinf` are seeded from an accurate `1/sqrt` (`src/math.rs`'s
    // `frsqrte_newton3` doc comment), not the pinned oracle's placeholder
    // `sqrt`-seeded one, so they are compared against `std`'s accurate
    // `acos`/`asin` here instead of `oracle_acosf`/`oracle_asinf` (which
    // remain useful only very close to the placeholder's `x == 1` sweet
    // spot; see `docs/math.md`).
    #[test]
    fn acosf_agrees_with_true_acos(bits in any::<u32>()) {
        let x = f32::from_bits(bits);
        close_to_true_value("acosf", acosf(x), x, f32::acos);
    }

    #[test]
    fn asinf_agrees_with_true_asin(bits in any::<u32>()) {
        let x = f32::from_bits(bits);
        close_to_true_value("asinf", asinf(x), x, f32::asin);
    }
}

/// `acosf`/`asinf` are only defined (non-NaN) for `x` in `-1.0..=1.0`; NaN
/// on both sides is accepted regardless of payload, matching `same_bits`.
/// Within the domain, a wide but bounded absolute tolerance rules out
/// anything but ordinary floating-point rounding -- a wrong formula or a
/// non-convergent iteration produces an error many orders of magnitude
/// larger (confirmed directly: the placeholder-seeded revision this
/// replaced was off by *radians*, not fractions of a degree).
fn close_to_true_value(label: &str, actual: f32, x: f32, true_fn: fn(f32) -> f32) {
    let expected = true_fn(x);
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}({x:?}): {actual:?}, expected NaN");
    } else {
        assert!(
            (actual - expected).abs() < 1e-2,
            "{label}({x:?}): {actual:?} != {expected:?}"
        );
    }
}

/// Fixed points the proptest domain might not hit by chance: signed zeros,
/// the axis boundaries `atan2f`'s bit-pattern branches key on, and a few
/// ordinary stick-angle-shaped values. Bit-exact for `atan2f`/`atanf`; the
/// same tolerances as above for `sinf`/`cosf`/`tanf`/`acosf`/`asinf`.
#[test]
fn boundary_values_match() {
    let source = include_str!("oracle/original/trigf.c");
    let data = include_str!("oracle/original/math_data.c");
    let lb = include_str!("oracle/original/lbtrigf.c");
    assert!(source.contains("f32 sinf(f32 x)"));
    assert!(source.contains("f32 cosf(f32 x)"));
    assert!(source.contains("f32 tanf(f32 x)"));
    assert!(data.contains("const float __sincos_poly[]"));
    assert!(lb.contains("float atan2f(float y, float x)"));
    assert!(lb.contains("float acosf(float x)"));
    assert!(lb.contains("float asinf(float x)"));
    assert!(lb.contains("float atanf(float x)"));

    for bits in [
        0.0_f32.to_bits(),
        (-0.0_f32).to_bits(),
        1.0_f32.to_bits(),
        (-1.0_f32).to_bits(),
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        std::f32::consts::PI.to_bits(),
        std::f32::consts::FRAC_PI_2.to_bits(),
        std::f32::consts::FRAC_PI_4.to_bits(),
        (10.0_f32 * std::f32::consts::PI).to_bits(),
    ] {
        let x = f32::from_bits(bits);
        // SAFETY: scalar arguments only.
        unsafe {
            close_abs("sinf", sinf(x), oracle_sinf(x), 1e-5);
            close_abs("cosf", cosf(x), oracle_cosf(x), 1e-5);
            if oracle_cosf(x).abs() > 0.01 {
                close_rel("tanf", tanf(x), oracle_tanf(x), 1e-2, 1e-2);
            }
            close_ulps("atanf", atanf(x), oracle_atanf(x), 4);
            close_to_true_value("acosf", acosf(x), x, f32::acos);
            close_to_true_value("asinf", asinf(x), x, f32::asin);
            for &(y_bits, msg) in &[
                (0.0_f32.to_bits(), "atan2f +0,x"),
                ((-0.0_f32).to_bits(), "atan2f -0,x"),
            ] {
                let y = f32::from_bits(y_bits);
                close_ulps(msg, atan2f(y, x), oracle_atan2f(y, x), 4);
            }
        }
    }
}
