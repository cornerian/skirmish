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
    begin_pass_as(f, data, geometry, velocity_y, Action::Pass, false)
}

/// The same geometric/state portion as [`begin_pass`], generalized for a
/// caller-supplied destination and an optional frame-preserving entry
/// (`ftCo_8009A184`, `ftCo_Pass.c:76+`: identical to `ftCo_8009A228` except
/// the destination motion state and start frame are caller-supplied instead
/// of always the generic `ftCo_MS_Pass` at frame 0 -- Fox/Falco's Reflector
/// platform drop uses this to keep its own Start/Loop phase, converting
/// straight to the aerial variant at the current frame instead of routing
/// through the shared `Pass` action).
pub(crate) fn begin_pass_as(
    f: &mut Fighter,
    data: &FighterData,
    geometry: &StageGeometry,
    velocity_y: f32,
    destination: Action,
    keep_frame: bool,
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
    if keep_frame {
        let frame = f.action_frame;
        simulation::enter(f, destination);
        f.action_frame = frame;
    } else {
        simulation::enter(f, destination);
    }
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
            flags: _,
        } => {
            let mut world = [[0.0; 2]; 6];
            for (point, &index) in world.iter_mut().zip(indices) {
                let matrix = pose.world_matrix(index).map_err(physics)?;
                *point = [matrix[0][3], matrix[1][3]];
            }
            f.ecb
                .load_joints(world, f.position, parameters, load_flags(f.grounded));
        }
    }
    Ok(())
}

