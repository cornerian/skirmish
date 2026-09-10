//! `MakeTextureMtx` is checked against the pinned original C. Unrotated
//! matrices must agree bitwise; rotated ones use a documented tolerance for
//! the independent host and Rust trig libraries.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::{
    collision::bones::{IDENTITY, Matrix},
    presentation::texture_matrix::{
        TOBJ_FLT_EPSILON, TextureTransform, WrapMode, rotation_matrix, texture_matrix,
    },
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_tobj_make_mtx(
        rotate: *const f32,
        scale: *const f32,
        translate: *const f32,
        wrap_t: i32,
        repeat_s: u32,
        repeat_t: u32,
        out: *mut Matrix,
    );
    fn HSD_MkRotationMtx(out: *mut Matrix, rotation: *const f32);
}

fn reference_matrix(transform: &TextureTransform) -> Matrix {
    let mut result = IDENTITY;
    let wrap_t = match transform.wrap_t {
        WrapMode::Clamp => 0,
        WrapMode::Repeat => 1,
        WrapMode::Mirror => 2,
    };
    // SAFETY: each input array holds three live floats, the output twelve, and
    // the repeat counts are forwarded as the C adapter's unsigned integers.
    unsafe {
        oracle_tobj_make_mtx(
            transform.rotation.as_ptr(),
            transform.scale.as_ptr(),
            transform.translation.as_ptr(),
            wrap_t,
            u32::from(transform.repeat[0]),
            u32::from(transform.repeat[1]),
            &mut result,
        );
    }
    result
}

fn reference_rotation(rotation: [f32; 3]) -> Matrix {
    let mut result = IDENTITY;
    // SAFETY: the rotation holds three live floats and the output twelve.
    unsafe { HSD_MkRotationMtx(&mut result, rotation.as_ptr()) };
    result
}

fn close_matrix(actual: Matrix, expected: Matrix) {
    for (row, (a, e)) in actual.iter().zip(&expected).enumerate() {
        for (column, (a, e)) in a.iter().zip(e).enumerate() {
            let scale = 1.0_f32.max(e.abs());
            assert!(
                (a - e).abs() <= 1.0e-5 * scale,
                "[{row}][{column}] {a} != {e}"
            );
        }
    }
}

fn wrap_strategy() -> impl Strategy<Value = WrapMode> {
    prop_oneof![
        Just(WrapMode::Clamp),
        Just(WrapMode::Repeat),
        Just(WrapMode::Mirror)
    ]
}

fn scale_strategy() -> impl Strategy<Value = f32> {
    prop_oneof![
        -8.0_f32..8.0,
        Just(0.0_f32),
        Just(TOBJ_FLT_EPSILON),
        Just(-TOBJ_FLT_EPSILON / 2.0),
        1.0e-12_f32..1.0e-9,
        Just(1.0_f32),
    ]
}

proptest! {
    #[test]
    fn unrotated_texture_matrices_match_c_bitwise(
        scale in prop::array::uniform3(scale_strategy()),
        translation in prop::array::uniform3(-16.0_f32..16.0),
        repeat in prop::array::uniform2(1_u8..=8),
        wrap_t in wrap_strategy(),
    ) {
        let transform = TextureTransform {
            rotation: [0.0; 3],
            scale,
            translation,
            repeat,
            wrap_t,
        };
        let actual = texture_matrix(&transform).unwrap();
        let expected = reference_matrix(&transform);
        prop_assert_eq!(
            actual.map(|row| row.map(f32::to_bits)),
            expected.map(|row| row.map(f32::to_bits))
        );
    }

    #[test]
    fn rotated_texture_matrices_track_c_within_host_trig_tolerance(
        rotation in prop::array::uniform3(-6.3_f32..6.3),
        scale in prop::array::uniform3(0.125_f32..8.0),
        translation in prop::array::uniform3(-4.0_f32..4.0),
        repeat in prop::array::uniform2(1_u8..=4),
        wrap_t in wrap_strategy(),
    ) {
        let transform = TextureTransform {
            rotation,
            scale,
            translation,
            repeat,
            wrap_t,
        };
        close_matrix(texture_matrix(&transform).unwrap(), reference_matrix(&transform));
        close_matrix(rotation_matrix(rotation), reference_rotation(rotation));
    }
}

#[test]
fn zero_rotation_matrix_is_bitwise_identity_in_both_implementations() {
    let zero = [0.0_f32; 3];
    assert_eq!(rotation_matrix(zero), IDENTITY);
    assert_eq!(reference_rotation(zero), IDENTITY);
}
