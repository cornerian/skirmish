//! HSD linear, Bézier, B-spline, and cardinal interpolation and arc-length lookup.
//!
//! Port of `src/sysdolphin/baselib/spline.c`. Arithmetic order and the original
//! endpoint rules are intentional. Invalid pointers/counts become checked errors;
//! an out-of-range point parameter returns an error instead of leaving an output
//! pointer unchanged. Control-point and coefficient values retain IEEE semantics.

/// Three scalar coordinates, with no SIMD reassociation of interpolation sums.
pub type Point = [f32; 3];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SplineType {
    Linear = 0,
    Bezier = 1,
    BSpline = 2,
    Cardinal = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SplineError {
    #[error("spline knot count must be between 2 and 32767")]
    KnotCount,
    #[error("spline has too few control points")]
    ControlPoints,
    #[error("point parameter must be finite and in [0, 1]")]
    PointParameter,
    #[error("arc-length parameter must not be NaN")]
    ArcParameter,
    #[error("cumulative arc lengths must be finite, nondecreasing, and span [0, 1]")]
    ArcLengths,
    #[error(
        "curved splines require a finite nonnegative total length and a polynomial per segment"
    )]
    ArcPolynomials,
}

/// `knots` is upstream `numcv`: the number of segment boundaries, not necessarily
/// the number of stored points. Bézier needs `3 * (knots - 1) + 1` points;
/// B-spline and cardinal need `knots + 2`, and linear needs `knots`.
pub struct Spline<'a> {
    pub kind: SplineType,
    pub knots: usize,
    pub tension: f32,
    pub points: &'a [Point],
}

/// Cumulative lengths are normalized; quartic coefficients are ordered from t⁴
/// to the constant term. Linear splines ignore `total` and `polynomials`.
pub struct ArcLength<'a> {
    pub total: f32,
    pub cumulative: &'a [f32],
    pub polynomials: &'a [[f32; 5]],
}

/// Upstream `splGetHelmite`, including its original inverse-duration convention.
pub fn hermite(fterm: f32, time: f32, p0: f32, p1: f32, d0: f32, d1: f32) -> f32 {
    let time2 = time * time;
    let fterm2 = fterm * fterm;
    let t2_t = time2 * fterm;
    let t3_t2 = fterm2 * (time2 * time);
    let two_t3_t3 = 2.0 * t3_t2 * fterm;
    let three_t2_t2 = 3.0 * time2 * fterm2;
    d1 * (t3_t2 - t2_t)
        + (d0 * (time + ((t3_t2 - t2_t) - t2_t))
            + (p0 * (1.0 + (two_t3_t3 - three_t2_t2)) + p1 * (-two_t3_t3 + three_t2_t2)))
}

