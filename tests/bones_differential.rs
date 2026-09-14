//! Matrix arithmetic is checked bitwise against original scalar C. Euler SRT
//! uses a documented tolerance for the independent host and Rust trig libraries.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::collision::bones::{
    Bone, BoneCapsule, IDENTITY, LocalTransform, Matrix, Pose, Quaternion, Vector, concat, srt,
    srt_quat, transform_point, transform_vector,
};

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
    /// `tests/oracle/bones_pose.c`: builds `count` `HSD_JObj`s from the flat
    /// per-axis arrays (bone order, matching `bones::Bone`/`LocalTransform`'s
    /// own field order) and calls the real, pinned `HSD_JObjMakeMatrix`
    /// (`sysdolphin/baselib/jobj.c`) on each in the supplied order, writing
    /// every joint's resulting 3x4 world matrix into `out_matrices`
    /// (row-major, 12 floats per joint). `parent[i] < 0` means no parent;
    /// every other `parent[i]` must be `< i` (already resolved).
    /// `use_quaternion[i]` selects `JOBJ_USE_QUATERNION`, driving that
    /// joint's rotation from the corresponding four floats of `quaternion`
    /// (x, y, z, w) through `HSD_MtxSRTQuat` instead of `rotation`/
    /// `HSD_MtxSRT`.
    fn oracle_bones_pose(
        parent: *const i32,
        classical_scale: *const i32,
        scale: *const f32,
        rotation: *const f32,
        translation: *const f32,
        use_quaternion: *const i32,
        quaternion: *const f32,
        count: i32,
        out_matrices: *mut f32,
    );
}

