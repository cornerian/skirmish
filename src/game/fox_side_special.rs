//! Fox/Falco side special (Illusion/Phantasm) from `ftfoxspecials.c`, its
//! common dispatch (`ftCo_SpecialS.c`, `ftCo_SpecialAir.c`) and landing
//! (`ftCo_Landing.c`'s `ftCo_LandingFallSpecial_Enter`). Gated purely on
//! resource presence, like the neutral-special shell in `special.rs`; the
//! Slippi/animation character mapping for the six new actions belongs to
//! the observation layer, as the neutral shell's own 341/344 mapping does.
//!
//! The ghost item (`itfoxillusion.c`, `it_803F6818`) is confirmed hitbox-free:
//! every one of its three `Coll` callbacks (`itFoxillusion_UnkMotion{0,1,2}_
//! Coll`) unconditionally returns `false`, and `itFoxIllusion_Logic14_
//! DmgDealt` only clears a bookkeeping field it can never reach. The ghost
//! is GFX-only (a trailing echo of `ghostEffectPos`/`blendFrames`) and stays
//! entirely unmodeled, along with `cmd_vars[2]`'s creation bookkeeping and
//! rumble suppression (`Ft_MF_SkipRumble`).
use super::{
    Action, BUTTON_B, Controller, Error, Fighter,
    data::{Attack, FighterData},
};
use crate::fighter::{aerial as landing_math, fox_side_special as math};
use serde::{Deserialize, Serialize};

/// Common side-special stick rules (`ftCommonData`), paired with each
/// fighter's own `SideSpecial::ground_speed_retention`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `x218`: side-special stick threshold, shared by the grounded and
    /// aerial dispatchers.
    pub side_stick_threshold: f32,
    /// `x220`: turn-around threshold against the current facing.
    pub turn_threshold: f32,
    /// `x21C`: aerial up/down special stick threshold. Fox has
    /// SpecialAirHi/SpecialAirLw; both stay unmodeled, so a stick past this
    /// threshold must still suppress the side branch rather than fire it
    /// (`ftCo_SpecialAir_CheckInput`, `ftCo_SpecialAir.c:11-56`).
    pub vertical_threshold: f32,
}

pub(crate) fn validate_rules(rules: &Rules) -> Result<(), Error> {
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    if !finite(rules.side_stick_threshold)
        || rules.side_stick_threshold < 0.0
        || !finite(rules.turn_threshold)
        || rules.turn_threshold < 0.0
        || !finite(rules.vertical_threshold)
        || rules.vertical_threshold < 0.0
    {
        return Err(Error::Data("invalid side-special stick rules".into()));
    }
    Ok(())
}

/// `Fighter::side_special` (`fp->x1C_actionStateList` slice for
/// `ftFx_MS_SpecialSStart..=ftFx_MS_SpecialAirSEnd`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideSpecial {
    /// `co_attrs.specials_ground_speed_retention`, read by the common
    /// `doEnter` blend (`ftCo_SpecialS.c:41-49`) before this move's own
    /// entry, paired with `Rules` (both invented attribute names).
    pub ground_speed_retention: f32,
    pub start: Phase,
    pub dash: Dash,
    pub end: Phase,
    pub attributes: Attributes,
}

