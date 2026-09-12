//! Fox/Falco neutral special (Blaster): resource shape and the Start/Loop/
//! End state machine that fires the generic `game::projectile` laser. When
//! `NeutralSpecial::script` is supplied, the loop-arming window
//! (`ftFox_SpecialN_CheckLoopInput`) and the fire timing
//! (`CreateBlasterShot`/Loop's own inline check) are driven directly from
//! the subaction scripts' own per-frame `cmd_vars`, exactly as the source
//! evaluates them; without it, both fall back to this port's older
//! approximation (fire on Loop's own entry frame; Start never arms, Loop
//! always can) -- see `docs/fox-neutral-special.md` for the full citation
//! list and why this move needs neither a `transfer_ground_air` nor a
//! `land` override (its grounded phases have no ground<->air conversion of
//! their own at all -- leaving the ground always falls through to the
//! generic `Action::Fall` fallback, and a mid-flight ground contact always
//! falls through to the generic `Action::Landing` fallback, per this
//! batch's own conservative simplification of `ftCo_AirCatchHit_Coll`'s
//! unconfirmed velocity threshold). Gated purely on resource presence, like
//! the shared neutral shell it replaces for Fox.
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

use super::side;
pub(crate) use super::side::{Phase, ScriptPhase};

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
    /// `specials.neutral.script` (exporter): the Start/Loop/End subaction
    /// scripts' own per-frame `SetCmdVar` trace, decoded straight from
    /// `ftaction.c`'s opcode 19 stream. `None` for Falco/pre-batch fixtures
    /// (the exporter has not supplied it yet); every reader below falls
    /// back to this move's own pre-existing approximation in that case --
    /// see the call sites and module doc. Boxed: `Specials` is an enum over
    /// every character's own full moveset, so its size is its largest
    /// variant's; this field's own per-frame vectors are heap data already,
    /// but the surrounding `NeutralScript`/`ScriptPhase` structs are plain
    /// (unboxed) aggregates, and inlining them here was enough to overflow
    /// an unrelated deeply-recursive test's default stack (`cargo test`'s
    /// `game_damage_floor::malformed_floor_profiles_are_rejected_
    /// transactionally`, which never even touches Fox's specials) purely by
    /// growing every `MatchData` on the stack; boxing keeps this optional,
    /// sparsely-populated field off that hot path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Box<NeutralScript>>,
}

/// `specials.neutral.script`'s own three phases, each a ground/air pair of
/// per-frame `cmd_vars`/`allow_interrupt` traces (`side::ScriptPhase`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeutralScript {
    pub start: ScriptPhase,
    pub loop_phase: ScriptPhase,
    pub end: ScriptPhase,
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
    if let Some(script) = &parameters.script {
        for (label, script_phase, attack_phase) in [
            ("start", &script.start, &parameters.start),
            ("loop", &script.loop_phase, &parameters.loop_phase),
            ("end", &script.end, &parameters.end),
        ] {
            side::validate_script_frames(
                &script_phase.ground,
                attack_phase.ground.frames.len(),
                &format!("neutral {label} ground"),
            )?;
            side::validate_script_frames(
                &script_phase.air,
                attack_phase.air.frames.len(),
                &format!("neutral {label} air"),
            )?;
        }
    }
    Ok(())
}

/// Persistent per-fighter state. `repeat_armed` mirrors `isBlasterLoop`,
/// approximated as settable only during Loop when `script` is absent (see
/// module doc); `fire` is this port's own signal from dispatch (which
/// cannot itself reach `State::projectiles`) up to `simulation::advance`'s
/// per-frame loop, which spawns the laser and clears the flag (see
/// `game::simulation`). `cmd_vars` mirrors `Fighter::cmd_vars[0..4]`
/// (`ftFox_SpecialN_InitializeState` zeroes all four only on a fresh
/// Start/AirStart entry -- every internal Start->Loop/Loop->Loop/Loop->End
/// transition below explicitly preserves it around `simulation::enter`'s own
/// blanket per-move-state reset, the same pattern `characters::fox::side`'s
/// `gravity_delay` already uses); read only when `script` is present.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub repeat_armed: bool,
    pub fire: bool,
    pub cmd_vars: [u32; 4],
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

