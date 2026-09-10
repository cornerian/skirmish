//! HSD texture-coordinate matrices, translated from `MakeTextureMtx` in the
//! pinned `tobj.c`.
//!
//! The runtime builds one matrix per TObj from its authored rotation, the
//! animated scale/translation, the integer repeat counts, and the T wrap mode,
//! then GX applies rows 0 and 1 to the `(s, t, 1, 1)` input of a regular
//! texgen. Only the scalar Dolphin matrix paths are reproduced; `sinf`/`cosf`
//! come from `libm`, so rotated matrices agree with the host C reference
//! numerically rather than bitwise.

use crate::collision::bones::{self, Matrix};

/// `tobj.c` defines its own `FLT_EPSILON` before `MakeTextureMtx`; it is far
/// smaller than the C standard value and decides when a scale collapses to 0.
/// The upstream digits are kept verbatim; they round to the same binary32 as
/// `1e-10`, which the C differential checks bitwise.
#[allow(clippy::excessive_precision)]
pub const TOBJ_FLT_EPSILON: f32 = 1.000_000_013_35e-10;

/// GX texture wrap mode in its native enumeration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WrapMode {
    Clamp,
    Repeat,
    Mirror,
}

impl WrapMode {
    pub const fn from_gx(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Clamp),
            1 => Some(Self::Repeat),
            2 => Some(Self::Mirror),
            _ => None,
        }
    }
}

/// Complete TObj state consumed by `MakeTextureMtx`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureTransform {
    /// Authored Euler rotation; the runtime stores it in the quaternion's x/y/z.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub translation: [f32; 3],
    /// `repeat_s`, `repeat_t`; HSD asserts both are non-zero.
    pub repeat: [u8; 2],
    pub wrap_t: WrapMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextureMatrixError {
    #[error("HSD asserts non-zero texture repeat counts; found {0:?}")]
    ZeroRepeat([u8; 2]),
}

/// Original `HSD_MkRotationMtx`.
pub fn rotation_matrix(rotation: [f32; 3]) -> Matrix {
    let (sin_x, cos_x) = (libm::sinf(rotation[0]), libm::cosf(rotation[0]));
    let (sin_y, cos_y) = (libm::sinf(rotation[1]), libm::cosf(rotation[1]));
    let (sin_z, cos_z) = (libm::sinf(rotation[2]), libm::cosf(rotation[2]));
    let temp1 = sin_x * sin_y;
    let temp2 = cos_x * sin_y;
    [
        [
            cos_y * cos_z,
            (cos_z * temp1) - (cos_x * sin_z),
            (cos_z * temp2) + (sin_x * sin_z),
            0.0,
        ],
        [
            cos_y * sin_z,
            (sin_z * temp1) + (cos_x * cos_z),
            (sin_z * temp2) - (sin_x * cos_z),
            0.0,
        ],
        [-sin_y, sin_x * cos_y, cos_x * cos_y, 0.0],
    ]
}

fn translation_matrix(x: f32, y: f32, z: f32) -> Matrix {
    [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]
}

fn scale_matrix(x: f32, y: f32, z: f32) -> Matrix {
    [[x, 0.0, 0.0, 0.0], [0.0, y, 0.0, 0.0], [0.0, 0.0, z, 0.0]]
}

/// Original `MakeTextureMtx`: `Scale * Rotation * Translation` with the
/// repeat counts folded into the scale and the mirror offset into T.
pub fn texture_matrix(transform: &TextureTransform) -> Result<Matrix, TextureMatrixError> {
    let [repeat_s, repeat_t] = transform.repeat;
    if repeat_s == 0 || repeat_t == 0 {
        return Err(TextureMatrixError::ZeroRepeat(transform.repeat));
    }
    let [sx, sy, sz] = transform.scale;
    let scale = [
        if libm::fabsf(sx) < TOBJ_FLT_EPSILON {
            0.0
        } else {
            f32::from(repeat_s) / sx
        },
        if libm::fabsf(sy) < TOBJ_FLT_EPSILON {
            0.0
        } else {
            f32::from(repeat_t) / sy
        },
        sz,
    ];
    let rotation = [
        transform.rotation[0],
        transform.rotation[1],
        -transform.rotation[2],
    ];
    let [tx, ty, tz] = transform.translation;
    let mirror = if transform.wrap_t == WrapMode::Mirror {
        1.0 / (f32::from(repeat_t) / sy)
    } else {
        0.0
    };
    let translation = [-tx, -(ty + mirror), tz];

    let mut matrix = translation_matrix(translation[0], translation[1], translation[2]);
    matrix = bones::concat(&rotation_matrix(rotation), &matrix);
    matrix = bones::concat(&scale_matrix(scale[0], scale[1], scale[2]), &matrix);
    Ok(matrix)
}

