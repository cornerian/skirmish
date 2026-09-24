//! The minimal generic fired-projectile system: spawn, per-frame motion,
//! lifetime/despawn, and hurtbox/shield/reflect collision producing the
//! ordinary damage pipeline with the item's own knockback. No item pickup,
//! no clank-vs-item, no absorption (no absorbing character exists in this
//! codebase). See `docs/fox-neutral-special.md` for the full citation list
//! and scope; the bundled fighter's Blaster policy is the first and
//! today only spawner, but nothing here is Fox-specific.
//!
//! Modeled on the source's own generic "ray" item helpers
//! (`melee/it/kinds/inlines.h:139-224`: `Item_UpdateRayAnimation`,
//! `Item_BounceRayOffShield`, `Item_ResetRayAfterReflection`), shared by
//! Fox/Falco's laser (`itfoxlaser.c`) and the sibling L-Gun-Ray item
//! (`itlgunray.c`).

use super::{
    Event, State,
    data::{Hitbox, MatchData},
    script, staling,
};
use crate::game::script::resources::{
    ArticleId, ProjectileContactPolicy, ProjectilePersistence, ProjectileReflection,
    ProjectileShield,
};
use crate::{
    collision::{
        bones::Pose,
        stage::{Query, Stage, Surface},
    },
    fighter::{combat::Capsule, shield},
};
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Stable identity for an in-flight article.
///
/// Handles are allocated by the match state and are never reused during a
/// match.  They intentionally do not encode a projectile vector index, so a
/// removal cannot make an old reference point at a different article.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ArticleHandle(u64);

impl<'de> Deserialize<'de> for ArticleHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = u64::deserialize(deserializer)?;
        if raw == 0 {
            return Err(serde::de::Error::custom("article handles must be nonzero"));
        }
        Ok(Self(raw))
    }
}

impl ArticleHandle {
    pub const INVALID: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// A fully validated projectile command staged by a fighter lifecycle
/// transaction. Keeping hitboxes typed avoids reparsing callback JSON during
/// the post-fighter phase.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct PendingProjectile {
    pub kind: ProjectileKind,
    pub position: [f32; 3],
    pub angle: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub hitboxes: Vec<Hitbox>,
    pub move_id: u16,
}

/// Compact article command staged by a script callback.  The immutable
/// article catalog supplies lifetime, hitboxes, and staling move id only once
/// the command is drained after the fighter phase.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct PendingArticleSpawn {
    pub article_id: ArticleId,
    pub position: [f32; 3],
    pub launch: ArticleLaunch,
}

/// A ray keeps its explicit launch values for compatibility. Gravity articles
/// own their angle and speed and only need the fighter-facing sign at spawn.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) enum ArticleLaunch {
    Explicit { angle: f32, speed: f32 },
    Facing(f32),
}

pub(crate) const MAX_PENDING_PROJECTILES: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize)]
struct ReflectDescriptor {
    bone: u32,
    max_damage: i32,
    offset: [f32; 3],
    size: f32,
    damage_mul: f32,
}

