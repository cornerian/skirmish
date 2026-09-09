//! Six HSD quaternion routines from `sysdolphin/baselib/quatlib.c`.
//!
//! Angles are radians. Formulas retain the reference's operation order, including
//! its unusual interpolation of opposite quaternions. Scalar `glam` supplies
//! vector lengths, dot products, blends, trigonometry and axis-angle conversion.
//! The remaining formulas adapt HSD's ordering: generic quaternion multiply,
//! Euler/matrix conversion and slerp use different groupings or branches.
//! `libm` replaces platform math; final-bit PowerPC equivalence is unverified.

use glam::{Quat, Vec2, Vec3, Vec4};

pub type Vector = [f32; 3];
/// Quaternion components in x, y, z, w order.
pub type Quaternion = [f32; 4];
/// Row-major affine matrix, including the ignored translation column.
pub type Matrix = [[f32; 4]; 3];

/// `MatToQuat`: remove column scales, then extract the rotation.
/// Degenerate matrices produce the reference's floating-point NaNs/infinities.
pub fn from_matrix(m: &Matrix) -> Quaternion {
    let len: [f32; 3] = std::array::from_fn(|i| Vec3::new(m[0][i], m[1][i], m[2][i]).length());
    let trace = m[0][0] / len[0] + m[1][1] / len[1] + m[2][2] / len[2];
    if trace > 0.0 {
        let s = libm::sqrtf(1.0 + trace);
        let scale = 0.5 / s;
        [
            scale * (m[2][1] / len[1] - m[1][2] / len[2]),
            scale * (m[0][2] / len[2] - m[2][0] / len[0]),
            scale * (m[1][0] / len[0] - m[0][1] / len[1]),
            0.5 * s,
        ]
    } else {
        let mut i = usize::from(m[1][1] / len[1] > m[0][0] / len[0]);
        if m[2][2] / len[2] > m[i][i] / len[i] {
            i = 2;
        }
        let j = (i + 1) % 3;
        let k = (j + 1) % 3;
        let s = libm::sqrtf(1.0 + ((m[i][i] / len[i] - m[j][j] / len[j]) - m[k][k] / len[k]));
        let scale = 0.5 / s;
        let mut q = [0.0; 4];
        q[i] = 0.5 * s;
        q[3] = scale * (m[k][j] / len[j] - m[j][k] / len[k]);
        q[j] = scale * (m[j][i] / len[i] + m[i][j] / len[j]);
        q[k] = scale * (m[k][i] / len[i] + m[i][k] / len[k]);
        q
    }
}

/// `HSD_QuatLib_8037EB28`: extract XYZ Euler angles, including the pole branch.
pub fn matrix_to_euler(m: &Matrix) -> Vector {
    let len = Vec2::new(m[0][0], m[1][0]).length();
    // The unsuffixed C threshold is double precision.
    if f64::from(len) > 1e-5 {
        [
            libm::atan2f(m[2][1], m[2][2]),
            libm::atan2f(-m[2][0], len),
            libm::atan2f(m[1][0], m[0][0]),
        ]
    } else {
        [
            libm::atan2f(-m[1][2], m[1][1]),
            libm::atan2f(-m[2][0], len),
            0.0,
        ]
    }
}

/// `HSD_QuatLib_8037EC4C`: Hamilton product with the original summation order.
pub fn multiply(p: Quaternion, q: Quaternion) -> Quaternion {
    let [px, py, pz, pw] = p;
    let [qx, qy, qz, qw] = q;
    [
        qw * px + pw * qx + (py * qz - qy * pz),
        qw * py + pw * qy + (qx * pz - px * qz),
        qw * pz + pw * qz + (px * qy - qx * py),
        pw * qw - (pz * qz + (px * qx + py * qy)),
    ]
}

/// `HSD_QuatLib_8037ECE0`: normalize an axis and apply its angle.
/// `None` is the reference's -1 result, which leaves the output untouched.
pub fn from_axis_angle(axis: Vector, angle: f32) -> Option<Quaternion> {
    let axis = Vec3::from_array(axis);
    let len = axis.length();
    if len.abs() < f32::MIN_POSITIVE {
        return None;
    }
    Some(Quat::from_axis_angle(axis * (1.0 / len), angle).to_array())
}

/// `EulerToQuat`: compose XYZ Euler angles.
pub fn from_euler(euler: Vector) -> Quaternion {
    let (sine, cosine) = (Vec3::from_array(euler) * 0.5).sin_cos();
    let [cx, cy, cz] = cosine.to_array();
    let [sx, sy, sz] = sine.to_array();
    let ss = sy * sz;
    let cc = cy * cz;
    [
        sx * cc - cx * ss,
        cz * (cx * sy) + sz * (sx * cy),
        sz * (cx * cy) - cz * (sx * sy),
        cx * cc + sx * ss,
    ]
}

/// `HSD_QuatLib_8037EF28`: interpolate without normalizing or changing signs.
///
/// Opposite quaternions use the original two half-interval sine blends. The
/// reference's preliminary orthogonal output is overwritten for distinct inputs
/// and output, as used by both upstream callers. A generic shortest-path slerp
/// would change this behavior. Values outside 0..=1 are not clamped.
pub fn interpolate(p: Quaternion, q: Quaternion, t: f32) -> Quaternion {
    let (p, q) = (Vec4::from_array(p), Vec4::from_array(q));
    let cosine = p.dot(q);
    let (sp, sq) = if 1.0 + cosine > 1e-10 {
        if 1.0 - cosine > 1e-10 {
            let theta = libm::acosf(cosine);
            let sine = libm::sinf(theta);
            (
                libm::sinf((1.0 - t) * theta) / sine,
                libm::sinf(t * theta) / sine,
            )
        } else {
            ((1.0 - f64::from(t)) as f32, t)
        }
    } else {
        let t = if t < 0.5 { t } else { t - 0.5 };
        let doubled = 2.0 * t;
        (
            libm::sinf((std::f64::consts::FRAC_PI_2 * f64::from(1.0 - doubled)) as f32),
            libm::sinf((std::f64::consts::FRAC_PI_2 * f64::from(doubled)) as f32),
        )
    };
    (p * sp + q * sq).to_array()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsd_product_preserves_a_term_lost_by_generic_grouping() {
        let p = [1.0, 1.0, 1.0, 0.0];
        let q = [0.0, 16_777_216.0, 16_777_216.0, 1.0];
        assert_eq!(multiply(p, q)[0], 1.0);
        assert_eq!((Quat::from_array(p) * Quat::from_array(q)).x, 0.0);
    }
}
