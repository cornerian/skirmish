//! Ports of the game's own trigonometry, replacing generic `libm`.
//!
//! `sinf`/`cosf`/`tanf` port `src/MSL/trigf.c` (Metrowerks MSL runtime); their
//! `__sincos_on_quadrant`/`__sincos_poly` tables and the `__four_over_pi_m1`
//! range-reduction constants come from `src/MSL/math_data.c`. `atan2f`/
//! `atanf`/`acosf`/`asinf` port `src/melee/lb/lbtrigf.c` (the HSD "lb" math
//! library that the fighter/common code actually calls); `atanf` uses the
//! `__MWERKS__`-only lookup-table body, which is what the retail Metrowerks
//! build actually compiled (the non-`__MWERKS__` branch is unused on real
//! hardware).
//!
//! Every function's control flow, branch structure and constant tables come
//! from the pinned decompiled C; but which individual multiply-adds are
//! *fused* (a single correctly-rounded PowerPC `fmadds`/`fmsubs`/`fnmadds`/
//! `fnmsubs`, not two independently-rounded operations) comes from Capstone
//! disassembly of the actual compiled instructions in the retail `main.dol`
//! (function addresses cross-checked against `config/GALE01/symbols.txt`),
//! not from the decompiled C text: decompilers reconstruct *value-equivalent*
//! C, not instruction-for-instruction C, so which operations the original
//! compiler fused is not visible in `trigf.c`/`lbtrigf.c` at all -- guessing
//! from the C text alone (this module's own earlier revision, and the
//! measure-first methodology `docs/math.md` records for `sinf`/`cosf`) can
//! only tell *whether* fusing something changes real-replay agreement, not
//! *which* operations the real binary actually fused. Every fused operation
//! in this module is annotated with the disassembly address it comes from;
//! `docs/math.md` has the full methodology and per-function findings.
//!
//! `acosf`/`asinf` additionally depend on `__frsqrte`. The disassembly
//! settles a question this project's own decompiled source cannot answer:
//! real hardware calls the genuine PowerPC `frsqrte` estimate instruction,
//! not `sqrt(x)` -- the latter is only this decompilation project's own
//! host-tooling stand-in for that macro (`placeholder.h`, `#ifndef
//! MWERKS_GEKKO`), and seeding the following Newton-Raphson refinement with
//! it, as that stand-in and this project's own C oracle do, does not
//! converge except very close to `x == 1` (confirmed empirically:
//! `acosf`/`asinf(0.9999)` returned roughly `2.7°` instead of the true
//! `~89.2°`, before this disassembly). Modeling the exact hardware estimate
//! table is still out of scope (like `sqrtf` generally), but seeding the
//! same disassembly-derived Newton refinement from an accurate `1/sqrt(x)`
//! instead should reach the same converged result as real hardware for
//! essentially every input, so `acosf`/`asinf` are wired to their real call
//! sites. See `frsqrte_newton3`'s doc comment and `docs/math.md`.

/// `MSL_TrigF_8040077{0,4}`/`tmp_float` in `trigf.c`: the constant table a
/// static constructor would otherwise copy into `__four_over_pi_m1` before
/// first use. The values never change at runtime, so they are inlined here.
/// Kept at the pinned source's own literal precision (verbatim, for audit),
/// not reduced to each value's minimal `f32` representation.
#[allow(clippy::excessive_precision)]
const FOUR_OVER_PI_M1: [f32; 4] = [0.25, 0.0232393741608, 1.70555722434e-7, 1.86736494323e-11];

/// `__sincos_on_quadrant` (`math_data.c`).
const SINCOS_ON_QUADRANT: [f32; 8] = [0.0, 1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0];

