//! Fox/Falco neutral special (Blaster): resource shape and the Start/Loop/
//! End state machine that fires the generic `game::projectile` laser. See
//! `docs/fox-neutral-special.md` for the full citation list, the fire-
//! timing and repeat-window approximations, and why this move needs
//! neither a `transfer_ground_air` nor a `land` override (its grounded
//! phases have no ground<->air conversion of their own at all -- leaving
//! the ground always falls through to the generic `Action::Fall` fallback,
//! and a mid-flight ground contact always falls through to the generic
//! `Action::Landing` fallback, per this batch's own conservative
//! simplification of `ftCo_AirCatchHit_Coll`'s unconfirmed velocity
//! threshold). Gated purely on resource presence, like the shared neutral
//! shell it replaces for Fox.
//!
//! Pinned decomp `src/melee/ft/kinds/ftFox/ftfoxspecialn.c` (rev
//! `0bac93a5`); the cosmetic hand-held gun model
//! (`fp->u.fx.x222C_blasterGObj`, `itfoxblaster.c`) is confirmed
//! hitbox-free and entirely unmodeled.

use crate::{
    fighter::special::neutral_input,
    game::{
        Action, BUTTON_B, Controller, Fighter,
        data::{Attack, FighterData, Hitbox, Rules as MatchRules},
        projectile::{self, ProjectileKind},
        simulation,
        specials::{SpecialMove, helpers},
        validation::validate_animation_pose,
    },
};
use serde::{Deserialize, Serialize};

pub(crate) use super::side::Phase;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeutralSpecial {
    /// `ftCo_800D67C4`'s own strict ground/air stick-neutral bounds, shared
    /// with every character's plain neutral-B branch.
    pub neutral_thresholds: [f32; 2],
    pub start: Phase,
    pub loop_phase: Phase,
    pub end: Phase,
    pub attributes: Attributes,
    pub laser: Laser,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// `x10_FOX_BLASTER_ANGLE`, radians, mirrored across facing
    /// (`ftFox_SpecialN_PrepareBlasterShot`). Exporter-confirmed `0.0`.
    pub angle: f32,
    /// `x14_FOX_BLASTER_VEL`. Exporter- and real-recording-confirmed `7.0`.
    pub speed: f32,
    /// `x18_FOX_BLASTER_LANDING_LAG`: End-air's own natural-clip-end
    /// exit only (exporter-confirmed `0.0` for Fox, so this always takes
    /// the `ftCo_Fall_Enter` branch, never `FallSpecial`); unrelated to a
    /// mid-flight ground contact, which always uses the generic
    /// `Action::Landing` fallback (see module doc). Falco's own value is
    /// not yet exported and may differ.
    pub landing_lag: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Laser {
    /// `FoxLaserAttr.lifetime` (`+0`). Exporter- and real-recording-
    /// confirmed `35.0` frames. `+4` (`max_scale`, exporter-confirmed
    /// `3.0`) is a visual beam-length clamp consumed only by
    /// `Item_UpdateRayAnimation`, not gameplay, and is deliberately not
    /// modeled here (GFX-only, matching this project's existing
    /// precedent for such fields).
    pub lifetime: f32,
    /// The laser's own fixed hitboxes: exporter-confirmed **four**
    /// capsules (`it_803F67D0` state 0's own command stream, staggered
    /// along the item's local `-X` axis to cover the growing beam), all
    /// `bone: 0` (meaningless for a projectile; `center` is instead this
    /// port's own item-local offset from the projectile's current
    /// position, mirrored by facing like everything else ray-shaped --
    /// see `game::projectile`). `group`/`clank`/`rebound`/`element` are
    /// also unused (defaults). Exporter-confirmed, every hitbox: damage
    /// `3`, angle `361` (Sakurai angle, already handled generically by
    /// `fighter::damage`), growth/fixed/base `0` (zero knockback is real
    /// -- a laser flinches without pushing), shield damage `0`; offsets/
    /// sizes (`0.003906`-scaled raw integers, not `1/256`): id 0
    /// `x = -0.7812` size `1.1718`, id 1 `x = -3.6442978` size `1.1718`,
    /// id 2 `x = -6.5073957` size `1.1718`, id 3 `x = -14.0616` size
    /// `1.5624` (all `y = z = 0`). The item's own per-victim re-hit
    /// cooldown (previously mis-hypothesized here as a "hitlag
    /// multiplier", exporter-confirmed value `16`) is irrelevant: this
    /// port's laser always despawns after its first hit (no piercing), so
    /// no re-hit can ever occur, and is not modeled.
    pub hitboxes: Vec<Hitbox>,
    /// `FtMoveId_SpecialN` (`ft/forward.h`, confirmed `18` by counting
    /// declaration order from `FtMoveId_None == 0`).
    pub move_id: u16,
}

pub(crate) fn validate(
    parameters: &NeutralSpecial,
    fighter: &FighterData,
) -> Result<(), crate::game::Error> {
    use crate::game::Error;
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    if !parameters
        .neutral_thresholds
        .into_iter()
        .all(|t| finite(t) && t > 0.0 && t <= 1.0)
    {
        return Err(Error::Data(
            "invalid neutral-special stick thresholds".into(),
        ));
    }
    let a = &parameters.attributes;
    if !finite(a.angle)
        || !finite(a.speed)
        || a.speed < 0.0
        || !finite(a.landing_lag)
        || a.landing_lag < 0.0
    {
        return Err(Error::Data("invalid neutral-special attributes".into()));
    }
    let l = &parameters.laser;
    if !finite(l.lifetime) || l.lifetime <= 0.0 {
        return Err(Error::Data("invalid laser lifetime".into()));
    }
    if l.hitboxes.is_empty() || l.hitboxes.len() > 4 {
        return Err(Error::Data("laser requires 1..4 hitboxes".into()));
    }
    for hit in &l.hitboxes {
        if !finite(hit.radius)
            || hit.radius < 0.0
            || !finite(hit.angle_degrees)
            || !hit.center.iter().copied().all(finite)
        {
            return Err(Error::Data("invalid laser hitbox".into()));
        }
    }
    for attack in [
        &parameters.start.ground,
        &parameters.start.air,
        &parameters.loop_phase.ground,
        &parameters.loop_phase.air,
        &parameters.end.ground,
        &parameters.end.air,
    ] {
        if attack.frames.is_empty() || attack.frames.len() > 4096 {
            return Err(Error::Data(
                "neutral special requires 1..4096 physics samples per phase".into(),
            ));
        }
        for frame in &attack.frames {
            validate_animation_pose(&frame.bones, fighter)?;
        }
    }
    Ok(())
}

/// Persistent per-fighter state. `repeat_armed` mirrors `isBlasterLoop`,
/// approximated as settable only during Loop (see module doc); `fire` is
/// this port's own signal from dispatch (which cannot itself reach
/// `State::projectiles`) up to `simulation::advance`'s per-frame loop,
/// which spawns the laser and clears the flag (see `game::simulation`).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub repeat_armed: bool,
    pub fire: bool,
}