/// A ground/air pose pair with no hitboxes (the Start and End phases have
/// none in the source; the Dash phase's own pair is `Dash` below).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub ground: Attack,
    pub air: Attack,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dash {
    pub ground: Attack,
    /// `ft_800850E0`'s root-motion branch (`fp->x594_b0`): per-pose TransN
    /// z delta. `None` at a pose applies ordinary ground friction instead
    /// (`ft_800850E0`'s `else` branch, the fighter's own `ground_friction`).
    pub ground_trans_n: Vec<Option<f32>>,
    pub air: Attack,
    /// `ft_80085134`: per-pose TransN `[z, y]`, applied unconditionally (the
    /// air variant has no root-motion gate in the source).
    pub air_trans_n: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// `x24_FOX_ILLUSION_GRAVITY_DELAY`.
    pub gravity_delay: f32,
    /// `x28_FOX_ILLUSION_GROUND_VEL_X`: divides both the ground `gr_vel`
    /// and the air `self_vel.x` at entry.
    pub entry_speed_div: f32,
    /// `x2C_FOX_ILLUSION_UNK1`: Start air's `ftCommon_ApplyFrictionAir`.
    pub start_air_friction: f32,
    /// `x30_FOX_ILLUSION_UNK2`: Start air's `ftCommon_Fall` gravity argument.
    pub start_fall_accel: f32,
    /// `x34_FOX_ILLUSION_GROUND_END_VEL_X`.
    pub ground_end_speed: f32,
    /// `x38_FOX_ILLUSION_GROUND_FRICTION`: End ground's
    /// `ftCommon_ApplyFrictionGround`, not the fighter's ordinary attribute.
    pub end_ground_friction: f32,
    /// `x3C_FOX_ILLUSION_AIR_END_VEL_X`.
    pub air_end_speed: f32,
    /// `x40_FOX_ILLUSION_AIR_MUL_X`: End air's `ftCommon_ApplyFrictionAir`.
    pub end_air_friction: f32,
    /// `x44_FOX_ILLUSION_FALL_ACCEL` in the source's own (misleading) name;
    /// it is assigned directly into `mv.fx.SpecialS.gravityDelay` by
    /// `ftFox_SpecialSEnd_SetVars`, i.e. it is the End phase's gravity delay.
    pub end_gravity_delay: f32,
    /// `x48_FOX_ILLUSION_TERMINAL_VELOCITY` in the source's own (misleading)
    /// name; it is passed as `ftCommon_Fall`'s gravity argument in End air
    /// physics, with `co_attrs.terminal_velocity` supplying the real
    /// terminal-velocity argument, exactly like Start's `x30`.
    pub end_fall_accel: f32,
    /// `x4C_FOX_ILLUSION_FREEFALL_MOBILITY`: `ftCo_80096900`'s mobility
    /// multiplier for the FallSpecial exit (`ca->air_drift_max * x4C`).
    pub freefall_mobility: f32,
    /// `x50_FOX_ILLUSION_LANDING_LAG`: End air's landing lag, shared by both
    /// `ftFx_SpecialAirSEnd_Coll`'s direct `ftCo_LandingFallSpecial_Enter`
    /// call and the FallSpecial exit's own landing.
    pub landing_lag: f32,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &SideSpecial,
    fighter: &FighterData,
) -> Result<(), Error> {
    validate_rules(rules)?;
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    let a = &parameters.attributes;
    if !parameters.ground_speed_retention.is_finite()
        || !(0.0..=1.0).contains(&parameters.ground_speed_retention)
        || !finite(a.gravity_delay)
        || a.gravity_delay < 0.0
        || !finite(a.entry_speed_div)
        || a.entry_speed_div == 0.0
        || !finite(a.start_air_friction)
        || a.start_air_friction < 0.0
        || !finite(a.start_fall_accel)
        || !finite(a.ground_end_speed)
        || !finite(a.end_ground_friction)
        || a.end_ground_friction < 0.0
        || !finite(a.air_end_speed)
        || !finite(a.end_air_friction)
        || a.end_air_friction < 0.0
        || !finite(a.end_gravity_delay)
        || a.end_gravity_delay < 0.0
        || !finite(a.end_fall_accel)
        || !finite(a.freefall_mobility)
        || a.freefall_mobility < 0.0
        || !finite(a.landing_lag)
        || a.landing_lag <= 0.0
    {
        return Err(Error::Data("invalid side-special attributes".into()));
    }
    for attack in [
        &parameters.start.ground,
        &parameters.start.air,
        &parameters.dash.ground,
        &parameters.dash.air,
        &parameters.end.ground,
        &parameters.end.air,
    ] {
        if attack.frames.is_empty() || attack.frames.len() > 4096 {
            return Err(Error::Data(
                "side special requires 1..4096 physics samples per phase".into(),
            ));
        }
        for frame in &attack.frames {
            super::validation::validate_animation_pose(&frame.bones, fighter)?;
        }
    }
    if parameters.dash.ground_trans_n.len() != parameters.dash.ground.frames.len()
        || parameters.dash.air_trans_n.len() != parameters.dash.air.frames.len()
    {
        return Err(Error::Data(
            "side-special dash TransN must cover every dash pose".into(),
        ));
    }
    if parameters
        .dash
        .ground_trans_n
        .iter()
        .flatten()
        .any(|z| !z.is_finite() || z.abs() > 1_000_000.0)
        || parameters
            .dash
            .air_trans_n
            .iter()
            .any(|[z, y]| !finite(*z) || !finite(*y))
    {
        return Err(Error::Data(
            "side-special dash TransN must be finite".into(),
        ));
    }
    Ok(())
}

