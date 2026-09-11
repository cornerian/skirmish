//! Jab combos from `ftCo_Attack1.c` and the rapid jab from
//! `ftCo_Attack100.c`. The first jab is the fighter's ordinary `jab` attack;
//! this optional profile adds its decoded script flags, the second and third
//! jabs, the rapid jab's start/loop/end animations and the follow-up windows.
//! Item branches and the Game & Watch, Pikachu/Pichu and Marth overrides are
//! not modeled.
use super::{
    Action, Controller, Error, Fighter,
    data::{Attack, FighterData},
    tilt::Chain,
};
use crate::fighter::jab as math;
use serde::{Deserialize, Serialize};

pub use crate::fighter::jab::Stage;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub attributes: Attributes,
    /// The ordinary `jab` attack's decoded script.
    pub first: Script,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second: Option<JabAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub third: Option<JabAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rapid: Option<RapidJab>,
}

/// `ftCo_DatAttrs` jab fields.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// `jab_2_input_window`: frames after the first jab starts during which
    /// a press queues the second.
    pub second_window: f32,
    /// `jab_3_input_window`.
    pub third_window: f32,
    /// `rapid_jab_window`: logical A presses plus releases that start the
    /// rapid jab.
    pub rapid_window: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// One decoded command sample per pose.
    pub flags: Vec<FrameFlags>,
    /// Per-pose TransN delta consumed by `ft_80084FA8`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_translations: Option<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JabAttack {
    pub attack: Attack,
    #[serde(flatten)]
    pub script: Script,
}

/// Attack100Start, the looping Attack100Loop figatree and Attack100End.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RapidJab {
    pub start: JabAttack,
    pub cycle: JabAttack,
    pub end: JabAttack,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameFlags {
    /// `Fighter::allow_interrupt` after this pose's commands.
    pub allow_interrupt: bool,
    /// `x2218_b1` after this pose's commands: the jab-combo command was
    /// issued on or before it.
    #[serde(default)]
    pub follow_up_ready: bool,
    /// The jab-rapid command's state when it is issued on this pose; the
    /// flag otherwise keeps its value across jabs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rapid: Option<bool>,
    /// Loop only: the throw-flag command that runs the loop's continuation
    /// check on this pose.
    #[serde(default)]
    pub loop_check: bool,
    /// The script's clear-hitboxes command on this pose.
    #[serde(default)]
    pub clear_hits: bool,
}

/// `hitlag_mul` (as the jab timer), `unk_msid`, `mv.co.attack1.x0`,
/// `x2218_b1`, `x2218_b2`, `x1A54` and `mv.co.attack100`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub window: f32,
    pub last: Option<Stage>,
    pub buffered: bool,
    pub follow_up: bool,
    pub rapid: bool,
    pub presses: i32,
    pub loop_started: bool,
    pub loop_input: bool,
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::Jab
            | Action::Attack12
            | Action::Attack13
            | Action::Attack100Start
            | Action::Attack100Loop
            | Action::Attack100End
    )
}

/// Actions that keep the jab timer across `Fighter_ChangeMotionState`
/// (motion ids 14..=17: Wait and the walks).
pub(crate) fn keeps_window(action: Action) -> bool {
    matches!(action, Action::Wait | Action::Walk)
}

