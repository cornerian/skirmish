//! Fox/Falco side special (Illusion/Phantasm): resource shape, validation
//! and the pieces of its behaviour genuinely specific to this move (its
//! input gate ordering, the Start/Dash/End phase table, the ghost-item
//! deviation). Everything reusable across moves -- gravity-delayed fall,
//! pose-driven velocity, ground/air conversion keeping the frame, the
//! `FallSpecial`/landing exits -- comes from `game::specials::helpers`
//! instead of being re-derived here. Gated purely on resource presence,
//! like the shared neutral shell.
//!
//! The ghost item (`itfoxillusion.c`, `it_803F6818`) is confirmed hitbox-free:
//! every one of its three `Coll` callbacks (`itFoxillusion_UnkMotion{0,1,2}_
//! Coll`) unconditionally returns `false`, and `itFoxIllusion_Logic14_
//! DmgDealt` only clears a bookkeeping field it can never reach. The ghost
//! is GFX-only (a trailing echo of `ghostEffectPos`/`blendFrames`) and stays
//! entirely unmodeled, along with `cmd_vars[2]`'s creation bookkeeping and
//! rumble suppression (`Ft_MF_SkipRumble`).

use crate::{
    fighter::{Movement, characters::fox as math, edge::Mode},
    game::{
        Action, BUTTON_B, Controller, Error, Fighter,
        data::{Attack, FighterData, Rules as MatchRules},
        simulation,
        specials::{SpecialMove, helpers},
        validation::validate_animation_pose,
    },
};
use serde::{Deserialize, Serialize};

/// Common `ftCommonData` fields this move family shares, paired with each
/// fighter's own `SideSpecial::ground_speed_retention`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `x218`: side-special stick threshold, shared by the grounded and
    /// aerial dispatchers.
    pub side_stick_threshold: f32,
    /// `x220`: turn-around threshold against the current facing.
    pub turn_threshold: f32,
    /// `x21C`: aerial up/down special stick threshold, shared with
    /// `characters::fox::down`'s own aerial entry and `characters::
    /// fox::up`'s own grounded/aerial dispatch gate. A stick past this
    /// threshold must still suppress the side branch rather than fire it
    /// (`ftCo_SpecialAir_CheckInput`, `ftCo_SpecialAir.c:11-56`).
    pub vertical_threshold: f32,
    /// `x1FC` (`struct ftCommonData`, `ft/types.h:181`): the common over-
    /// drift-maximum deceleration step `ftCommon_8007CF58`/`ftCommon_
    /// 8007D050` (`ftcommon.c:283-330`) use to bring `self_vel.x` back
    /// toward a drift maximum once already past it. Read by
    /// `characters::fox::down`'s own air phases through `specials::
    /// helpers::drift_or_friction_air`; unused by the side special itself
    /// (its own air phases apply a fixed custom friction, never this
    /// common drift function).
    pub air_drift_recovery_step: f32,
}

pub(crate) fn validate_rules(rules: &Rules) -> Result<(), Error> {
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    if !finite(rules.side_stick_threshold)
        || rules.side_stick_threshold < 0.0
        || !finite(rules.turn_threshold)
        || rules.turn_threshold < 0.0
        || !finite(rules.vertical_threshold)
        || rules.vertical_threshold < 0.0
        || !finite(rules.air_drift_recovery_step)
        || rules.air_drift_recovery_step < 0.0
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
    /// `specials.side.script` (exporter), present once the exporter supplies
    /// it for this fighter; `None` for Falco/pre-batch fixtures, matching
    /// `neutral::NeutralSpecial::script`'s own fallback. Boxed for the same
    /// reason as that field -- see its own doc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Box<SideScript>>,
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

/// Ground/air pair of one phase's per-frame `SetCmdVar` (opcode 19,
/// `ftaction.c:454-471`) trace, shared between the neutral special's own
/// three-phase script (`neutral::NeutralScript`) and this move's own
/// single-phase (`Dash`) script below.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptPhase {
    pub ground: ScriptFrames,
    pub air: ScriptFrames,
}