/// Persistent per-fighter state (`mv.fx.SpecialS.gravityDelay`; the ghost's
/// own bookkeeping fields are unmodeled, see the module documentation).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub gravity_delay: f32,
}

/// Physics/hurtbox pose for the current phase, mirroring `special::attack`.
pub(crate) fn attack(action: Action, parameters: &SideSpecial) -> Option<&Attack> {
    match action {
        Action::SpecialSStart => Some(&parameters.start.ground),
        Action::SpecialAirSStart => Some(&parameters.start.air),
        Action::SpecialS => Some(&parameters.dash.ground),
        Action::SpecialAirS => Some(&parameters.dash.air),
        Action::SpecialSEnd => Some(&parameters.end.ground),
        Action::SpecialAirSEnd => Some(&parameters.end.air),
        _ => None,
    }
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::SpecialSStart
            | Action::SpecialS
            | Action::SpecialSEnd
            | Action::SpecialAirSStart
            | Action::SpecialAirS
            | Action::SpecialAirSEnd
    )
}

/// `ftCo_SpecialS_CheckInput`/`ftCo_SpecialAir_CheckInput`'s side branch,
/// plus `ftFx_SpecialS_IASA`/`ftFx_SpecialAirS_IASA`'s B-press shortening.
/// `ground`/`air` are the same per-chain eligibility `special::update_actions`
/// computes for the neutral special (this move fires from the identical
/// state lists). Returns whether this frame's dispatch is consumed.
pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    ground: bool,
    air: bool,
    input: Controller,
) -> bool {
    let (Some(rules), Some(parameters)) = (rules, data.side_special.as_ref()) else {
        return false;
    };
    let fresh_b = input.buttons & !fighter.previous_input.buttons & BUTTON_B != 0;
    if matches!(fighter.action, Action::SpecialS | Action::SpecialAirS) {
        // ftFx_SpecialS_IASA / ftFx_SpecialAirS_IASA: a fresh B press
        // shortens the dash into the End phase by `ground_or_air`,
        // regardless of which Dash variant currently owns dispatch.
        if fresh_b {
            enter_end(fighter, parameters);
        }
        return true;
    }
    if owns_action(fighter.action) {
        return true;
    }
    if !(ground || air) || !math::has_input(fresh_b, input.stick[0], rules.side_stick_threshold) {
        return false;
    }
    if air {
        if input.stick[1].abs() >= rules.vertical_threshold {
            // SpecialAirHi/SpecialAirLw's own branches take priority and
            // stay unmodeled; the side branch must not fire in their place.
            return false;
        }
    } else if fighter.locomotion.side_special_b_age != 0 {
        // ftCo_SpecialS_CheckInput's `fp->x688 == 0` gate; the aerial
        // dispatcher has no equivalent age check in the source.
        return false;
    }
    if math::should_turn(input.stick[0], fighter.facing, rules.turn_threshold) {
        fighter.facing = -fighter.facing;
    }
    if ground {
        // doEnter, then ftFx_SpecialSStart_Enter's own x28 division.
        fighter.ground_velocity =
            math::entry_ground_velocity(fighter.ground_velocity, parameters.ground_speed_retention);
        super::simulation::enter(fighter, Action::SpecialSStart);
        fighter.ground_velocity /= parameters.attributes.entry_speed_div;
    } else {
        fighter.velocity[1] = 0.0;
        fighter.velocity[0] /= parameters.attributes.entry_speed_div;
        super::simulation::enter(fighter, Action::SpecialAirSStart);
        fighter.locomotion.jumps_used = data
            .locomotion
            .as_ref()
            .map_or(fighter.locomotion.jumps_used, |p| p.max_jumps);
    }
    fighter.fox_side_special.gravity_delay = parameters.attributes.gravity_delay;
    true
}