impl Spline<'_> {
    fn segments(&self) -> Result<usize, SplineError> {
        if !(2..=i16::MAX as usize).contains(&self.knots) {
            return Err(SplineError::KnotCount);
        }
        Ok(self.knots - 1)
    }

    /// Upstream `splGetSplinePoint`. The endpoint is copied for linear, Bézier,
    /// and cardinal splines, preserving signed zero and NaN payloads.
    pub fn point(&self, u: f32) -> Result<Point, SplineError> {
        let segments = self.segments()?;
        let required = match self.kind {
            SplineType::Linear => self.knots,
            SplineType::Bezier => 3 * segments + 1,
            SplineType::BSpline | SplineType::Cardinal => self.knots + 2,
        };
        if self.points.len() < required {
            return Err(SplineError::ControlPoints);
        }
        if !(0.0..=1.0).contains(&u) {
            return Err(SplineError::PointParameter);
        }
        if u == 1.0 {
            return Ok(match self.kind {
                SplineType::Linear => self.points[segments],
                SplineType::Bezier => self.points[segments * 3],
                SplineType::BSpline => bspline(&self.points[segments - 1..], 1.0),
                SplineType::Cardinal => self.points[segments + 1],
            });
        }
        let time = u * segments as f32;
        let index = time as usize;
        let t = time - index as f32;
        let cp = &self.points[index..];
        Ok(match self.kind {
            SplineType::Linear => {
                std::array::from_fn(|axis| t * (cp[1][axis] - cp[0][axis]) + cp[0][axis])
            }
            SplineType::Bezier => bezier(&self.points[index * 3..], t),
            SplineType::BSpline => bspline(cp, t),
            SplineType::Cardinal => cardinal(cp, self.tension, t),
        })
    }

    /// Upstream `splArcLengthGetParameter`: clamp the requested distance, locate
    /// its segment, and invert the stored length polynomial with Simpson's rule.
    pub fn parameter(&self, arc: &ArcLength<'_>, u: f32) -> Result<f32, SplineError> {
        let segments = self.segments()?;
        if u.is_nan() {
            return Err(SplineError::ArcParameter);
        }
        if u <= 0.0 {
            return Ok(0.0);
        }
        if u >= 1.0 {
            return Ok(1.0);
        }
        let lengths = arc
            .cumulative
            .get(..self.knots)
            .ok_or(SplineError::ArcLengths)?;
        if lengths[0] != 0.0
            || lengths[segments] != 1.0
            || lengths.iter().any(|x| !x.is_finite())
            || lengths.windows(2).any(|w| w[0] > w[1])
        {
            return Err(SplineError::ArcLengths);
        }
        let index = lengths[1..].partition_point(|&length| length < u);
        let result = if self.kind == SplineType::Linear {
            (u - lengths[index]) / (lengths[index + 1] - lengths[index])
        } else {
            if !arc.total.is_finite() || arc.total < 0.0 || arc.polynomials.len() < segments {
                return Err(SplineError::ArcPolynomials);
            }
            let coeffs = &arc.polynomials[index];
            let mut remaining = arc.total * (u - lengths[index]);
            let (mut start, mut end, mut result) = (0.0_f32, 1.0_f32, 0.0_f32);
            while (start - end).abs() >= 0.00001 {
                result = (start + end) / 2.0;
                let dx = (result - start) / 8.0;
                let mut t = start + dx;
                let mut middle = 0.0;
                for i in 2..=8 {
                    middle +=
                        (if i & 1 == 0 { 4.0 } else { 2.0 }) * arc_length_polynomial(coeffs, t);
                    t += dx;
                }
                let simpsons = dx
                    * (middle
                        + arc_length_polynomial(coeffs, start)
                        + arc_length_polynomial(coeffs, result))
                    / 3.0;
                if remaining < 0.00001 + simpsons {
                    end = result;
                } else {
                    start = result;
                    remaining -= simpsons;
                }
            }
            result
        };
        Ok((result + index as f32) / segments as f32)
    }

    /// Upstream `splArcLengthPoint`.
    pub fn arc_point(&self, arc: &ArcLength<'_>, u: f32) -> Result<Point, SplineError> {
        self.point(self.parameter(arc, u)?)
    }
}

fn weighted(cp: &[Point], weights: [f32; 4]) -> Point {
    std::array::from_fn(|a| {
        cp[0][a] * weights[0]
            + cp[1][a] * weights[1]
            + cp[2][a] * weights[2]
            + cp[3][a] * weights[3]
    })
}

fn cardinal(cp: &[Point], tension: f32, u: f32) -> Point {
    let u2 = u * u;
    let u3 = u2 * u;
    weighted(
        cp,
        [
            tension * (-u3 + 2.0 * u2 - u),
            (2.0 - tension) * u3 + (tension - 3.0) * u2 + 1.0,
            (tension - 2.0) * u3 + (3.0 - 2.0 * tension) * u2 + tension * u,
            tension * (u3 - u2),
        ],
    )
}

fn bspline(cp: &[Point], u: f32) -> Point {
    let u2 = u * u;
    let u3 = u2 * u;
    let u1 = 1.0 - u;
    let sixth = 1.0 / 6.0;
    weighted(
        cp,
        [
            sixth * u1 * u1 * u1,
            sixth * (4.0 + (3.0 * u3 - 6.0 * u2)),
            sixth * (3.0 * (-u3 + u2 + u) + 1.0),
            sixth * u3,
        ],
    )
}

fn bezier(cp: &[Point], u: f32) -> Point {
    let u1 = 1.0 - u;
    let u2 = u * u;
    let u12 = u1 * u1;
    weighted(cp, [u12 * u1, 3.0 * u * u12, 3.0 * u2 * u1, u2 * u])
}