pub(crate) fn attack(action: Action, parameters: &NeutralSpecial) -> Option<&Attack> {
    match action {
        Action::SpecialNStart => Some(&parameters.start.ground),
        Action::SpecialAirNStart => Some(&parameters.start.air),
        Action::SpecialNLoop => Some(&parameters.loop_phase.ground),
        Action::SpecialAirNLoop => Some(&parameters.loop_phase.air),
        Action::SpecialNEnd => Some(&parameters.end.ground),
        Action::SpecialAirNEnd => Some(&parameters.end.air),
        _ => None,
    }
}

fn owned(action: Action) -> bool {
    matches!(
        action,
        Action::SpecialNStart
            | Action::SpecialNLoop
            | Action::SpecialNEnd
            | Action::SpecialAirNStart
            | Action::SpecialAirNLoop
            | Action::SpecialAirNEnd
    )
}

fn enter_loop(fighter: &mut Fighter, ground: bool) {
    simulation::enter(
        fighter,
        if ground {
            Action::SpecialNLoop
        } else {
            Action::SpecialAirNLoop
        },
    );
    // Firing on Loop's own entry frame approximates the source's real
    // mid-clip accessory-callback timing; see module doc.
    fighter.fox_neutral_special.fire = true;
}

fn enter_end(fighter: &mut Fighter, ground: bool) {
    simulation::enter(
        fighter,
        if ground {
            Action::SpecialNEnd
        } else {
            Action::SpecialAirNEnd
        },
    );
}

/// This move's handle in Fox's registry (`MOVES` in `characters::fox`).
pub(crate) struct Move;

pub(crate) const MOVE: Move = Move;