fn enter_end(fighter: &mut Fighter, parameters: &SideSpecial) {
    if fighter.grounded {
        fighter.ground_velocity = parameters.attributes.ground_end_speed * fighter.facing;
        super::simulation::enter(fighter, Action::SpecialSEnd);
    } else {
        fighter.velocity[0] = parameters.attributes.air_end_speed * fighter.facing;
        fighter.velocity[1] = 0.0;
        super::simulation::enter(fighter, Action::SpecialAirSEnd);
    }
    fighter.fox_side_special.gravity_delay = parameters.attributes.end_gravity_delay;
}

/// `ftFx_SpecialSStart_Anim`/`ftFx_SpecialAirSStart_Anim`,
/// `ftFx_SpecialS_Anim`/`ftFx_SpecialAirS_Anim` (natural end, not the
/// IASA shortening above), `ftFx_SpecialSEnd_Anim`/`ftFx_SpecialAirSEnd_
/// Anim`: called once per frame alongside `special::update_animation`.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) {
    let Some(parameters) = data.side_special.as_ref() else {
        return;
    };
    match fighter.action {
        Action::SpecialSStart
            if fighter.action_frame as usize >= parameters.start.ground.frames.len() =>
        {
            super::simulation::enter(fighter, Action::SpecialS);
        }
        Action::SpecialAirSStart
            if fighter.action_frame as usize >= parameters.start.air.frames.len() =>
        {
            super::simulation::enter(fighter, Action::SpecialAirS);
        }
        Action::SpecialS
            if fighter.action_frame as usize >= parameters.dash.ground.frames.len() =>
        {
            enter_end(fighter, parameters);
        }
        Action::SpecialAirS
            if fighter.action_frame as usize >= parameters.dash.air.frames.len() =>
        {
            enter_end(fighter, parameters);
        }
        Action::SpecialSEnd
            if fighter.action_frame as usize >= parameters.end.ground.frames.len() =>
        {
            // ft_8008A2BC: ordinary Wait re-entry.
            super::simulation::enter(fighter, Action::Wait);
        }
        Action::SpecialAirSEnd
            if fighter.action_frame as usize >= parameters.end.air.frames.len() =>
        {
            // ftCo_80096900(1, 0, true, x4C, x50): FallSpecial with
            // allow_interrupt, the scaled freefall mobility, and every jump
            // restored (the source's `unk` argument is true).
            super::simulation::enter(fighter, Action::FallSpecial);
            fighter.aerial.allow_interrupt = true;
            fighter.aerial.mobility = parameters.attributes.freefall_mobility;
            fighter.locomotion.jumps_used = data
                .locomotion
                .as_ref()
                .map_or(fighter.locomotion.jumps_used, |p| p.max_jumps);
        }
        _ => {}
    }
}

/// `ft_800850E0`'s root-motion branch, read by `move_fighter`'s ground
/// target-velocity chain (`ft_80085030`'s pattern already used by
/// `dash`/`taunt`). `None` when the current pose has no TransN, falling
/// through to the ordinary ground-friction branch (matching the source's
/// own `else` branch, the fighter's plain `ground_friction`).
pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    if fighter.action != Action::SpecialS {
        return None;
    }
    let slot = data
        .side_special
        .as_ref()?
        .dash
        .ground_trans_n
        .get(fighter.action_frame as usize)?;
    (*slot).map(|z| z * fighter.facing)
}

