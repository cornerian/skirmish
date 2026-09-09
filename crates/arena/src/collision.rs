//! Experimental composition of source ECB arithmetic and static stage queries.
//! This response policy covers ordinary floor/ceiling/side contacts; Melee's
//! complete corner, squeeze, ledge, moving-platform and damage callback graph
//! is not implied by the translated primitives used here.
use crate::{Action, Error, Event, Fighter, data::*, simulation};
use physics::{
    bones::Pose,
    ecb,
    stage::{self, Query, Surface},
};
use std::borrow::Cow;

/// `mpColl_IsOnPlatform`: passability belongs to the supporting source line.
pub(crate) fn on_platform(f: &Fighter, map: &Stage) -> bool {
    f.grounded
        && f.ground_line.is_some_and(|id| {
            map.geometry
                .as_ref()
                .and_then(|geometry| geometry.lines.get(id))
                .is_some_and(|line| u32::from(line.material_flags) & stage::PLATFORM != 0)
        })
}

/// Geometric/state portion of `ftCo_8009A228` (`ftCo_Pass.c`). Input-window and
/// squat-delay checks belong to locomotion. The source clears floor_skip during
/// ChangeMotionState, then assigns the old supporting line; no timed/global
/// platform exclusion is used. Later action changes clear it again.
pub(crate) fn begin_pass(
    f: &mut Fighter,
    data: &FighterData,
    map: &Stage,
    velocity_y: f32,
) -> bool {
    if !on_platform(f, map) {
        return false;
    }
    let support = f.ground_line;
    let mut movement = physics::Movement {
        attributes: data.movement.physics(),
        self_velocity: [f.velocity[0], f.velocity[1], 0.0],
        ..Default::default()
    };
    movement.clamp_air_drift();
    f.velocity = [movement.self_velocity[0], velocity_y];
    f.ground_velocity = 0.0;
    f.grounded = false;
    f.ground_line = None;
    f.contacts[0] = None;
    simulation::enter(f, Action::Pass);
    f.skip_floor = support;
    // ftCommon_8007D5D4 locks the old ECB bottom for ten map callbacks.
    f.ecb_lock = 10;
    f.ecb.bottom_locked = true;
    true
}

pub(crate) fn geometry(data: &Stage) -> Cow<'_, StageGeometry> {
    if let Some(geometry) = &data.geometry {
        return Cow::Borrowed(geometry);
    }
    Cow::Owned(StageGeometry {
        lines: vec![stage::Line {
            start: [data.floor.left, data.floor.y],
            end: [data.floor.right, data.floor.y],
            flags: stage::ENABLED | stage::FLOOR,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [data.floor.left, data.floor.y],
            bounds_max: [data.floor.right, data.floor.y],
            floor: 0..1,
            ..Default::default()
        }],
    })
}

pub(crate) fn sample(f: &mut Fighter, data: &FighterData, pose: &Pose) -> Result<(), Error> {
    match &data.collision_box {
        CollisionBox::Fixed { source } => f.ecb.load_fixed(source, f.facing as i32),
        CollisionBox::Bones {
            indices,
            parameters,
            flags,
        } => {
            let mut world = [[0.0; 2]; 6];
            for (point, &index) in world.iter_mut().zip(indices) {
                let matrix = pose.world_matrix(index).map_err(physics)?;
                *point = [matrix[0][3], matrix[1][3]];
            }
            f.ecb.load_joints(world, f.position, parameters, *flags);
        }
    }
    Ok(())
}

pub(crate) fn initialize(f: &mut Fighter, data: &FighterData, map: &Stage) -> Result<(), Error> {
    sample(f, data, &simulation::pose(f, data)?)?;
    f.ecb.interpolate(1.0).map_err(physics)?;
    let geometry = geometry(map);
    let stage = stage::Stage::new(&geometry.lines, &geometry.joints).map_err(physics)?;
    let bottom = add(f.position, f.ecb.current.bottom);
    // Starting on a supplied floor is a native-data convention. Only eligible
    // stage queries can establish support; disabled/out-of-range lines cannot.
    if let Some(contact) = stage
        .sweep(
            Surface::Floor,
            Query {
                from: [bottom[0], bottom[1] + 0.001],
                to: [bottom[0], bottom[1] - 0.001],
                ..Default::default()
            },
        )
        .map_err(physics)?
    {
        f.grounded = true;
        f.ground_line = Some(contact.line_id);
        f.floor_normal = contact.normal;
        f.action = Action::Wait;
        crate::locomotion::landed(f);
    } else {
        f.grounded = false;
        f.ground_line = None;
        f.action = Action::Fall;
    }
    Ok(())
}

