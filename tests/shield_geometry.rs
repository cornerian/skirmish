//! Bone/matrix-to-contact integration for the original shield narrow phase.
use skirmish::{
    collision::{
        bones::{self, Bone, LocalTransform, Pose},
        shield::{self, Contact},
    },
    fighter::combat::Capsule,
};

fn point(center: [f32; 3], radius: f32) -> Capsule {
    Capsule {
        start: center,
        end: center,
        radius,
    }
}

#[test]
fn anisotropic_shield_touches_along_its_long_axis_but_misses_along_its_short_axis() {
    let matrix = [
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let contact =
        shield::shield_contact(&point([2.5, 0.0, 0.0], 0.5), [0.0; 3], &matrix, 1.0, 20.0)
            .unwrap()
            .unwrap();
    assert_eq!(contact.position, [2.0, 0.0, 0.0]);
    assert_eq!(contact.overlap, 0.0);
    assert!(
        shield::shield_contact(&point([0.0, 2.5, 0.0], 0.5), [0.0; 3], &matrix, 1.0, 20.0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn bone_hierarchy_translation_and_nonuniform_scale_feed_shield_contact() {
    let pose = Pose::evaluate(&[
        Bone {
            local: LocalTransform {
                translation: [10.0, 3.0, 0.0],
                ..Default::default()
            },
            ..Default::default()
        },
        Bone {
            parent: Some(0),
            local: LocalTransform {
                translation: [1.0, 2.0, 0.0],
                scale: [-2.0, 1.0, 1.0],
                ..Default::default()
            },
            ..Default::default()
        },
    ])
    .unwrap();
    let matrix = pose.world_matrix(1).unwrap();
    let center = bones::transform_point(matrix, [0.0; 3]);
    let contact = shield::shield_contact(&point([13.5, 5.0, 0.0], 0.5), center, matrix, 1.0, 20.0)
        .unwrap()
        .unwrap();
    assert_eq!(center, [11.0, 5.0, 0.0]);
    assert_eq!(contact.position, [13.0, 5.0, 0.0]);
    assert_eq!(contact.overlap, 0.0);
}

#[test]
fn shear_and_swept_hitboxes_use_the_matrix_aware_path() {
    let shear = [
        [1.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let contact = shield::shield_contact(&point([1.0, 1.0, 0.0], 0.0), [0.0; 3], &shear, 1.0, 20.0)
        .unwrap()
        .unwrap();
    assert_eq!(contact.position, [1.0, 1.0, 0.0]);
    assert_eq!(contact.overlap, 0.0);
    let sweep = Capsule {
        start: [-10.0, 0.0, 0.0],
        end: [10.0, 0.0, 0.0],
        radius: 0.25,
    };
    let contact = shield::shield_contact(&sweep, [0.0; 3], &shear, 1.0, 20.0)
        .unwrap()
        .unwrap();
    assert_eq!(contact.position, [0.0; 3]);
    assert_eq!(contact.overlap, 1.25);
}

#[test]
fn broadphase_preserves_outputs_but_a_narrow_miss_writes_signed_overlap() {
    let hurt = point([0.0; 3], 1.0);
    let mut contact = Contact {
        position: [9.0; 3],
        overlap: 7.0,
        ..Default::default()
    };
    let initial = contact;
    assert!(
        !shield::capsule_matrix(
            &point([50.0, 0.0, 0.0], 0.0),
            &hurt,
            &bones::IDENTITY,
            20.0,
            &mut contact
        )
        .unwrap()
    );
    assert_eq!(contact, initial);
    assert!(
        !shield::capsule_matrix(
            &point([3.0, 0.0, 0.0], 0.5),
            &hurt,
            &bones::IDENTITY,
            20.0,
            &mut contact
        )
        .unwrap()
    );
    assert_eq!(contact.position, [1.0, 0.0, 0.0]);
    assert_eq!(contact.overlap, -1.5);
    // Source's explicit broadphase is deliberately independent of matrix size.
    let oversized = [
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
    ];
    assert!(
        shield::shield_contact(
            &point([50.0, 0.0, 0.0], 0.0),
            [0.0; 3],
            &oversized,
            1.0,
            20.0
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn singular_fallback_is_defined_and_nonfinite_inputs_are_transactional_errors() {
    assert_eq!(shield::inverse(&[[0.0; 4]; 3]).unwrap(), bones::IDENTITY);
    assert!(
        shield::shield_contact(
            &point([1.5, 0.0, 0.0], 0.5),
            [0.0; 3],
            &[[0.0; 4]; 3],
            1.0,
            20.0
        )
        .unwrap()
        .is_some()
    );
    let mut contact = Contact {
        position: [2.0; 3],
        ..Default::default()
    };
    let saved = contact;
    assert_eq!(
        shield::capsule_matrix(
            &point([f32::NAN, 0.0, 0.0], 1.0),
            &point([0.0; 3], 1.0),
            &bones::IDENTITY,
            20.0,
            &mut contact
        ),
        Err(shield::CollisionError::InvalidInput)
    );
    assert_eq!(contact, saved);
    assert_eq!(
        shield::shield_contact(
            &point([0.0; 3], -1.0),
            [0.0; 3],
            &bones::IDENTITY,
            1.0,
            20.0
        ),
        Err(shield::CollisionError::InvalidInput)
    );
}
