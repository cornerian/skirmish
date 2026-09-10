//! Experimental composition of source ECB arithmetic and sampled stage queries.
//! This response policy covers ordinary floor/ceiling/side contacts and opposing
//! surface squeezes; Melee's complete adjacency and damage callback graph is not
//! implied by the translated primitives used here.
use super::{Action, Error, Event, Fighter, data::*, simulation};
use crate::{
    collision::{
        bones::Pose,
        ecb,
        stage::{self, Query, Surface},
    },
    fighter::Movement,
};
use std::borrow::Cow;

/// `mpColl_IsOnPlatform`: passability belongs to the supporting source line.
pub(crate) fn on_platform(f: &Fighter, geometry: &StageGeometry) -> bool {
    f.grounded
        && f.ground_line.is_some_and(|id| {
            geometry
                .lines
                .get(id)
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
    geometry: &StageGeometry,
    velocity_y: f32,
) -> bool {
    if !on_platform(f, geometry) {
        return false;
    }
    let support = f.ground_line;
    let mut movement = Movement {
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
            material_flags: stage::LEDGE as u16,
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

pub(crate) fn initialize(
    f: &mut Fighter,
    data: &FighterData,
    geometry: &StageGeometry,
) -> Result<(), Error> {
    sample(f, data, &simulation::pose(f, data)?)?;
    f.ecb.interpolate(1.0).map_err(physics)?;
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
        super::locomotion::landed(f);
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
    environment: (&stage::Stage<'_>, &StageGeometry, &StageGeometry),
    player: usize,
    events: &mut Vec<Event>,
    resources: (&FighterData, &Rules, super::Controller),
) -> Result<(), Error> {
    let (stage, geometry, previous_geometry) = environment;
    let (data, rules, input) = resources;
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
    let mut responded = false;
    for step in 0..plan.steps {
        f.ecb
            .interpolate(1.0 / (plan.steps - step) as f32)
            .map_err(physics)?;
        let previous = f.position;
        f.position = add(previous, [plan.velocity[0], plan.velocity[1]]);
        let mut wall_positions = [None; 2];
        let mut ceiling_position = None;
        let mut surface_contacts = [None; 2];
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
            let world_point = add(f.position, point);
            let contact = stage
                .sweep(
                    surface,
                    Query {
                        from: add(previous, old),
                        to: add(f.position, point),
                        ..Default::default()
                    },
                )
                .map_err(physics)?;
            let axis = usize::from(surface == Surface::Ceiling);
            let correction = if let Some(contact) = contact {
                // A sloped contact must project the final tangent coordinate;
                // using the impact coordinate alone can leave the ECB inside.
                stage
                    .project(surface, contact.line_id, world_point)
                    .map_err(physics)?
                    .map(|projection| (projection.line_id, projection.delta, projection.normal))
                    .or(Some((
                        contact.line_id,
                        contact.position[axis] - world_point[axis],
                        contact.normal,
                    )))
            } else {
                moved_projection(
                    stage,
                    geometry,
                    previous_geometry,
                    surface,
                    world_point,
                    None,
                    true,
                )?
                .map(|projection| (projection.line_id, projection.delta, projection.normal))
            };
            if let Some((line_id, delta, normal)) = correction {
                f.position[axis] += delta;
                f.contacts[slot] = Some(line_id);
                if surface == Surface::LeftWall {
                    wall_positions[0] = Some(f.position[0]);
                } else if surface == Surface::RightWall {
                    wall_positions[1] = Some(f.position[0]);
                } else {
                    ceiling_position = Some(f.position[1]);
                }
                let response_eligible = !responded
                    && (super::damage::can_surface_tech(f, surface, &rules.damage)
                        || super::damage::can_reflect(f, surface, &rules.damage));
                if response_eligible {
                    let candidate = &mut surface_contacts[usize::from(surface == Surface::Ceiling)];
                    candidate.get_or_insert((surface, normal, line_id));
                } else {
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
        }
        if let [Some(after_left), Some(after_right)] = wall_positions {
            f.ecb
                .squeeze_horizontal(&mut f.position, after_right, after_left);
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
                if let Some(after_ceiling) = ceiling_position {
                    let after_floor = f.position[1];
                    f.ecb
                        .squeeze_vertical(&mut f.position, false, after_ceiling, after_floor);
                }
                continue;
            }
            f.grounded = false;
            f.ground_line = None;
            f.ground_knockback = 0.0;
            f.ground_velocity = 0.0;
            f.fast_fall = false;
            // Ground-to-air conversion consumes the grounded jump slot, even
            // when walking off an edge instead of pressing jump.
            f.locomotion.jumps_used = f.locomotion.jumps_used.max(1);
            if !super::grab::transfer_capture_family(f, true)
                && !super::special::transfer_ground_air(f, false)
                && !matches!(
                    f.action,
                    Action::Damage
                        | Action::DamageFall
                        | Action::DownDamage
                        | Action::FlyReflectWall
                        | Action::FlyReflectCeiling
                )
            {
                simulation::enter(f, Action::Fall);
            }
        }
        let floor_query = Query {
            from: add(previous, f.ecb.previous.bottom),
            to: add(f.position, f.ecb.current.bottom),
            skip_line: f.skip_floor,
            ..Default::default()
        };
        let contact = stage.sweep(Surface::Floor, floor_query).map_err(physics)?;
        let moved_floor = if contact.is_none() {
            moved_projection(
                stage,
                geometry,
                previous_geometry,
                Surface::Floor,
                add(f.position, f.ecb.current.bottom),
                f.skip_floor,
                f.velocity[1] <= 0.0,
            )?
        } else {
            None
        };
        if let Some(contact) = contact {
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
            land(f, rules, data, input, events, player)?;
        } else if let Some(projection) = moved_floor {
            f.position[1] += projection.delta;
            f.ground_line = Some(projection.line_id);
            f.floor_normal = projection.normal;
            land(f, rules, data, input, events, player)?;
        }
        if f.grounded
            && let Some(after_ceiling) = ceiling_position
        {
            let after_floor = f.position[1];
            f.ecb
                .squeeze_vertical(&mut f.position, false, after_ceiling, after_floor);
        }
        if !f.grounded && !responded {
            let [wall, ceiling] = surface_contacts;
            let order = match f.action {
                Action::FlyReflectWall => [
                    (ceiling, true),
                    (ceiling, false),
                    (wall, true),
                    (wall, false),
                ],
                Action::FlyReflectCeiling => {
                    [(wall, true), (wall, false), (None, true), (None, false)]
                }
                _ => [
                    (wall, true),
                    (ceiling, true),
                    (wall, false),
                    (ceiling, false),
                ],
            };
            for (candidate, tech) in order {
                let Some((surface, normal, line)) = candidate else {
                    continue;
                };
                if tech && super::damage::can_surface_tech(f, surface, &rules.damage) {
                    let jump = super::damage::surface_tech(f, surface, &rules.damage, input);
                    events.push(Event::SurfaceTeched {
                        player,
                        surface,
                        line,
                        jump,
                    });
                } else if !tech && super::damage::can_reflect(f, surface, &rules.damage) {
                    super::damage::reflect(f, surface, normal, &rules.damage);
                    events.push(Event::SurfaceReflected {
                        player,
                        surface,
                        line,
                    });
                } else {
                    continue;
                }
                responded = true;
                break;
            }
        }
    }
    Ok(())
}

fn land(
    f: &mut Fighter,
    rules: &Rules,
    data: &FighterData,
    input: super::Controller,
    events: &mut Vec<Event>,
    player: usize,
) -> Result<(), Error> {
    f.contacts[0] = f.ground_line;
    f.velocity[1] = 0.0;
    f.knockback = [0.0; 2];
    f.ground_knockback = 0.0;
    f.ground_velocity = f.velocity[0];
    f.grounded = true;
    f.fast_fall = false;
    super::locomotion::landed(f);
    f.ecb_lock = 0;
    f.ecb.bottom_locked = false;
    f.skip_floor = None;
    if !super::grab::transfer_capture_family(f, false) {
        if matches!(f.action, Action::ShieldBreakFly | Action::ShieldBreakFall) {
            simulation::enter(f, Action::ShieldBreakDown);
        } else if matches!(
            f.action,
            Action::Damage
                | Action::DamageFall
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        ) {
            let pose = simulation::pose(f, data)?;
            super::damage::land(f, data, &pose, &rules.damage, input)?;
        } else if !super::special::transfer_ground_air(f, true) && !super::aerial::land(f, data)? {
            simulation::enter(f, Action::Landing);
        }
    }
    events.push(Event::Landed { player });
    Ok(())
}

fn moved_projection(
    stage: &stage::Stage<'_>,
    geometry: &StageGeometry,
    previous: &StageGeometry,
    surface: Surface,
    point: [f32; 2],
    skip_line: Option<usize>,
    platform_allowed: bool,
) -> Result<Option<stage::SurfaceProjection>, Error> {
    for (id, (line, old)) in geometry.lines.iter().zip(&previous.lines).enumerate() {
        if line.start == old.start && line.end == old.end
            || skip_line == Some(id)
            || line.flags & (surface.flag() | stage::ENABLED) != surface.flag() | stage::ENABLED
            || line.flags & (stage::EMPTY | stage::HIDDEN) != 0
            || surface == Surface::Floor
                && u32::from(line.material_flags) & stage::PLATFORM != 0
                && !platform_allowed
            || !active_line(geometry, surface, id)
        {
            continue;
        }
        let Some(projection) = stage.project(surface, id, point).map_err(physics)? else {
            continue;
        };
        // Surface motion only owns a crossing when the same point was clear of
        // the previous supporting plane. This rejects a floor sliding sideways
        // above an already-behind fighter while retaining inward translations.
        let old_delta = line_delta(surface, old, point)?;
        if penetrates(surface, projection.delta) && !penetrates(surface, old_delta) {
            return Ok(Some(projection));
        }
    }
    Ok(None)
}

fn penetrates(surface: Surface, delta: f32) -> bool {
    match surface {
        Surface::Floor | Surface::RightWall => delta > 0.0,
        Surface::Ceiling | Surface::LeftWall => delta < 0.0,
    }
}

fn line_delta(surface: Surface, line: &stage::Line, point: [f32; 2]) -> Result<f32, Error> {
    let [x0, y0] = line.start;
    let [x1, y1] = line.end;
    let delta = if matches!(surface, Surface::LeftWall | Surface::RightWall) {
        x0 + (x1 - x0) * (point[1] - y0) / (y1 - y0) - point[0]
    } else {
        y0 + (y1 - y0) * (point[0] - x0) / (x1 - x0) - point[1]
    };
    if delta.is_finite() {
        Ok(delta)
    } else {
        Err(Error::Physics("nonfinite moving-surface projection".into()))
    }
}

fn active_line(geometry: &StageGeometry, surface: Surface, id: usize) -> bool {
    geometry.joints.iter().any(|joint| {
        joint.flags & stage::ENABLED != 0
            && joint.flags & stage::HIDDEN == 0
            && (match surface {
                Surface::Floor => &joint.floor,
                Surface::Ceiling => &joint.ceiling,
                Surface::LeftWall => &joint.left_wall,
                Surface::RightWall => &joint.right_wall,
            }
            .contains(&id)
                || joint.dynamic.contains(&id))
    })
}

fn add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
