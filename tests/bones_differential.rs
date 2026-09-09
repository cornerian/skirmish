//! Matrix arithmetic is checked bitwise against original scalar C. Euler SRT
//! uses a documented tolerance for the independent host and Rust trig libraries.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use melee_physics::bones::{
    Bone, BoneCapsule, IDENTITY, LocalTransform, Matrix, Pose, Vector, concat, srt,
    transform_point, transform_vector,
};
use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn C_MTXConcat(a: *const Matrix, b: *const Matrix, out: *mut Matrix);
    fn oracle_bones_transform(
        matrix: *const Matrix,
        input: *const f32,
        output: *mut f32,
        vector_only: i32,
    );
    fn oracle_bones_srt(
        scale: *const f32,
        rotation: *const f32,
        translation: *const f32,
        parent: *const f32,
        out: *mut Matrix,
    );
}

fn reference_srt(local: LocalTransform, parent: Option<Vector>) -> Matrix {
    let mut result = IDENTITY;
    let parent_ptr = parent
        .as_ref()
        .map_or(core::ptr::null(), |scale| scale.as_ptr());
    // SAFETY: every input has three live floats, the output has twelve, and the
    // optional parent pointer is either null or points to three live floats.
    unsafe {
        oracle_bones_srt(
            local.scale.as_ptr(),
            local.rotation.as_ptr(),
            local.translation.as_ptr(),
            parent_ptr,
            &mut result,
        )
    };
    result
}

fn reference_concat(a: &Matrix, b: &Matrix) -> Matrix {
    let mut result = IDENTITY;
    // SAFETY: all matrix references contain twelve valid contiguous floats.
    unsafe { C_MTXConcat(a, b, &mut result) };
    result
}

fn reference_transform(matrix: &Matrix, input: Vector, vector: bool) -> Vector {
    let mut result = [0.0; 3];
    // SAFETY: all input/output arrays contain the scalar elements C reads/writes.
    unsafe {
        oracle_bones_transform(
            matrix,
            input.as_ptr(),
            result.as_mut_ptr(),
            i32::from(vector),
        )
    };
    result
}

fn same(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn close_matrix(actual: Matrix, expected: Matrix) {
    for (a, b) in actual
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            (a - b).abs() <= 4e-6 * b.abs().max(1.0),
            "{actual:?} != {expected:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn scalar_matrix_composition_matches_c_bitwise(
        a in prop::array::uniform3(prop::array::uniform4(-100.0_f32..100.0)),
        b in prop::array::uniform3(prop::array::uniform4(-100.0_f32..100.0)),
    ) {
        let actual = concat(&a, &b);
        let expected = reference_concat(&a, &b);
        for (a, b) in actual.into_iter().flatten().zip(expected.into_iter().flatten()) { same(a, b); }
    }

    #[test]
    fn scalar_point_and_vector_transforms_match_c_bitwise(
        matrix in prop::array::uniform3(prop::array::uniform4(-100.0_f32..100.0)),
        point in prop::array::uniform3(-100.0_f32..100.0),
    ) {
        for (actual, vector) in [(transform_point(&matrix, point), false), (transform_vector(&matrix, point), true)] {
            let expected = reference_transform(&matrix, point, vector);
            for (a, b) in actual.into_iter().zip(expected) { same(a, b); }
        }
    }

    #[test]
    fn euler_srt_tracks_original_with_host_trig_tolerance(
        scale in prop::array::uniform3(-5.0_f32..5.0),
        rotation in prop::array::uniform3(-6.3_f32..6.3),
        translation in prop::array::uniform3(-100.0_f32..100.0),
        parent in prop::option::of(prop::array::uniform3(0.25_f32..4.0)),
    ) {
        let local = LocalTransform { scale, rotation, translation };
        close_matrix(srt(local, parent), reference_srt(local, parent));
    }

    #[test]
    fn zero_angle_scale_compensation_matches_c_bitwise(
        scale in prop::array::uniform3(-100.0_f32..100.0),
        translation in prop::array::uniform3(-100.0_f32..100.0),
        parent in prop::option::of(prop::array::uniform3(0.1_f32..10.0)),
    ) {
        let local = LocalTransform { scale, rotation: [0.0; 3], translation };
        for (a, b) in srt(local, parent).into_iter().flatten()
            .zip(reference_srt(local, parent).into_iter().flatten()) { same(a, b); }
    }
}

#[test]
fn matrix_signed_zero_and_nonfinite_results_preserve_c_classes() {
    for value in [
        -0.0,
        0.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        let matrix = [[value; 4]; 3];
        for vector in [false, true] {
            let point = [1.0, -1.0, 0.0];
            let actual = if vector {
                transform_vector(&matrix, point)
            } else {
                transform_point(&matrix, point)
            };
            for (a, b) in actual
                .into_iter()
                .zip(reference_transform(&matrix, point, vector))
            {
                same(a, b);
            }
        }
        for (a, b) in concat(&matrix, &IDENTITY)
            .into_iter()
            .flatten()
            .zip(reference_concat(&matrix, &IDENTITY).into_iter().flatten())
        {
            same(a, b);
        }
    }
}

#[test]
fn hierarchical_capsule_endpoints_match_original_matrix_operations() {
    let parent = LocalTransform {
        translation: [4.0, 5.0, 6.0],
        scale: [2.0, 3.0, 4.0],
        ..LocalTransform::default()
    };
    let child = LocalTransform {
        translation: [2.0, -3.0, 1.0],
        scale: [0.5; 3],
        ..LocalTransform::default()
    };
    let bones = [
        Bone {
            parent: Some(1),
            local: child,
            ..Bone::default()
        },
        Bone {
            local: parent,
            ..Bone::default()
        },
    ];
    let pose = Pose::evaluate(&bones).unwrap();
    let matrix = reference_concat(
        &reference_srt(parent, None),
        &reference_srt(child, Some(parent.scale)),
    );
    let capsule = BoneCapsule {
        bone: 0,
        start: [-1.0, 2.0, 0.0],
        end: [1.0, 3.0, 0.0],
        radius: 0.75,
    };
    let world = capsule.transform(&pose, 1.0).unwrap();
    assert_eq!(
        world.start,
        reference_transform(&matrix, capsule.start, false)
    );
    assert_eq!(world.end, reference_transform(&matrix, capsule.end, false));
    assert_eq!(world.radius, 0.75);
}