/// `__sincos_poly` (`math_data.c`). Kept at the pinned source's own literal
/// precision (verbatim, for audit), not reduced to each value's minimal
/// `f32` representation; the last entry is close to, but not exactly,
/// `FRAC_PI_4`, and is not that constant in the pinned source either.
#[allow(clippy::excessive_precision, clippy::approx_constant)]
const SINCOS_POLY: [f32; 10] = [
    0.0000035287617,
    0.0000003089747,
    -0.0003259365,
    -0.00003657235,
    0.015854323,
    0.0024903931,
    -0.30842513,
    -0.08074551,
    1.0,
    0.7853982,
];

/// `__epsilon` in `trigf.c`.
#[allow(clippy::excessive_precision)]
const EPSILON: f32 = 3.45266983e-4;

/// `sinf`/`cosf`'s `(int)` cast in `trigf.c` is undefined behavior in C for a
/// NaN, infinite or out-of-range operand. This project's C oracle is
/// compiled for this same host/target with plain `cc` (no
/// `-fsanitize=undefined`), and on x86-64 that cast lowers to `cvttss2si`,
/// whose hardware-defined result for exactly those inputs is the "integer
/// indefinite" value, `i32::MIN` — confirmed directly against this host's
/// `cc` (see `docs/math.md`). Rust's `as i32` instead saturates (`i32::MAX`
/// for positive overflow, `0` for NaN), so it must not be used here.
fn c_int_cast(x: f32) -> i32 {
    if x.is_nan() || !(-2147483648.0..2147483648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

/// `fabsf__Ff` (`src/MSL/math_1.c`): a thin, bit-exact alias for `fabsf`.
fn fabsf(x: f32) -> f32 {
    x.abs()
}

/// Shared range reduction from `sinf`/`cosf`, ported from the retail binary's
/// actual compiled instructions (Capstone disassembly of `main.dol`'s `sinf`/
/// `cosf` at `0x803263D4`/`0x80326240`, cross-checked against
/// `config/GALE01/symbols.txt`; see `docs/math.md`), not literally from
/// `trigf.c`'s decompiled C text: the two are value-equivalent, but the
/// compiled code fuses the four `__four_over_pi_m1[i] * x` terms into the
/// running total (four PowerPC `fmadds`) rather than computing each product
/// and the final sum as separate roundings. `x - n*2` itself stays an
/// unfused subtraction (matching a real `fsubs`), computed here in `f64` so
/// the intermediate `n*2` product is never rounded to `f32` before the
/// subtraction, matching the compiled code's int-to-double conversion
/// (`lis`/`stw`/`lfd` building an exact double, then subtracting a magic
/// constant) exactly for every `n` an `i32` can hold, including ones a naive
/// `as f32` conversion of `n*2` would itself round. The quadrant is rounded
/// using `x`'s own sign rather than `z`'s.
fn reduce(x: f32) -> (i32, f32) {
    let z = (2.0 / std::f32::consts::PI) * x;
    let n = if x.to_bits() & 0x8000_0000 != 0 {
        c_int_cast(z - 0.5)
    } else {
        c_int_cast(z + 0.5)
    };
    let n2 = i64::from(n) * 2;
    let mut y = (f64::from(x) - n2 as f64) as f32;
    y = FOUR_OVER_PI_M1[0].mul_add(x, y);
    y = FOUR_OVER_PI_M1[1].mul_add(x, y);
    y = FOUR_OVER_PI_M1[2].mul_add(x, y);
    y = FOUR_OVER_PI_M1[3].mul_add(x, y);
    (n & 3, y)
}

/// `sinf` (`trigf.c`). The small-angle branch's final `+ on_quadrant[n]` is a
/// single fused step in the compiled binary (`(y * on_quadrant[n+1])` is a
/// plain multiply; that product's `* poly[9] + on_quadrant[n]` is one
/// `fmadds`), not three independently-rounded operations; see `docs/math.md`.
pub fn sinf(x: f32) -> f32 {
    let (mut n, y) = reduce(x);
    if fabsf(y) < EPSILON {
        n <<= 1;
        let n = n as usize;
        return (y * SINCOS_ON_QUADRANT[n + 1]).mul_add(SINCOS_POLY[9], SINCOS_ON_QUADRANT[n]);
    }
    let ysq = y * y;
    if n & 1 != 0 {
        n <<= 1;
        let z = SINCOS_POLY[0]
            .mul_add(ysq, SINCOS_POLY[2])
            .mul_add(ysq, SINCOS_POLY[4])
            .mul_add(ysq, SINCOS_POLY[6])
            .mul_add(ysq, SINCOS_POLY[8]);
        z * SINCOS_ON_QUADRANT[n as usize]
    } else {
        n <<= 1;
        let z = SINCOS_POLY[1]
            .mul_add(ysq, SINCOS_POLY[3])
            .mul_add(ysq, SINCOS_POLY[5])
            .mul_add(ysq, SINCOS_POLY[7])
            .mul_add(ysq, SINCOS_POLY[9])
            * y;
        z * SINCOS_ON_QUADRANT[n as usize + 1]
    }
}

/// `cosf` (`trigf.c`). The small-angle branch is a single fused
/// negate-multiply-subtract in the compiled binary (`fnmsubs`), not a
/// separate multiply then subtract; see `docs/math.md`.
pub fn cosf(x: f32) -> f32 {
    let (mut n, y) = reduce(x);
    if fabsf(y) < EPSILON {
        n <<= 1;
        let n = n as usize;
        return y.mul_add(-SINCOS_ON_QUADRANT[n], SINCOS_ON_QUADRANT[n + 1]);
    }
    let ysq = y * y;
    if n & 1 != 0 {
        n <<= 1;
        let z = -(SINCOS_POLY[1]
            .mul_add(ysq, SINCOS_POLY[3])
            .mul_add(ysq, SINCOS_POLY[5])
            .mul_add(ysq, SINCOS_POLY[7])
            .mul_add(ysq, SINCOS_POLY[9]))
            * y;
        z * SINCOS_ON_QUADRANT[n as usize]
    } else {
        n <<= 1;
        let z = SINCOS_POLY[0]
            .mul_add(ysq, SINCOS_POLY[2])
            .mul_add(ysq, SINCOS_POLY[4])
            .mul_add(ysq, SINCOS_POLY[6])
            .mul_add(ysq, SINCOS_POLY[8]);
        z * SINCOS_ON_QUADRANT[n as usize + 1]
    }
}

/// `tanf` (`trigf.c`): `sin__Ff(x) / cos__Ff(x)`, the pinned source's own
/// non-inlined aliases for `sinf`/`cosf`.
pub fn tanf(x: f32) -> f32 {
    sinf(x) / cosf(x)
}

/// PowerPC `fnmsub` (`__fnmsubs` in `atanf`'s Metrowerks intrinsics): a single
/// correctly-rounded `b - a * c`, not two separately-rounded operations.
fn fnmsubs(a: f32, c: f32, b: f32) -> f32 {
    a.mul_add(-c, b)
}

/// `atanf_lookup` (`lbtrigf.c`). Kept at the pinned source's own literal
/// precision (verbatim, for audit); one entry is close to, but not exactly,
/// `FRAC_PI_4`, and is not that constant in the pinned source either.
#[allow(clippy::excessive_precision, clippy::approx_constant)]
const ATANF_LOOKUP: [f32; 46] = [
    1.0,
    -0.3333333134651184,
    0.1999988704919815,
    -0.14281649887561798,
    0.11041180044412613,
    -0.08459755778312683,
    0.04714243486523628,
    6.828420162200928,
    3.239828109741211,
    2.0,
    1.4464620351791382,
    1.1715729236602783,
    1.039566159248352,
    7.1350000325764995e-06,
    8.200000252145401e-07,
    0.0,
    6.299999881775875e-07,
    0.0,
    0.0,
    0.0,
    0.3926900029182434,
    0.5890486240386963,
    0.7853981256484985,
    0.9817469716072083,
    1.1780970096588135,
    1.3744460344314575,
    0.0,
    9.081698408408556e-06,
    2.3000000126671694e-08,
    6.30000016599297e-08,
    7.040000014058023e-07,
    2.499999993688107e-07,
    7.900000014160469e-07,
    2.414212942123413,
    1.4966057538986206,
    1.0,
    0.6681786179542542,
    0.4142135679721832,
    0.1989123672246933,
    5.620000251838064e-07,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0,
];

/// `atanf` (`lbtrigf.c`, the `__MWERKS__` body actually compiled into the
/// retail binary): silver-ratio range reduction into a five-way piecewise
/// table, a degree-13 odd minimax polynomial, then table-selected offsets.
///
/// `lookup_index` stays the source's signed `-1` outside the middle range;
/// the source still unconditionally rereads `lookup_ptr =
/// &atanf_lookup[lookup_index]` afterward and adds `lookup_ptr[27]`/
/// `lookup_ptr[20]` (`atanf_lookup[26]` and `atanf_lookup[19]`, both `0.0`,
/// when `lookup_index == -1`) — not a no-op, since adding `+0.0` can turn a
/// `-0.0` result into `+0.0`.
#[allow(clippy::excessive_precision)]
pub fn atanf(x: f32) -> f32 {
    const SILVER_RATIO: f32 = 2.4142136573791504;
    const SILVER_RATIO_CONJUGATE: f32 = 0.4142135679721832;

    let sign_bit_x = x.to_bits() & 0x8000_0000;
    let x = f32::from_bits(x.to_bits() & !0x8000_0000);

    let mut lookup_index: i32 = -1;
    let x_ge_ratio;
    let mut result;

    if x >= SILVER_RATIO {
        x_ge_ratio = true;
        result = 1.0 / x;
    } else if SILVER_RATIO_CONJUGATE < x {
        x_ge_ratio = false;
        lookup_index = 0;
        const BITWISE_0_5: u32 = 0x3F000000;
        const BITWISE_1_0: u32 = 0x3F800000;
        const BITWISE_2_0: u32 = 0x40000000;
        const BITWISE_INF: u32 = 0x7F800000;
        const BITWISE_THRESHOLD_0: i32 = 0x3F08D5B9;
        const BITWISE_THRESHOLD_1: i32 = 0x3F521801;
        const BITWISE_THRESHOLD_2: i32 = 0x3F9BF7EC;
        const BITWISE_THRESHOLD_3: i32 = 0x3FEF789E;
        let signed_bits = x.to_bits() as i32;
        match x.to_bits() & BITWISE_INF {
            BITWISE_0_5 => {
                if !(signed_bits < BITWISE_THRESHOLD_0) {
                    lookup_index = 1;
                }
                if !(signed_bits < BITWISE_THRESHOLD_1) {
                    lookup_index += 1;
                }
            }
            BITWISE_1_0 => {
                lookup_index = 2;
                if !(signed_bits < BITWISE_THRESHOLD_2) {
                    lookup_index = 3;
                }
                if !(signed_bits < BITWISE_THRESHOLD_3) {
                    lookup_index += 1;
                }
            }
            BITWISE_2_0 => {
                lookup_index = 4;
            }
            _ => {}
        }
        let offset_39 = ATANF_LOOKUP[(lookup_index + 39) as usize];
        let offset_33 = ATANF_LOOKUP[(lookup_index + 33) as usize];
        result = 1.0 / (offset_33 + (x + offset_39));
        result = fnmsubs(result, ATANF_LOOKUP[(lookup_index + 7) as usize], offset_33)
            + fnmsubs(
                result,
                ATANF_LOOKUP[(lookup_index + 13) as usize],
                offset_39,
            );
    } else {
        x_ge_ratio = false;
        result = x;
    }

    // The compiled binary (Capstone disassembly of `atanf` at `0x80022E68`;
    // see `docs/math.md`) computes this differently from how `trigf.c`'s
    // decompiled C text groups it, though the two are value-equivalent:
    // `result_squared` and `result_cubed` (`result * result_squared`) are
    // each a separate plain multiply, computed once and reused; the five
    // Horner combines are each a fused `fmadds`; and the final
    // `result_cubed * poly + result` is itself one more fused `fmadds`,
    // not a separate multiply and add.
    let result_squared = result * result;
    let result_cubed = result * result_squared;
    let mut poly = result_squared.mul_add(ATANF_LOOKUP[6], ATANF_LOOKUP[5]);
    poly = result_squared.mul_add(poly, ATANF_LOOKUP[4]);
    poly = result_squared.mul_add(poly, ATANF_LOOKUP[3]);
    poly = result_squared.mul_add(poly, ATANF_LOOKUP[2]);
    poly = result_squared.mul_add(poly, ATANF_LOOKUP[1]);
    result = result_cubed.mul_add(poly, result);

    result += ATANF_LOOKUP[(lookup_index + 27) as usize];
    result += ATANF_LOOKUP[(lookup_index + 20) as usize];

    if x_ge_ratio {
        result -= std::f32::consts::FRAC_PI_2;
        return if sign_bit_x != 0 { result } else { -result };
    }

    f32::from_bits(result.to_bits() | sign_bit_x)
}

/// `atan2f` (`lbtrigf.c`), including its signed-zero and `x == 0` bit-pattern
/// branches.
pub fn atan2f(y: f32, x: f32) -> f32 {
    let sign_x = x.to_bits() & 0x8000_0000;
    let sign_y = y.to_bits() & 0x8000_0000;
    if sign_x == sign_y {
        if sign_x != 0 {
            return if x == -0.0 {
                -std::f32::consts::FRAC_PI_2
            } else {
                atanf(y / x) - std::f32::consts::PI
            };
        }
        return if x != 0.0 {
            atanf(y / x)
        } else {
            std::f32::consts::FRAC_PI_2
        };
    }
    if x < 0.0 {
        return std::f32::consts::PI + atanf(y / x);
    }
    if x != 0.0 {
        return atanf(y / x);
    }
    f32::from_bits(sign_y.wrapping_add(0x3FC9_0FDB))
}

/// The Newton-Raphson refinement shared by `acosf`'s inline computation and
/// `lbtrigf.c`'s static `lb_sqrtf` (called by `asinf`): three iterations of
/// `g = (0.5*g) * fnmsubs(x, g*g, 3.0)`, ported from the compiled binary's
/// actual instructions (Capstone disassembly of `acosf`/`lb_sqrtf` at
/// `0x80022D1C`/`0x80022DF8` in `main.dol`, cross-checked against
/// `config/GALE01/symbols.txt`; see `docs/math.md`), not literally from
/// `lbtrigf.c`'s decompiled C text: each iteration computes `g*g` and `0.5*g`
/// as separate plain multiplies, but fuses `3.0 - x*(g*g)` into one
/// `fnmsubs`.
///
/// The disassembly also settles the *seed*: real hardware calls `frsqrte`,
/// the genuine PowerPC reciprocal-square-root estimate instruction (accurate
/// to roughly 1/4096 relative error, per the architecture manual) -- not
/// `sqrt(x)`, which is only this decompilation project's own host-tooling
/// stand-in for the `__frsqrte` macro (`placeholder.h`, `#ifndef
/// MWERKS_GEKKO`; ported and confirmed empirically to diverge sharply from
/// real behavior in this module's earlier revision, before this
/// disassembly). Modeling the exact hardware estimate table (a specific
/// bit-manipulation lookup) is out of scope here, like `sqrtf` generally, so
/// this seeds from an accurate `1/sqrt(x)` instead of either the coarse
/// hardware estimate or the placeholder's wrong-direction `sqrt(x)`. Three
/// Newton iterations converge quadratically regardless of which
/// sufficiently-close seed they start from, so this should reach the same
/// correctly-rounded fixed point as real hardware for essentially every
/// input -- expected to actually work, unlike the placeholder-seeded version
/// this replaces, though (without the real estimate table) not proven
/// bit-exact against real silicon the way the rest of this module is proven
/// bit-exact against the pinned C oracle.
fn frsqrte_newton3(x: f32) -> f32 {
    if x > 0.0 {
        let mut guess = (1.0_f64 / f64::from(x).sqrt()) as f32;
        for _ in 0..3 {
            let guess_squared = guess * guess;
            let inner = guess_squared.mul_add(-x, 3.0);
            guess = (0.5 * guess) * inner;
        }
        guess
    } else if x != 0.0 {
        f32::NAN
    } else {
        f32::INFINITY
    }
}

/// `acosf` (`lbtrigf.c`). `1.0 - x*x` is a single fused `fnmsubs` in the
/// compiled binary, not a separate multiply and subtract; see
/// `docs/math.md`. Wired to real call sites (`fighter::damage::
/// vector_angle`, `game::characters::fox::up::angle_xy`,
/// `quaternion::interpolate`): unlike a bit-exact port of the pinned C
/// oracle's placeholder-seeded Newton iteration, this real-hardware-seeded
/// version (`frsqrte_newton3`'s doc comment) behaves like an ordinary
/// `acosf` for ordinary inputs.
pub fn acosf(x: f32) -> f32 {
    let result = x.mul_add(-x, 1.0);
    let result = frsqrte_newton3(result);
    std::f32::consts::FRAC_PI_2 - atanf(x * result)
}

/// `asinf` (`lbtrigf.c`), including its static `lb_sqrtf` helper, which
/// computes the same fused `1.0 - x*x` as `acosf` above (not the literal
/// `-(x*x - 1.0)` the decompiled C text shows -- value-equivalent, and the
/// compiled binary computes it as one `fnmsubs` either way). See `acosf`'s
/// doc comment.
pub fn asinf(x: f32) -> f32 {
    atanf(x * frsqrte_newton3(x.mul_add(-x, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_agree_with_std_at_ordinary_angles() {
        for degrees in [0, 30, 45, 60, 90, 135, 180, -90, -45, 270] {
            let radians = f64::from(degrees) * std::f64::consts::PI / 180.0;
            let (expected_sin, expected_cos) = (radians.sin() as f32, radians.cos() as f32);
            assert!(
                (sinf(radians as f32) - expected_sin).abs() < 1e-4,
                "sin({degrees})"
            );
            assert!(
                (cosf(radians as f32) - expected_cos).abs() < 1e-4,
                "cos({degrees})"
            );
        }
    }

    #[test]
    fn atan2_matches_quadrant_expectations() {
        assert!((atan2f(1.0, 1.0) - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert!((atan2f(1.0, -1.0) - 3.0 * std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert!((atan2f(0.0, 1.0)).abs() < 1e-6);
        assert_eq!(atan2f(0.0, 0.0), std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn acos_asin_agree_with_std_at_ordinary_values() {
        for x in [-1.0, -0.5, 0.0, 0.5, 1.0_f32] {
            assert!((acosf(x) - x.acos()).abs() < 1e-4, "acos({x})");
            assert!((asinf(x) - x.asin()).abs() < 1e-4, "asin({x})");
        }
    }

    /// The real-hardware-seeded Newton refinement (`frsqrte_newton3`'s doc
    /// comment) converges across the whole domain, including close to the
    /// edges where this module's earlier, placeholder-seeded revision
    /// diverged sharply (`acosf`/`asinf(0.9999)` used to return roughly
    /// `2.7°` instead of the true `~89.2°`; see `docs/math.md`).
    #[test]
    fn acos_asin_agree_with_std_near_the_domain_edges() {
        for x in [0.9999_f32, -0.9999, 0.999999, -0.999999] {
            assert!((asinf(x) - x.asin()).abs() < 1e-3, "asin({x})");
            assert!((acosf(x) - x.acos()).abs() < 1e-3, "acos({x})");
        }
    }

    #[test]
    fn tanf_matches_sin_over_cos() {
        assert!((tanf(0.5) - sinf(0.5) / cosf(0.5)).abs() < 1e-9);
    }
}
