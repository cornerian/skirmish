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
    /// `item->xDD4_itemVar.ray.scale`'s own growth toward the article's own
    /// `max_scale` cap (`Item_UpdateRayAnimation`,
    /// `characters::fox::neutral::Laser::scale`'s own doc has the full
    /// citation and hitbox-offset-vs-radius distinction). `None` when the
    /// spawning move's own resource omits `laser.scale`: hitbox offsets are
    /// then used unscaled, this port's pre-existing behavior.
    pub scale: Option<RayScale>,
    /// `item->scl`, initialised from the spawning move's own item common
    /// attribute (`xCC_item_attr->x60_scale`, `it/item.c:672`) and never
    /// changed afterward for a laser. Multiplies every one of this
    /// instance's own hitbox radii in the hurtbox, shield and reflect
    /// tests below (`lbColl_80007B78(mtx, hurt, ip->scl, fp->x34_scale.y)`,
    /// `ft/ft_07C6.c:110` -> `lb/lbcollision.c:1570-1575`'s own `a_val =
    /// a->scale * x` term); the victim's own hurtbox radius is scaled by
    /// the *other* factor there (`fp->x34_scale.y`, `1.0` in a VS match,
    /// not `model_scaling`) and so is deliberately left alone by this
    /// field -- see the hurtbox capsule construction in `step` below.
    /// Defaults to `1.0` (`characters::fox::neutral::Laser::item_scale`'s
    /// own doc) when the spawning move's own resource omits
    /// `laser.item_scale`.
    pub item_scale: f32,
    /// `true` only for this instance's own first hit-test frame. Mirrors
    /// `it_8027129C`'s `HitCapsule_Enabled` state (`it/itcoll.c:866-891`):
    /// on the frame a hit capsule is (re-)enabled, the source sets both
    /// `x4C` (current world offset) and `x58` (swept-start) to the *same*
    /// freshly computed value -- a point capsule, not a sweep -- and only
    /// from the next frame on (`HitCapsule_Unk3`) does `x58` carry the
    /// *previous* frame's `x4C` into the swept test's start. A laser is
    /// spawned with every hit capsule already `Enabled`, so this flag
    /// models that first frame for the whole instance (all of a laser's
    /// hitboxes enable simultaneously, `it_802725D4`'s own per-index
    /// loop), and is cleared for every frame after.
    pub first_hit_frame: bool,
}

/// `item->xDD4_itemVar.ray.scale`'s own current value, growth rate and cap.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RayScale {
    pub current: f32,
    /// `|ray.speed| / 11.25`, precomputed once at spawn since `ray.speed`
    /// itself never changes after (this projectile system never
    /// re-accelerates a shot).
    pub per_frame: f32,
    pub cap: f32,
}

