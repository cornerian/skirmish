//! Ordered-body, stage-neighbor and physics-bone composition. Integration here
//! is explicit test code; the native match has not yet acquired nudge phases.
#[path = "support/nudge.rs"]
mod support;
use skirmish::{
    collision::{
        bones::{self, Bone, Pose},
        stage::{self, Line, Stage},
    },
    fighter::{
        combat,
        nudge::{self, Body, Error, Neighbors},
    },
};
use support::{body, floor, rules};

fn velocities(bodies: &[Body], floors: &[Neighbors]) -> Vec<[f32; 2]> {
    (0..bodies.len())
        .map(|i| nudge::velocity(i, bodies, floors, &rules()).unwrap())
        .collect()
}

#[test]
fn ordered_prephysics_positions_produce_fixed_steps_before_bone_world_transforms() {
    let mut bodies = [body(0, 0.0), body(1, 1.75)];
    let shifts = velocities(&bodies, &floor());
    assert_eq!(shifts, [[-0.25, -0.125], [0.25, 0.125]]);
    // Collect first, integrate second: moving the first body before sampling
    // the second would incorrectly turn its strict overlap into mere touching.
    for (b, [x, z]) in bodies.iter_mut().zip(shifts) {
        b.position[0] += x;
        b.position[2] += z;
    }
    assert_eq!(bodies[0].position, [-0.25, 0.0, -0.125]);
    assert_eq!(bodies[1].position, [2.0, 0.0, 0.125]);
    for b in &bodies {
        let mut root = bones::IDENTITY;
        for (row, position) in root.iter_mut().zip(b.position) {
            row[3] = position;
        }
        let pose = Pose::evaluate_with_root(&[Bone::default()], &root).unwrap();
        assert_eq!(
            bones::transform_point(pose.world_matrix(0).unwrap(), [0.0; 3]),
            b.position
        );
    }
    // The saved complete inputs reproduce the same result; no hidden mutable
    // state or remembered left/right ordering lives inside the helper.
    let saved = bodies;
    assert_eq!(velocities(&bodies, &floor()), [[0.0, 0.125], [0.0, -0.125]]);
    assert_eq!(velocities(&saved, &floor()), velocities(&bodies, &floor()));
}

#[test]
fn coincident_ties_follow_entity_order_and_never_snap_to_full_separation() {
    let bodies = [body(0, 0.0), body(1, 0.0)];
    assert_eq!(
        velocities(&bodies, &floor()),
        [[-0.25, -0.125], [0.25, 0.125]]
    );
    let reversed = [bodies[1], bodies[0]];
    assert_eq!(
        velocities(&reversed, &floor()),
        [[-0.25, -0.125], [0.25, 0.125]]
    );
    // A same-player entry encountered before the subject changes tie polarity,
    // even when inactive. This is not simply the subject's numeric array index.
    let mut earlier_owner = body(0, 100.0);
    earlier_owner.inactive = true;
    let world = [earlier_owner, bodies[1], bodies[0]];
    assert_eq!(
        nudge::velocity(2, &world, &floor(), &rules()).unwrap(),
        [-0.25, -0.125]
    );
    assert_eq!(
        nudge::velocity(1, &[bodies[1], bodies[0]], &floor(), &rules()).unwrap(),
        [0.25, 0.125]
    );
}

