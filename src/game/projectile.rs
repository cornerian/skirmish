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
    shield, staling,
};
use crate::{collision::bones::Pose, fighter::combat::Capsule};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileKind {
    FoxLaser,
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
) -> Result<(), super::Error> {
    let mut index = 0;
    while index < state.projectiles.len() {
        match step(data, state, poses, index)? {
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

    // Terrain despawn: approximated as leaving the stage's own outer bounding
    // box, not a true swept ray-vs-terrain-line cast (`it_8026E9A4`); see
    // docs/fox-neutral-special.md.
    let [left, right, bottom, top] = data.stage.blast;
    let [x, y] = [
        state.projectiles[index].position[0],
        state.projectiles[index].position[1],
    ];
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
        if overlaps && (damage as i32) <= down.reflect.max_damage {
            state.projectiles[index].owner = victim;
            state.projectiles[index].angle += core::f32::consts::PI;
            for hit in &mut state.projectiles[index].hitboxes {
                let scaled = hit.damage as f32 * down.reflect.damage_mul;
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
                    damage::apply_hit(
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
                    )?;
                    if data.rules.staling.is_some() {
                        state.fighters[owner]
                            .staling
                            .queue
                            .record(staled.identity, false);
                    }
                    state.events.push(Event::ProjectileHit { owner, victim });
                    break 'hitboxes;
                }
            }
        }
        if !state.projectiles[index].hitboxes.is_empty()
            && state
                .events
                .iter()
                .any(|e| matches!(e, Event::ProjectileHit { owner: o, victim: v } if *o == owner && *v == victim))
        {
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