/// Evaluate the upstream quartic speed polynomial, including its small-negative
/// roundoff clamp. Values at or below -0.001 produce NaN through `sqrt`.
pub fn arc_length_polynomial(coeffs: &[f32; 5], t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let mut result = coeffs[0] * t4 + coeffs[1] * t3 + coeffs[2] * t2 + coeffs[3] * t + coeffs[4];
    if result < 0.0 && result > -0.001 {
        result = 0.0;
    }
    result.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_examples() {
        let points = [[-1.0; 3], [0.0; 3], [1.0; 3], [2.0; 3]];
        for (kind, midpoint, endpoint) in [
            (SplineType::Linear, -0.5, 0.0),
            (SplineType::Bezier, 0.5, 2.0),
            (SplineType::BSpline, 0.5, 1.0),
            (SplineType::Cardinal, 0.5, 1.0),
        ] {
            let spline = Spline {
                kind,
                knots: 2,
                tension: 0.5,
                points: &points,
            };
            for coordinate in spline.point(0.5).unwrap() {
                assert!((coordinate - midpoint).abs() < 0.000001);
            }
            assert_eq!(spline.point(1.0).unwrap(), [endpoint; 3]);
        }
        assert_eq!(hermite(0.5, 0.0, 3.0, 9.0, 2.0, -1.0), 3.0);
        assert_eq!(hermite(0.5, 2.0, 3.0, 9.0, 2.0, -1.0), 9.0);
    }

    #[test]
    fn arc_length_known_linear_and_constant_speed_curves() {
        let points = [[0.0; 3], [1.0; 3], [2.0; 3], [3.0; 3]];
        let arc = ArcLength {
            total: 2.0,
            cumulative: &[0.0, 1.0],
            polynomials: &[[0.0, 0.0, 0.0, 0.0, 4.0]],
        };
        for kind in KINDS {
            let spline = Spline {
                kind,
                knots: 2,
                tension: 0.5,
                points: &points,
            };
            let u = spline.parameter(&arc, 0.25).unwrap();
            assert!((u - 0.25).abs() < 0.00002);
        }
        assert_eq!(
            arc_length_polynomial(&[0.0, 0.0, 0.0, 0.0, -0.0005], 1.0),
            0.0
        );
        assert!(arc_length_polynomial(&[0.0, 0.0, 0.0, 0.0, -0.001], 1.0).is_nan());
    }

    const KINDS: [SplineType; 4] = [
        SplineType::Linear,
        SplineType::Bezier,
        SplineType::BSpline,
        SplineType::Cardinal,
    ];

    #[test]
    fn malformed_inputs_are_errors() {
        let mut spline = Spline {
            kind: SplineType::Linear,
            knots: 1,
            tension: 0.5,
            points: &[[0.0; 3], [1.0; 3]],
        };
        assert_eq!(spline.point(0.5), Err(SplineError::KnotCount));
        spline.knots = 3;
        assert_eq!(spline.point(0.5), Err(SplineError::ControlPoints));
        spline.knots = 2;
        for u in [-1.0, 1.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(spline.point(u), Err(SplineError::PointParameter));
        }
        let arc = ArcLength {
            total: 1.0,
            cumulative: &[0.0, 0.5],
            polynomials: &[],
        };
        assert_eq!(spline.parameter(&arc, 0.5), Err(SplineError::ArcLengths));
        assert_eq!(
            spline.parameter(&arc, f32::NAN),
            Err(SplineError::ArcParameter)
        );
        let arc = ArcLength {
            total: 1.0,
            cumulative: &[0.0, 1.0],
            polynomials: &[],
        };
        spline.kind = SplineType::Bezier;
        assert_eq!(
            spline.parameter(&arc, 0.5),
            Err(SplineError::ArcPolynomials)
        );
    }

    #[test]
    fn zero_length_segments_follow_first_equal_boundary() {
        let spline = Spline {
            kind: SplineType::Linear,
            knots: 5,
            tension: 0.0,
            points: &[[0.0; 3]; 5],
        };
        let arc = ArcLength {
            total: 1.0,
            cumulative: &[0.0, 0.0, 0.5, 0.5, 1.0],
            polynomials: &[],
        };
        assert_eq!(spline.parameter(&arc, 0.5), Ok(0.5));
        assert_eq!(spline.parameter(&arc, 0.75), Ok(0.875));
        assert_eq!(spline.parameter(&arc, f32::NEG_INFINITY), Ok(0.0));
        assert_eq!(spline.parameter(&arc, f32::INFINITY), Ok(1.0));
    }
}
