use skirmish::quaternion::{self as quat, Matrix};

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

fn close(actual: &[f32], expected: &[f32]) {
    for (&a, &b) in actual.iter().zip(expected) {
        if a.is_nan() && b.is_nan() {
            continue;
        }
        if a == b {
            continue;
        }
        assert!(
            (a - b).abs() <= 4e-6 * b.abs().max(1.0),
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn identity_scale_and_half_turns() {
    assert_eq!(quat::from_matrix(&IDENTITY), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(quat::matrix_to_euler(&IDENTITY), [0.0, 0.0, 0.0]);
    assert_eq!(quat::from_euler([0.0; 3]), [0.0, 0.0, 0.0, 1.0]);
    for axis in 0..3 {
        let mut matrix = [[0.0; 4]; 3];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = if i == axis { 2.0 } else { -3.0 };
            row[3] = 123.0;
        }
        let mut expected = [0.0; 4];
        expected[axis] = 1.0;
        assert_eq!(quat::from_matrix(&matrix), expected);
    }
}

#[test]
fn quaternion_order_axis_normalization_and_poles() {
    assert_eq!(
        quat::multiply([1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]),
        [24.0, 48.0, 48.0, -6.0]
    );
    assert_eq!(quat::from_axis_angle([0.0; 3], 1.0), None);
    assert_eq!(
        quat::from_axis_angle([f32::MIN_POSITIVE, 0.0, 0.0], 1.0),
        None
    );
    close(
        &quat::from_axis_angle([0.0, 0.0, 7.0], std::f32::consts::PI).unwrap(),
        &[0.0, 0.0, 1.0, 0.0],
    );
    let pole = [
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0],
    ];
    close(
        &quat::matrix_to_euler(&pole),
        &[0.0, std::f32::consts::FRAC_PI_2, 0.0],
    );
}

#[test]
fn interpolation_preserves_hsd_antipodal_behavior() {
    let p = [1.0, 0.0, 0.0, 0.0];
    let q = [-1.0, 0.0, 0.0, 0.0];
    for (t, expected) in [
        (0.0, 1.0),
        (0.25, 0.0),
        (0.5, 1.0),
        (0.75, 0.0),
        (1.0, -1.0),
    ] {
        close(&quat::interpolate(p, q, t), &[expected, 0.0, 0.0, 0.0]);
    }
    for t in [-2.0, 0.0, 0.5, 1.0, 2.0] {
        close(&quat::interpolate(p, p, t), &p);
    }
}

#[cfg(feature = "c-oracle")]
#[allow(unsafe_code)]
mod oracle {
    use super::*;
    use proptest::prelude::*;
    use skirmish::quaternion::Quaternion;

    unsafe extern "C" {
        fn oracle_quaternion(op: u32, a: *const f32, b: *const f32, t: f32, out: *mut f32) -> i32;
    }

    fn reference(op: u32, a: &[f32], b: &[f32], t: f32) -> (i32, Quaternion) {
        assert!(
            a.len()
                >= match op {
                    0 | 1 => 12,
                    2 | 5 => 4,
                    3 | 4 => 3,
                    _ => panic!("invalid test op"),
                }
        );
        assert!(!matches!(op, 2 | 5) || b.len() >= 4);
        let mut out = [99.0; 4];
        // SAFETY: lengths above match the C adapter's reads. Four output scalars
        // are initialized, writable and separate from both input allocations.
        let status = unsafe { oracle_quaternion(op, a.as_ptr(), b.as_ptr(), t, out.as_mut_ptr()) };
        (status, out)
    }

    fn exact(actual: &[f32], expected: &[f32]) {
        for (&a, &b) in actual.iter().zip(expected) {
            if a.is_nan() && b.is_nan() {
                continue;
            }
            assert_eq!(a.to_bits(), b.to_bits(), "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn branches_and_error_status_match_upstream() {
        for axis in [
            [0.0; 3],
            [f32::MIN_POSITIVE, 0.0, 0.0],
            [f32::NAN, 0.0, 0.0],
        ] {
            let (status, expected) = reference(3, &axis, &[], 1.0);
            match quat::from_axis_angle(axis, 1.0) {
                None => {
                    assert_eq!(status, -1);
                    assert_eq!(expected, [99.0; 4]);
                }
                Some(actual) => {
                    assert_eq!(status, 0);
                    close(&actual, &expected);
                }
            }
        }
        let mut matrices = vec![IDENTITY, [[0.0; 4]; 3]];
        for axis in 0..3 {
            let mut m = IDENTITY;
            for (i, row) in m.iter_mut().enumerate() {
                row[i] = if i == axis { 2.0 } else { -3.0 };
            }
            matrices.push(m);
        }
        for near_pole in [0.0, 1e-6, f32::from_bits(0x3727_c5ac), 1e-4] {
            let mut m = IDENTITY;
            m[0][0] = near_pole;
            m[2][0] = -1.0;
            matrices.push(m);
        }
        for m in matrices {
            exact(
                &quat::from_matrix(&m),
                &reference(0, m.as_flattened(), &[], 0.0).1,
            );
            close(
                &quat::matrix_to_euler(&m),
                &reference(1, m.as_flattened(), &[], 0.0).1[..3],
            );
        }
        for p in [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [2.0, 1.0, 0.0, 0.0],
        ] {
            for q in [p, p.map(|v| -v)] {
                for t in [-1.0, 0.0, 0.25, 0.5, 0.75, 1.0, 2.0, f32::NAN] {
                    close(&quat::interpolate(p, q, t), &reference(5, &p, &q, t).1);
                }
            }
        }
    }

    proptest! {
        #[test]
        fn multiplication_matches_c_bits(p in prop::array::uniform4(-100_f32..100_f32), q in prop::array::uniform4(-100_f32..100_f32)) {
            exact(&quat::multiply(p, q), &reference(2, &p, &q, 0.0).1);
        }

        #[test]
        fn matrix_operations_match_c(m in prop::array::uniform3(prop::array::uniform4(-10_f32..10_f32))) {
            exact(&quat::from_matrix(&m), &reference(0, m.as_flattened(), &[], 0.0).1);
            close(&quat::matrix_to_euler(&m), &reference(1, m.as_flattened(), &[], 0.0).1[..3]);
        }

        #[test]
        fn rotations_and_interpolation_match_c(
            axis in prop::array::uniform3(-100_f32..100_f32),
            euler in prop::array::uniform3(-3_f32..3_f32),
            other in prop::array::uniform3(-3_f32..3_f32), t in -1_f32..2_f32,
        ) {
            let (status, expected) = reference(3, &axis, &[], t);
            match quat::from_axis_angle(axis, t) {
                None => prop_assert_eq!(status, -1),
                Some(actual) => { prop_assert_eq!(status, 0); close(&actual, &expected); }
            }
            let p = quat::from_euler(euler);
            close(&p, &reference(4, &euler, &[], 0.0).1);
            let q = quat::from_euler(other);
            close(&quat::interpolate(p, q, t), &reference(5, &p, &q, t).1);
        }
    }
}