/// Source subdivision also considers ECB growth, so animation alone can
/// require multiple collision callbacks even with zero position displacement.
pub(crate) fn resolve(
    f: &mut Fighter,
    previous_position: [f32; 2],
    stage: &stage::Stage<'_>,
    player: usize,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    let plan = ecb::SubstepPlan::new(
        [previous_position[0], previous_position[1], 0.0],
        [f.position[0], f.position[1], 0.0],
        f.ecb.current,
        f.ecb.desired,
    )
    .map_err(physics)?;
    // Reject unusable native configurations transactionally instead of letting
    // one invalid rollout consume millions of collision callbacks.
    if plan.steps > 4096 {
        return Err(Error::Physics(
            "collision subdivision exceeds 4096 steps".into(),
        ));
    }
    f.position = previous_position;
    f.contacts = [None; 4];
    for step in 0..plan.steps {
        f.ecb
            .interpolate(1.0 / (plan.steps - step) as f32)
            .map_err(physics)?;
        let previous = f.position;
        f.position = add(previous, [plan.velocity[0], plan.velocity[1]]);
        for (surface, slot, old, point) in [
            (
                Surface::LeftWall,
                2,
                f.ecb.previous.right,
                f.ecb.current.right,
            ),
            (
                Surface::RightWall,
                3,
                f.ecb.previous.left,
                f.ecb.current.left,
            ),
            (Surface::Ceiling, 1, f.ecb.previous.top, f.ecb.current.top),
        ] {
            if let Some(contact) = stage
                .sweep(
                    surface,
                    Query {
                        from: add(previous, old),
                        to: add(f.position, point),
                        ..Default::default()
                    },
                )
                .map_err(physics)?
            {
                let axis = if surface == Surface::Ceiling { 1 } else { 0 };
                // A sloped contact must project the final tangent coordinate;
                // using the impact coordinate alone can leave the ECB inside.
                if let Some(projection) = stage
                    .project(surface, contact.line_id, add(f.position, point))
                    .map_err(physics)?
                {
                    f.position[axis] += projection.delta;
                    f.contacts[slot] = Some(projection.line_id);
                } else {
                    f.position[axis] = contact.position[axis] - point[axis];
                    f.contacts[slot] = Some(contact.line_id);
                }
                // This slice stops motion into a surface. Damage wall/ceiling
                // bounces, techs and velocity projection remain separate work.
                let inward = if surface == Surface::RightWall {
                    -1.0
                } else {
                    1.0
                };
                if f.velocity[axis] * inward > 0.0 {
                    f.velocity[axis] = 0.0;
                }
                if f.knockback[axis] * inward > 0.0 {
                    f.knockback[axis] = 0.0;
                }
                if axis == 0 {
                    f.ground_velocity = 0.0;
                }
            }
        }
        if f.grounded {
            if let Some(line) = f.ground_line
                && let Some(projection) = stage
                    .project_floor(line, add(f.position, f.ecb.current.bottom))
                    .map_err(physics)?
            {
                f.position[1] += projection.vertical_delta;
                f.ground_line = Some(projection.line_id);
                f.floor_normal = projection.normal;
                f.contacts[0] = Some(projection.line_id);
                continue;
            }
            f.grounded = false;
            f.ground_line = None;
            f.ground_velocity = 0.0;
            f.fast_fall = false;
            // Ground-to-air conversion consumes the grounded jump slot, even
            // when walking off an edge instead of pressing jump.
            f.locomotion.jumps_used = f.locomotion.jumps_used.max(1);
            if f.action != Action::Damage {
                simulation::enter(f, Action::Fall);
            }
        }
        if let Some(contact) = stage
            .sweep(
                Surface::Floor,
                Query {
                    from: add(previous, f.ecb.previous.bottom),
                    to: add(f.position, f.ecb.current.bottom),
                    skip_line: f.skip_floor,
                    ..Default::default()
                },
            )
            .map_err(physics)?
        {
            let bottom = add(f.position, f.ecb.current.bottom);
            if let Some(projection) = stage
                .project_floor(contact.line_id, bottom)
                .map_err(physics)?
            {
                f.position[1] += projection.vertical_delta;
                f.ground_line = Some(projection.line_id);
                f.floor_normal = projection.normal;
            } else {
                f.position = [
                    contact.position[0] - f.ecb.current.bottom[0],
                    contact.position[1] - f.ecb.current.bottom[1],
                ];
                f.ground_line = Some(contact.line_id);
                f.floor_normal = contact.normal;
            }
            f.contacts[0] = f.ground_line;
            f.velocity[1] = 0.0;
            f.knockback = [0.0; 2];
            f.ground_velocity = f.velocity[0];
            f.grounded = true;
            f.fast_fall = false;
            crate::locomotion::landed(f);
            f.ecb_lock = 0;
            f.ecb.bottom_locked = false;
            f.skip_floor = None;
            if f.action != Action::Damage {
                simulation::enter(f, Action::Landing);
            }
            events.push(Event::Landed { player });
        }
    }
    Ok(())
}

fn add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