pub(crate) fn validate(
    parameters: &Parameters,
    fighter: &FighterData,
    staling: bool,
) -> Result<(), Error> {
    let attributes = &parameters.attributes;
    if [attributes.second_window, attributes.third_window]
        .into_iter()
        .any(|window| !window.is_finite() || !(0.0..=1_000_000.0).contains(&window))
        || attributes.rapid_window < 0
    {
        return Err(Error::Data("invalid jab attributes".into()));
    }
    if fighter.locomotion.is_none() {
        return Err(Error::Data(
            "jab combos require locomotion parameters for their input chains".into(),
        ));
    }
    validate_script(&parameters.first, fighter.jab.frames.len(), false)?;
    let mut rapid_possible = parameters.first.flags.iter().any(|f| f.rapid == Some(true));
    let mut follow_up = parameters.first.flags.iter().any(|f| f.follow_up_ready);
    for (attack, next_follow_up) in [
        (parameters.second.as_ref(), parameters.third.is_some()),
        (parameters.third.as_ref(), false),
    ] {
        let Some(attack) = attack else {
            if follow_up {
                return Err(Error::Data(
                    "a jab with the combo flag needs its follow-up jab".into(),
                ));
            }
            break;
        };
        validate_script(&attack.script, attack.attack.frames.len(), false)?;
        rapid_possible |= attack.script.flags.iter().any(|f| f.rapid == Some(true));
        follow_up = attack.script.flags.iter().any(|f| f.follow_up_ready);
        if follow_up && !next_follow_up {
            return Err(Error::Data(
                "a jab with the combo flag needs its follow-up jab".into(),
            ));
        }
    }
    match &parameters.rapid {
        Some(rapid) => {
            validate_script(&rapid.start.script, rapid.start.attack.frames.len(), false)?;
            validate_script(&rapid.cycle.script, rapid.cycle.attack.frames.len(), true)?;
            validate_script(&rapid.end.script, rapid.end.attack.frames.len(), false)?;
            if !rapid.cycle.script.flags.iter().any(|f| f.loop_check) {
                return Err(Error::Data(
                    "the rapid-jab loop needs its continuation check".into(),
                ));
            }
            if staling
                && [&rapid.cycle, &rapid.end]
                    .into_iter()
                    .any(|attack| attack.attack.move_id != rapid.start.attack.move_id)
            {
                return Err(Error::Data(
                    "rapid-jab animations share one native move identity".into(),
                ));
            }
        }
        None if rapid_possible => {
            return Err(Error::Data(
                "a jab with the rapid flag needs the rapid jab".into(),
            ));
        }
        None => {}
    }
    Ok(())
}

fn validate_script(script: &Script, frames: usize, cycle: bool) -> Result<(), Error> {
    if script.flags.len() != frames || frames == 0 {
        return Err(Error::Data(
            "jab command flags must cover every attack pose".into(),
        ));
    }
    if !cycle && script.flags.iter().any(|f| f.loop_check) {
        return Err(Error::Data(
            "only the rapid-jab loop runs the continuation check".into(),
        ));
    }
    if let Some(roots) = &script.root_translations
        && (roots.len() != frames
            || roots
                .iter()
                .any(|root| !root.is_finite() || root.abs() > 1_000_000.0))
    {
        return Err(Error::Data(
            "jab root motion must supply one finite delta per pose".into(),
        ));
    }
    Ok(())
}

/// Every supplied jab attack beyond the ordinary first jab.
pub(crate) fn attacks(parameters: &Parameters) -> impl Iterator<Item = &Attack> {
    parameters
        .second
        .iter()
        .chain(parameters.third.iter())
        .chain(
            parameters
                .rapid
                .iter()
                .flat_map(|rapid| [&rapid.start, &rapid.cycle, &rapid.end]),
        )
        .map(|attack| &attack.attack)
}

pub(crate) fn attack(parameters: &Parameters, action: Action) -> Option<&Attack> {
    match action {
        Action::Attack12 => parameters.second.as_ref().map(|a| &a.attack),
        Action::Attack13 => parameters.third.as_ref().map(|a| &a.attack),
        Action::Attack100Start => parameters.rapid.as_ref().map(|r| &r.start.attack),
        Action::Attack100Loop => parameters.rapid.as_ref().map(|r| &r.cycle.attack),
        Action::Attack100End => parameters.rapid.as_ref().map(|r| &r.end.attack),
        _ => None,
    }
}

fn script(parameters: &Parameters, action: Action) -> Option<&Script> {
    match action {
        Action::Jab => Some(&parameters.first),
        Action::Attack12 => parameters.second.as_ref().map(|a| &a.script),
        Action::Attack13 => parameters.third.as_ref().map(|a| &a.script),
        Action::Attack100Start => parameters.rapid.as_ref().map(|r| &r.start.script),
        Action::Attack100Loop => parameters.rapid.as_ref().map(|r| &r.cycle.script),
        Action::Attack100End => parameters.rapid.as_ref().map(|r| &r.end.script),
        _ => None,
    }
}