/// Apply a texture matrix the way a regular GX texgen does: rows 0 and 1 over
/// the `(s, t, 1, 1)` input, so the z column contributes like a translation.
pub fn apply_texcoord(matrix: &Matrix, st: [f32; 2]) -> [f32; 2] {
    std::array::from_fn(|row| {
        matrix[row][0] * st[0] + matrix[row][1] * st[1] + matrix[row][2] + matrix[row][3]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_transform() -> TextureTransform {
        TextureTransform {
            rotation: [0.0; 3],
            scale: [1.0; 3],
            translation: [0.0; 3],
            repeat: [1, 1],
            wrap_t: WrapMode::Clamp,
        }
    }

    #[test]
    fn authored_identity_yields_the_identity_texgen() {
        let matrix = texture_matrix(&identity_transform()).unwrap();
        assert_eq!(matrix, bones::IDENTITY);
        assert_eq!(apply_texcoord(&matrix, [0.25, 0.75]), [0.25, 0.75]);
    }

    #[test]
    fn translation_moves_the_sampled_window_backwards_and_repeat_scales_it() {
        let matrix = texture_matrix(&TextureTransform {
            translation: [0.5, -0.25, 0.0],
            repeat: [2, 1],
            ..identity_transform()
        })
        .unwrap();
        assert_eq!(apply_texcoord(&matrix, [0.5, 0.5]), [0.0, 0.75]);
        assert_eq!(apply_texcoord(&matrix, [1.0, 0.0]), [1.0, 0.25]);
    }

    #[test]
    fn mirror_wrap_offsets_t_by_one_repeated_period() {
        let mirrored = texture_matrix(&TextureTransform {
            scale: [1.0, 2.0, 1.0],
            repeat: [1, 4],
            wrap_t: WrapMode::Mirror,
            ..identity_transform()
        })
        .unwrap();
        // 1 / (repeat_t / scale.y) = 0.5 is subtracted from t before scaling by 2.
        assert_eq!(apply_texcoord(&mirrored, [0.0, 1.0]), [0.0, 1.0]);
        assert_eq!(apply_texcoord(&mirrored, [0.0, 0.5]), [0.0, 0.0]);
    }

    #[test]
    fn tobj_epsilon_collapses_only_sub_epsilon_scales() {
        let collapsed = texture_matrix(&TextureTransform {
            scale: [1.0e-11, 1.0, 1.0],
            ..identity_transform()
        })
        .unwrap();
        assert_eq!(collapsed[0][0], 0.0);
        let kept = texture_matrix(&TextureTransform {
            scale: [1.0e-7, 1.0, 1.0],
            ..identity_transform()
        })
        .unwrap();
        assert_eq!(kept[0][0], 1.0 / 1.0e-7);
        assert_eq!(
            texture_matrix(&TextureTransform {
                repeat: [0, 1],
                ..identity_transform()
            }),
            Err(TextureMatrixError::ZeroRepeat([0, 1]))
        );
    }

    #[test]
    fn z_rotation_is_negated_before_the_rotation_matrix() {
        let transform = TextureTransform {
            rotation: [0.0, 0.0, 0.5],
            ..identity_transform()
        };
        let matrix = texture_matrix(&transform).unwrap();
        let expected = rotation_matrix([0.0, 0.0, -0.5]);
        assert_eq!(matrix[0][..3], expected[0][..3]);
        assert_eq!(matrix[1][..3], expected[1][..3]);
        assert_eq!(WrapMode::from_gx(2), Some(WrapMode::Mirror));
        assert_eq!(WrapMode::from_gx(3), None);
    }
}