/// A single root `HSD_JObj` (no parent) has no separate concat step, so its
/// world matrix *is* its own local `HSD_MtxSRTQuat` result -- a direct,
/// uncomposed comparison against `srt_quat(.., None)`.
fn reference_srt_quat(scale: Vector, quaternion: Quaternion, translation: Vector) -> Matrix {
    let mut out = [0.0_f32; 12];
    let rotation = [0.0_f32; 3];
    // SAFETY: every input array has exactly the documented number of live
    // elements for a single joint (`count == 1`), and `out` has the twelve
    // floats `oracle_bones_pose` writes per joint.
    unsafe {
        oracle_bones_pose(
            [-1_i32].as_ptr(),
            [0_i32].as_ptr(),
            scale.as_ptr(),
            rotation.as_ptr(),
            translation.as_ptr(),
            [1_i32].as_ptr(),
            quaternion.as_ptr(),
            1,
            out.as_mut_ptr(),
        )
    };
    core::array::from_fn(|row| core::array::from_fn(|column| out[row * 4 + column]))
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
        // The absolute floor (not just a scaled-by-`b` relative term) covers
        // `sinf`/`cosf`'s own bounded absolute difference from this crate's
        // real trigonometry oracle (`docs/math.md`'s fused-multiply-add
        // finding), amplified by up to `scale`'s `5.0` and composed rotation
        // multiplies -- not meaningful near a matrix entry that itself
        // rounds to nearly zero.
        assert!(
            (a - b).abs() <= 4e-6 * b.abs().max(1.0) + 2e-3,
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
        let local = LocalTransform { scale, rotation, rotation_quaternion: None, translation };
        close_matrix(srt(local, parent), reference_srt(local, parent));
    }

    #[test]
    fn quaternion_srt_tracks_original_with_host_trig_free_tolerance(
        scale in prop::array::uniform3(-5.0_f32..5.0),
        quaternion in prop::array::uniform4(-5.0_f32..5.0)
            .prop_filter("avoid MTXQuat's own zero-quaternion assert/divide", |q| {
                q.iter().map(|x| x * x).sum::<f32>() >= 0.25
            }),
        translation in prop::array::uniform3(-100.0_f32..100.0),
    ) {
        let actual = srt_quat(scale, quaternion, translation, None);
        let expected = reference_srt_quat(scale, quaternion, translation);
        // `HSD_MtxSRTQuat` has no `sinf`/`cosf` of its own (`MTXQuat` is
        // pure multiply/add), so unlike the Euler `srt` above this has no
        // host-libm-vs-MSL trig noise to tolerate; `close_matrix`'s wider
        // bound is kept anyway since `scale`/`quaternion` here range much
        // larger than the capsule/hierarchy unit tests below.
        close_matrix(actual, expected);
    }

    #[test]
    fn zero_angle_scale_compensation_matches_c_bitwise(
        scale in prop::array::uniform3(-100.0_f32..100.0),
        translation in prop::array::uniform3(-100.0_f32..100.0),
        parent in prop::option::of(prop::array::uniform3(0.1_f32..10.0)),
    ) {
        let local = LocalTransform { scale, rotation: [0.0; 3], rotation_quaternion: None, translation };
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

/// `Pose::evaluate` end to end (not just `srt_quat` in isolation) on a
/// two-joint hierarchy: a plain-Euler scaled parent and a quaternion-flagged
/// child, exercising `HSD_JObjMakeMatrix`'s own parent-scale-compensated
/// `HSD_MtxSRTQuat` branch (`jobj.c:167-174`) through the real pinned oracle,
/// the same joint-count/topology shape `real_pose`'s 73-joint test drives
/// for the Euler-only branch.
#[test]
fn quaternion_joint_matches_oracle_under_a_scaled_parent() {
    let parent = LocalTransform {
        translation: [4.0, 5.0, 6.0],
        scale: [2.0, 3.0, 4.0],
        ..LocalTransform::default()
    };
    // A quarter-turn about Y as a quaternion, `EulerToQuat`-equivalent but
    // supplied directly here to isolate `srt_quat`/`HSD_MtxSRTQuat` from
    // `quaternion::from_euler`.
    let half = core::f32::consts::FRAC_PI_4;
    let child = LocalTransform {
        translation: [2.0, -3.0, 1.0],
        scale: [0.5, 1.5, 0.25],
        rotation_quaternion: Some([0.0, half.sin(), 0.0, half.cos()]),
        ..LocalTransform::default()
    };
    let bones = [
        Bone {
            local: parent,
            ..Bone::default()
        },
        Bone {
            parent: Some(0),
            local: child,
            ..Bone::default()
        },
    ];
    let rust = Pose::evaluate(&bones).unwrap();

    // One combined call, exactly like `real_pose::oracle_pose` below: the
    // driver builds the real parent/child `HSD_JObj` link itself, so
    // `has_scl(jobj->parent)`/`jobj->parent->scl` supplies the correct
    // accumulated parent scale to the child's own `HSD_MtxSRTQuat` call,
    // and `PSMTXConcat` composes the world matrix internally -- unlike two
    // separate single-joint calls, which would each see `parent == NULL`
    // and silently skip the parent-scale compensation this test exists to
    // cover.
    let parent_idx = [-1_i32, 0];
    let classical = [0_i32, 0];
    let use_quaternion = [0_i32, 1];
    let scale: Vec<f32> = parent.scale.into_iter().chain(child.scale).collect();
    let rotation: Vec<f32> = parent.rotation.into_iter().chain([0.0; 3]).collect();
    let translation: Vec<f32> = parent
        .translation
        .into_iter()
        .chain(child.translation)
        .collect();
    let quaternion: Vec<f32> = [0.0; 4]
        .into_iter()
        .chain(child.rotation_quaternion.unwrap())
        .collect();
    let mut out = [0.0_f32; 24];
    // SAFETY: every array has exactly two live elements per joint's own
    // field width (documented on `oracle_bones_pose`), and `out` has the
    // `12 * count == 24` floats the driver writes.
    unsafe {
        oracle_bones_pose(
            parent_idx.as_ptr(),
            classical.as_ptr(),
            scale.as_ptr(),
            rotation.as_ptr(),
            translation.as_ptr(),
            use_quaternion.as_ptr(),
            quaternion.as_ptr(),
            2,
            out.as_mut_ptr(),
        );
    }
    let oracle_parent: Matrix =
        core::array::from_fn(|row| core::array::from_fn(|column| out[row * 4 + column]));
    let oracle_child_world: Matrix =
        core::array::from_fn(|row| core::array::from_fn(|column| out[12 + row * 4 + column]));

    close_matrix(*rust.world_matrix(0).unwrap(), oracle_parent);
    close_matrix(*rust.world_matrix(1).unwrap(), oracle_child_world);
}

/// Real 73-joint pose, real classical-scale flags, real HSD_JObjMakeMatrix:
/// unlike the synthetic two-bone case above, this drives the pinned decomp
/// function itself (`tests/oracle/bones_pose.c`, `HSD_JObjMakeMatrix` +
/// `has_scl` from `sysdolphin/baselib/jobj.c`) over Fox's actual exported
/// `SpecialAirNLoop` frame-6 pose and asserts `Pose::evaluate` matches it
/// bit-for-bit for every one of the 73 joints, then reports (and pins) the
/// same joint-67 muzzle position `simulation::pose`/`docs/validation.md`'s
/// own root-facing entry discusses, built exactly the way
/// `game::simulation::pose` builds it: bone 0's own local rotation forced to
/// `(0, FRAC_PI_2 * facing, 0)`, the external root supplying only the
/// fighter's world translation (`cur_pos`).
mod real_pose {
    use super::{Matrix, Pose, reference_concat, transform_point};
    use skirmish::game::data::MatchData;
    use std::{env, fs, path::PathBuf};

    fn oracle_pose(bones: &[skirmish::collision::bones::Bone]) -> Vec<Matrix> {
        let count = bones.len();
        let parent: Vec<i32> = bones
            .iter()
            .map(|bone| bone.parent.map_or(-1, |p| p as i32))
            .collect();
        let classical_scale: Vec<i32> = bones
            .iter()
            .map(|bone| i32::from(bone.classical_scale))
            .collect();
        let mut scale = Vec::with_capacity(count * 3);
        let mut rotation = Vec::with_capacity(count * 3);
        let mut translation = Vec::with_capacity(count * 3);
        let mut use_quaternion = Vec::with_capacity(count);
        let mut quaternion = Vec::with_capacity(count * 4);
        for bone in bones {
            scale.extend_from_slice(&bone.local.scale);
            translation.extend_from_slice(&bone.local.translation);
            match bone.local.rotation_quaternion {
                Some(q) => {
                    rotation.extend_from_slice(&[0.0; 3]);
                    use_quaternion.push(1);
                    quaternion.extend_from_slice(&q);
                }
                None => {
                    rotation.extend_from_slice(&bone.local.rotation);
                    use_quaternion.push(0);
                    quaternion.extend_from_slice(&[0.0; 4]);
                }
            }
        }
        let mut out = vec![0.0_f32; count * 12];
        // SAFETY: every array has exactly `count` (or `3 * count`/`4 *
        // count`) live elements as documented on the FFI declaration, and
        // `out` has `12 * count` live elements for the driver to write into.
        unsafe {
            super::oracle_bones_pose(
                parent.as_ptr(),
                classical_scale.as_ptr(),
                scale.as_ptr(),
                rotation.as_ptr(),
                translation.as_ptr(),
                use_quaternion.as_ptr(),
                quaternion.as_ptr(),
                count as i32,
                out.as_mut_ptr(),
            );
        }
        out.as_chunks::<12>()
            .0
            .iter()
            .map(|chunk| {
                core::array::from_fn(|row| core::array::from_fn(|column| chunk[row * 4 + column]))
            })
            .collect()
    }

    /// `SKIRMISH_GAMEPLAY_DATA/fox-fd/match-data.json`'s `fighters[0]`,
    /// matching every other real-pack test's discovery convention
    /// (`crates/cli/tests/real_parity.rs`); unset or missing skips cleanly,
    /// exactly like those tests, so this always runs (and always passes the
    /// skip path) in ordinary CI.
    fn load_frame_6_bones() -> Option<Vec<skirmish::collision::bones::Bone>> {
        let root = env::var("SKIRMISH_GAMEPLAY_DATA").ok()?;
        let path = PathBuf::from(&root).join("fox-fd").join("match-data.json");
        if !path.exists() {
            println!(
                "skip: {} does not exist; SKIRMISH_GAMEPLAY_DATA={root} is set, but the fox-fd \
                 export has not landed there yet.",
                path.display()
            );
            return None;
        }
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let data: MatchData =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        let specials = data.fighters[0]
            .specials
            .as_ref()
            .and_then(|specials| match specials {
                skirmish::characters::Specials::Fox { neutral, .. }
                | skirmish::characters::Specials::Falco { neutral, .. } => neutral.as_ref(),
            })
            .expect("fox-fd's P1 fighter carries Fox's own neutral-special resources");
        let frame = specials
            .loop_phase
            .air
            .frames
            .get(6)
            .expect("SpecialAirNLoop's own air loop supplies at least 7 sampled frames");
        // `game::data::Bone::physics` (the production conversion this
        // mirrors field-for-field) is `pub(crate)`, unreachable from an
        // external integration test; `bones::Bone`'s own fields are public.
        Some(
            frame
                .bones
                .iter()
                .map(|bone| skirmish::collision::bones::Bone {
                    parent: bone.parent,
                    classical_scale: bone.classical_scale,
                    local: skirmish::collision::bones::LocalTransform {
                        translation: bone.translation,
                        rotation: bone.rotation,
                        rotation_quaternion: None,
                        scale: bone.scale,
                    },
                })
                .collect(),
        )
    }

    #[test]
    fn fox_special_air_n_loop_frame_6_matches_oracle_for_every_joint() {
        let Some(mut bones) = load_frame_6_bones() else {
            println!(
                "skip: SKIRMISH_GAMEPLAY_DATA is not set; the real 73-joint pose differential \
                 needs the published gameplay export (docs/gameplay-export.md)."
            );
            return;
        };
        assert_eq!(bones.len(), 73, "fox-fd's exported skeleton has 73 joints");
        // `ft/fighter.c:1172-1174`, `game::simulation::pose`: bone 0's own
        // local rotation is hard-set from facing every state change, not
        // sampled animation data. Facing +1 (matching `docs/validation.md`'s
        // own fox-fd-3.slp frame -14 probe, `cur_pos (-28.223993, 8.3501)`).
        bones[0].local.rotation = [0.0, core::f32::consts::FRAC_PI_2, 0.0];

        let rust = Pose::evaluate(&bones).unwrap();
        let oracle = oracle_pose(&bones);
        assert_eq!(oracle.len(), 73);
        // Not bit-for-bit: `math_differential.rs`'s own
        // `sinf_matches_the_pinned_msl_body`/`cosf_matches_the_pinned_msl_
        // body` already establish (`docs/math.md`) that even this crate's
        // own MSL-derived `compat::math::trig::sinf`/`cosf` only tracks the
        // pinned MSL C body within a `1e-5` absolute tolerance, not bitwise
        // -- real hardware ships `sinf`/`cosf` with fused multiply-adds this
        // port's control-flow-faithful translation does not exactly
        // reproduce. A 73-joint chain concatenates dozens of rotated
        // bones, so this per-joint noise compounds; `close_matrix` (this
        // file's own existing tolerance, already used by
        // `euler_srt_tracks_original_with_host_trig_tolerance` for exactly
        // this reason) is the right comparison here, not raw bit equality.
        for (index, expected) in oracle.iter().enumerate() {
            let actual = *rust.world_matrix(index).unwrap();
            for (row, (a_row, e_row)) in actual.iter().zip(expected.iter()).enumerate() {
                for (column, (a, e)) in a_row.iter().zip(e_row.iter()).enumerate() {
                    assert!(
                        (a - e).abs() <= 4e-6 * e.abs().max(1.0) + 2e-3,
                        "joint {index} row {row} column {column}: {a:?} != {e:?}"
                    );
                }
            }
        }

        // `game::simulation::pose`'s own external root: translation-only,
        // `fighter.position`/`fighter.depth` plus a (here, zero)
        // `death.camera_offset`. The laser muzzle bone's own local offset is
        // `simulation`'s `attack_frame`/laser-muzzle wiring (`docs/
        // validation.md`); this test only reproduces the geometry, not that
        // dispatch.
        let cur_pos = [-28.223993_f32, 8.3501_f32, 0.0_f32];
        let root: Matrix = [
            [1.0, 0.0, 0.0, cur_pos[0]],
            [0.0, 1.0, 0.0, cur_pos[1]],
            [0.0, 0.0, 1.0, cur_pos[2]],
        ];
        let offset = [0.0_f32, 1.232_500_1_f32, 4.2636_f32];

        let rust_world = Pose::evaluate_with_root(&bones, &root).unwrap();
        let rust_matrix = *rust_world.world_matrix(67).unwrap();
        let rust_muzzle = transform_point(&rust_matrix, offset);

        // Cross-check the exact same root application and point transform
        // through the C oracle too (`C_MTXConcat`/`oracle_bones_transform`,
        // pure arithmetic -- already checked bitwise against `concat`/
        // `transform_point` above in this file's own proptests), applied to
        // the trig-tolerant oracle pose rather than Rust's own.
        let oracle_matrix = reference_concat(&root, &oracle[67]);
        for (a, b) in rust_matrix
            .into_iter()
            .flatten()
            .zip(oracle_matrix.into_iter().flatten())
        {
            assert!(
                (a - b).abs() <= 4e-6 * b.abs().max(1.0) + 2e-3,
                "{a:?} != {b:?}"
            );
        }
        let oracle_muzzle = super::reference_transform(&oracle_matrix, offset, false);
        for (a, b) in rust_muzzle.into_iter().zip(oracle_muzzle) {
            assert!(
                (a - b).abs() <= 4e-6 * b.abs().max(1.0) + 2e-3,
                "{a:?} != {b:?}"
            );
        }

        println!(
            "joint 67 muzzle, z forced to 0: rust ({}, {}), oracle ({}, {})",
            rust_muzzle[0], rust_muzzle[1], oracle_muzzle[0], oracle_muzzle[1]
        );
        // Regression-pins the value this test found the first time it ran
        // against the real pack (v13-snapshot-20260914): the C oracle above
        // confirms, within the documented trig tolerance, that Skirmish's
        // `bones::Pose` agrees with the pinned `HSD_JObjMakeMatrix`. See
        // `docs/validation.md`'s root-facing entry for the reconciliation
        // against the recording and the exporter's independent port.
        assert_eq!(rust_muzzle[0].to_bits(), (-20.912_f32).to_bits());
        assert_eq!(rust_muzzle[1].to_bits(), (20.391_638_f32).to_bits());
    }
}