fn current<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a Script> {
    script(data.jab_combo.as_ref()?, fighter.action)
}

fn flags(fighter: &Fighter, data: &FighterData) -> Option<FrameFlags> {
    current(fighter, data)?
        .flags
        .get(fighter.action_frame as usize)
        .copied()
}

/// The chain an interruptible jab exposes on this frame.
pub(crate) fn interrupt_chain(fighter: &Fighter, data: &FighterData) -> Option<Chain> {
    if !fighter.grounded || !flags(fighter, data)?.allow_interrupt {
        return None;
    }
    match fighter.action {
        Action::Jab | Action::Attack12 => Some(Chain::Jab),
        Action::Attack13 => Some(Chain::Wait),
        _ => None,
    }
}

/// `ft_80084FA8`'s TransN target for a jab sample, if supplied.
pub(crate) fn ground_target_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    let roots = current(fighter, data)?.root_translations.as_ref()?;
    Some(roots.get(fighter.action_frame as usize)? * fighter.facing)
}

fn a_released(fighter: &Fighter, input: Controller) -> bool {
    super::smash::logical_a(fighter.previous_input.buttons)
        && !super::smash::logical_a(input.buttons)
}

/// The pose's script commands: the combo flag sample, the rapid command's
/// state and the clear-hitboxes command.
fn apply_flags(fighter: &mut Fighter, flags: FrameFlags) {
    fighter.jab.follow_up = flags.follow_up_ready;
    if let Some(rapid) = flags.rapid {
        fighter.jab.rapid = rapid;
    }
    if flags.clear_hits {
        fighter.hit_groups = 0;
    }
}

/// Entry runs the entry pose's commands (`ftAnim_8006EBA4` in
/// `checkAttack11` and `ftCo_800D6B00`; the next animation update for
/// `doAttack12Normal` and `doAttack13`, before any callback reads them).
fn enter_pose(fighter: &mut Fighter, parameters: &Parameters, action: Action) {
    super::simulation::enter(fighter, action);
    if let Some(flags) = script(parameters, action).and_then(|s| s.flags.first().copied()) {
        apply_flags(fighter, flags);
    }
}

/// `checkAttack11`: the first jab resets the combo bookkeeping.
fn start_first(fighter: &mut Fighter, data: &FighterData) {
    let Some(parameters) = data.jab_combo.as_ref() else {
        super::simulation::enter(fighter, Action::Jab);
        return;
    };
    super::simulation::enter(fighter, Action::Jab);
    fighter.jab = State {
        window: parameters.attributes.second_window,
        last: Some(Stage::First),
        ..State::default()
    };
    if let Some(flags) = parameters.first.flags.first().copied() {
        apply_flags(fighter, flags);
    }
}

/// `doAttack12Normal`.
fn start_second(fighter: &mut Fighter, parameters: &Parameters) {
    fighter.jab.follow_up = false;
    enter_pose(fighter, parameters, Action::Attack12);
    fighter.jab.window = parameters.attributes.third_window;
    fighter.jab.last = Some(Stage::Second);
    fighter.jab.buffered = false;
}

/// `doAttack13`.
fn start_third(fighter: &mut Fighter, parameters: &Parameters) {
    fighter.jab.follow_up = false;
    enter_pose(fighter, parameters, Action::Attack13);
}

/// `ftCo_800D6B00`.
fn start_rapid(fighter: &mut Fighter, parameters: &Parameters) {
    fighter.jab.loop_started = false;
    fighter.jab.loop_input = false;
    enter_pose(fighter, parameters, Action::Attack100Start);
}

