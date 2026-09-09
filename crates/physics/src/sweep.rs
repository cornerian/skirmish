//! Swept sphere/isotropic capsule tests from `src/melee/lb/lbcollision.c`.
//!
//! `lbColl_80006094` supplies closest centerline points and a collision boolean;
//! it does not calculate time of impact. In particular, its nearly parallel
//! branch chooses an endpoint of the first segment, even when a generic geometry
//! library would choose a closer interior point. These source rules are retained.
//! The separate matrix-dependent hurt/shield routine `lbColl_80006E58` is not
//! implemented here. Both endpoints and radii must already be in world space.

use crate::combat::Capsule;

type Vector = [f32; 3];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClosestPair {
    pub first: Vector,
    pub second: Vector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentDistance {
    pub squared_distance: f32,
    /// Parameter on the segment, clamped to [0, 1] when not NaN.
    pub parameter: f32,
}

/// `lbColl_80005EBC`. A zero-length segment yields NaN, as in the source;
/// the capsule routine handles its own approximately zero length cases.
pub fn point_segment(start: Vector, end: Vector, point: Vector) -> SegmentDistance {
    point_segment_dimensions(start, end, point, 3)
}

/// `lbColl_80005FC0`: squared distance and segment parameter in XY, ignoring Z.
/// A segment with zero XY length yields NaN.
pub fn point_segment_xy(start: Vector, end: Vector, point: Vector) -> SegmentDistance {
    point_segment_dimensions(start, end, point, 2)
}

fn point_segment_dimensions(
    start: Vector,
    end: Vector,
    point: Vector,
    dimensions: usize,
) -> SegmentDistance {
    let delta = sub(end, start);
    let offset = sub(start, point);
    // This helper uses (X + Y) + Z; the capsule solver below uses Z + (X + Y).
    let dot = |a: Vector, b: Vector| {
        let xy = a[0] * b[0] + a[1] * b[1];
        if dimensions == 3 {
            xy + a[2] * b[2]
        } else {
            xy
        }
    };
    let parameter = (-dot(delta, offset) / dot(delta, delta)).clamp(0.0, 1.0);
    let separation = core::array::from_fn(|i| delta[i] * parameter + start[i] - point[i]);
    SegmentDistance {
        squared_distance: dot(separation, separation),
        parameter,
    }
}

fn sub(a: Vector, b: Vector) -> Vector {
    core::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: Vector, b: Vector) -> f32 {
    a[2] * b[2] + (a[0] * b[0] + a[1] * b[1])
}

fn approximately_zero(value: f32) -> bool {
    value < 0.00001 && value > -0.00001
}

/// `lbColl_80006094`. A moving sphere is represented by its previous/current
/// centers and radius. `closest` is unchanged on AABB rejection; after that
/// rejection phase, both closest outputs are written even on a narrow miss.
/// Touching radii collide. Source endpoint tie-breaking and floating-point
/// operation order are preserved, including the handling of degenerate segments.
#[allow(clippy::neg_cmp_op_on_partial_ord, clippy::manual_range_contains)]
pub fn capsule_capsule(a: &Capsule, b: &Capsule, closest: &mut ClosestPair) -> bool {
    let radius = a.radius + b.radius;
    for i in 0..3 {
        let (low, high) = if a.start[i] > a.end[i] {
            (a.end[i] - radius, a.start[i] + radius)
        } else {
            (a.start[i] - radius, a.end[i] + radius)
        };
        if (low > b.start[i] && low > b.end[i]) || (high < b.start[i] && high < b.end[i]) {
            return false;
        }
    }

    let da = sub(a.end, a.start);
    let db = sub(b.end, b.start);
    let offset = sub(a.start, b.start);
    let aa = dot(da, da);
    let bb = dot(db, db);
    let ab = dot(da, db);
    let bo = dot(db, offset);
    let ao = dot(da, offset);
    let determinant = aa * bb - ab * ab;
    let (mut s, mut t);

    if approximately_zero(bb) {
        t = 0.0;
        s = if approximately_zero(aa) {
            0.0
        } else {
            (-ao / aa).clamp(0.0, 1.0)
        };
    } else if approximately_zero(determinant) {
        // Upstream's unsuffixed 0.5 promotes the whole expression to double.
        let midpoint =
            core::array::from_fn(|i| (0.5 * f64::from(db[i]) + f64::from(b.start[i])) as f32);
        let start_offset = sub(a.start, midpoint);
        let end_offset = sub(a.end, midpoint);
        let endpoint = if dot(start_offset, start_offset) < dot(end_offset, end_offset) {
            s = 0.0;
            a.start
        } else {
            s = 1.0;
            a.end
        };
        t = (-dot(db, sub(b.start, endpoint)) / bb).clamp(0.0, 1.0);
    } else {
        s = (ab * bo - bb * ao) / determinant;
        t = (aa * bo - ab * ao) / determinant;
        // Negated range containment would take a different branch for NaNs.
        if s > 1.0 || s < 0.0 || t > 1.0 || t < 0.0 {
            let (candidate_s, endpoint) = if s < 0.0 {
                (0.0, a.start)
            } else {
                (1.0, a.end)
            };
            let candidate_a = point_segment(b.start, b.end, endpoint);
            let endpoint = if t < 0.0 {
                t = 0.0;
                b.start
            } else {
                t = 1.0;
                b.end
            };
            let candidate_b = point_segment(a.start, a.end, endpoint);
            if candidate_a.squared_distance < candidate_b.squared_distance {
                s = candidate_s;
                t = candidate_a.parameter;
            } else {
                s = candidate_b.parameter;
            }
        }
    }
    closest.first = core::array::from_fn(|i| da[i] * s + a.start[i]);
    closest.second = core::array::from_fn(|i| db[i] * t + b.start[i]);
    let separation = sub(closest.first, closest.second);
    !(radius * radius < dot(separation, separation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start: Vector, end: Vector, radius: f32) -> Capsule {
        Capsule { start, end, radius }
    }

    #[test]
    fn fast_crossing_collides_between_disjoint_frame_endpoints() {
        let moving = segment([-10.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.5);
        let target = segment([0.0, -2.0, 0.0], [0.0, 2.0, 0.0], 0.5);
        let mut closest = ClosestPair::default();
        assert!(capsule_capsule(&moving, &target, &mut closest));
        assert_eq!(closest.first, [0.0; 3]);
        assert_eq!(closest.second, [0.0; 3]);
    }

    #[test]
    fn upstream_parallel_tie_selects_first_capsules_end() {
        let a = segment([0.0; 3], [10.0, 0.0, 0.0], 0.0);
        let b = segment([4.0, 0.0, 0.0], [6.0, 0.0, 0.0], 0.0);
        let mut closest = ClosestPair::default();
        assert!(!capsule_capsule(&a, &b, &mut closest));
        assert_eq!(closest.first, a.end);
        assert_eq!(closest.second, b.end);
    }

    #[test]
    fn rejection_preserves_outputs_and_point_helper_keeps_degenerate_nan() {
        let mut closest = ClosestPair {
            first: [7.0; 3],
            second: [8.0; 3],
        };
        let saved = closest;
        assert!(!capsule_capsule(
            &segment([0.0; 3], [0.0; 3], 1.0),
            &segment([3.0; 3], [3.0; 3], 1.0),
            &mut closest
        ));
        assert_eq!(closest, saved);
        let distance = point_segment([1.0; 3], [1.0; 3], [2.0; 3]);
        assert!(distance.parameter.is_nan() && distance.squared_distance.is_nan());
    }
}