/// `mpColl_LoadECB_inline`'s flags argument (`mp/mpcoll.c`), chosen per
/// collision path rather than read from the resource pack (the resource's own
/// `CollisionBox::Bones.flags` is unused; see its doc comment). Every
/// `sample` call site in this codebase (the ordinary per-frame collision at
/// the bottom of the main loop, the hitlag/rebirth/ledge-attach/grab-capture
/// variants) samples the same fighter's own ECB for the same purpose, so the
/// only input the flags need is whether that fighter is currently grounded:
/// - airborne: `ft_CheckGroundAndLedge`'s `mpColl_800473CC` (mpcoll.c:2752-
///   2757) and `ft_80083090_inline`/`ft_800831CC`'s `mpColl_80047AC8`/
///   `mpColl_80047E14` (2802-2837, ft_081B.c:641-694) all pass
///   `mpColl_LoadECB_inline(coll, 6)`: 0x4 (`CollisionFlagAir_CanGrabLedge`,
///   skip the two-unit padding) | 0x2 (unread by the loader itself --
///   `CollisionFlagAir_PlatformPassCallback` only matters to the sweep, see
///   below).
/// - grounded: `ft_800827A0`'s `mpColl_8004B2DC` (4010-4015) passes
///   `mpColl_LoadECB_inline(coll, 5)`: 0x4 (still no padding) | 0x1 (anchor
///   the bottom to 0, `CollisionFlagAir_StayAirborne`'s bit reused here with
///   an unrelated loader-side meaning).
///
/// The narrow-width (9, `ft_800843FC`'s `mpColl_8004B5C4`) and two-unit-
/// height (0x12, the special-state entry points at 2823/2852/2878/2897/2916)
/// variants exist for fighter states this codebase does not yet model as
/// collision paths distinct from ordinary grounded/airborne movement;
/// `load_joints` already accepts any flags value, so adding such a path only
/// needs its own call to this function's pattern, not a loader change.
///
/// Bit 0x2 (`CollisionFlagAir_PlatformPassCallback`) governs a different
/// parameter in the source: the sweep engine `mpColl_80046904`'s own `flags`
/// (the "mode" `i` passed to `inline0`/`inline1`, chosen independently of the
/// loader's flags at each entry point -- e.g. `ft_80083090_inline` passes
/// loader-flags 6 but sweep-mode 2 or 6 depending on ledge cooldown, while
/// `ft_CheckGroundAndLedge` passes the same loader-flags 6 but sweep-mode 0
/// or 4, never setting bit 0x2). When set, it installs the fighter's own
/// floor callback (`mpColl_804D64A0`) so `mpColl_80044628_Floor` can let a
/// one-way platform pass instead of always counting it as ground
/// (`mpcoll.c:1396-1462`); this codebase already ports exactly that
/// substitution as `escape_air::platforms_land` (FallSpecial's stick-down
/// pass-through, `ftCo_80099A58`) feeding `stage::sweep_filtered` in
/// `resolve`'s floor query below, and the `skip_floor` field already ports
/// the unconditional platform-index skip (`floor_skip`) that applies
/// regardless of this bit. The grounded mode argument (`ft_800827A0`'s `2`)
/// is a wholly separate "clamp/teeter" selector into `mpColl_8004ACE4`
/// (3848-3967, `flags & 1`/`flags & 2`), already ported as `floor_end_clamp`'s
/// `Mode` enum (`docs/edges.md`'s "Ground collision modes" table). No sweep
/// code change is needed for this batch; both are already correct.
fn load_flags(grounded: bool) -> u32 {
    if grounded { 5 } else { 6 }
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
    let position_delta_x = f.position[0] - previous_position[0];
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
    f.edge_contact = None;
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
                        || super::damage::can_reflect(f, surface, &rules.damage)
                        || super::specials::wants_redirect(f.action));
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
            if let Some(line) = f.ground_line
                && let Some(geom_line) = geometry.lines.get(line)
                && floor_end_clamp(f, rules, line, geom_line, stage, input)?
            {
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
                && !super::specials::transfer_ground_air(f, false)
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
                // ftCo_Fall_Enter (ftCo_Fall.c:47-70) locks the ECB bottom
                // whenever Fall is entered while `ground_or_air` was still
                // Ground, i.e. exactly this "just walked/ran off an edge"
                // transition (a jump or an already-airborne fall-through
                // reaching Fall does not take this branch, matching the
                // source's own condition): ftCommon_8007D5D4 (ftcommon.c:515)
                // sets `ecb_lock = 10` and locks `x130_flags`'s bottom bit,
                // so `mpColl_LoadECB_JObj`'s six-joint sample stops moving
                // the ECB's bottom edge for the next ten `Fighter_procMap`
                // collision callbacks -- it stays at the last grounded
                // bottom while the falling pose's own (higher) sampled
                // bottom would otherwise report the character's true
                // in-air silhouette immediately.
                f.ecb_lock = 10;
                f.ecb.bottom_locked = true;
                // ftCo_Fall_Enter unconditionally calls ftCommon_ClampAirDrift
                // right after Fighter_ChangeMotionState (ftCo_Fall.c:63),
                // clamping self_vel.x to +/-ca->air_drift_max: running or
                // walking off a platform edge carries the ground speed
                // straight into this clamp, so the fighter's first airborne
                // frame drifts at the (much lower) air-drift maximum instead
                // of at whatever speed it was running/walking at. The same
                // clamp is already inlined in `begin_pass_as` for the
                // explicit platform-drop path (`ftCo_8009A184`/
                // `ftCo_8009A228`); this is the ordinary edge-loss path's own
                // copy of it.
                let mut movement = Movement {
                    attributes: data.movement.physics(),
                    self_velocity: [f.velocity[0], f.velocity[1], 0.0],
                    ..Default::default()
                };
                movement.clamp_air_drift();
                f.velocity[0] = movement.self_velocity[0];
            }
        }
        let floor_query = Query {
            from: add(previous, f.ecb.previous.bottom),
            to: add(f.position, f.ecb.current.bottom),
            skip_line: f.skip_floor,
            ..Default::default()
        };
        // ftCo_80096CC8: a FallSpecial fighter holding the stick down passes
        // through one-way platforms; every other state lands on them.
        let platforms_land = super::escape_air::platforms_land(f, rules.escape_air.as_ref(), input);
        let contact = stage
            .sweep_filtered(Surface::Floor, floor_query, |id| {
                platforms_land
                    || geometry
                        .lines
                        .get(id)
                        .is_none_or(|line| u32::from(line.material_flags) & stage::PLATFORM == 0)
            })
            .map_err(physics)?;
        let moved_floor = if contact.is_none() {
            moved_projection(
                stage,
                geometry,
                previous_geometry,
                Surface::Floor,
                add(f.position, f.ecb.current.bottom),
                f.skip_floor,
                f.velocity[1] <= 0.0 && platforms_land,
            )?
        } else {
            None
        };
        if let Some(contact) = contact {
            // mpColl_80046904's `ecb_unlocked = coll->ecb.bottom.y > 0.0F`,
            // forwarded as `mpColl_80044838_Floor`'s `ignore_bottom`: a raw
            // (unanchored, airborne-flags) ECB bottom that samples *above*
            // the fighter's own position -- exactly the ordinary case for a
            // falling pose, since `load_joints` only clamps the bottom to be
            // no lower than 0, never forces it negative -- rests `position`
            // itself on the floor instead of resting the ECB's bottom point
            // on it. Skipping this (always using `position + ecb.bottom`)
            // lands the fighter with position.y offset by the ECB's own
            // height above the floor instead of at the floor.
            let unlocked = f.ecb.current.bottom[1] > 0.0;
            let bottom = if unlocked {
                f.position
            } else {
                add(f.position, f.ecb.current.bottom)
            };
            if let Some(projection) = stage
                .project_floor(contact.line_id, bottom)
                .map_err(physics)?
            {
                f.position[1] += projection.vertical_delta;
                f.ground_line = Some(projection.line_id);
                f.floor_normal = projection.normal;
            } else if unlocked {
                f.position = [contact.position[0], contact.position[1]];
                f.ground_line = Some(contact.line_id);
                f.floor_normal = contact.normal;
            } else {
                f.position = [
                    contact.position[0] - f.ecb.current.bottom[0],
                    contact.position[1] - f.ecb.current.bottom[1],
                ];
                f.ground_line = Some(contact.line_id);
                f.floor_normal = contact.normal;
            }
            land(f, rules, data, input, events, player, geometry)?;
        } else if let Some(projection) = moved_floor {
            f.position[1] += projection.delta;
            f.ground_line = Some(projection.line_id);
            f.floor_normal = projection.normal;
            land(f, rules, data, input, events, player, geometry)?;
        }
        if f.grounded
            && let Some(after_ceiling) = ceiling_position
        {
            let after_floor = f.position[1];
            f.ecb
                .squeeze_vertical(&mut f.position, false, after_ceiling, after_floor);
        }
        if !f.grounded && !responded && super::specials::wants_redirect(f.action) {
            let [wall, ceiling] = surface_contacts;
            // `ftFx_SpecialAirHi_Coll`'s own non-ground branch: a single
            // hook decides both candidates together (ceiling checked
            // first, then whichever wall, matching the source's own
            // do-while order internally) rather than this file's own
            // per-candidate tech/reflect loop below, since only one move
            // (today) ever wants this and its own angle gate needs both
            // contacts available at once.
            let strip_surface =
                |c: Option<(Surface, [f32; 3], usize)>| c.map(|(_, normal, line)| (normal, line));
            if super::specials::air_contact(f, data, strip_surface(ceiling), strip_surface(wall)) {
                responded = true;
            }
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
    if f.grounded
        && super::edge::owns_action(f.action)
        && let Some(edge_rules) = rules.edge.as_ref()
        && let Some(line) = f.ground_line
        && let Some(geom_line) = geometry.lines.get(line)
    {
        // ftCo_Ottotto_Coll / ftCo_OttottoWait_Coll: `mpFloorGetRight` for
        // facing +1, `mpFloorGetLeft` for facing -1. The ground-lost branch
        // is already the ordinary Fall entry above (mode 2 clamp failing).
        let (left_pt, right_pt) = super::edge::line_ends(geom_line);
        let floor_end_x = if f.facing > 0.0 {
            right_pt[0]
        } else {
            left_pt[0]
        };
        super::edge::check_exit(f, edge_rules, floor_end_x);
    }
    if !f.grounded && !responded && f.wall_jump.startup_timer == 0 {
        let contact = wall_jump_contact(f, geometry, previous_geometry, position_delta_x);
        if let (Some(jump_rules), Some(attributes)) = (&rules.wall_jump, data.wall_jump.as_ref())
            && let Some(trigger) = super::wall_jump::interrupt(
                &mut f.wall_jump,
                attributes.can_walljump,
                contact,
                attributes.minimum_approach_speed,
                input,
                f.locomotion.tilt_x_age,
                jump_rules,
            )
        {
            let line = f.contacts[3].or(f.contacts[2]).unwrap();
            super::wall_jump::enter(f, jump_rules, trigger);
            events.push(Event::WallJumped { player, line });
        }
    }
    Ok(())
}