/// `ftFx_SpecialSStart_Phys`/`ftFx_SpecialSEnd_Phys`'s shared countdown,
/// unconditional even though the ground never reads the delayed value.
pub(crate) fn tick_ground_gravity_delay(fighter: &mut Fighter) {
    if matches!(fighter.action, Action::SpecialSStart | Action::SpecialSEnd)
        && fighter.fox_side_special.gravity_delay > 0.0
    {
        fighter.fox_side_special.gravity_delay -= 1.0;
    }
}

/// `ftFx_SpecialSEnd_Phys`'s `ftCommon_ApplyFrictionGround(x38)`, distinct
/// from the fighter's ordinary `ground_friction` attribute.
pub(crate) fn end_ground_friction(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    if fighter.action != Action::SpecialSEnd {
        return None;
    }
    Some(data.side_special.as_ref()?.attributes.end_ground_friction)
}

/// Owns the Start/Dash/End air phases' physics entirely: `true` means the
/// caller's ordinary airborne chain (fast fall, drift, damage locks, ...)
/// must not also run this frame.
pub(crate) fn air_physics(
    fighter: &mut Fighter,
    data: &FighterData,
    movement: &mut crate::fighter::Movement,
) -> bool {
    let Some(parameters) = data.side_special.as_ref() else {
        return false;
    };
    match fighter.action {
        Action::SpecialAirSStart => {
            if fighter.fox_side_special.gravity_delay > 0.0 {
                fighter.fox_side_special.gravity_delay -= 1.0;
            } else {
                movement.fall(
                    parameters.attributes.start_fall_accel,
                    movement.attributes.terminal_velocity,
                );
            }
            movement.friction_air(parameters.attributes.start_air_friction);
            true
        }
        Action::SpecialAirSEnd => {
            if fighter.fox_side_special.gravity_delay > 0.0 {
                fighter.fox_side_special.gravity_delay -= 1.0;
            } else {
                movement.fall(
                    parameters.attributes.end_fall_accel,
                    movement.attributes.terminal_velocity,
                );
            }
            movement.friction_air(parameters.attributes.end_air_friction);
            true
        }
        Action::SpecialAirS => {
            // ft_80085134: unconditional, no root-motion gate in the source.
            if let Some(&[z, y]) = parameters
                .dash
                .air_trans_n
                .get(fighter.action_frame as usize)
            {
                movement.self_velocity = [z * fighter.facing, y, movement.self_velocity[2]];
            }
            true
        }
        _ => false,
    }
}

/// `ftFx_SpecialAirSEnd_Coll`'s direct `ftCo_LandingFallSpecial_Enter(gobj,
/// false, x50)`: unlike the Start/Dash phases, End air's own landing skips
/// SpecialSEnd entirely and enters `LandingFallSpecial` at once, sharing the
/// fighter-wide `x2EC` end frame already modeled by `escape_air::Parameters`.
pub(crate) fn land(fighter: &mut Fighter, data: &FighterData) -> Result<bool, Error> {
    if fighter.action != Action::SpecialAirSEnd {
        return Ok(false);
    }
    let (Some(parameters), Some(escape_air_parameters)) =
        (data.side_special.as_ref(), data.escape_air.as_ref())
    else {
        return Err(Error::Data(
            "side-special landing requires explicit resources".into(),
        ));
    };
    super::simulation::enter(fighter, Action::LandingFallSpecial);
    fighter.aerial.allow_interrupt = false;
    fighter.aerial.landing_rate = landing_math::landing_animation_rate(
        escape_air_parameters.landing_animation_end,
        parameters.attributes.landing_lag,
    );
    Ok(true)
}