/// One ground/air variant's own per-frame trace, straight from the
/// exporter's script decoder. `cmd_vars[i]` is `Some` from the frame its
/// `SetCmdVar` instruction first executes onward (the source's own
/// persistent register, forward-filled by the exporter across every later
/// sampled frame; a `None` slot reads as that register's pre-`SetCmdVar`
/// value, `0`). `allow_interrupt` is exporter-confirmed `false` throughout
/// every phase either move uses this sidecar for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptFrames {
    pub cmd_vars: Vec<[Option<u32>; 4]>,
    pub allow_interrupt: Vec<bool>,
}

/// Checks a script sidecar's per-frame vectors against the phase's own pose
/// count, shared by the neutral and side specials' own `validate`.
pub(crate) fn validate_script_frames(
    frames: &ScriptFrames,
    pose_count: usize,
    context: &str,
) -> Result<(), Error> {
    if frames.cmd_vars.len() != pose_count || frames.allow_interrupt.len() != pose_count {
        return Err(Error::Data(format!(
            "{context} script frame count must match its phase's pose count"
        )));
    }
    Ok(())
}

/// `CreateGhostItem` (`ftfoxspecials.c:61-64, 247-266`): the Dash phase's
/// own script trace. The ghost item it gates (`cmd_vars[2] == 1` at frame 2)
/// is confirmed hitbox-free and GFX-only (module doc) -- this sidecar exists
/// purely so the spawn frame can be recorded/tested, not read by any
/// gameplay logic below.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideScript {
    pub dash: ScriptPhase,
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
            validate_animation_pose(&frame.bones, fighter)?;
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
    if let Some(script) = &parameters.script {
        validate_script_frames(
            &script.dash.ground,
            parameters.dash.ground.frames.len(),
            "side dash ground",
        )?;
        validate_script_frames(
            &script.dash.air,
            parameters.dash.air.frames.len(),
            "side dash air",
        )?;
    }
    Ok(())
}

/// Persistent per-fighter state (`mv.fx.SpecialS.gravityDelay`; the ghost's
/// own bookkeeping fields are unmodeled, see the module documentation).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub gravity_delay: f32,
}

/// Physics/hurtbox pose for the current phase, mirroring the neutral
/// shell's own `attack`.
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

fn enter_end(fighter: &mut Fighter, parameters: &SideSpecial) {
    if fighter.grounded {
        fighter.ground_velocity = parameters.attributes.ground_end_speed * fighter.facing;
        simulation::enter(fighter, Action::SpecialSEnd);
    } else {
        fighter.velocity[0] = parameters.attributes.air_end_speed * fighter.facing;
        fighter.velocity[1] = 0.0;
        simulation::enter(fighter, Action::SpecialAirSEnd);
    }
    fighter.fox_side_special.gravity_delay = parameters.attributes.end_gravity_delay;
}

/// This move's handle in Fox's registry (`MOVES` in `characters::fox`).
pub(crate) struct Move;

pub(crate) const MOVE: Move = Move;

impl SpecialMove for Move {
    fn owns(&self, action: Action) -> bool {
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

    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        attack(action, data.specials.as_ref()?.fox_side()?)
    }

