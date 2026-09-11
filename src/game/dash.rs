//! Dash-phase and AttackDash callbacks from `ftCo_Dash.c`, `ftCo_Run.c`
//! (`ftCo_Run_IASA`) and `ftCo_AttackDash.c`. See `docs/dash-attack.md` for
//! the exact per-phase dispatch order this module reproduces.
//!
//! When `rules.dash` is present, this module is the single place Dash, Run
//! and AttackDash frames are dispatched from: `simulation::update_actions`
//! calls [`update_actions`] before `grab::update_actions`, so the older
//! per-module Dash/Run arms in `grab.rs`, `shield.rs` and `locomotion.rs` are
//! never reached for a frame this module already handled. They remain the
//! exact behaviour when `rules.dash` is `None`, and are also reached
//! (redundantly but harmlessly, over the same unchanged input) whenever this
//! module finds nothing to do.
//!
//! `ftCo_Dash_IASA`'s `x54` friction tail runs whenever a phase branch falls
//! out of its `if`/`else` instead of returning: the early forward smash, the
//! early roll, the middle dash-back Turn, the middle shield entry, the late
//! re-dash/Turn and the late shield entry all fall through to it (`transition_friction`,
//! applied immediately after each in [`update_dash_or_run`]); `block_42`'s own
//! taunt (`ftCo_800DE9D8`, see [`taunt`](super::taunt)) is the only way past
//! it without an early return, so a taunt fired from Dash falls through to it
//! too; the catch, AttackDash entry, the jump-squat entry, the run transition
//! and "nothing fired" all `return` instead and never reach it. Run has no
//! such tail (`ftCo_Run_IASA` has none), even though its own IASA reaches
//! `ftCo_800DE9D8` at the same relative position as Dash's `block_42`
//! (modeled here by the same shared call, gated on `f.action == Action::Dash`
//! for the friction). A neutral special entered from Dash also falls through
//! to it (`ftCo_SpecialS_CheckInput` is the first check of both the early and
//! middle phases); `simulation::update_actions` applies it there, since
//! `special::update_actions` runs after this module.
use super::{
    Action, Controller, Error, Fighter,
    data::{Attack, FighterData, Rules as MatchRules},
    simulation,
    tilt::GroundFrameFlags,
};
use crate::fighter::dash as math;
use serde::{Deserialize, Serialize};

pub use crate::fighter::dash::Phase;

/// Common dash-phase data (`ftCommonData` x44/x48/x54/x50). The middle-phase
/// limit (x4C) and the AttackDash grab buffer (x68) already live in
/// `rules.grab.shield_grab` and are reused rather than duplicated.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// `x44`: last Dash frame eligible for the early-phase checks, and only
    /// for a Dash entered by input (`dash_from_input`).
    pub early_frames: f32,
    /// `x48`: last Dash frame eligible for the held-shoulder forward roll,
    /// itself only reachable inside the early phase.
    pub roll_frames: f32,
    /// `x54`: fraction of ground velocity removed by `ftCo_Dash_IASA`'s
    /// shared tail, applied on the same frame as the fall-through
    /// transitions listed on this module's own documentation.
    pub transition_friction: f32,
    /// `x50`: AttackDash friction multiplier over the fighter's ground
    /// friction, used when no root motion is supplied.
    pub attack_friction_multiplier: f32,
}

/// `Fighter::dash_attack`, paired with `rules.dash` like smashes are paired
/// with `rules.smash`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashAttack {
    pub attack: Attack,
    /// One decoded command-state sample per attack pose. Only
    /// `allow_interrupt` is meaningful; `repeat_ready` is rejected.
    pub flags: Vec<GroundFrameFlags>,
    /// Per-pose TransN delta consumed by `ft_80085030`, if supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_translations: Option<Vec<f32>>,
}

/// `Fighter::mv.co.attackdash`, reset by every motion change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub grab_buffer: f32,
}

pub(crate) fn validate(
    rules: &Rules,
    attack: &DashAttack,
    fighter: &FighterData,
    grab_rules: Option<&super::grab::Rules>,
) -> Result<(), Error> {
    if ![
        rules.early_frames,
        rules.roll_frames,
        rules.transition_friction,
        rules.attack_friction_multiplier,
    ]
    .into_iter()
    .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
    {
        return Err(Error::Data("invalid ordinary dash rules".into()));
    }
    if grab_rules.is_none_or(|rules| rules.shield_grab.is_none()) {
        return Err(Error::Data(
            "dash rules require grab shield-grab rules for the buffer and x4C limit".into(),
        ));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "dash rules require locomotion parameters for their phase thresholds".into(),
        ));
    }
    let frames = attack.attack.frames.len();
    if attack.flags.len() != frames || attack.flags.iter().any(|flags| flags.repeat_ready) {
        return Err(Error::Data(
            "dash attack command flags must cover every pose without repeat flags".into(),
        ));
    }
    if let Some(roots) = &attack.root_translations
        && (roots.len() != frames
            || roots
                .iter()
                .any(|root| !root.is_finite() || root.abs() > 1_000_000.0))
    {
        return Err(Error::Data(
            "dash attack root motion must supply one finite delta per pose".into(),
        ));
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    action == Action::AttackDash
}

