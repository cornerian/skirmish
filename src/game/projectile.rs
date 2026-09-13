//! The minimal generic fired-projectile system: spawn, per-frame motion,
//! lifetime/despawn, and hurtbox/shield/reflect collision producing the
//! ordinary damage pipeline with the item's own knockback. No item pickup,
//! no clank-vs-item, no absorption (no absorbing character exists in this
//! codebase). See `docs/fox-neutral-special.md` for the full citation list
//! and scope; Fox's Blaster (`characters::fox::neutral`) is the first and
//! today only spawner, but nothing here is Fox-specific.
//!
//! Modeled on the source's own generic "ray" item helpers
//! (`melee/it/kinds/inlines.h:139-224`: `Item_UpdateRayAnimation`,
//! `Item_BounceRayOffShield`, `Item_ResetRayAfterReflection`), shared by
//! Fox/Falco's laser (`itfoxlaser.c`) and the sibling L-Gun-Ray item
//! (`itlgunray.c`).

use super::{
    Event, State, damage,
    data::{Hitbox, MatchData},
    script, shield, staling,
};
use crate::{
    collision::{
        bones::Pose,
        stage::{Query, Stage, Surface},
    },
    fighter::combat::Capsule,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileKind {
    FoxLaser,
    /// Falco's own Laser (`It_Kind_Falco_Laser`, `melee/it/forward.h:182`,
    /// the enum value immediately after `It_Kind_Fox_Laser`). A label only:
    /// `melee/it/it_3F2F.c`'s own per-item logic table gives "Falco laser"
    /// the byte-identical stanza "Fox laser" uses (same `it_803F67D0` state
    /// table, same `itFoxLaser_Logic94_*` callbacks -- confirmed by
    /// `tests/falco_laser_table_differential.rs`, not assumed), and there is
    /// no `itfalcolaser.c` anywhere in the pinned decomp. Every function in
    /// this file is already generic over `kind`; this variant exists purely
    /// so observation/replay code can report which character's laser a
    /// spawn was (matching a real recording's own distinct `FALCO_LASER`
    /// vs. `FOX_LASER` Slippi item type), not to change any behavior here.
    FalcoLaser,
}

/// One in-flight projectile. `hitboxes` are this instance's own fixed
/// capsules and damage/knockback attributes (`data::Hitbox`, the same
/// shape a fighter's own attack hitboxes use -- `bone`/`group`/`clank`/
/// `rebound`/`element` are unused by a projectile and stay at their
/// defaults; `center` is instead this port's own item-local offset from
/// `position`, mirrored by `facing` before use, matching how the source's
/// own item hitbox offsets are defined relative to the item's own root).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Projectile {
    pub kind: ProjectileKind,
    /// Player index (0/1) this projectile currently belongs to. Flips on a
    /// successful Reflector hand-off (`itFoxLaser_Logic94_Reflected`).
    pub owner: usize,
    /// World-space position, matching `Item::pos`.
    pub position: [f32; 3],
    /// `item->xDD4_itemVar.ray.angle`, radians.
    pub angle: f32,
    /// `item->xDD4_itemVar.ray.speed`.
    pub speed: f32,
    /// `item->facing_dir`: `sign(cos(angle))` after each recompute,
    /// exposed for observation/replay use.
    pub facing: f32,
    /// Frames remaining (`it_80275158`'s own countdown).
    pub lifetime: f32,
    pub hitboxes: Vec<Hitbox>,
    /// One `staling::Entry` allocated at spawn, matching a fighter's own
    /// attack-instance allocation on first use of a distinct attack.
    pub staling_identity: crate::fighter::stale::Entry,
}

impl Projectile {
    /// The single collision radius used for the shield-bounce and
    /// Reflector swept-capsule tests (which do not need per-hitbox
    /// precision, unlike the hurtbox damage test below): the largest of
    /// this instance's own hitbox radii, or a minimal fallback.
    fn collision_radius(&self) -> f32 {
        self.hitboxes
            .iter()
            .map(|h| h.radius)
            .fold(0.0_f32, f32::max)
            .max(0.01)
    }
}