impl SpecialMove for Move {
    fn owns(&self, action: Action) -> bool {
        owned(action)
    }

    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        attack(action, data.specials.as_ref()?.fox_neutral()?)
    }

    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        _rules: &MatchRules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_neutral()) else {
            return false;
        };
        let fresh_b = input.buttons & !fighter.previous_input.buttons & BUTTON_B != 0;
        match fighter.action {
            Action::SpecialNStart | Action::SpecialAirNStart => {
                // `ftFox_SpecialN_CheckLoopInput`: this port approximates
                // `cmd_vars[0]` as never armed during Start (see module
                // doc), so a fresh press here has no effect beyond
                // dispatch already being owned.
                true
            }
            Action::SpecialNLoop | Action::SpecialAirNLoop => {
                if fresh_b {
                    fighter.fox_neutral_special.repeat_armed = true;
                }
                true
            }
            Action::SpecialNEnd | Action::SpecialAirNEnd => true,
            _ => {
                if self.owns(fighter.action) {
                    return true;
                }
                if !(ground || air) {
                    return false;
                }
                let pressed = input.buttons & !fighter.previous_input.buttons;
                if !neutral_input(pressed, input.stick, parameters.neutral_thresholds) {
                    return false;
                }
                simulation::enter(
                    fighter,
                    if ground {
                        Action::SpecialNStart
                    } else {
                        Action::SpecialAirNStart
                    },
                );
                if ground {
                    fighter.ground_velocity = 0.0;
                }
                fighter.velocity = [0.0, 0.0];
                true
            }
        }
    }

    fn update_animation(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        _input: Controller,
        _on_platform: bool,
    ) {
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_neutral()) else {
            return;
        };
        match fighter.action {
            Action::SpecialNStart
                if fighter.action_frame as usize >= parameters.start.ground.frames.len() =>
            {
                enter_loop(fighter, true);
            }
            Action::SpecialAirNStart
                if fighter.action_frame as usize >= parameters.start.air.frames.len() =>
            {
                enter_loop(fighter, false);
            }
            Action::SpecialNLoop
                if fighter.action_frame as usize >= parameters.loop_phase.ground.frames.len() =>
            {
                if fighter.fox_neutral_special.repeat_armed {
                    enter_loop(fighter, true);
                } else {
                    enter_end(fighter, true);
                }
            }
            Action::SpecialAirNLoop
                if fighter.action_frame as usize >= parameters.loop_phase.air.frames.len() =>
            {
                if fighter.fox_neutral_special.repeat_armed {
                    enter_loop(fighter, false);
                } else {
                    enter_end(fighter, false);
                }
            }
            Action::SpecialNEnd
                if fighter.action_frame as usize >= parameters.end.ground.frames.len() =>
            {
                helpers::exit_to_wait_or_fall(fighter);
            }
            Action::SpecialAirNEnd
                if fighter.action_frame as usize >= parameters.end.air.frames.len() =>
            {
                // `ftFx_SpecialAirNEnd_Anim`'s own natural-clip-end exit
                // (distinct from a mid-flight ground contact, which always
                // uses the generic `Action::Landing` fallback instead --
                // see module doc): `x18 == 0` skips FallSpecial entirely.
                if parameters.attributes.landing_lag <= 0.0 {
                    simulation::enter(fighter, Action::Fall);
                } else {
                    helpers::enter_fall_special(
                        fighter,
                        data,
                        1.0,
                        Some(parameters.attributes.landing_lag),
                    );
                }
            }
            _ => {}
        }
    }
}

/// Drains `fighter.fox_neutral_special.fire`, spawning the laser into
/// `state.projectiles`. Called from `simulation::advance`'s per-player
/// loop after ordinary update-actions/animation, matching "item logic
/// runs after fighters" (`docs/fox-neutral-special.md`).
///
/// Spawn position is `owner.cur_pos + (0, 0.5 * (ecb.top.y +
/// ecb.bottom.y), 0)` (`Item_InitRaySpawnPosition` -> `it_8026BB68` ->
/// `ftLib_80086990`, exporter-confirmed): the fighter's own live ECB
/// vertical midpoint, no bone lookup at all (the `RThumbNb` hold joint
/// this batch had originally assumed only seeds the ray-cast anchor, never
/// the drawn/hit position).
pub(crate) fn drain_pending_shot(
    fighter: &mut Fighter,
    data: &FighterData,
    player: usize,
    attack_instances: &mut crate::fighter::stale::InstanceCounter,
) -> Option<crate::game::projectile::Projectile> {
    if !fighter.fox_neutral_special.fire {
        return None;
    }
    fighter.fox_neutral_special.fire = false;
    let parameters = data.specials.as_ref()?.fox_neutral()?;
    let ecb = fighter.ecb.current;
    let position = [
        fighter.position[0],
        fighter.position[1] + 0.5 * (ecb.top[1] + ecb.bottom[1]),
        fighter.depth,
    ];
    let angle = if fighter.facing == 1.0 {
        parameters.attributes.angle
    } else {
        core::f32::consts::PI - parameters.attributes.angle
    };
    Some(projectile::spawn(
        ProjectileKind::FoxLaser,
        player,
        position,
        angle,
        parameters.attributes.speed,
        parameters.laser.lifetime,
        parameters.laser.hitboxes.clone(),
        parameters.laser.move_id,
        attack_instances,
    ))
}