pub(crate) fn attack(attack: &DashAttack) -> &Attack {
    &attack.attack
}

fn current<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a DashAttack> {
    if !owns_action(fighter.action) {
        return None;
    }
    data.dash_attack.as_ref()
}

/// Whether this frame's flags open the Wait chain.
pub(crate) fn interruptible(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.grounded
        && current(fighter, data)
            .and_then(|attack| attack.flags.get(fighter.action_frame as usize))
            .is_some_and(|flags| flags.allow_interrupt)
}

/// `ft_80085030`'s target for an AttackDash sample: root motion when
/// supplied, otherwise one frame of `x50 * ground_friction` deceleration.
pub(crate) fn ground_target_velocity(
    fighter: &Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
) -> Option<f32> {
    let dash_attack = current(fighter, data)?;
    if let Some(roots) = &dash_attack.root_translations {
        return Some(*roots.get(fighter.action_frame as usize)? * fighter.facing);
    }
    let rules = rules?;
    let friction = rules.attack_friction_multiplier * data.movement.ground_friction;
    Some(math::apply_friction(fighter.ground_velocity, friction))
}

/// `ftCo_AttackDash_Anim`: end of poses returns to Wait, or Fall airborne.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        return Ok(());
    }
    let attack = current(fighter, data)
        .ok_or_else(|| Error::Data("dash attack action without its supplied attack".into()))?;
    if fighter.action_frame as usize >= attack.attack.frames.len() {
        simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
    }
    Ok(())
}

/// `ftCo_800D8A38`: a fresh logical A press with the logical shoulder held
/// starts CatchDash. Checked first in every Dash phase and in Run.
fn catch_dash(f: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let a_pressed = super::smash::a_pressed(f, input);
    let shoulder_held = input.shield_held() || input.buttons & super::BUTTON_Z != 0;
    if f.grounded && a_pressed && shoulder_held && data.grab.is_some() {
        simulation::enter(f, Action::CatchDash);
        true
    } else {
        false
    }
}

/// `ftCo_AttackDash_CheckInput` + `ftCo_AttackDash_SetMv0`: a fresh logical A
/// press enters AttackDash and arms the catch buffer to `x68`.
fn attack_dash_enter(
    f: &mut Fighter,
    shield_grab: &super::grab::ShieldGrabRules,
    input: Controller,
) -> bool {
    if super::smash::a_pressed(f, input) {
        simulation::enter(f, Action::AttackDash);
        f.dash.grab_buffer = shield_grab.dash_buffer_frames;
        true
    } else {
        false
    }
}

/// `ftCo_Dash_IASA`'s shared tail, applied on the same frame as the
/// fall-through transitions this module lists in its own documentation.
/// `ft_GetGroundFrictionMultiplier` (stage/metal multiplier) is fixed at 1.0.
pub(crate) fn apply_transition_friction(f: &mut Fighter, dash_rules: &Rules) {
    f.ground_velocity =
        math::transition_friction(f.ground_velocity, dash_rules.transition_friction, 1.0);
}

fn shield_from_dash(
    f: &mut Fighter,
    rules: &MatchRules,
    input: Controller,
    dash_grab_buffer: f32,
) -> bool {
    let Some(r) = rules.shield.as_ref() else {
        return false;
    };
    super::shield::enter_from_neutral(f, r, input, dash_grab_buffer)
}

/// `ftCo_800D8AE0` and, when it does not fire, `ftCo_AttackDash_IASA`'s
/// exposure of the complete Wait chain via `tilt::interrupt_chain`.
fn update_attack_dash(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    input: Controller,
) -> bool {
    if !rules.grab.as_ref().is_some_and(|g| g.shield_grab.is_some()) {
        return false;
    }
    let lr_held = input.shield_held() || input.buttons & super::BUTTON_Z != 0;
    if math::attack_dash_grab(lr_held, &mut f.dash.grab_buffer) && data.grab.is_some() {
        simulation::enter(f, Action::CatchDash);
        return true;
    }
    false
}