#[test]
fn facing_offsets_and_resolved_stage_links_control_strict_overlap() {
    let lines = [
        Line {
            start: [-5.0, 0.0],
            end: [0.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            next: [None, Some(1)],
            ..Default::default()
        },
        Line {
            start: [0.0, 0.0],
            end: [5.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            previous: [Some(0), None],
            ..Default::default()
        },
        Line {
            start: [5.0, 10.0],
            end: [10.0, 10.0],
            flags: stage::FLOOR | stage::ENABLED,
            ..Default::default()
        },
    ];
    let stage = Stage::new(&lines, &[]).unwrap();
    let neighbors: Vec<_> = (0..3)
        .map(|i| Neighbors {
            previous: stage.neighbor(i, false).unwrap(),
            next: stage.neighbor(i, true).unwrap(),
        })
        .collect();
    let mut bodies = [body(0, -1.0), body(1, 1.0)];
    bodies[1].floor = Some(1);
    assert_eq!(velocities(&bodies, &neighbors), [[0.0; 2]; 2]);
    bodies[0].center_offset = 0.25;
    assert_eq!(
        velocities(&bodies, &neighbors),
        [[-0.25, -0.125], [0.25, 0.125]]
    );
    bodies[0].facing = -1.0;
    assert_eq!(velocities(&bodies, &neighbors), [[0.0; 2]; 2]);
    bodies[0].facing = 1.0;
    bodies[1].floor = Some(2);
    assert_eq!(velocities(&bodies, &neighbors), [[0.0; 2]; 2]);
}

#[test]
fn subject_hitlag_disables_its_nudge_but_it_remains_a_target_for_other_bodies() {
    let mut bodies = [body(0, 0.0), body(1, 1.0)];
    bodies[0].hitlag = true;
    assert_eq!(velocities(&bodies, &floor()), [[0.0; 2], [0.25, 0.125]]);
    bodies[0].hitlag = false;
    bodies[0].floor = None;
    assert_eq!(velocities(&bodies, &floor()), [[0.0; 2]; 2]);
    bodies[0].floor = Some(0);
    bodies[0].inactive = true;
    // Direct E0E4 still handles the inactive subject; skipping its callback is
    // an outer scheduler responsibility. Other subjects exclude it as target.
    assert_eq!(velocities(&bodies, &floor()), [[-0.25, -0.125], [0.0; 2]]);
}

#[test]
fn follower_owner_bias_uses_distinct_filters_and_discards_horizontal_nudge() {
    let mut bodies = [body(0, 0.0), body(0, 0.0), body(1, 0.0)];
    bodies[0].holds_victim = true;
    bodies[1].follower_of = Some(0);
    let shifts = velocities(&bodies, &floor());
    assert_eq!(shifts[1], [0.0, -0.5]); // -.375 owner bias, -.125 ordinary target.
    assert_eq!(shifts[2], [0.0; 2]); // owner holds a victim; follower is excluded.
    bodies[1].overlap_disabled = true;
    bodies[1].position[2] = 0.125;
    assert_eq!(velocities(&bodies, &floor())[1], [0.0, -0.125]);
}

#[test]
fn follower_depth_changes_physics_contact_while_horizontal_positions_stay_equal() {
    let mut bodies = [body(0, 0.0); 2];
    bodies[1].follower_of = Some(0);
    let shifts = velocities(&bodies, &floor());
    assert_eq!(shifts, [[0.0; 2], [0.0, -0.375]]);
    let contacts = |bodies: &[Body; 2]| {
        let shapes = bodies.map(|b| {
            let mut root = bones::IDENTITY;
            for (row, coordinate) in root.iter_mut().zip(b.position) {
                row[3] = coordinate;
            }
            let pose = Pose::evaluate_with_root(&[Bone::default()], &root).unwrap();
            bones::BoneCapsule::sphere(0, [0.0; 3], 0.1)
                .transform(&pose, 1.0)
                .unwrap()
        });
        combat::capsule_sphere(
            &combat::Capsule {
                start: shapes[0].start,
                end: shapes[0].end,
                radius: shapes[0].radius,
            },
            shapes[1].start,
            shapes[1].radius,
            &mut [0.0; 3],
        )
    };
    assert!(contacts(&bodies));
    for (b, [x, z]) in bodies.iter_mut().zip(shifts) {
        b.position[0] += x;
        b.position[2] += z;
    }
    assert_eq!(bodies[0].position[..2], bodies[1].position[..2]);
    assert!(!contacts(&bodies));
}

#[test]
fn deferred_depth_recenters_caps_and_preserves_the_sources_overshoot_order() {
    let mut b = body(0, 0.0);
    b.position[2] = 0.5;
    b.deferred_position = [2.0, 3.0, -0.25];
    b.overlap_disabled = true;
    assert_eq!(b.effective_position(), [2.0, 3.0, 0.25]);
    let r = nudge::Rules {
        depth_step: 1.0,
        depth_limit: 0.1,
        ..rules()
    };
    assert_eq!(nudge::velocity(0, &[b], &floor(), &r).unwrap(), [0.0, -0.1]);
    b.position[2] = 3.0;
    b.deferred_position = [0.0; 3];
    assert_eq!(
        nudge::velocity(0, &[b], &floor(), &rules()).unwrap(),
        [0.0, -2.5]
    );
    b.nudge_disabled = true;
    assert_eq!(
        nudge::velocity(0, &[b], &floor(), &rules()).unwrap(),
        [0.0; 2]
    );
}

#[test]
fn invalid_numeric_inputs_and_world_references_are_explicit_errors() {
    let mut b = body(0, 0.0);
    b.half_width = -1.0;
    assert_eq!(
        nudge::velocity(0, &[b], &floor(), &rules()),
        Err(Error::Input)
    );
    b.half_width = 1.0;
    b.follower_of = Some(0);
    assert_eq!(
        nudge::velocity(0, &[b], &floor(), &rules()),
        Err(Error::Reference)
    );
    b.follower_of = None;
    b.floor = Some(1);
    assert_eq!(
        nudge::velocity(0, &[b], &floor(), &rules()),
        Err(Error::Reference)
    );
    assert_eq!(
        nudge::velocity(0, &[], &floor(), &rules()),
        Err(Error::Reference)
    );
    let r = nudge::Rules {
        depth_step: f32::NAN,
        ..rules()
    };
    assert_eq!(
        nudge::velocity(0, &[body(0, 0.0)], &floor(), &r),
        Err(Error::Input)
    );
    let mut world = [body(0, 0.0); 4];
    world[2].follower_of = Some(0);
    world[3].follower_of = Some(1);
    assert_eq!(
        nudge::velocity(2, &world, &floor(), &rules()),
        Err(Error::Reference)
    );
}