/// `preserve_armed`: `ftFox_SpecialN_StartAnimation` (Start->Loop) never
/// touches `isBlasterLoop`, so a B press armed during Start's own tail
/// (`cmd_vars[0] != 0`, `update_actions` above) survives into the Loop
/// instance it starts -- `true` here. A repeat Loop pass instead goes
/// through `ftFox_SpecialN_BeginLoopTransition`/`FinishLoopTransition`,
/// which explicitly resets `isBlasterLoop = false` for the new pass -- so
/// callers pass `false` there and let `simulation::enter`'s own blanket
/// per-move-state reset (below) supply that zero.
fn enter_loop(fighter: &mut Fighter, ground: bool, has_script: bool, preserve_armed: bool) {
    // `cmd_vars` is preserved across every internal phase transition (only a
    // fresh outer Start/AirStart entry, via `ftFox_SpecialN_InitializeState`,
    // zeroes it) -- `simulation::enter`'s own blanket per-move-state reset
    // would otherwise drop it, the same pattern `characters::fox::side`'s
    // `gravity_delay` already uses around its own phase transitions.
    let cmd_vars = fighter.fox_neutral_special.cmd_vars;
    let repeat_armed = fighter.fox_neutral_special.repeat_armed;
    simulation::enter(
        fighter,
        if ground {
            Action::SpecialNLoop
        } else {
            Action::SpecialAirNLoop
        },
    );
    fighter.fox_neutral_special.cmd_vars = cmd_vars;
    if preserve_armed {
        fighter.fox_neutral_special.repeat_armed = repeat_armed;
    }
    if !has_script {
        // Firing on Loop's own entry frame approximates the source's real
        // mid-clip accessory-callback timing when the script trace isn't
        // supplied for this fighter yet (see module doc). With `script`
        // present, `update_animation`'s own per-frame fire check drives this
        // instead, at the exact frame the script sets `cmd_vars[2]`.
        fighter.fox_neutral_special.fire = true;
    }
}

fn enter_end(fighter: &mut Fighter, ground: bool) {
    let cmd_vars = fighter.fox_neutral_special.cmd_vars;
    simulation::enter(
        fighter,
        if ground {
            Action::SpecialNEnd
        } else {
            Action::SpecialAirNEnd
        },
    );
    fighter.fox_neutral_special.cmd_vars = cmd_vars;
}

/// The script table for one of the six actions this move owns, or `None`
/// when `script` is absent or `action` belongs to another move.
fn script_table(script: &NeutralScript, action: Action) -> Option<&Vec<[Option<u32>; 4]>> {
    Some(match action {
        Action::SpecialNStart => &script.start.ground.cmd_vars,
        Action::SpecialAirNStart => &script.start.air.cmd_vars,
        Action::SpecialNLoop => &script.loop_phase.ground.cmd_vars,
        Action::SpecialAirNLoop => &script.loop_phase.air.cmd_vars,
        Action::SpecialNEnd => &script.end.ground.cmd_vars,
        Action::SpecialAirNEnd => &script.end.air.cmd_vars,
        _ => return None,
    })
}