/// `ftCo_Dash_IASA` and `ftCo_Run_IASA` in one place: catch, dash attack,
/// (Dash only) dash-back turn / forward roll / forward smash by phase,
/// shield, jump, then Dash's run transition or Run's turn/brake. If nothing
/// above fired, `block_42` in the pinned source falls to the unmodeled taunt
/// check and its friction tail; both are absent here, so this returns
/// `false` (nothing happened) exactly like Run already could.
fn update_dash_or_run(
    f: &mut Fighter,
    data: &FighterData,
    dash_rules: &Rules,
    rules: &MatchRules,
    input: Controller,
) -> Result<bool, Error> {
    let Some(p) = data.locomotion.as_ref() else {
        return Ok(false);
    };
    let Some(shield_grab) = rules.grab.as_ref().and_then(|g| g.shield_grab.as_ref()) else {
        return Ok(false);
    };

    if catch_dash(f, data, input) {
        return Ok(true);
    }

    if f.action == Action::Dash {
        match math::phase(
            f.locomotion.dash_from_input,
            f.action_frame as f32,
            dash_rules.early_frames,
            shield_grab.dash_buffer_frame_limit,
        ) {
            Phase::Early => {
                if let Some((action, facing)) =
                    super::smash::select_dash(f, data, rules.smash.as_ref(), input)
                {
                    super::smash::start(f, data, action, facing);
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
                if f.action_frame as f32 <= dash_rules.roll_frames
                    && super::escape::dash_forward_roll(f, data, rules.escape.as_ref(), input)?
                {
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
            }
            Phase::Middle => {
                if attack_dash_enter(f, shield_grab, input) {
                    return Ok(true);
                }
                if input.stick[0] * f.facing < 0.0 && super::locomotion::try_dash(f, p, input) {
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
                let buffer = super::grab::shield_entry_buffer(f, rules.grab.as_ref());
                if shield_from_dash(f, rules, input, buffer) {
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
            }
            Phase::Late => {
                if super::locomotion::try_dash(f, p, input) {
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
                let buffer = super::grab::shield_entry_buffer(f, rules.grab.as_ref());
                if shield_from_dash(f, rules, input, buffer) {
                    apply_transition_friction(f, dash_rules);
                    return Ok(true);
                }
            }
        }
    } else {
        if attack_dash_enter(f, shield_grab, input) {
            return Ok(true);
        }
        let buffer = super::grab::shield_entry_buffer(f, rules.grab.as_ref());
        if shield_from_dash(f, rules, input, buffer) {
            return Ok(true);
        }
    }

    // block_42: `ftCo_800DE9D8` (D-pad up), the pinned source's only way to
    // fall through this block into the friction tail instead of returning
    // (`ftCo_Dash.c`'s `block_42` literally reads `if (!ftCo_800DE9D8(gobj))
    // { ...return...} ` -- taunt success falls through to the shared x54
    // friction application below instead of any of the early returns), then
    // the shared jump dispatch (fn_800CAF78), then Dash's run transition or
    // Run's turn/brake logic (cmd_vars[0], the script's run flag set by the
    // animation's set-cmd-var command, `ftaction.c:462`, modeled here by
    // `dash_run_frame`, then fn_800CA5F0). `ftCo_Run_IASA` reaches
    // `ftCo_800DE9D8` at the same relative position (right before its own
    // `fn_800CAF78` jump check) but has no friction tail of its own, so only
    // a taunt fired while `f.action == Action::Dash` applies it.
    let taunt_from_dash = f.action == Action::Dash;
    if super::taunt::try_taunt(f, data, input)? {
        if taunt_from_dash {
            apply_transition_friction(f, dash_rules);
        }
        return Ok(true);
    }
    if let Some(source) = super::locomotion::jump_input(f, p, input, true) {
        f.short_hop = false;
        f.locomotion.jump_input = source;
        super::locomotion::enter(f, Action::JumpSquat);
        return Ok(true);
    }
    if f.action == Action::Dash {
        if f.action_frame >= p.dash_run_frame && input.stick[0] * f.facing >= p.run_threshold {
            super::locomotion::enter_run(f, 0.0);
            return Ok(true);
        }
        return Ok(false);
    }
    // ftCo_Run_IASA (ftCo_Run.c:125-126): while run.x0 > 0.0 the RunTurn and
    // RunBrake entry checks below are skipped entirely.
    if f.locomotion.run_lockout > 0.0 {
        return Ok(false);
    }
    if input.stick[0] * f.facing <= p.turn_threshold {
        super::locomotion::start_run_turn(f, 0);
        return Ok(true);
    }
    if input.stick[0].abs() < p.run_threshold {
        super::locomotion::enter(f, Action::RunBrake);
        f.locomotion.run_brake_frames = p.run_brake_max_frames;
        f.locomotion.run_brake_frozen = false;
        return Ok(true);
    }
    Ok(false)
}

/// Entry point called from `simulation::update_actions` before
/// `grab::update_actions`, whenever `rules.dash` is supplied.
pub(crate) fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    input: Controller,
) -> Result<bool, Error> {
    let Some(dash_rules) = rules.dash.as_ref() else {
        return Ok(false);
    };
    match f.action {
        Action::AttackDash => Ok(update_attack_dash(f, data, rules, input)),
        Action::Dash | Action::Run => update_dash_or_run(f, data, dash_rules, rules, input),
        _ => Ok(false),
    }
}