fn wall_jump_contact(
    fighter: &Fighter,
    geometry: &StageGeometry,
    previous: &StageGeometry,
    position_delta_x: f32,
) -> Option<super::wall_jump::Contact> {
    let (line_id, wall_side, point) = if let Some(line) = fighter.contacts[3] {
        (line, -1.0, fighter.ecb.current.left)
    } else {
        (fighter.contacts[2]?, 1.0, fighter.ecb.current.right)
    };
    let current = geometry.lines.get(line_id)?;
    let old = previous.lines.get(line_id)?;
    let point = add(fighter.position, point);
    let remapped = stage::remap_point([current.start, current.end], [old.start, old.end], point);
    Some(super::wall_jump::Contact {
        wall_side,
        wall_velocity_x: Some(point[0] - remapped[0]),
        position_delta_x,
    })
}

/// Ground collision modes (`inline2(coll, mode)`), applied once the ordinary
/// floor projection has already failed past the current line's end. See
/// docs/edges.md's "Ground collision modes" table for every source line.
/// Returns whether a clamp (mode 2) or teeter clamp (mode 1) held this frame.
fn floor_end_clamp(
    f: &mut Fighter,
    rules: &Rules,
    line: usize,
    geom_line: &stage::Line,
    stage: &stage::Stage<'_>,
    input: super::Controller,
) -> Result<bool, Error> {
    use crate::fighter::edge as math;
    let mode = super::edge::mode_for_action(f.action, rules.edge.is_some());
    if mode == math::Mode::Plain {
        return Ok(false);
    }
    let (left_pt, right_pt) = super::edge::line_ends(geom_line);
    let bottom_x = f.position[0] + f.ecb.current.bottom[0];
    let Some(side) = math::passed_side(bottom_x, left_pt[0], right_pt[0]) else {
        return Ok(false);
    };
    let edge_point = if side == math::Side::Left {
        left_pt
    } else {
        right_pt
    };
    let blocked = wall_blocks(stage, f, side, edge_point)?;
    let stick_limit = rules.edge.as_ref().map_or(0.75, |r| r.teeter_stick_limit);
    let query = math::EdgeQuery {
        bottom_x,
        left_x: left_pt[0],
        right_x: right_pt[0],
        facing: f.facing,
        stick_x: input.stick[0],
        stick_limit,
    };
    let Some(resolution) = math::resolve(mode, query, blocked) else {
        return Ok(false);
    };
    f.position = math::clamped_position(edge_point, f.ecb.current.bottom);
    f.ground_line = Some(line);
    f.contacts[0] = Some(line);
    f.edge_contact = Some(resolution.side);
    if resolution.enter_teeter {
        super::edge::enter(f);
    }
    Ok(true)
}

