//! These tests execute the pinned C implementation on the host. The only
//! adaptations are its includes, scalar/struct ABI declarations, and sqrt name.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::spline::{ArcLength, Point, Spline, SplineType, arc_length_polynomial, hermite};

#[repr(C)]
struct RawSpline {
    kind: u8,
    knots: i16,
    tension: f32,
    points: *const Point,
    total: f32,
    cumulative: *const f32,
    polynomials: *const [f32; 5],
}

unsafe extern "C" {
    fn splGetHelmite(fterm: f32, time: f32, p0: f32, p1: f32, d0: f32, d1: f32) -> f32;
    fn splGetSplinePoint(point: *mut Point, spline: *const RawSpline, u: f32);
    fn splArcLengthGetParameter(spline: *const RawSpline, u: f32) -> f32;
    fn splArcLengthPoint(point: *mut Point, spline: *const RawSpline, u: f32);
    fn oracle_spline_polynomial(coeffs: *const f32, t: f32) -> f32;
}

const KINDS: [SplineType; 4] = [
    SplineType::Linear,
    SplineType::Bezier,
    SplineType::BSpline,
    SplineType::Cardinal,
];

fn raw(spline: &Spline<'_>, arc: &ArcLength<'_>) -> RawSpline {
    RawSpline {
        kind: spline.kind as u8,
        knots: spline.knots as i16,
        tension: spline.tension,
        points: spline.points.as_ptr(),
        total: arc.total,
        cumulative: arc.cumulative.as_ptr(),
        polynomials: arc.polynomials.as_ptr(),
    }
}

fn same(actual: f32, expected: f32) {
    // C and Rust may propagate different NaN payloads; finite values and signed
    // zeros must match bit for bit. Nonfinite tests require the same NaN class.
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} differs from C NaN");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn same_point(actual: Point, expected: Point) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        same(actual, expected);
    }
}

fn compare_point(spline: &Spline<'_>, u: f32) {
    let arc = ArcLength {
        total: 0.0,
        cumulative: &[],
        polynomials: &[],
    };
    let raw = raw(spline, &arc);
    let mut expected = [f32::NAN; 3];
    // SAFETY: Spline fixture has enough live contiguous control points for its
    // kind and knot count. u is finite and in [0, 1]; C only reads spline data.
    unsafe { splGetSplinePoint(&mut expected, &raw, u) };
    same_point(spline.point(u).unwrap(), expected);
}

fn compare_arc(spline: &Spline<'_>, arc: &ArcLength<'_>, u: f32) {
    let raw = raw(spline, arc);
    let mut expected_point = [f32::NAN; 3];
    // SAFETY: all fixture arrays outlive these calls and have the lengths the
    // original C expects. Normalized lengths are sorted with endpoints 0 and 1.
    let expected = unsafe {
        splArcLengthPoint(&mut expected_point, &raw, u);
        splArcLengthGetParameter(&raw, u)
    };
    same(spline.parameter(arc, u).unwrap(), expected);
    same_point(spline.arc_point(arc, u).unwrap(), expected_point);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn hermite_agrees_with_c(values in prop::array::uniform6(-1000.0_f32..1000.0)) {
        let [fterm, time, p0, p1, d0, d1] = values;
        // SAFETY: scalar function has no pointer or input-domain preconditions.
        let expected = unsafe { splGetHelmite(fterm, time, p0, p1, d0, d1) };
        same(hermite(fterm, time, p0, p1, d0, d1), expected);
    }

    #[test]
    fn every_kind_matches_c_at_arbitrary_parameters(
        points in prop::collection::vec(prop::array::uniform3(-10000.0_f32..10000.0), 61),
        knots in 2_usize..=21,
        tension in -5.0_f32..5.0,
        u in 0.0_f32..1.0,
    ) {
        for kind in KINDS {
            let spline = Spline { kind, knots, tension, points: &points };
            compare_point(&spline, u);
            compare_point(&spline, 0.0);
            compare_point(&spline, 1.0);
        }
    }

    #[test]
    fn arc_inversion_and_points_match_c(
        points in prop::collection::vec(prop::array::uniform3(-100.0_f32..100.0), 25),
        weights in prop::collection::vec(0.01_f32..10.0, 1..=8),
        coeffs in prop::collection::vec(prop::array::uniform5(0.0_f32..10.0), 8),
        tension in -2.0_f32..2.0,
        u in 0.0_f32..1.0,
    ) {
        let total: f32 = weights.iter().sum();
        let mut cumulative = vec![0.0];
        let mut length = 0.0;
        for weight in &weights {
            length += weight;
            cumulative.push(length / total);
        }
        let arc = ArcLength { total, cumulative: &cumulative, polynomials: &coeffs };
        for kind in KINDS {
            let spline = Spline { kind, knots: weights.len() + 1, tension, points: &points };
            compare_arc(&spline, &arc, u);
        }
    }

    #[test]
    fn quartic_speed_matches_c(
        coeffs in prop::array::uniform5(-1000.0_f32..1000.0),
        t in -10.0_f32..10.0,
    ) {
        // SAFETY: coeffs contains five live f32 values and C only reads it.
        let expected = unsafe { oracle_spline_polynomial(coeffs.as_ptr(), t) };
        same(arc_length_polynomial(&coeffs, t), expected);
    }
}

