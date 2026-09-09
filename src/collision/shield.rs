//! Matrix-aware capsule contact from lbColl_80006E58 and the ordinary geometric
//! branch of lbColl_80007BCC. Endpoints are world coordinates; `matrix` maps the
//! hurt volume's local coordinates to world coordinates. Its inverse determines
//! the directional hurt radius, including nonuniform scale and shear.
//!
//! This preserves source endpoint ties, inclusive touching, and HSD's identity
//! fallback for nearly singular matrices. It computes contact/overlap, not time
//! of impact. Bone-position caching, forced hits, and caller-specific transforms
//! from lbColl_80007BCC remain the caller's responsibility.
use super::{bones, sweep};
use crate::fighter::combat::Capsule;
use bones::{Matrix, Vector};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub closest: sweep::ClosestPair,
    pub position: Vector,
    /// Signed source overlap; a narrow miss writes a negative value.
    pub overlap: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CollisionError {
    #[error("collision inputs must be finite with nonnegative radii and broadphase scale")]
    InvalidInput,
    #[error("matrix contact arithmetic produced a nonfinite result")]
    NonFinite,
}

/// HSD_MtxInverse. The source substitutes identity when |determinant| < 1e-10;
/// it does not divide by zero or report a singular-matrix error in that branch.
pub fn inverse(matrix: &Matrix) -> Result<Matrix, CollisionError> {
    if !matrix.iter().flatten().all(|value| value.is_finite()) {
        return Err(CollisionError::InvalidInput);
    }
    let m = matrix;
    let determinant =
        m[0][0] * m[1][1] * m[2][2] + m[0][1] * m[1][2] * m[2][0] + m[0][2] * m[1][0] * m[2][1]
            - m[2][0] * m[1][1] * m[0][2]
            - m[1][0] * m[0][1] * m[2][2]
            - m[0][0] * m[2][1] * m[1][2];
    if determinant.abs() < 0.0000000001 {
        return Ok(bones::IDENTITY);
    }
    let d = 1.0 / determinant;
    let mut out = [
        [
            (m[1][1] * m[2][2] - m[2][1] * m[1][2]) * d,
            -(m[0][1] * m[2][2] - m[2][1] * m[0][2]) * d,
            (m[0][1] * m[1][2] - m[1][1] * m[0][2]) * d,
            0.0,
        ],
        [
            -(m[1][0] * m[2][2] - m[2][0] * m[1][2]) * d,
            (m[0][0] * m[2][2] - m[2][0] * m[0][2]) * d,
            -(m[0][0] * m[1][2] - m[1][0] * m[0][2]) * d,
            0.0,
        ],
        [
            (m[1][0] * m[2][1] - m[2][0] * m[1][1]) * d,
            -(m[0][0] * m[2][1] - m[2][0] * m[0][1]) * d,
            (m[0][0] * m[1][1] - m[1][0] * m[0][1]) * d,
            0.0,
        ],
    ];
    for row in &mut out {
        row[3] = -(row[2] * m[2][3] - (-row[0] * m[0][3] - row[1] * m[1][3]));
    }
    if !out.iter().flatten().all(|value| value.is_finite()) {
        return Err(CollisionError::NonFinite);
    }
    Ok(out)
}

/// lbColl_80006E58 on finite physical inputs. AABB misses leave `contact`
/// unchanged; narrow misses write closest points, surface contact and overlap.
/// Errors leave it unchanged too. The broadphase scale is explicit and is not
/// inferred from the matrix: the shield caller supplies 20 * its native scale.
pub fn capsule_matrix(
    hit: &Capsule,
    hurt: &Capsule,
    matrix: &Matrix,
    broadphase_scale: f32,
    contact: &mut Contact,
) -> Result<bool, CollisionError> {
    if !hit
        .start
        .into_iter()
        .chain(hit.end)
        .chain(hurt.start)
        .chain(hurt.end)
        .chain(matrix.iter().flatten().copied())
        .all(|value| value.is_finite())
        || ![hit.radius, hurt.radius, broadphase_scale]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    {
        return Err(CollisionError::InvalidInput);
    }
    let broadphase_radius = hurt.radius * broadphase_scale + hit.radius;
    if !broadphase_radius.is_finite() {
        return Err(CollisionError::NonFinite);
    }
    if !sweep::passes_broadphase(hit, hurt, broadphase_radius) {
        return Ok(false);
    }
    let closest = sweep::closest_axes(hit, hurt);
    let separation = sweep::sub(closest.first, closest.second);
    let distance = libm::sqrtf(sweep::dot(separation, separation));
    let (position, overlap, collides) = if sweep::approximately_zero(distance) {
        (closest.first, (hit.radius + hurt.radius) - distance, true)
    } else {
        let inverse = inverse(matrix)?;
        // Transform each point separately: cancelling translation algebraically
        // changes rounding and loses the original large-coordinate behavior.
        let local_hit = bones::transform_point(&inverse, closest.first);
        let local_hurt = bones::transform_point(&inverse, closest.second);
        let local_delta = sweep::sub(local_hit, local_hurt);
        let local_distance = libm::sqrtf(sweep::dot(local_delta, local_delta));
        let hurt_radius = (hurt.radius * distance) / local_distance;
        let lerp = hurt_radius / distance;
        let allowed = hit.radius + hurt_radius;
        let position = core::array::from_fn(|i| {
            lerp * (closest.first[i] - closest.second[i]) + closest.second[i]
        });
        (position, allowed - distance, allowed >= distance)
    };
    if !closest
        .first
        .into_iter()
        .chain(closest.second)
        .chain(position)
        .chain([overlap])
        .all(|value| value.is_finite())
    {
        return Err(CollisionError::NonFinite);
    }
    *contact = Contact {
        closest,
        position,
        overlap,
    };
    Ok(collides)
}

/// lbColl_80007BCC's ordinary shield test after bone position/matrix updates.
/// `hit.radius` already includes the caller's hitbox scale; shield size and the
/// 20 * broadphase scale remain independent of the matrix's scaled basis.
pub fn shield_contact(
    hit: &Capsule,
    center: Vector,
    matrix: &Matrix,
    radius: f32,
    broadphase_scale: f32,
) -> Result<Option<Contact>, CollisionError> {
    let hurt = Capsule {
        start: center,
        end: center,
        radius,
    };
    let mut contact = Contact::default();
    Ok(capsule_matrix(hit, &hurt, matrix, broadphase_scale, &mut contact)?.then_some(contact))
}