    /// The grounded/aerial dispatch chains check this move's own input
    /// ahead of the shared neutral shell (`characters::fox::MOVES`'s own
    /// order encodes that), plus the Dash phase's B-press shortening.
    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let (Some(rules), Some(parameters)) = (
            rules.specials.as_ref(),
            data.specials.as_ref().and_then(|s| s.fox_side()),
        ) else {
            return false;
        };
        let fresh_b = input.buttons & !fighter.previous_input.buttons & BUTTON_B != 0;
        if matches!(fighter.action, Action::SpecialS | Action::SpecialAirS) {
            // A fresh B press shortens the dash into the End phase by
            // whichever of ground/air the fighter is in at that moment, not
            // by which Dash variant currently owns dispatch.
            if fresh_b {
                enter_end(fighter, parameters);
            }
            return true;
        }
        if self.owns(fighter.action) {
            return true;
        }
        if !(ground || air) || !math::has_input(fresh_b, input.stick[0], rules.side_stick_threshold)
        {
            return false;
        }
        if air {
            if input.stick[1].abs() >= rules.vertical_threshold {
                // SpecialAirHi/SpecialAirLw's own branches take priority and
                // stay unmodeled; the side branch must not fire in their place.
                return false;
            }
        } else if fighter.locomotion.side_special_b_age != 0 {
            // The grounded dispatcher only fires on the exact frame a fresh
            // press crosses the stick threshold; the aerial one has no
            // equivalent age gate.
            return false;
        }
        if math::should_turn(input.stick[0], fighter.facing, rules.turn_threshold) {
            fighter.facing = -fighter.facing;
        }
        if ground {
            fighter.ground_velocity = math::entry_ground_velocity(
                fighter.ground_velocity,
                parameters.ground_speed_retention,
            );
            simulation::enter(fighter, Action::SpecialSStart);
            fighter.ground_velocity /= parameters.attributes.entry_speed_div;
        } else {
            fighter.velocity[1] = 0.0;
            fighter.velocity[0] /= parameters.attributes.entry_speed_div;
            simulation::enter(fighter, Action::SpecialAirSStart);
            helpers::max_out_jumps(fighter, data);
        }
        // `ftFx_SpecialSStart_Enter`/`ftFx_SpecialAirSStart_Enter` each make
        // an extra, explicit `ftAnim_8006EBA4(gobj)` call immediately after
        // `Fighter_ChangeMotionState` lands `cur_anim_frame` on `0.0` -- the
        // same second advance `ftCo_Dash_Enter`/`ftCo_Turn_Enter` make
        // (`locomotion::start_dash`'s own comment, `docs/validation.md`'s
        // entry-advance table). Modeled the same way, at the source:
        // `action_frame` is 1 (not 0) from this frame on.
        fighter.action_frame = 1;
        fighter.fox_side_special.gravity_delay = parameters.attributes.gravity_delay;
        true
    }

    fn update_animation(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        input: Controller,
        on_platform: bool,
    ) {
        let _ = (input, on_platform);
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_side()) else {
            return;
        };
        match fighter.action {
            Action::SpecialSStart
                if fighter.action_frame as usize >= parameters.start.ground.frames.len() =>
            {
                simulation::enter(fighter, Action::SpecialS);
            }
            Action::SpecialAirSStart
                if fighter.action_frame as usize >= parameters.start.air.frames.len() =>
            {
                simulation::enter(fighter, Action::SpecialAirS);
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
                helpers::exit_to_wait_or_fall(fighter);
            }
            Action::SpecialAirSEnd
                if fighter.action_frame as usize >= parameters.end.air.frames.len() =>
            {
                // No per-instance landing lag threaded here: this move's
                // own FallSpecial exit predates that resource, and this
                // preserves its already-audited behavior (landing at the
                // shared `escape_air::Rules`' own rate) unchanged.
                helpers::enter_fall_special(
                    fighter,
                    data,
                    parameters.attributes.freefall_mobility,
                    None,
                );
            }
            _ => {}
        }
    }

    fn ground_target_velocity(&self, fighter: &Fighter, data: &FighterData) -> Option<f32> {
        if fighter.action != Action::SpecialS {
            return None;
        }
        let parameters = data.specials.as_ref()?.fox_side()?;
        helpers::ground_pose_velocity(
            &parameters.dash.ground_trans_n,
            fighter.action_frame,
            fighter.facing,
        )
    }

    fn ground_friction_override(&self, fighter: &Fighter, data: &FighterData) -> Option<f32> {
        if fighter.action != Action::SpecialSEnd {
            return None;
        }
        Some(
            data.specials
                .as_ref()?
                .fox_side()?
                .attributes
                .end_ground_friction,
        )
    }

    fn air_physics(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        movement: &mut Movement,
    ) -> bool {
        let _ = rules;
        let Some(parameters) = data.specials.as_ref().and_then(|s| s.fox_side()) else {
            return false;
        };
        let terminal_velocity = movement.attributes.terminal_velocity;
        match fighter.action {
            Action::SpecialAirSStart => {
                helpers::gravity_delayed_fall(
                    &mut fighter.fox_side_special.gravity_delay,
                    movement,
                    parameters.attributes.start_fall_accel,
                    terminal_velocity,
                    parameters.attributes.start_air_friction,
                );
                true
            }
            Action::SpecialAirSEnd => {
                helpers::gravity_delayed_fall(
                    &mut fighter.fox_side_special.gravity_delay,
                    movement,
                    parameters.attributes.end_fall_accel,
                    terminal_velocity,
                    parameters.attributes.end_air_friction,
                );
                true
            }
            Action::SpecialAirS => {
                // Unconditional, unlike the ground dash: every dash pose
                // supplies both axes directly, with no root-motion gate.
                if let Some([z, y]) =
                    helpers::air_pose_velocity(&parameters.dash.air_trans_n, fighter.action_frame)
                {
                    movement.self_velocity = [z * fighter.facing, y, movement.self_velocity[2]];
                }
                true
            }
            _ => false,
        }
    }

    fn tick_ground_timers(&self, fighter: &mut Fighter) {
        if matches!(fighter.action, Action::SpecialSStart | Action::SpecialSEnd) {
            // Both grounded phases count their gravity delay down even
            // though gravity is never applied on the ground, so a mid-move
            // ground/air conversion sees the same countdown the air phase
            // would have reached.
            helpers::tick_ground_delay(&mut fighter.fox_side_special.gravity_delay);
        }
    }

    fn transfer_ground_air(&self, fighter: &mut Fighter, grounded: bool) -> bool {
        let destination = match (fighter.action, grounded) {
            (Action::SpecialSStart, false) => Action::SpecialAirSStart,
            (Action::SpecialAirSStart, true) => Action::SpecialSStart,
            (Action::SpecialS, false) => Action::SpecialAirS,
            (Action::SpecialAirS, true) => Action::SpecialS,
            // The End phase's own conversions are asymmetric: ground
            // leaving the floor enters ordinary Fall (the caller's generic
            // fallback), and air landing enters `LandingFallSpecial`
            // directly through `land` below, bypassing SpecialSEnd/
            // SpecialAirSEnd entirely. Neither is a Start/Dash-style
            // same-phase conversion, so End is deliberately absent here.
            _ => return false,
        };
        let gravity_delay = fighter.fox_side_special.gravity_delay;
        helpers::transfer_frame(fighter, destination);
        fighter.fox_side_special.gravity_delay = gravity_delay;
        true
    }

    /// End air's own landing skips SpecialSEnd entirely and enters
    /// `LandingFallSpecial` at once, sharing the fighter-wide end-frame
    /// resource the ordinary air-dodge landing already models.
    fn land(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        on_platform: bool,
        pre_landing: &Fighter,
    ) -> Result<bool, Error> {
        let _ = (on_platform, pre_landing);
        if fighter.action != Action::SpecialAirSEnd {
            return Ok(false);
        }
        let (Some(parameters), Some(escape_air_parameters)) = (
            data.specials.as_ref().and_then(|s| s.fox_side()),
            data.escape_air.as_ref(),
        ) else {
            return Err(Error::Data(
                "side-special landing requires explicit resources".into(),
            ));
        };
        helpers::enter_landing_fall_special(
            fighter,
            escape_air_parameters.landing_animation_end,
            parameters.attributes.landing_lag,
        );
        Ok(true)
    }

    fn collision_mode(&self, action: Action) -> Option<Mode> {
        // The Start/Dash phases are plain (the unmatched default in
        // `edge::mode_for_action` already gives them that); only End clamps.
        matches!(action, Action::SpecialSEnd).then_some(Mode::Clamp)
    }

    fn ledge_catchable(&self, action: Action) -> bool {
        // Each aerial phase's own collision check reuses the ordinary
        // ledge-catch scan, exactly like the aerial attacks.
        matches!(
            action,
            Action::SpecialAirSStart | Action::SpecialAirS | Action::SpecialAirSEnd
        )
    }
}