/// `ftCo_Attack1_CheckInput` with the logical A pressed, reached from a
/// Wait-family chain. Returns whether a jab started.
pub(crate) fn press(fighter: &mut Fighter, data: &FighterData) -> bool {
    let Some(parameters) = data.jab_combo.as_ref() else {
        super::simulation::enter(fighter, Action::Jab);
        return true;
    };
    match math::wait_press(fighter.jab.window, fighter.jab.follow_up, fighter.jab.last) {
        Some(Stage::First) => start_first(fighter, data),
        Some(Stage::Second) => start_second(fighter, parameters),
        Some(Stage::Third) => start_third(fighter, parameters),
        None => {
            math::decay(&mut fighter.jab.window);
            return false;
        }
    }
    true
}

/// `ftCo_Attack1_CheckInput` without a press.
pub(crate) fn decay(fighter: &mut Fighter) {
    math::decay(&mut fighter.jab.window);
}

/// `ftCo_Attack_800D6A50` then `checkAttack12`/`checkAttack13` on every
/// frame of the first three jabs, and `ftCo_Attack100Loop_IASA`. Returns
/// whether a transition started.
pub(crate) fn update_actions(fighter: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let Some(parameters) = data.jab_combo.as_ref() else {
        return false;
    };
    let pressed = super::smash::a_pressed(fighter, input);
    let released = a_released(fighter, input);
    match fighter.action {
        Action::Jab | Action::Attack12 | Action::Attack13 => {
            if math::rapid_count(
                &mut fighter.jab.presses,
                pressed,
                released,
                parameters.attributes.rapid_window,
                fighter.jab.rapid,
            ) {
                start_rapid(fighter, parameters);
                return true;
            }
            let follow_up = fighter.jab.follow_up;
            let (window, buffered) = (&mut fighter.jab.window, &mut fighter.jab.buffered);
            match fighter.action {
                Action::Jab if math::buffer_follow_up(window, pressed, buffered, follow_up) => {
                    start_second(fighter, parameters);
                    true
                }
                Action::Attack12
                    if math::buffer_follow_up(window, pressed, buffered, follow_up) =>
                {
                    start_third(fighter, parameters);
                    true
                }
                _ => false,
            }
        }
        Action::Attack100Loop => {
            if pressed || released {
                fighter.jab.loop_input = true;
            }
            false
        }
        _ => false,
    }
}

/// The script samples, the loop's restart and continuation check, and the
/// animation callbacks' endings.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if !owns_action(fighter.action) {
        return Ok(());
    }
    let Some(parameters) = data.jab_combo.as_ref() else {
        if fighter.action == Action::Jab && fighter.action_frame as usize >= data.jab.frames.len() {
            finish(fighter);
        }
        return Ok(());
    };
    let script = script(parameters, fighter.action)
        .ok_or_else(|| Error::Data("jab action without its supplied animation".into()))?;
    let frames = script.flags.len();
    if fighter.action == Action::Attack100Loop {
        // The loop figatree wraps; frame zero re-creates its hitboxes and
        // runs ft_800892A0 and ft_80089824.
        if fighter.action_frame as usize >= frames {
            fighter.action_frame = 0;
        }
        if fighter.action_frame == 0 {
            fighter.jab.loop_started = true;
            fighter.hit_groups = 0;
            crate::fighter::action_instance::queue(&mut fighter.action_instance, 0);
            crate::fighter::action_instance::queue(&mut fighter.action_instance, 0);
            super::staling::restart_identity(fighter);
        }
    } else if fighter.action_frame as usize >= frames {
        if fighter.action == Action::Attack100Start {
            super::simulation::enter(fighter, Action::Attack100Loop);
            return update_animation(fighter, data);
        }
        finish(fighter);
        return Ok(());
    }
    let flags = script.flags[fighter.action_frame as usize];
    apply_flags(fighter, flags);
    if flags.loop_check
        && fighter.action == Action::Attack100Loop
        && math::loop_check(fighter.jab.loop_started, &mut fighter.jab.loop_input)
    {
        super::simulation::enter(fighter, Action::Attack100End);
    }
    Ok(())
}

/// `ft_8008A2BC`.
fn finish(fighter: &mut Fighter) {
    super::simulation::enter(
        fighter,
        if fighter.grounded {
            Action::Wait
        } else {
            Action::Fall
        },
    );
}