fn velocity(angle: f32, speed: f32) -> [f32; 2] {
    [speed * libm::cosf(angle), speed * libm::sinf(angle)]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    kind: ProjectileKind,
    owner: usize,
    position: [f32; 3],
    angle: f32,
    speed: f32,
    lifetime: f32,
    hitboxes: Vec<Hitbox>,
    move_id: u16,
    attack_instances: &mut crate::fighter::stale::InstanceCounter,
) -> Projectile {
    let [vx, _] = velocity(angle, speed);
    let mut staling_identity = crate::fighter::stale::Entry::INACTIVE;
    staling_identity.change_move(move_id, attack_instances);
    Projectile {
        kind,
        owner,
        position,
        angle,
        speed,
        facing: if vx >= 0.0 { 1.0 } else { -1.0 },
        lifetime,
        hitboxes,
        staling_identity,
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    core::array::from_fn(|i| a[i] - b[i])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = libm::sqrtf(dot(v, v));
    (len > 1e-6).then(|| v.map(|x| x / len))
}

/// `lbVector_Mirror`: reflects `velocity` across the unit `normal`.
fn mirror(velocity: [f32; 2], normal: [f32; 3]) -> [f32; 2] {
    let v = [velocity[0], velocity[1], 0.0];
    let d = dot(v, normal);
    let reflected: [f32; 3] = core::array::from_fn(|i| v[i] - 2.0 * d * normal[i]);
    [reflected[0], reflected[1]]
}

/// Runs every active projectile's motion, lifetime and collision for this
/// frame, in a single pass after the ordinary fighter update (matching
/// "item logic runs after fighters at its own GObj priority"). `poses` are
/// this frame's already-computed fighter poses (`simulation::pose`),
/// reused rather than recomputed.
pub(crate) fn advance(
    data: &MatchData,
    state: &mut State,
    poses: &[Pose; 2],
    stage: &Stage<'_>,
) -> Result<(), super::Error> {
    let mut index = 0;
    while index < state.projectiles.len() {
        match step(data, state, poses, stage, index)? {
            Outcome::Despawn => {
                state.projectiles.remove(index);
            }
            Outcome::Keep => index += 1,
        }
    }
    Ok(())
}

enum Outcome {
    Keep,
    Despawn,
}

fn step(
    data: &MatchData,
    state: &mut State,
    poses: &[Pose; 2],
    stage: &Stage<'_>,
    index: usize,
) -> Result<Outcome, super::Error> {
    let owner = state.projectiles[index].owner;
    let victim = 1 - owner;

    // Motion: `position += velocity` every frame (`Item_UpdateRayAnimation`).
    let [vx, vy] = velocity(
        state.projectiles[index].angle,
        state.projectiles[index].speed,
    );
    let previous_position = state.projectiles[index].position;
    state.projectiles[index].position[0] += vx;
    state.projectiles[index].position[1] += vy;
    state.projectiles[index].facing = if vx >= 0.0 { 1.0 } else { -1.0 };

    // Terrain despawn: a real swept ray-vs-stage-line cast (`it_8026E9A4` ->
    // `mpCheckAllRemap` -> `mpCheckMultiple`, checking floor|ceiling|
    // left-wall|right-wall, `checks & 0xF`), reusing the exact pinned line-
    // intersection/remap primitives `collision::stage::Stage` already
    // exposes for fighters' own ECB collision (the same `sweep` call
    // `collision::resolve` makes) rather than the stage's outer bounding
    // box alone. `mpCheckAllRemap`'s own `checks & 0x10` bit additionally
    // selects the "Remap" floor/ceiling/wall variants, which track a line's
    // *previous* frame position for moving platforms; this codebase's own
    // moving-platform remap plumbing (`collision::resolve`'s
    // `previous_geometry` parameter) is not threaded through the projectile
    // system, so a moving platform is checked at its current position only
    // -- exact for the (much more common) static-geometry case, and a
    // documented simplification otherwise (see docs/fox-neutral-special.md).
    // `mpCheckMultiple`'s own full line-array scan is not itself pinned as a
    // C-oracle differential: it requires the complete stage collision-line
    // data set the way `ledge_snap.c`'s own adapter already declines to
    // reproduce for the analogous `mpCheckMultiple` obstruction scan.
    let previous_xy = [previous_position[0], previous_position[1]];
    let current_xy = [
        state.projectiles[index].position[0],
        state.projectiles[index].position[1],
    ];
    let hit_terrain = [
        Surface::Floor,
        Surface::Ceiling,
        Surface::LeftWall,
        Surface::RightWall,
    ]
    .into_iter()
    .try_fold(false, |hit, surface| {
        if hit {
            return Ok(true);
        }
        stage
            .sweep(
                surface,
                Query {
                    from: previous_xy,
                    to: current_xy,
                    ..Default::default()
                },
            )
            .map(|contact| contact.is_some())
    })
    .map_err(|e: crate::collision::stage::StageError| super::Error::Physics(e.to_string()))?;
    if hit_terrain {
        return Ok(Outcome::Despawn);
    }
    // The stage's own outer bounding box remains a cheap secondary net for a
    // shot that flies clean off the arena without ever crossing a line
    // (open blast zones beyond the stage's own collision geometry).
    let [left, right, bottom, top] = data.stage.blast;
    let [x, y] = current_xy;
    if x < left || x > right || y < bottom || y > top {
        return Ok(Outcome::Despawn);
    }

    // Reflector: gated on the target's own `shield.reflecting` bit and
    // `down::Reflect` geometry, and on the laser's damage not exceeding
    // `down::Reflect.max_damage` (`ftColl_80077464`, `ftcoll.c:764`).
    if state.fighters[victim].shield.reflecting
        && let Some(specials) = data.fighters[victim].specials.as_ref()
        && let Some(down) = specials.fox_down()
    {
        let swept = Capsule {
            start: previous_position,
            end: state.projectiles[index].position,
            radius: state.projectiles[index].collision_radius(),
        };
        let bone_matrix = poses[victim]
            .world_matrix(down.reflect.bone as usize)
            .map_err(|e| super::Error::Physics(e.to_string()))?;
        let center = crate::collision::bones::transform_point(bone_matrix, down.reflect.offset);
        let reflect_capsule = Capsule {
            start: center,
            end: center,
            radius: down.reflect.size,
        };
        let mut contact = crate::collision::shield::Contact::default();
        let overlaps = crate::collision::shield::capsule_matrix(
            &swept,
            &reflect_capsule,
            bone_matrix,
            1.0,
            &mut contact,
        )
        .map_err(|e| super::Error::Physics(e.to_string()))?;
        let damage = state.projectiles[index]
            .hitboxes
            .iter()
            .map(|h| h.damage)
            .max()
            .unwrap_or(0);
        let accepts = if overlaps {
            let fighter = &state.fighters[victim];
            let mut flags = BTreeMap::new();
            flags.insert("reflecting".to_owned(), fighter.shield.reflecting);
            let view = script::FighterView {
                id: victim as u8,
                action: format!("{:?}", fighter.action),
                action_frame: fighter.action_frame,
                velocity: fighter.velocity,
                grounded: fighter.grounded,
                percent: fighter.percent,
                hitlag: fighter.hitlag,
                hitstun: fighter.hitstun,
                flags,
            };
            let program = if let Some(program) = data.fighters[victim].script.as_ref() {
                if program
                    .has_hook(script::Hook::OnProjectileContact)
                    .map_err(|error| super::Error::Data(error.to_string()))?
                {
                    Some(program.clone())
                } else {
                    script::bundled_source(data.fighters[victim].specials.as_ref())
                        .map(script::Program::new)
                        .transpose()
                        .map_err(|error| super::Error::Data(error.to_string()))?
                }
            } else {
                script::bundled_source(data.fighters[victim].specials.as_ref())
                    .map(script::Program::new)
                    .transpose()
                    .map_err(|error| super::Error::Data(error.to_string()))?
            };
            let Some(program) = program else {
                return Ok(Outcome::Keep);
            };
            let locals = state.fighters[victim].script_state.clone();
            let hit = script::HitView {
                damage: damage as f32,
                max_damage: down.reflect.max_damage,
                projectile: true,
                ..Default::default()
            };
            let result = program
                .dispatch(
                    script::Hook::OnProjectileContact,
                    &view,
                    Some(&hit),
                    &locals,
                )
                .map_err(|error| super::Error::Data(error.to_string()))?;
            state.fighters[victim].script_state = result.locals;
            script::apply_commands(state, victim, &result.commands)?;
            result.hit.is_some_and(|patch| patch.reflect)
        } else {
            false
        };
        if accepts {
            state.projectiles[index].owner = victim;
            state.projectiles[index].angle += core::f32::consts::PI;
            for hit in &mut state.projectiles[index].hitboxes {
                // `item.c:1613-1619` (`Item_80269F14`): `hit.damage * xC6C +
                // 0.99f`, truncated toward zero -- not a plain product. The
                // `+ 0.99` term was missing here (a real discrepancy this
                // batch's own differential exposed, `tests/oracle/item.c`'s
                // `oracle_reflect_damage_scaling`); the global cap
                // (`it_804D6D28->xD8`) stays unmodeled, as before, since its
                // real runtime value is not in the pinned decomp.
                let scaled = hit.damage as f32 * down.reflect.damage_mul + 0.99;
                hit.damage = scaled.max(0.0) as u32;
            }
            // `down::Reflect.speed_mul` is deliberately not applied: Fox's
            // own laser reflect callback never touches speed, unlike the
            // sibling L-Gun-Ray item (`docs/fox-neutral-special.md`).
            state.events.push(Event::ProjectileReflected { owner });
            return Ok(Outcome::Keep);
        }
    }

    // Shield: a true velocity mirror across the contact normal
    // (`Item_BounceRayOffShield`/`lbVector_Mirror`), confirmed against a
    // real recording (see docs/fox-neutral-special.md). No shield-health
    // depletion is modeled.
    if shield::active(&state.fighters[victim])
        && let Some(rules) = &data.rules.shield
    {
        let (center, matrix) = shield::geometry(
            &state.fighters[victim],
            &data.fighters[victim],
            rules,
            &poses[victim],
        )
        .map_err(|e| super::Error::Physics(e.to_string()))?;
        let swept = Capsule {
            start: previous_position,
            end: state.projectiles[index].position,
            radius: state.projectiles[index].collision_radius(),
        };
        let shield_capsule = Capsule {
            start: center,
            end: center,
            radius: 1.0,
        };
        let mut contact = crate::collision::shield::Contact::default();
        let overlaps = crate::collision::shield::capsule_matrix(
            &swept,
            &shield_capsule,
            &matrix,
            1.0,
            &mut contact,
        )
        .map_err(|e| super::Error::Physics(e.to_string()))?;
        if overlaps && let Some(normal) = normalize(sub(contact.position, center)) {
            let mirrored = mirror([vx, vy], normal);
            state.projectiles[index].angle = libm::atan2f(mirrored[1], mirrored[0]);
            state.projectiles[index].speed = libm::sqrtf(dot(
                [mirrored[0], mirrored[1], 0.0],
                [mirrored[0], mirrored[1], 0.0],
            ));
            state.events.push(Event::ProjectileHit { owner, victim });
            return Ok(Outcome::Keep);
        }
    }

    // Hurtbox: the same eligibility gates and `capsule_matrix` primitive
    // `simulation::advance`'s own fighter-vs-fighter hit loop uses.
    let target = &state.fighters[victim];
    if target.invincibility == 0
        && target.intangibility == 0
        && target.body_state.accepts_contact()
        && target.grab.captor.is_none()
        && !shield::break_invulnerable(target.action)
        && !matches!(
            target.action,
            super::Action::Respawn | super::Action::Eliminated
        )
        && !super::rebirth::invulnerable(target.action)
        && !super::death::owns_action(target.action)
    {
        let facing = state.projectiles[index].facing;
        let position = state.projectiles[index].position;
        let mut connected = false;
        'hitboxes: for hit in state.projectiles[index].hitboxes.clone() {
            let offset = [
                position[0] + hit.center[0] * facing,
                position[1] + hit.center[1],
                position[2] + hit.center[2],
            ];
            let previous_offset = [
                previous_position[0] + hit.center[0] * facing,
                previous_position[1] + hit.center[1],
                previous_position[2] + hit.center[2],
            ];
            let swept = Capsule {
                start: previous_offset,
                end: offset,
                radius: hit.radius,
            };
            for (hurt_index, hurtbox) in data.fighters[victim].hurtboxes.iter().enumerate() {
                if !super::simulation::hurtbox_state(
                    &state.fighters[victim],
                    &data.fighters[victim],
                    hurt_index,
                )?
                .accepts_contact()
                {
                    continue;
                }
                let hurt = hurtbox
                    .physics()
                    .transform(&poses[victim], 1.0)
                    .map_err(|e| super::Error::Physics(e.to_string()))?;
                let capsule = Capsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let world_matrix = poses[victim]
                    .world_matrix(hurtbox.bone)
                    .map_err(|e| super::Error::Physics(e.to_string()))?;
                let mut contact = crate::collision::shield::Contact::default();
                let overlaps = crate::collision::shield::capsule_matrix(
                    &swept,
                    &capsule,
                    world_matrix,
                    1.0,
                    &mut contact,
                )
                .map_err(|e| super::Error::Physics(e.to_string()))?;
                if overlaps {
                    let height = data.fighters[victim]
                        .damage_poses
                        .as_ref()
                        .map_or_else(Default::default, |p| p.hurtbox_heights[hurt_index]);
                    let staled = staling::Hit {
                        identity: state.projectiles[index].staling_identity,
                        group: hit.group,
                        base_damage: hit.damage,
                        damage: data
                            .rules
                            .staling
                            .as_ref()
                            .map_or(hit.damage as f32, |rules| {
                                state.fighters[owner].staling.queue.damage(
                                    state.projectiles[index].staling_identity.move_id as i32,
                                    hit.damage as f32,
                                    rules,
                                )
                            }),
                    };
                    let accepted = damage::apply_hit(
                        data,
                        state,
                        owner,
                        &hit,
                        staled,
                        height,
                        damage::HitDirection::FighterContact(damage::FighterContact {
                            hurt_start: hurt.start,
                            hurt_end: hurt.end,
                            position: contact.position,
                        }),
                        true,
                    )?;
                    if !accepted {
                        continue;
                    }
                    if data.rules.staling.is_some() {
                        state.fighters[owner]
                            .staling
                            .queue
                            .record(staled.identity, false);
                    }
                    state.events.push(Event::ProjectileHit { owner, victim });
                    connected = true;
                    break 'hitboxes;
                }
            }
        }
        if connected {
            return Ok(Outcome::Despawn);
        }
    }

    // Lifetime countdown (`it_80275158`).
    state.projectiles[index].lifetime -= 1.0;
    if state.projectiles[index].lifetime <= 0.0 {
        return Ok(Outcome::Despawn);
    }
    Ok(Outcome::Keep)
}