/// Applies this frame's `SetCmdVar` events (`ftaction.c:456-475`, opcode 19)
/// from the script trace into the persistent `cmd_vars` register.
///
/// The exporter forward-fills each slot's value across every later sampled
/// frame once its `SetCmdVar` executes (the source's own register is
/// persistent, and the exporter does not simulate the native side's own
/// consumption of it -- e.g. `CreateBlasterShot`'s `cmd_vars[2] = 0` right
/// after firing). A literal per-frame overwrite of the whole row would
/// therefore re-trigger an already-consumed one-shot flag on every later
/// frame of the same phase pass. Comparing against the previous frame's own
/// row recovers the single frame the instruction actually executes (a
/// transition into a new value, exactly matching `ftAction_80071820`'s own
/// one-time assignment); only that frame updates the persistent register,
/// so a flag this move's own logic clears mid-frame (the fire check below)
/// stays cleared until the *next* genuine script transition -- which, on a
/// repeat Loop pass, is this same table's own frame re-examined from a
/// freshly reset `action_frame`, so each pass re-fires exactly once.
fn apply_script_frame(cmd_vars: &mut [u32; 4], table: &[[Option<u32>; 4]], frame: usize) {
    let Some(row) = table.get(frame) else {
        return;
    };
    let previous = frame.checked_sub(1).and_then(|f| table.get(f));
    for slot in 0..4 {
        if let Some(value) = row[slot] {
            let is_new_assignment = match previous {
                Some(p) => p[slot] != Some(value),
                None => true,
            };
            if is_new_assignment {
                cmd_vars[slot] = value;
            }
        }
    }
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
            Action::SpecialNStart
            | Action::SpecialAirNStart
            | Action::SpecialNLoop
            | Action::SpecialAirNLoop => {
                // `ftFox_SpecialN_CheckLoopInput` (`ftFox/inlines.h:7-14`):
                // `cmd_vars[0] != 0 && B pressed`, checked identically by
                // Start's and Loop's own IASA. `update_animation` applies
                // this frame's script row (if any) before this runs, so
                // `cmd_vars[0]` already reflects the frame the script sets
                // it (Start frame 4, Loop frame 0). Without `script`, this
                // port keeps its pre-existing approximation instead: Start
                // never arms, Loop always can (see module doc).
                let armed = if parameters.script.is_some() {
                    fighter.fox_neutral_special.cmd_vars[0] != 0
                } else {
                    matches!(
                        fighter.action,
                        Action::SpecialNLoop | Action::SpecialAirNLoop
                    )
                };
                if fresh_b && armed {
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
                // `ftFx_SpecialN_Enter`/`ftFx_SpecialAirN_Enter` each call
                // `ftFox_SpecialN_InitializeState`, which makes an extra,
                // explicit `ftAnim_8006EBA4(gobj)` call immediately after
                // `Fighter_ChangeMotionState` lands `cur_anim_frame` on
                // `0.0` -- the same second advance `ftCo_Dash_Enter`/
                // `ftCo_Turn_Enter` make (`locomotion::start_dash`'s own
                // comment, `docs/validation.md`'s entry-advance table).
                // Modeled the same way, at the source: `action_frame` is 1
                // (not 0) from this frame on, so every later `action_frame`-
                // based Start gate compares against the decomp's own
                // thresholds unadjusted.
                fighter.action_frame = 1;
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
        let has_script = parameters.script.is_some();
        if let Some(script) = &parameters.script {
            if let Some(table) = script_table(script, fighter.action) {
                apply_script_frame(
                    &mut fighter.fox_neutral_special.cmd_vars,
                    table,
                    fighter.action_frame as usize,
                );
            }
            // `ftFx_SpecialNLoop_Anim`/`ftFx_SpecialAirNLoop_Anim`'s own
            // bottom-of-function fire check (`ftfoxspecialn.c:371-386,
            // 447-459`), independent of the loop/end transition decided
            // below: `cmd_vars[2] != 0` fires the shot and clears the flag
            // the same frame (`ftFox_SpecialN_CreateBlasterShot`'s own
            // `cmd_vars[2] = 0`), matching the accessory-callback path this
            // port cannot separately model (see module doc).
            if matches!(
                fighter.action,
                Action::SpecialNLoop | Action::SpecialAirNLoop
            ) && fighter.fox_neutral_special.cmd_vars[2] != 0
            {
                fighter.fox_neutral_special.cmd_vars[2] = 0;
                fighter.fox_neutral_special.fire = true;
            }
        }
        match fighter.action {
            Action::SpecialNStart
                if fighter.action_frame as usize >= parameters.start.ground.frames.len() =>
            {
                enter_loop(fighter, true, has_script, true);
            }
            Action::SpecialAirNStart
                if fighter.action_frame as usize >= parameters.start.air.frames.len() =>
            {
                enter_loop(fighter, false, has_script, true);
            }
            Action::SpecialNLoop
                if fighter.action_frame as usize >= parameters.loop_phase.ground.frames.len() =>
            {
                if fighter.fox_neutral_special.repeat_armed {
                    enter_loop(fighter, true, has_script, false);
                } else {
                    enter_end(fighter, true);
                }
            }
            Action::SpecialAirNLoop
                if fighter.action_frame as usize >= parameters.loop_phase.air.frames.len() =>
            {
                if fighter.fox_neutral_special.repeat_armed {
                    enter_loop(fighter, false, has_script, false);
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