/// `mpCheckLeftWall`/`mpCheckRightWall`, simplified: this reuses the
/// generic wall sweep (already this codebase's own translation of the
/// wall-line crossing test, including joint bounding/extension) rather than
/// hand-porting their distinct `joint_id_skip`/`joint_id_only` traversal and
/// NULL-output variant. Both check for a wall between the edge point (one
/// unit inward and up) and the far ECB side.
fn wall_blocks(
    stage: &stage::Stage<'_>,
    f: &Fighter,
    side: crate::fighter::edge::Side,
    edge: [f32; 2],
) -> Result<bool, Error> {
    let (surface, inward_x, far_offset) = match side {
        crate::fighter::edge::Side::Left => (Surface::LeftWall, 1.0, f.ecb.current.right),
        crate::fighter::edge::Side::Right => (Surface::RightWall, -1.0, f.ecb.current.left),
    };
    let from = [edge[0] + inward_x, edge[1] + 1.0];
    let clamped = crate::fighter::edge::clamped_position(edge, f.ecb.current.bottom);
    let to = add(clamped, far_offset);
    Ok(stage
        .sweep(
            surface,
            Query {
                from,
                to,
                ..Default::default()
            },
        )
        .map_err(physics)?
        .is_some())
}

fn land(
    f: &mut Fighter,
    rules: &Rules,
    data: &FighterData,
    input: super::Controller,
    events: &mut Vec<Event>,
    player: usize,
    geometry: &StageGeometry,
) -> Result<(), Error> {
    // Captured before this ordinary landing bookkeeping runs: a move's own
    // `land()` hook (Fox's up special's shallow-floor-angle graze) can
    // restore this snapshot wholesale to decline the landing and continue
    // airborne, rather than needing to hand-revert every individual field
    // this function and `locomotion`/`wall_jump::landed` below are about
    // to touch (grounded, velocity, jumps_used, wall-jump timers, ...).
    let pre_landing = f.clone();
    f.contacts[0] = f.ground_line;
    // `ftCommon_8007D6A4` (run from `ftCo_Landing_Enter` via `ftCommon_
    // 8007D7FC`, the callback every ordinary landing call site in
    // `ft_081B.c` reaches through `ftCo_Landing_Enter_Basic`) sets
    // `gr_vel = self_vel.x` and flips `ground_or_air`, but never assigns
    // `self_vel.y`: the vertical self-velocity computed by this same
    // frame's fall integration survives the landing untouched (confirmed
    // against `fox-fd-3.slp`'s recorded `velocities.self_y`, which still
    // reports that fall value, not zero, on the frame a fighter lands).
    // The very next grounded frame overwrites both self-velocity axes
    // regardless (`Movement::project_ground` fully replaces them from
    // `ground_velocity`/`floor_normal`, matching `ftCommon_
    // ApplyGroundMovementNoSlide`), so leaving this frame's Y velocity
    // alone only changes what gets reported for this one frame, not any
    // later physics.
    f.knockback = [0.0; 2];
    f.ground_knockback = 0.0;
    f.ground_velocity = f.velocity[0];
    f.grounded = true;
    f.fast_fall = false;
    super::locomotion::landed(f);
    super::wall_jump::landed(f);
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
        } else if !super::specials::transfer_ground_air(f, true)
            && !super::specials::land(f, data, on_platform(f, geometry), &pre_landing)?
            && !super::escape_air::land(f, data, rules.escape_air.as_ref())?
            && !super::aerial::land(f, data)?
        {
            simulation::enter(f, Action::Landing);
            // ftCo_Landing_Enter_Basic passes allow_interrupt = true.
            f.landing_allow_interrupt = true;
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