#[test]
fn segment_boundaries_and_their_adjacent_floats_match_c() {
    let points: Vec<Point> = (0..19)
        .map(|i| [i as f32, (i * i) as f32, -(i as f32)])
        .collect();
    let cumulative = [0.0, 0.125, 0.25, 0.25, 0.5, 0.75, 1.0];
    let polynomials = [[0.25, 0.5, 1.0, 2.0, 1.0]; 6];
    let arc = ArcLength {
        total: 6.0,
        cumulative: &cumulative,
        polynomials: &polynomials,
    };
    for kind in KINDS {
        let spline = Spline {
            kind,
            knots: 7,
            tension: 0.5,
            points: &points,
        };
        for i in 1..6 {
            let u = i as f32 / 6.0;
            for u in [u.next_down(), u, u.next_up()] {
                compare_point(&spline, u);
                compare_arc(&spline, &arc, u);
            }
        }
        for u in cumulative {
            compare_arc(&spline, &arc, u);
        }
        for u in [
            -10.0,
            -0.0,
            0.0,
            1.0,
            10.0,
            f32::NEG_INFINITY,
            f32::INFINITY,
        ] {
            compare_arc(&spline, &arc, u);
        }
    }
}

#[test]
fn endpoint_copies_preserve_payloads_and_signed_zero() {
    let points = [[-0.0, f32::from_bits(0x7fc0_0042), f32::INFINITY]; 7];
    for kind in [SplineType::Linear, SplineType::Bezier, SplineType::Cardinal] {
        let spline = Spline {
            kind,
            knots: 3,
            tension: 0.5,
            points: &points,
        };
        let point = spline.point(1.0).unwrap();
        assert_eq!(point.map(f32::to_bits), points[0].map(f32::to_bits));
        compare_point(&spline, 1.0);
    }
}

#[test]
fn polynomial_roundoff_clamp_and_exceptional_values_match_c() {
    for constant in [
        -0.001_f32,
        (-0.001_f32).next_up(),
        -0.0005,
        -0.0,
        0.0,
        1.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        let coeffs = [0.0, 0.0, 0.0, 0.0, constant];
        // SAFETY: C reads exactly five coefficients from this live array.
        let expected = unsafe { oracle_spline_polynomial(coeffs.as_ptr(), 0.25) };
        same(arc_length_polynomial(&coeffs, 0.25), expected);
    }
}

#[test]
fn zero_and_negative_speed_polynomials_keep_original_iteration_rules() {
    let points = [[0.0, 1.0, 2.0]; 7];
    for constant in [0.0, -0.0005, -0.001, f32::NAN] {
        let polys = [[0.0, 0.0, 0.0, 0.0, constant]; 2];
        let arc = ArcLength {
            total: 1.0,
            cumulative: &[0.0, 0.4, 1.0],
            polynomials: &polys,
        };
        for kind in [
            SplineType::Bezier,
            SplineType::BSpline,
            SplineType::Cardinal,
        ] {
            let spline = Spline {
                kind,
                knots: 3,
                tension: 0.5,
                points: &points,
            };
            compare_arc(&spline, &arc, 0.1);
            compare_arc(&spline, &arc, 0.75);
        }
    }
}

#[test]
fn c_out_of_range_point_calls_leave_the_destination_unchanged() {
    let spline = Spline {
        kind: SplineType::Linear,
        knots: 2,
        tension: 0.0,
        points: &[[0.0; 3], [1.0; 3]],
    };
    let arc = ArcLength {
        total: 0.0,
        cumulative: &[],
        polynomials: &[],
    };
    let raw = raw(&spline, &arc);
    for u in [-1.0, 1.1, f32::NEG_INFINITY, f32::INFINITY] {
        let mut output = [3.0, 4.0, 5.0];
        // SAFETY: these out-of-range values return before any array access.
        unsafe { splGetSplinePoint(&mut output, &raw, u) };
        assert_eq!(output, [3.0, 4.0, 5.0]);
        assert!(spline.point(u).is_err());
    }
}