impl Projectile {
    /// The single collision radius used for the shield-bounce and
    /// Reflector swept-capsule tests (which do not need per-hitbox
    /// precision, unlike the hurtbox damage test below): the largest of
    /// this instance's own hitbox radii after `item_scale` (`Projectile::
    /// item_scale`'s own doc), or a minimal fallback.
    fn collision_radius(&self) -> f32 {
        self.hitboxes
            .iter()
            .map(|h| h.radius * self.item_scale)
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
    scale_cap: Option<f32>,
    item_scale: Option<f32>,
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
        scale: scale_cap.map(|cap| RayScale {
            current: 0.0,
            per_frame: speed.abs() / 11.25,
            cap,
        }),
        item_scale: item_scale.unwrap_or(1.0),
        first_hit_frame: true,
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

    // `Item_UpdateRayAnimation`'s own scale growth (`Laser::scale`'s own
    // doc has the full citation): the Anim callback this models runs
    // before Coll, so this frame's own hitbox test below already sees the
    // incremented value, including a freshly spawned shot's own first
    // increment the same frame it spawns (matching this function's own
    // same-frame Anim/Phys/Coll convention). `it_8027129C`'s own hitbox
    // state machine (`itcoll.c:1108-1119`) carries the *previous* frame's
    // world offset into the swept test's start (`hit->x58 = hit->x4C`)
    // before recomputing `x4C` fresh -- so the swept segment's two ends use
    // two different scale factors during the shot's own growth window, not
    // one shared value; `previous_scale_factor`/`scale_factor` reproduce
    // that pairing (both fall back to `1.0`, an unscaled offset, when this
    // shot's own `scale` is `None`).
    let previous_scale_factor = state.projectiles[index]
        .scale
        .map_or(1.0, |scale| scale.current);
    if let Some(scale) = &mut state.projectiles[index].scale {
        scale.current = (scale.current + scale.per_frame).min(scale.cap);
    }
    let scale_factor = state.projectiles[index]
        .scale
        .map_or(1.0, |scale| scale.current);

    // `Projectile::first_hit_frame`'s own doc: this frame's hit-capsule
    // state transition (`it_8027129C`) runs unconditionally, independent
    // of whether a hit is actually found below, so the flag is captured
    // and cleared here rather than only on the path that reaches the
    // hurtbox loop.
    let first_hit_frame = state.projectiles[index].first_hit_frame;
    state.projectiles[index].first_hit_frame = false;

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
        // Broadphase fast-reject radius `hurt.radius * broadphase_scale +
        // hit.radius`: the item-vs-fighter test passes `3.0 *
        // fp->x34_scale.y` (`= 3.0` in a VS match) here, the same constant
        // `simulation.rs:1019`/`grab.rs:1182` already use for the
        // fighter-vs-fighter and grab paths (`lbColl_8000805C`/
        // `lbColl_80008248`'s own last argument, `lb/lbcollision.c:
        // 1690-1740`) -- not `1.0`, which was this port's own unfixed
        // guess.
        let overlaps = crate::collision::shield::capsule_matrix(
            &swept,
            &reflect_capsule,
            bone_matrix,
            3.0,
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
        // `3.0` broadphase scale, not `1.0` -- see the Reflector test's own
        // citation above.
        let overlaps = crate::collision::shield::capsule_matrix(
            &swept,
            &shield_capsule,
            &matrix,
            3.0,
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
            // Growth scales the hitbox's own offset from the tracked
            // position, not its radius (`Laser::scale`'s own doc:
            // `lb_8000B1CC` only ever writes a position, never
            // `HitCapsule.scale`, the separate radius field).
            let offset = [
                position[0] + hit.center[0] * facing * scale_factor,
                position[1] + hit.center[1] * scale_factor,
                position[2] + hit.center[2] * scale_factor,
            ];
            // `Projectile::first_hit_frame`'s own doc, `it_8027129C`
            // (`it/itcoll.c:866-891`): on this instance's own first
            // hit-test frame the source's `HitCapsule_Enabled` branch sets
            // `x58 = x4C`, i.e. the swept test's start is literally this
            // same frame's `offset`, not a separately-scaled previous
            // position -- reusing `offset` here (rather than recomputing
            // from `previous_position`/`previous_scale_factor`) is the
            // point, since those two can already disagree with
            // `scale_factor` on a freshly spawned, still-growing shot.
            // From the second frame on (`HitCapsule_Unk3`), `x58` carries
            // the previous frame's own `x4C` forward, modeled by
            // `previous_position`/`previous_scale_factor` as before.
            let previous_offset = if first_hit_frame {
                offset
            } else {
                [
                    previous_position[0] + hit.center[0] * facing * previous_scale_factor,
                    previous_position[1] + hit.center[1] * previous_scale_factor,
                    previous_position[2] + hit.center[2] * previous_scale_factor,
                ]
            };
            let swept = Capsule {
                start: previous_offset,
                end: offset,
                // `Projectile::item_scale`'s own doc:
                // `lbColl_80007B78(mtx, hurt, ip->scl, fp->x34_scale.y)`
                // (`ft/ft_07C6.c:110` -> `lb/lbcollision.c:1570-1575`)
                // scales the item's own hit-capsule radius by `ip->scl`,
                // not the hurtbox below.
                radius: hit.radius * state.projectiles[index].item_scale,
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
                // Deliberately unscaled: `lbColl_80007B78`'s own hurtbox
                // factor is `fp->x34_scale.y` (`1.0` in a VS match), not
                // `ip->scl`/`item_scale` and not `model_scaling` -- see
                // `Projectile::item_scale`'s own doc.
                let capsule = Capsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let world_matrix = poses[victim]
                    .world_matrix(hurtbox.bone)
                    .map_err(|e| super::Error::Physics(e.to_string()))?;
                let mut contact = crate::collision::shield::Contact::default();
                // `3.0` broadphase scale, not `1.0` -- see the Reflector
                // test's own citation above.
                let overlaps = crate::collision::shield::capsule_matrix(
                    &swept,
                    &capsule,
                    world_matrix,
                    3.0,
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
                    // The `true` (`projectile`) argument also suppresses
                    // this hit's own attacker hitlag; see
                    // `damage::resolve_prepared_hit`'s own citation.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::data::HitElement;

    fn hitbox(radius: f32) -> Hitbox {
        Hitbox {
            clank: false,
            rebound: false,
            element: HitElement::default(),
            group: 0,
            bone: 0,
            center: [0.0; 3],
            radius,
            damage: 3,
            shield_damage: 0,
            angle_degrees: 361.0,
            growth: 0,
            fixed: 0,
            base: 0,
        }
    }

    fn spawn_with(hitboxes: Vec<Hitbox>, item_scale: Option<f32>) -> Projectile {
        let mut counter = crate::fighter::stale::InstanceCounter::default();
        spawn(
            ProjectileKind::FoxLaser,
            0,
            [0.0, 0.0, 0.0],
            0.0,
            7.0,
            35.0,
            hitboxes,
            18,
            None,
            item_scale,
            &mut counter,
        )
    }

    #[test]
    fn omitted_item_scale_defaults_to_1_and_leaves_radii_unscaled() {
        let projectile = spawn_with(vec![hitbox(1.1718), hitbox(1.5624)], None);
        assert_eq!(projectile.item_scale, 1.0);
        assert!((projectile.collision_radius() - 1.5624).abs() < 1e-6);
    }

    /// A synthetic value, not the real Fox/Falco `x60_scale` -- confirmed
    /// separately to be `1.0` for both (`characters::fox::neutral::
    /// Laser::item_scale`'s own doc), so `item_scale` is a data-driven
    /// no-op for either character today and this only pins the
    /// multiplication mechanism itself (`Projectile::collision_radius`,
    /// shared by the shield and reflect tests) against every one of this
    /// instance's own hitbox radii, largest included, for whatever future
    /// item does need a non-`1.0` value.
    #[test]
    fn item_scale_multiplies_every_laser_hitbox_radius() {
        let synthetic = 1.25;
        let projectile = spawn_with(vec![hitbox(1.1718), hitbox(1.5624)], Some(synthetic));
        assert_eq!(projectile.item_scale, synthetic);
        assert!((projectile.collision_radius() - 1.5624 * synthetic).abs() < 1e-6);
    }

    #[test]
    fn a_freshly_spawned_projectile_starts_on_its_first_hit_frame() {
        let projectile = spawn_with(vec![hitbox(1.0)], None);
        assert!(projectile.first_hit_frame);
    }

    /// `crate::collision::shield::capsule_matrix`'s own broadphase margin
    /// (`hurt.radius * broadphase_scale + hit.radius`, `lbColl_8000805C`/
    /// `lbColl_80008248`'s own last argument, `lb/lbcollision.c:
    /// 1690-1740`) is not redundant with the narrowphase test below it: a
    /// non-uniform or larger-than-1 hurtbox bone matrix inflates the real
    /// (transformed) acceptance radius well past `hit.radius + hurt.radius`
    /// (the narrowphase branch's own `hurt_radius = hurt.radius * distance
    /// / local_distance` term, `src/collision/shield.rs`), so a broadphase
    /// scale of `1.0` -- this port's own previous, unfixed guess at every
    /// one of `game::projectile::step`'s three `capsule_matrix` call sites
    /// -- can reject a pair the narrowphase test would otherwise confirm.
    /// This reproduces exactly that: a hurtbox bone matrix scaled `10x`
    /// (a stand-in for a larger fighter model, not a specific real value)
    /// makes a `0.05`-radius hurtbox reach a real `0.5` world-space radius;
    /// a hit `0.2` world units away clears the real (narrowphase) contact
    /// test but sits outside the old `1.0` broadphase's `0.15` fast-reject
    /// radius and inside the fixed `3.0` broadphase's `0.25` one.
    #[test]
    fn broadphase_scale_3_finds_a_contact_the_old_1_would_have_missed() {
        let hit = Capsule {
            start: [0.0, 0.0, 0.0],
            end: [0.0, 0.0, 0.0],
            radius: 0.1,
        };
        let hurt = Capsule {
            start: [0.2, 0.0, 0.0],
            end: [0.2, 0.0, 0.0],
            radius: 0.05,
        };
        let matrix = [
            [10.0, 0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0, 0.0],
            [0.0, 0.0, 10.0, 0.0],
        ];

        let mut rejected = crate::collision::shield::Contact::default();
        assert_eq!(
            crate::collision::shield::capsule_matrix(&hit, &hurt, &matrix, 1.0, &mut rejected),
            Ok(false),
            "the old broadphase_scale=1.0 should still reject this pair before narrowphase runs"
        );

        let mut accepted = crate::collision::shield::Contact::default();
        assert_eq!(
            crate::collision::shield::capsule_matrix(&hit, &hurt, &matrix, 3.0, &mut accepted),
            Ok(true),
            "broadphase_scale=3.0 (the fixed constant) must pass this pair through to a \
             narrowphase test that confirms real contact"
        );
    }
}