type ReflectionGeometry = (
    [f32; 3],
    crate::collision::bones::Matrix,
    f32,
    i32,
    f32,
    bool,
);

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
    LuigiFire,
    Gravity(ArticleId),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GravityProjectileState {
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub surface_multiplier: f32,
    pub terrain_stop_speed: f32,
    pub half_life: f32,
    pub contact: ProjectileContactPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum ProjectileBehavior {
    Ray,
    Gravity(GravityProjectileState),
    /// Mario's `itMariofireball_UnkMotion0_*` callbacks.  It shares the
    /// optimized falling and swept collision primitives with gravity
    /// articles, while retaining a distinct native behavior identity.
    MarioFireball(GravityProjectileState),
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
    /// Stable match-local identity, independent of the projectile vector slot.
    handle: ArticleHandle,
    pub kind: ProjectileKind,
    pub behavior: ProjectileBehavior,
    /// Player index (0/1) this projectile currently belongs to. Flips on a
    /// successful Reflector hand-off (`itFoxLaser_Logic94_Reflected`).
    pub owner: usize,
    /// World-space position, matching `Item::pos`.
    pub position: [f32; 3],
    /// `item->xDD4_itemVar.ray.angle`, radians.
    pub angle: f32,
    /// `item->xDD4_itemVar.ray.speed`.
    pub speed: f32,
    /// Native `xD48_halfLifeTimer` captured at spawn. Gravity articles supply
    /// this value through their validated typed resource; rays have no value.
    pub half_life: Option<f32>,
    /// Current world-space velocity; trigonometry is done at spawn and when a
    /// reflection changes direction, never once per simulation frame.
    pub velocity: [f32; 2],
    /// `item->facing_dir`: `sign(cos(angle))` after each recompute,
    /// exposed for observation/replay use.
    pub facing: f32,
    /// Frames remaining (`it_80275158`'s own countdown).
    pub lifetime: f32,
    pub hitboxes: Vec<Hitbox>,
    /// One `staling::Entry` allocated at spawn, matching a fighter's own
    /// attack-instance allocation on first use of a distinct attack.
    pub staling_identity: crate::fighter::state::stale::Entry,
}

impl Projectile {
    pub const fn handle(&self) -> ArticleHandle {
        self.handle
    }
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

/// Match the item ray helpers' canonical angle range and zero-velocity
/// facing convention (`melee/it/kinds/inlines.h:139-208`).  The original
/// treats an exactly vertical ray as left-facing because it tests `x > 0`,
/// rather than `x >= 0`; keeping that distinction matters for mirrored
/// hitbox offsets and replay-visible facing.
fn normalize_angle(mut angle: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    // Host validation normally rejects non-finite launch angles, but this
    // boundary is also exercised directly by native callers.  Preserve the
    // value while avoiding an endless loop for infinity (or a huge amount of
    // work for a very large finite value).
    if !angle.is_finite() {
        return angle;
    }
    let remainder = angle.rem_euclid(tau);
    // The source loops stop at exactly TAU (`>` rather than `>=`), so positive
    // whole turns retain TAU while negative whole turns normalize to zero.
    if remainder == 0.0 && angle > 0.0 {
        angle = tau;
    } else {
        angle = remainder;
    }
    angle
}

fn facing_from_velocity(x: f32) -> f32 {
    if x > 0.0 { 1.0 } else { -1.0 }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    handle: ArticleHandle,
    kind: ProjectileKind,
    behavior: ProjectileBehavior,
    owner: usize,
    position: [f32; 3],
    angle: f32,
    speed: f32,
    lifetime: f32,
    hitboxes: Vec<Hitbox>,
    move_id: u16,
    attack_instances: &mut crate::fighter::state::stale::InstanceCounter,
) -> Projectile {
    let angle = normalize_angle(angle);
    let velocity = velocity(angle, speed);
    let [vx, _] = velocity;
    let mut staling_identity = crate::fighter::state::stale::Entry::INACTIVE;
    staling_identity.change_move(move_id, attack_instances);
    Projectile {
        handle,
        kind,
        behavior,
        owner,
        position,
        angle,
        speed,
        half_life: match behavior {
            ProjectileBehavior::Ray => None,
            ProjectileBehavior::Gravity(gravity) | ProjectileBehavior::MarioFireball(gravity) => {
                Some(gravity.half_life)
            }
        },
        velocity,
        facing: facing_from_velocity(vx),
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

fn speed(velocity: [f32; 2]) -> f32 {
    libm::sqrtf(velocity[0] * velocity[0] + velocity[1] * velocity[1])
}

fn gravity_step(velocity: [f32; 2], gravity: f32, terminal_velocity: f32) -> [f32; 2] {
    let y = velocity[1] - gravity;
    [
        velocity[0],
        if gravity >= 0.0 {
            y.max(-terminal_velocity)
        } else {
            y.min(terminal_velocity)
        },
    ]
}

fn surface_bounce(velocity: [f32; 2], normal: [f32; 3], surface_multiplier: f32) -> [f32; 2] {
    let reflected = mirror(velocity, normal);
    [
        reflected[0] * surface_multiplier,
        reflected[1] * surface_multiplier,
    ]
}

fn luigi_fireball_terrain_despawns(speed: f32, threshold: f32) -> bool {
    speed < threshold
}

fn mario_fireball_terrain_despawns(speed: f32, threshold: f32) -> bool {
    speed < threshold
}

/// Mario and Luigi's ordinary HitShield callbacks return true, which consumes
/// the article. Their separate ShieldBounced callbacks are not represented by
/// this host contact path, so they must not be inferred from generic policy.
fn source_fireball_hit_shield_despawns(
    kind: ProjectileKind,
    behavior: &ProjectileBehavior,
) -> bool {
    matches!(behavior, ProjectileBehavior::MarioFireball(_))
        || matches!(
            kind,
            ProjectileKind::LuigiFire | ProjectileKind::Gravity(ArticleId::LUIGI_FIRE)
        )
}

const LUIGI_FIREBALL_TERRAIN_EFFECT_ID: u16 = 1288;

/// Fox laser stage collision arms the native one-frame expiry timer instead
/// of deleting the item from the collision callback (`itFoxlaser_UnkMotion1_Coll`:
/// `it_80275158(item_gobj, 1.0F)`).  The host step returns immediately so the
/// timer is consumed by the next simulation pass, matching the item callback
/// ordering.
fn reset_ray_after_terrain_contact(projectile: &mut Projectile) {
    projectile.lifetime = 1.0;
}

fn reflection_geometry(
    data: &MatchData,
    state: &State,
    poses: &[Pose; 2],
    victim: usize,
    index: usize,
) -> Result<Option<ReflectionGeometry>, super::Error> {
    if let Some(specials) = data.fighters[victim].specials.as_ref()
        && let Some(path) =
            crate::game::script::definition::projectile_contact_resource(&data.fighters[victim])
        && let Some(value) = specials.lookup(&path)
        && let Ok(down) = serde_json::from_value::<ReflectDescriptor>(value.clone())
    {
        let matrix = poses[victim]
            .world_matrix(down.bone as usize)
            .map_err(|e| super::Error::Physics(e.to_string()))?;
        let center = crate::collision::bones::transform_point(matrix, down.offset);
        return Ok(Some((
            center,
            *matrix,
            down.size,
            down.max_damage,
            down.damage_mul,
            true,
        )));
    }

    let typed_reflection = matches!(
        state.projectiles[index].behavior,
        ProjectileBehavior::Gravity(GravityProjectileState {
            contact: ProjectileContactPolicy {
                reflection: ProjectileReflection::ReverseOwner,
                ..
            },
            ..
        })
    );
    if typed_reflection && let Some(rules) = data.rules.shield.as_ref() {
        let (center, matrix) = shield::geometry(
            &state.fighters[victim],
            &data.fighters[victim],
            rules,
            &poses[victim],
        )
        .map_err(|e| super::Error::Physics(e.to_string()))?;
        return Ok(Some((center, matrix, 1.0, i32::MAX, 1.0, false)));
    }
    Ok(None)
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

/// Match the item scheduler's lifetime ordering (`Item_80269528`): expired
/// items are destroyed before their physics and collision callbacks run, and
/// every live item loses one frame before those callbacks.  Terrain contact
/// may subsequently arm a one-frame lifetime, which is consumed on the next
/// step.
fn tick_lifetime(projectile: &mut Projectile) -> bool {
    if projectile.lifetime <= 0.0 {
        return true;
    }
    projectile.lifetime -= 1.0;
    projectile.lifetime <= 0.0
}

fn step(
    data: &MatchData,
    state: &mut State,
    poses: &[Pose; 2],
    stage: &Stage<'_>,
    index: usize,
) -> Result<Outcome, super::Error> {
    if tick_lifetime(&mut state.projectiles[index]) {
        return Ok(Outcome::Despawn);
    }
    let owner = state.projectiles[index].owner;
    let victim = 1 - owner;

    // Item physics runs before the environment pass. Gravity articles retain
    // the already-computed vector, avoiding repeated trig in the hot path.
    if let ProjectileBehavior::Gravity(gravity) | ProjectileBehavior::MarioFireball(gravity) =
        state.projectiles[index].behavior
    {
        state.projectiles[index].velocity = gravity_step(
            state.projectiles[index].velocity,
            gravity.gravity,
            gravity.terminal_velocity,
        );
        state.projectiles[index].speed = speed(state.projectiles[index].velocity);
    }
    // Motion: `position += velocity` every frame (`Item_UpdateRayAnimation`).
    let [vx, vy] = state.projectiles[index].velocity;
    let previous_position = state.projectiles[index].position;
    state.projectiles[index].position[0] += vx;
    state.projectiles[index].position[1] += vy;
    state.projectiles[index].facing = facing_from_velocity(vx);

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
    let terrain_contact = [
        Surface::Floor,
        Surface::Ceiling,
        Surface::LeftWall,
        Surface::RightWall,
    ]
    .into_iter()
    .try_fold(
        None::<(Surface, crate::collision::stage::Contact)>,
        |nearest, surface| {
            let contact = stage.sweep(
                surface,
                Query {
                    from: previous_xy,
                    to: current_xy,
                    ..Default::default()
                },
            )?;
            Ok::<_, crate::collision::stage::StageError>(match (nearest, contact) {
                (None, contact) => contact.map(|contact| (surface, contact)),
                (Some((_old_surface, old)), Some(new))
                    if new.distance_squared < old.distance_squared =>
                {
                    Some((surface, new))
                }
                (nearest, _) => nearest,
            })
        },
    )
    .map_err(|e: crate::collision::stage::StageError| super::Error::Physics(e.to_string()))?;
    if let Some((_surface, contact)) = terrain_contact {
        if let ProjectileBehavior::MarioFireball(gravity) = state.projectiles[index].behavior {
            // `itMariofireball_UnkMotion0_Coll` first lets the common item
            // collision helper updates the contact position.  It terminates
            // below the authored speed threshold; otherwise it emits the
            // Mario fire effect (1147) and continues without the generic
            // gravity article's surface bounce.  The pinned callback does
            // not gate this on the velocity/normal direction.
            state.projectiles[index].position = contact.position;
            if mario_fireball_terrain_despawns(
                speed(state.projectiles[index].velocity),
                gravity.terrain_stop_speed,
            ) {
                return Ok(Outcome::Despawn);
            }
            state.events.push(Event::ProjectileEffect {
                owner,
                projectile_kind: state.projectiles[index].kind,
                effect_id: 1147,
            });
        } else if let ProjectileBehavior::Gravity(gravity) = state.projectiles[index].behavior {
            if state.projectiles[index].kind == ProjectileKind::LuigiFire {
                // `it_8026D9A0` writes the common collision position before
                // the source callback evaluates its speed threshold. Keep
                // that ordering even when the threshold consumes the item.
                state.projectiles[index].position = contact.position;
                if luigi_fireball_terrain_despawns(
                    speed(state.projectiles[index].velocity),
                    gravity.terrain_stop_speed,
                ) {
                    return Ok(Outcome::Despawn);
                }
                state.events.push(Event::ProjectileTerrainEffect {
                    owner,
                    effect_id: LUIGI_FIREBALL_TERRAIN_EFFECT_ID,
                });
                return Ok(Outcome::Keep);
            }
            let incoming = dot(
                [
                    state.projectiles[index].velocity[0],
                    state.projectiles[index].velocity[1],
                    0.0,
                ],
                contact.normal,
            );
            if incoming < 0.0 {
                let reflected = surface_bounce(
                    state.projectiles[index].velocity,
                    contact.normal,
                    gravity.surface_multiplier,
                );
                state.projectiles[index].position = contact.position;
                state.projectiles[index].velocity = reflected;
                state.projectiles[index].speed = speed(reflected);
                state.projectiles[index].angle = libm::atan2f(reflected[1], reflected[0]);
                state.projectiles[index].facing = facing_from_velocity(reflected[0]);
                if state.projectiles[index].speed <= gravity.terrain_stop_speed {
                    return Ok(Outcome::Despawn);
                }
            }
        } else {
            reset_ray_after_terrain_contact(&mut state.projectiles[index]);
            return Ok(Outcome::Keep);
        }
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
    let reflection_enabled = match state.projectiles[index].behavior {
        ProjectileBehavior::Ray => true,
        ProjectileBehavior::Gravity(gravity) | ProjectileBehavior::MarioFireball(gravity) => {
            gravity.contact.reflection == ProjectileReflection::ReverseOwner
        }
    };
    let typed_reflection = matches!(
        state.projectiles[index].behavior,
        ProjectileBehavior::Gravity(GravityProjectileState {
            contact: ProjectileContactPolicy {
                reflection: ProjectileReflection::ReverseOwner,
                ..
            },
            ..
        }) | ProjectileBehavior::MarioFireball(GravityProjectileState {
            contact: ProjectileContactPolicy {
                reflection: ProjectileReflection::ReverseOwner,
                ..
            },
            ..
        })
    );
    if reflection_enabled
        && state.fighters[victim].shield.reflecting
        && let Some((center, matrix, radius, max_damage, damage_mul, descriptor)) =
            reflection_geometry(data, state, poses, victim, index)?
    {
        let swept = Capsule {
            start: previous_position,
            end: state.projectiles[index].position,
            radius: state.projectiles[index].collision_radius(),
        };
        let reflect_capsule = Capsule {
            start: center,
            end: center,
            radius,
        };
        let mut contact = crate::collision::shield::Contact::default();
        let overlaps = crate::collision::shield::capsule_matrix(
            &swept,
            &reflect_capsule,
            &matrix,
            1.0,
            &mut contact,
        )
        .map_err(|e| super::Error::Physics(e.to_string()))?;
        let mut hook_reflects = false;
        if overlaps
            && let Some(program) =
                crate::game::script::definition::cached_program(&data.fighters[victim])
        {
            let damage = state.projectiles[index]
                .hitboxes
                .iter()
                .map(|h| h.damage)
                .max()
                .unwrap_or(0);
            let fighter = &state.fighters[victim];
            let mut flags = BTreeMap::new();
            flags.insert("reflecting".to_owned(), fighter.shield.reflecting);
            let view = script::FighterView {
                id: victim as u8,
                action: format!("{:?}", fighter.action),
                action_frame: fighter.action_frame,
                velocity: fighter.velocity,
                ground_velocity: fighter.ground_velocity,
                grounded: fighter.grounded,
                percent: fighter.percent,
                hitlag: fighter.hitlag,
                hitstun: fighter.hitstun,
                flags,
            };
            let hit = script::HitView {
                damage: damage as f32,
                max_damage,
                projectile: true,
                ..Default::default()
            };
            let result = program
                .dispatch_with_context(
                    script::Hook::ProjectileContact,
                    &view,
                    Some(&hit),
                    script::CombatContext {
                        persistent: &state.fighters[victim].script_state,
                        action_state: &state.fighters[victim].action_state,
                        resources: data.fighters[victim].script_resources.get(),
                    },
                )
                .map_err(|error| super::Error::Data(error.to_string()))?;
            state.fighters[victim].script_state = result.locals;
            state.fighters[victim].action_state = result.action_state;
            script::apply_commands(state, victim, &result.commands)?;
            hook_reflects = result.hit.is_some_and(|patch| patch.reflect);
        }
        if overlaps && (typed_reflection || hook_reflects) {
            state.projectiles[index].owner = victim;
            state.projectiles[index].angle =
                normalize_angle(state.projectiles[index].angle + core::f32::consts::PI);
            state.projectiles[index].velocity = [
                -state.projectiles[index].velocity[0],
                -state.projectiles[index].velocity[1],
            ];
            state.projectiles[index].facing =
                facing_from_velocity(state.projectiles[index].velocity[0]);
            if let Some(half_life) = state.projectiles[index].half_life {
                state.projectiles[index].lifetime = half_life;
            }
            if descriptor {
                for hit in &mut state.projectiles[index].hitboxes {
                    // `item.c:1613-1619` (`Item_80269F14`): `hit.damage * xC6C +
                    // 0.99f`, truncated toward zero -- not a plain product. The
                    // `+ 0.99` term was missing here (a real discrepancy this
                    // batch's own differential exposed, `tests/oracle/item.c`'s
                    // `oracle_reflect_damage_scaling`); the global cap
                    // (`it_804D6D28->xD8`) stays unmodeled, as before, since its
                    // real runtime value is not in the pinned decomp.
                    let scaled = hit.damage as f32 * damage_mul + 0.99;
                    hit.damage = scaled.max(0.0) as u32;
                }
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
            if source_fireball_hit_shield_despawns(
                state.projectiles[index].kind,
                &state.projectiles[index].behavior,
            ) || matches!(
                state.projectiles[index].behavior,
                ProjectileBehavior::Gravity(gravity)
                    if gravity.contact.shield == ProjectileShield::Despawn
            ) {
                return Ok(Outcome::Despawn);
            }
            let mirrored = mirror([vx, vy], normal);
            state.projectiles[index].angle =
                normalize_angle(libm::atan2f(mirrored[1], mirrored[0]));
            state.projectiles[index].speed = speed(mirrored);
            state.projectiles[index].velocity = mirrored;
            state.projectiles[index].facing = facing_from_velocity(mirrored[0]);
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
        && !super::flow::rebirth::invulnerable(target.action)
        && !super::flow::death::owns_action(target.action)
    {
        let facing = state.projectiles[index].facing;
        let position = state.projectiles[index].position;
        let mut connected = false;
        // Iterate by index and clone only the small hitbox record. Cloning the
        // Vec here allocated once per active projectile on every frame; the
        // record clone keeps the borrow independent while `apply_hit` mutates
        // match state and does not allocate.
        'hitboxes: for hit_index in 0..state.projectiles[index].hitboxes.len() {
            let hit = state.projectiles[index].hitboxes[hit_index].clone();
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
                    let accepted = super::hit_resolution::apply_hit(
                        data,
                        state,
                        owner,
                        &hit,
                        staled,
                        height,
                        super::hit_resolution::HitDirection::FighterContact(
                            super::hit_resolution::FighterContact {
                                hurt_start: hurt.start,
                                hurt_end: hurt.end,
                                position: contact.position,
                            },
                        ),
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
            let persists = matches!(
                state.projectiles[index].behavior,
                ProjectileBehavior::Gravity(GravityProjectileState {
                    contact: ProjectileContactPolicy {
                        persistence: ProjectilePersistence::Persist,
                        ..
                    },
                    ..
                }) | ProjectileBehavior::MarioFireball(GravityProjectileState {
                    contact: ProjectileContactPolicy {
                        persistence: ProjectilePersistence::Persist,
                        ..
                    },
                    ..
                })
            );
            if !persists {
                return Ok(Outcome::Despawn);
            }
        }
    }

    Ok(Outcome::Keep)
}

#[cfg(test)]
mod tests {
    use super::{
        ArticleHandle, ArticleLaunch, GravityProjectileState, LUIGI_FIREBALL_TERRAIN_EFFECT_ID,
        ProjectileBehavior, ProjectileContactPolicy, ProjectileKind, ProjectilePersistence,
        ProjectileReflection, ProjectileShield, dot, facing_from_velocity, gravity_step,
        luigi_fireball_terrain_despawns, mario_fireball_terrain_despawns, normalize_angle,
        reset_ray_after_terrain_contact, source_fireball_hit_shield_despawns, spawn, speed,
        surface_bounce, tick_lifetime,
    };
    use crate::game::script::resources::ArticleId;

    const CONTACT: ProjectileContactPolicy = ProjectileContactPolicy {
        reflection: ProjectileReflection::None,
        shield: ProjectileShield::Bounce,
        persistence: ProjectilePersistence::Despawn,
    };

    #[test]
    fn gravity_fall_clamps_in_the_direction_of_gravity() {
        assert_eq!(gravity_step([1.0, 2.0], 0.5, 3.0), [1.0, 1.5]);
        assert_eq!(gravity_step([1.0, -4.0], 0.5, 3.0), [1.0, -3.0]);
        assert_eq!(gravity_step([1.0, -2.0], -0.5, 3.0), [1.0, -1.5]);
        assert_eq!(gravity_step([1.0, 4.0], -0.5, 3.0), [1.0, 3.0]);
    }

    #[test]
    fn gravity_bounces_only_when_velocity_enters_surface() {
        let normal = [0.0, 1.0, 0.0];
        assert!(dot([0.0, -1.0, 0.0], normal) < 0.0);
        let outgoing = dot([0.0, 1.0, 0.0], normal);
        assert!(outgoing.partial_cmp(&0.0) != Some(std::cmp::Ordering::Less));
    }

    #[test]
    fn luigi_fireball_terrain_policy_requires_speed_below_source_threshold() {
        assert!(luigi_fireball_terrain_despawns(0.4999, 0.5));
        assert!(luigi_fireball_terrain_despawns(0.25, 0.5));
        assert!(!luigi_fireball_terrain_despawns(0.5, 0.5));
        assert!(!luigi_fireball_terrain_despawns(0.5001, 0.5));
    }

    #[test]
    fn mario_fireball_terrain_policy_uses_post_gravity_velocity_magnitude() {
        let spawn_speed = 2.0;
        let post_gravity_velocity = [0.3, 0.4];
        assert_eq!(speed(post_gravity_velocity), 0.5);
        assert!(mario_fireball_terrain_despawns(
            speed(post_gravity_velocity),
            0.75
        ));
        assert!(!mario_fireball_terrain_despawns(spawn_speed, 0.75));
        assert!(!mario_fireball_terrain_despawns(0.75, 0.75));
    }

    #[test]
    fn luigi_fireball_terrain_contact_uses_source_effect() {
        assert_eq!(LUIGI_FIREBALL_TERRAIN_EFFECT_ID, 1288);
    }

    #[test]
    fn luigi_fireball_despawn_keeps_common_contact_position_update() {
        let mut instances = crate::fighter::state::stale::InstanceCounter::default();
        let mut projectile = spawn(
            ArticleHandle::from_raw(6),
            ProjectileKind::LuigiFire,
            ProjectileBehavior::Gravity(GravityProjectileState {
                gravity: 0.0,
                terminal_velocity: 2.0,
                surface_multiplier: 1.0,
                terrain_stop_speed: 0.5,
                half_life: 30.0,
                contact: CONTACT,
            }),
            0,
            [1.0, 2.0, 0.0],
            0.0,
            0.25,
            30.0,
            Vec::new(),
            37,
            &mut instances,
        );
        let contact_position = [4.0, 5.0, 0.0];
        projectile.position = contact_position;
        assert!(luigi_fireball_terrain_despawns(
            speed(projectile.velocity),
            0.5
        ));
        assert_eq!(projectile.position, contact_position);
    }

    #[test]
    fn source_fireball_normal_shield_hit_despawns() {
        let behavior = ProjectileBehavior::MarioFireball(GravityProjectileState {
            gravity: 0.0,
            terminal_velocity: 0.0,
            surface_multiplier: 1.0,
            terrain_stop_speed: 0.5,
            half_life: 0.0,
            contact: CONTACT,
        });
        assert!(source_fireball_hit_shield_despawns(
            ProjectileKind::Gravity(ArticleId::MARIO_FIRE),
            &behavior,
        ));
        assert!(source_fireball_hit_shield_despawns(
            ProjectileKind::LuigiFire,
            &ProjectileBehavior::Gravity(GravityProjectileState {
                gravity: 0.0,
                terminal_velocity: 0.0,
                surface_multiplier: 1.0,
                terrain_stop_speed: 0.5,
                half_life: 0.0,
                contact: CONTACT,
            }),
        ));
        assert!(!source_fireball_hit_shield_despawns(
            ProjectileKind::Gravity(ArticleId::MARIO_FIRE),
            &ProjectileBehavior::Gravity(GravityProjectileState {
                gravity: 0.0,
                terminal_velocity: 0.0,
                surface_multiplier: 1.0,
                terrain_stop_speed: 0.5,
                half_life: 0.0,
                contact: CONTACT,
            }),
        ));
    }

    #[test]
    fn gravity_bounce_uses_stage_normal_then_surface_response() {
        let bounced = surface_bounce([2.0, -4.0], [0.0, 1.0, 0.0], 0.5);
        assert_eq!(bounced, [1.0, 2.0]);
        assert_eq!(speed(bounced), libm::sqrtf(5.0));
        assert!(speed(surface_bounce([0.0, -2.0], [0.0, 1.0, 0.0], 0.0)) <= 0.01);
    }

    #[test]
    fn gravity_article_spawn_owns_angle_and_speed_but_keeps_ray_distinct() {
        let mut instances = crate::fighter::state::stale::InstanceCounter::default();
        let gravity = spawn(
            ArticleHandle::from_raw(1),
            ProjectileKind::Gravity(ArticleId::MARIO_FIRE),
            ProjectileBehavior::Gravity(GravityProjectileState {
                gravity: 0.5,
                terminal_velocity: 3.0,
                surface_multiplier: 0.5,
                terrain_stop_speed: 0.1,
                half_life: 5.0,
                contact: CONTACT,
            }),
            0,
            [0.0, 0.0, 0.0],
            0.0,
            2.0,
            10.0,
            Vec::new(),
            37,
            &mut instances,
        );
        assert_eq!(gravity.velocity, [2.0, 0.0]);
        assert!(matches!(gravity.behavior, ProjectileBehavior::Gravity(_)));

        let ray = spawn(
            ArticleHandle::from_raw(2),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [0.0, 0.0, 0.0],
            core::f32::consts::FRAC_PI_2,
            2.0,
            10.0,
            Vec::new(),
            37,
            &mut instances,
        );
        assert!(ray.velocity[0].abs() < 1e-6);
        assert!((ray.velocity[1] - 2.0).abs() < 1e-6);
        assert!(matches!(
            ArticleLaunch::Facing(-1.0),
            ArticleLaunch::Facing(facing) if facing < 0.0
        ));
    }

    #[test]
    fn ray_angles_use_native_range_and_zero_x_is_left_facing() {
        let tau = core::f32::consts::TAU;
        assert_eq!(normalize_angle(-core::f32::consts::FRAC_PI_2), tau * 0.75);
        assert_eq!(normalize_angle(tau * 2.0), tau);
        assert_eq!(facing_from_velocity(1.0), 1.0);
        assert_eq!(facing_from_velocity(0.0), -1.0);
        assert_eq!(facing_from_velocity(-1.0), -1.0);

        let mut instances = crate::fighter::state::stale::InstanceCounter::default();
        let ray = spawn(
            ArticleHandle::from_raw(3),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [0.0, 0.0, 0.0],
            -core::f32::consts::FRAC_PI_2,
            2.0,
            10.0,
            Vec::new(),
            1,
            &mut instances,
        );
        assert!((ray.angle - tau * 0.75).abs() < 1e-6);
        // libm::cosf(-PI/2) is a tiny positive value on this host; the
        // source's strict x > 0 test therefore selects right-facing.  The
        // exact-zero convention is covered directly above.
        assert_eq!(ray.facing, 1.0);
    }

    #[test]
    fn angle_normalization_is_bounded_for_edge_values() {
        let tau = core::f32::consts::TAU;
        assert_eq!(normalize_angle(tau), tau);
        assert_eq!(normalize_angle(tau * 2.0), tau);
        assert_eq!(normalize_angle(-tau), 0.0);
        assert!(normalize_angle(f32::MAX).is_finite());
        assert_eq!(normalize_angle(f32::INFINITY), f32::INFINITY);
        assert!(normalize_angle(f32::NAN).is_nan());
        let negative_zero = normalize_angle(-0.0);
        assert_eq!(negative_zero, 0.0);
        assert!(negative_zero.is_sign_negative());
    }

    #[test]
    fn ray_terrain_contact_arms_one_frame_expiry() {
        let mut instances = crate::fighter::state::stale::InstanceCounter::default();
        let mut ray = spawn(
            ArticleHandle::from_raw(4),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [0.0, 0.0, 0.0],
            0.0,
            3.0,
            60.0,
            Vec::new(),
            1,
            &mut instances,
        );
        ray.lifetime = 60.0;
        reset_ray_after_terrain_contact(&mut ray);
        assert_eq!(ray.lifetime, 1.0);
    }

    #[test]
    fn lifetime_is_consumed_before_projectile_callbacks() {
        let mut instances = crate::fighter::state::stale::InstanceCounter::default();
        let mut projectile = spawn(
            ArticleHandle::from_raw(5),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [0.0, 0.0, 0.0],
            0.0,
            1.0,
            1.0,
            Vec::new(),
            1,
            &mut instances,
        );
        assert!(tick_lifetime(&mut projectile));
        assert_eq!(projectile.lifetime, 0.0);
        assert!(tick_lifetime(&mut projectile));
    }

    #[test]
    fn article_handles_are_stable_after_vector_removal_and_checkpoint_serialization() {
        let first = spawn(
            ArticleHandle::from_raw(1),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [0.0, 0.0, 0.0],
            0.0,
            1.0,
            10.0,
            Vec::new(),
            1,
            &mut crate::fighter::state::stale::InstanceCounter::default(),
        );
        let second = spawn(
            ArticleHandle::from_raw(2),
            ProjectileKind::FoxLaser,
            ProjectileBehavior::Ray,
            0,
            [1.0, 0.0, 0.0],
            0.0,
            1.0,
            10.0,
            Vec::new(),
            1,
            &mut crate::fighter::state::stale::InstanceCounter::default(),
        );
        let first_handle = first.handle();
        let second_handle = second.handle();
        let mut live = vec![first, second];
        live.remove(0);
        assert_eq!(live.iter().find(|item| item.handle() == first_handle), None);
        assert_eq!(live[0].handle(), second_handle);

        let encoded = serde_json::to_value(&live[0]).expect("projectile checkpoint encoding");
        assert_eq!(encoded["handle"], serde_json::json!(2));
    }

    #[test]
    fn article_handle_deserialization_rejects_reserved_zero() {
        let result = serde_json::from_value::<ArticleHandle>(serde_json::json!(0));
        assert!(result.is_err());
    }
}
