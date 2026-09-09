//! Exact native scalar C comparisons for lbColl_80006E58 and HSD_MtxInverse.
//! Adapters reuse existing source snapshots; every finite result compares bits.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::{
    collision::{
        bones::{self, Matrix},
        shield::{self, Contact},
        sweep::ClosestPair,
    },
    fighter::combat::Capsule,
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_shield_inverse(input: *const f32, output: *mut f32);
    fn oracle_shield_collision(input: *const f32, output: *mut f32) -> i32;
}

fn compare(input: [f32; 27]) {
    let hit = Capsule {
        start: input[..3].try_into().unwrap(),
        end: input[3..6].try_into().unwrap(),
        radius: input[24],
    };
    let hurt = Capsule {
        start: input[6..9].try_into().unwrap(),
        end: input[9..12].try_into().unwrap(),
        radius: input[25],
    };
    let matrix: Matrix =
        core::array::from_fn(|i| input[12 + i * 4..16 + i * 4].try_into().unwrap());
    let mut expected = [11.0, -0.0, 13.0, 14.0, 15.0, -0.0, 17.0, 18.0, 19.0, -20.0];
    let mut contact = Contact {
        closest: ClosestPair {
            first: expected[..3].try_into().unwrap(),
            second: expected[3..6].try_into().unwrap(),
        },
        position: expected[6..9].try_into().unwrap(),
        overlap: expected[9],
    };
    let saved = contact;
    // SAFETY: adapter copies exactly 27 inputs and 10 initialized output floats;
    // the selected routines have no shared mutable state or retained pointers.
    let c_hit = unsafe { oracle_shield_collision(input.as_ptr(), expected.as_mut_ptr()) } != 0;
    let actual = shield::capsule_matrix(&hit, &hurt, &matrix, input[26], &mut contact);
    if expected.iter().any(|value| !value.is_finite()) {
        assert_eq!(actual, Err(shield::CollisionError::NonFinite), "{input:?}");
        assert_eq!(contact, saved);
    } else {
        assert_eq!(actual, Ok(c_hit), "{input:?}; C={expected:?}");
        let actual: Vec<_> = contact
            .closest
            .first
            .into_iter()
            .chain(contact.closest.second)
            .chain(contact.position)
            .chain([contact.overlap])
            .map(f32::to_bits)
            .collect();
        assert_eq!(actual, expected.map(f32::to_bits), "{input:?}");
    }
}

fn input(hit: Capsule, hurt: Capsule, matrix: Matrix, broadphase: f32) -> [f32; 27] {
    let mut values = [0.0; 27];
    values[..3].copy_from_slice(&hit.start);
    values[3..6].copy_from_slice(&hit.end);
    values[6..9].copy_from_slice(&hurt.start);
    values[9..12].copy_from_slice(&hurt.end);
    values[12..24].copy_from_slice(matrix.as_flattened());
    values[24..].copy_from_slice(&[hit.radius, hurt.radius, broadphase]);
    values
}

fn compare_inverse(values: [f32; 12]) {
    let matrix = core::array::from_fn(|i| values[i * 4..i * 4 + 4].try_into().unwrap());
    let mut expected = [0.0; 12];
    // SAFETY: adapter copies 12 input floats and writes 12 output floats.
    unsafe { oracle_shield_inverse(values.as_ptr(), expected.as_mut_ptr()) };
    let actual = shield::inverse(&matrix);
    if expected.iter().all(|value| value.is_finite()) {
        assert_eq!(
            actual
                .unwrap()
                .into_iter()
                .flatten()
                .map(f32::to_bits)
                .collect::<Vec<_>>(),
            expected.map(f32::to_bits),
            "{values:?}"
        );
    } else {
        assert_eq!(actual, Err(shield::CollisionError::NonFinite));
    }
}

#[test]
fn degenerate_parallel_scaled_translated_and_reflected_contacts_match_c() {
    for length in [0.0, 0.001, 0.003, 0.0032, 1.0, 10.0] {
        for offset in [-10.0, -1.0, -0.0, 0.0, 0.001, 1.0, 10.0] {
            for scale in [0.0, 0.0001, 0.5, 1.0, 3.0, -2.0] {
                let matrix = [
                    [scale, 0.2, 0.0, 3.0],
                    [0.0, 1.5, 0.0, -2.0],
                    [0.0, 0.0, 2.0, 1.0],
                ];
                for radius in [0.0, 0.5, 2.0] {
                    let hit = Capsule {
                        start: [0.0; 3],
                        end: [length, 0.0, 0.0],
                        radius,
                    };
                    let hurt = Capsule {
                        start: [offset, 1.0, 0.0],
                        end: [offset + length, 1.0, 0.0],
                        radius: 1.0,
                    };
                    compare(input(hit, hurt, matrix, 20.0));
                    compare(input(hurt, hit, matrix, 0.0));
                }
            }
        }
    }
}

#[test]
fn coincident_axes_and_inverse_threshold_preserve_source_zero_bits() {
    let zero = Capsule {
        start: [0.0; 3],
        end: [0.0; 3],
        radius: 0.0,
    };
    for mask in 0..4096 {
        let mut values = input(zero, zero, bones::IDENTITY, 20.0);
        for (i, value) in values[..12].iter_mut().enumerate() {
            *value = if mask & (1 << i) != 0 { -0.0 } else { 0.0 };
        }
        compare(values);
    }
    for determinant in [
        0.0,
        -0.0,
        f32::from_bits(1),
        0.999e-10,
        1.0e-10,
        1.001e-10,
        -1.0e-10,
    ] {
        compare_inverse([
            determinant,
            0.0,
            -0.0,
            3.0,
            0.0,
            1.0,
            0.0,
            -4.0,
            0.0,
            0.0,
            1.0,
            5.0,
        ]);
    }
}

#[test]
fn finite_translation_cancellation_reports_the_sources_nonfinite_result() {
    let hit = Capsule {
        start: [1.0, 0.0, 0.0],
        end: [1.0, 0.0, 0.0],
        radius: 0.5,
    };
    let hurt = Capsule {
        start: [0.0; 3],
        end: [0.0; 3],
        radius: 1.0,
    };
    let mut matrix = bones::IDENTITY;
    matrix[0][3] = 1.0e10;
    compare(input(hit, hurt, matrix, 20.0));
}

proptest! {
    #![proptest_config(ProptestConfig {
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x06e58),
        ..ProptestConfig::with_cases(2048)
    })]
    #[test]
    fn arbitrary_matrix_contacts_match_c(
        coordinates in prop::array::uniform12(-20_f32..20_f32),
        matrix in prop::array::uniform12(-3_f32..3_f32),
        radii in prop::array::uniform2(0_f32..5_f32),
    ) {
        let mut values = [0.0;27];
        values[..12].copy_from_slice(&coordinates);
        values[12..24].copy_from_slice(&matrix);
        values[24..26].copy_from_slice(&radii);
        values[26]=20.0;
        compare(values);
        // Shield endpoints coincide, as in lbColl_80007BCC.
        values.copy_within(6..9,9);
        compare(values);
        compare_inverse(matrix);
    }
}
