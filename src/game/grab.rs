//! Bone-driven catch, paired capture, pummel, and throw scheduling.
//!
//! Resources supply every pose, collision volume, attachment, timer, and hit
//! coefficient. The relationship is headless physics state and is serialized
//! with match observations and checkpoints.

use super::{
    Action, Controller, Error, Event, Fighter, State as MatchState,
    data::{Bone, Capsule, FighterData, Hitbox, MatchData},
    simulation,
};
use crate::{
    collision::{bones::BoneCapsule, shield as body_collision},
    fighter::{
        combat::{self, Capsule as WorldCapsule},
        grab as input,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub horizontal_threshold: f32,
    pub up_threshold: f32,
    /// Signed negative common-data threshold.
    pub down_threshold: f32,
    /// Common x37C multiplier for victim-weight-dependent throw animation.
    pub throw_weight_scale: f32,
    /// Common x3C4 rise threshold, scaled by the victim's root bone Y scale.
    pub capture_lift_threshold: f32,
    pub escape: EscapeRules,
    /// Grabs dispatched from a raised shield; absent means unsupported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_grab: Option<ShieldGrabRules>,
}

/// Common data for `ftCo_Catch_CheckInput` and `ftCo_800D8B9C` from guard
/// states.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldGrabRules {
    /// `x68`: guard `x24` frames armed by `ftCo_80091B9C` when Run, or Dash
    /// past `dash_buffer_frame_limit`, raises the shield.
    pub dash_buffer_frames: f32,
    /// `x4C`: Dash animation frame that must be exceeded to arm the buffer.
    pub dash_buffer_frame_limit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscapeRules {
    pub timer_base: f32,
    pub timer_percent_scale: f32,
    pub timer_decrement: f32,
    pub mash_penalty: f32,
    pub stick_threshold: f32,
    pub release_speed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub catch: Catch,
    pub catch_dash: Catch,
    pub attachment: Attachment,
    pub pummel: Pummel,
    /// Complete victim physics poses for the two ordinary pummel reactions.
    pub capture_damage: CaptureDamage,
    pub escape: Escape,
    pub throws: Throws,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureDamage {
    pub high: Vec<Vec<Bone>>,
    pub low: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catch {
    /// One complete physics pose and its active catch volumes per frame.
    pub frames: Vec<CatchFrame>,
    pub pull_frames: u32,
    pub grounded_targets_only: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchFrame {
    pub bones: Vec<Bone>,
    pub grabboxes: Vec<Capsule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub holder_bone: usize,
    pub holder_point: [f32; 3],
    pub victim_bone: usize,
    pub victim_point: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pummel {
    /// Native move-table identity. Sentinel 1 is exempt from stale damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// One complete holder physics pose per frame.
    pub poses: Vec<Vec<Bone>>,
    /// The single captured-victim damage callback. Zero is not observable in
    /// this scheduler because input dispatch follows priority-1 callbacks.
    pub hit_frame: u32,
    pub damage: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escape {
    /// This fighter's holder-side CatchCut physics poses.
    pub catch_cut_poses: Vec<Vec<Bone>>,
    /// This fighter's victim-side CaptureCut physics poses.
    pub capture_cut_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throw {
    /// Native move-table identity. Sentinel 1 is exempt from stale damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// The fighter's per-direction weight-independent throw mask.
    #[serde(default)]
    pub weight_independent: bool,
    /// One complete holder physics pose per frame.
    pub poses: Vec<Vec<Bone>>,
    /// Scripted release event. Zero is excluded so entry is observable.
    pub release_frame: u32,
    pub hit: ThrowHit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throws {
    pub forward: Throw,
    pub backward: Throw,
    pub up: Throw,
    pub down: Throw,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrowHit {
    pub damage: u32,
    pub angle_degrees: f32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub victim: Option<usize>,
    pub captor: Option<usize>,
    pub pummel_hit: bool,
    pub escape_timer: f32,
    pub mash: input::MashState,
    /// Paired HSD animation time and rate while a throw still owns its victim.
    pub throw_elapsed: f32,
    pub throw_rate: f32,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
    staling: bool,
) -> Result<(), Error> {
    if let Some(shield_grab) = &rules.shield_grab
        && (!shield_grab.dash_buffer_frames.is_finite()
            || shield_grab.dash_buffer_frames <= 0.0
            || shield_grab.dash_buffer_frames > 1_000_000.0
            || !shield_grab.dash_buffer_frame_limit.is_finite()
            || !(0.0..=1_000_000.0).contains(&shield_grab.dash_buffer_frame_limit))
    {
        return Err(Error::Data("invalid shield-grab rules".into()));
    }
    if ![
        rules.horizontal_threshold,
        rules.up_threshold,
        rules.down_threshold,
        rules.throw_weight_scale,
        rules.capture_lift_threshold,
        rules.escape.timer_base,
        rules.escape.timer_percent_scale,
        rules.escape.timer_decrement,
        rules.escape.mash_penalty,
        rules.escape.stick_threshold,
        rules.escape.release_speed,
    ]
    .into_iter()
    .all(f32::is_finite)
        || !(0.0..=1.0).contains(&rules.horizontal_threshold)
        || rules.horizontal_threshold == 0.0
        || !(0.0..=1.0).contains(&rules.up_threshold)
        || rules.up_threshold == 0.0
        || !(-1.0..0.0).contains(&rules.down_threshold)
        || !(0.0..1_000_000.0).contains(&rules.throw_weight_scale)
        || !(0.0..1_000_000.0).contains(&rules.capture_lift_threshold)
        || rules.capture_lift_threshold == 0.0
        || !(0.0..1_000_000.0).contains(&rules.escape.timer_base)
        || !(0.0..1_000.0).contains(&rules.escape.timer_percent_scale)
        || !(0.0..1_000_000.0).contains(&rules.escape.timer_decrement)
        || !(0.0..1_000_000.0).contains(&rules.escape.mash_penalty)
        || !(0.0..=1.0).contains(&rules.escape.stick_threshold)
        || rules.escape.stick_threshold == 0.0
        || !(0.0..=1_000_000.0).contains(&rules.escape.release_speed)
        || rules.escape.timer_base + 999.0 * rules.escape.timer_percent_scale >= 1_000_000.0
        || parameters.attachment.holder_bone >= fighter.bones.len()
        || parameters.attachment.victim_bone >= fighter.bones.len()
        || parameters
            .attachment
            .holder_point
            .into_iter()
            .chain(parameters.attachment.victim_point)
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data("invalid explicit grab parameters".into()));
    }
    for catch in [&parameters.catch, &parameters.catch_dash] {
        validate_catch(catch, fighter)?;
    }
    let pummel = &parameters.pummel;
    if pummel.poses.is_empty()
        || pummel.poses.len() > 4096
        || pummel.hit_frame == 0
        || pummel.hit_frame as usize >= pummel.poses.len()
        || pummel.damage > 999
        || (staling && pummel.move_id.is_none_or(|id| id == 0))
    {
        return Err(Error::Data("invalid explicit pummel parameters".into()));
    }
    for pose in &pummel.poses {
        super::validation::validate_animation_pose(pose, fighter)?;
    }
    for poses in [
        &parameters.capture_damage.high,
        &parameters.capture_damage.low,
    ] {
        if poses.is_empty() || poses.len() > 4096 {
            return Err(Error::Data(
                "invalid explicit capture-damage animation".into(),
            ));
        }
        for pose in poses {
            super::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    for poses in [
        &parameters.escape.catch_cut_poses,
        &parameters.escape.capture_cut_poses,
    ] {
        if poses.is_empty() || poses.len() > 4096 {
            return Err(Error::Data("invalid explicit grab-escape animation".into()));
        }
        for pose in poses {
            super::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    for throw in [
        &parameters.throws.forward,
        &parameters.throws.backward,
        &parameters.throws.up,
        &parameters.throws.down,
    ] {
        if throw.poses.is_empty()
            || throw.poses.len() > 4096
            || throw.release_frame == 0
            || throw.release_frame as usize >= throw.poses.len()
            || throw.hit.damage > 999
            || throw.hit.growth > 1000
            || throw.hit.fixed > 1000
            || throw.hit.base > 1000
            || !(0.0..=361.0).contains(&throw.hit.angle_degrees)
            || throw.hit.angle_degrees.fract() != 0.0
            || (staling && throw.move_id.is_none_or(|id| id == 0))
        {
            return Err(Error::Data("invalid explicit throw parameters".into()));
        }
        for pose in &throw.poses {
            super::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

fn validate_catch(catch: &Catch, fighter: &FighterData) -> Result<(), Error> {
    if catch.frames.is_empty()
        || catch.frames.len() > 4096
        || catch.pull_frames == 0
        || catch.pull_frames >= 1_000_000
    {
        return Err(Error::Data("invalid explicit catch parameters".into()));
    }
    let mut active = false;
    for frame in &catch.frames {
        let pose = super::validation::validate_animation_pose(&frame.bones, fighter)?;
        if frame.grabboxes.len() > 4 {
            return Err(Error::Data("at most four grabboxes per frame".into()));
        }
        active |= !frame.grabboxes.is_empty();
        for grabbox in &frame.grabboxes {
            if grabbox.bone >= frame.bones.len()
                || grabbox
                    .start
                    .into_iter()
                    .chain(grabbox.end)
                    .chain([grabbox.radius])
                    .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value.abs()))
                || grabbox.radius < 0.0
            {
                return Err(Error::Data("invalid grabbox".into()));
            }
            grabbox
                .physics()
                .transform(&pose, 1.0)
                .map_err(|error| Error::Data(error.to_string()))?;
        }
    }
    if !active {
        return Err(Error::Data("catch requires an active grabbox frame".into()));
    }
    Ok(())
}

pub(crate) fn valid_relationship(fighters: &[Fighter; 2], player: usize) -> bool {
    let other = 1 - player;
    let fighter = &fighters[player];
    let partner = &fighters[other];
    if !fighter.grab.escape_timer.is_finite()
        || fighter
            .grab
            .mash
            .axes
            .into_iter()
            .any(|axis| !(-1..=1).contains(&axis))
        || !fighter.grab.throw_elapsed.is_finite()
        || !fighter.grab.throw_rate.is_finite()
        || fighter.grab.throw_elapsed < 0.0
        || fighter.grab.throw_rate < 0.0
        || fighter.grab.pummel_hit
            && (fighter.grab.victim.is_none() || fighter.action != Action::CatchAttack)
    {
        return false;
    }
    match (fighter.grab.victim, fighter.grab.captor) {
        (None, None) => fighter.grab == State::default(),
        (Some(victim), None) => {
            victim == other
                && fighter.grab.escape_timer == 0.0
                && fighter.grab.mash == input::MashState::default()
                && partner.grab.victim.is_none()
                && partner.grab.captor == Some(player)
                && !partner.grab.pummel_hit
                && valid_throw_clock(fighter, partner)
                && pair_actions(fighter.action, partner.action)
        }
        (None, Some(holder)) => {
            holder == other
                && partner.grab.victim == Some(player)
                && partner.grab.captor.is_none()
                && valid_throw_clock(partner, fighter)
                && pair_actions(partner.action, fighter.action)
        }
        (Some(_), Some(_)) => false,
    }
}

fn valid_throw_clock(holder: &Fighter, victim: &Fighter) -> bool {
    if matches!(
        holder.action,
        Action::ThrowF | Action::ThrowB | Action::ThrowHi | Action::ThrowLw
    ) {
        holder.grab.throw_rate > 0.0
            && holder.grab.throw_rate.to_bits() == victim.grab.throw_rate.to_bits()
            && holder.grab.throw_elapsed.to_bits() == victim.grab.throw_elapsed.to_bits()
    } else {
        holder.grab.throw_rate == 0.0
            && holder.grab.throw_elapsed == 0.0
            && victim.grab.throw_rate == 0.0
            && victim.grab.throw_elapsed == 0.0
    }
}

fn pair_actions(holder: Action, victim: Action) -> bool {
    matches!(
        (holder, victim),
        (
            Action::CatchPull | Action::CatchDashPull,
            Action::CapturePulledHi | Action::CapturePulledLw
        ) | (
            Action::CatchWait | Action::CatchAttack,
            Action::CaptureWaitHi | Action::CaptureWaitLw
        ) | (
            Action::CatchWait | Action::CatchAttack,
            Action::CaptureDamageHi | Action::CaptureDamageLw
        ) | (Action::ThrowF, Action::ThrownF)
            | (Action::ThrowB, Action::ThrownB)
            | (Action::ThrowHi, Action::ThrownHi)
            | (Action::ThrowLw, Action::ThrownLw)
    )
}

fn capture_pulled_action(grounded: bool) -> Action {
    if grounded {
        Action::CapturePulledLw
    } else {
        Action::CapturePulledHi
    }
}

fn capture_wait_action(action: Action) -> Option<Action> {
    match action {
        Action::CapturePulledHi | Action::CaptureWaitHi | Action::CaptureDamageHi => {
            Some(Action::CaptureWaitHi)
        }
        Action::CapturePulledLw | Action::CaptureWaitLw | Action::CaptureDamageLw => {
            Some(Action::CaptureWaitLw)
        }
        _ => None,
    }
}

fn capture_damage_action(action: Action) -> Option<Action> {
    match action {
        Action::CapturePulledHi | Action::CaptureWaitHi | Action::CaptureDamageHi => {
            Some(Action::CaptureDamageHi)
        }
        Action::CapturePulledLw | Action::CaptureWaitLw | Action::CaptureDamageLw => {
            Some(Action::CaptureDamageLw)
        }
        _ => None,
    }
}

fn capture_damage_poses(parameters: &Parameters, action: Action) -> Option<&[Vec<Bone>]> {
    match action {
        Action::CaptureDamageHi => Some(&parameters.capture_damage.high),
        Action::CaptureDamageLw => Some(&parameters.capture_damage.low),
        _ => None,
    }
}

fn capture_waiting(action: Action) -> bool {
    matches!(action, Action::CaptureWaitHi | Action::CaptureWaitLw)
}

fn captured(action: Action) -> bool {
    matches!(
        action,
        Action::CapturePulledHi
            | Action::CaptureWaitHi
            | Action::CaptureDamageHi
            | Action::CapturePulledLw
            | Action::CaptureWaitLw
            | Action::CaptureDamageLw
    )
}

pub(crate) fn transfer_capture_family(fighter: &mut Fighter, airborne: bool) -> bool {
    let action = match (fighter.action, airborne) {
        (Action::CapturePulledLw, true) => Action::CapturePulledHi,
        (Action::CaptureWaitLw, true) => Action::CaptureWaitHi,
        (Action::CaptureDamageLw, true) => Action::CaptureDamageHi,
        (Action::CapturePulledHi, false) => Action::CapturePulledLw,
        (Action::CaptureWaitHi, false) => Action::CaptureWaitLw,
        (Action::CaptureDamageHi, false) => Action::CaptureDamageLw,
        _ => return false,
    };
    let frame = fighter.action_frame;
    simulation::enter(fighter, action);
    fighter.action_frame = frame;
    if airborne {
        fighter.grounded = false;
        fighter.ground_line = None;
        fighter.ground_knockback = 0.0;
        fighter.ground_velocity = 0.0;
        fighter.fast_fall = false;
        fighter.locomotion.jumps_used = fighter.locomotion.jumps_used.max(1);
    }
    true
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::Catch
            | Action::CatchDash
            | Action::CatchPull
            | Action::CatchDashPull
            | Action::CatchWait
            | Action::CatchAttack
            | Action::CatchCut
            | Action::ThrowF
            | Action::ThrowB
            | Action::ThrowHi
            | Action::ThrowLw
            | Action::CapturePulledHi
            | Action::CaptureWaitHi
            | Action::CaptureDamageHi
            | Action::CapturePulledLw
            | Action::CaptureWaitLw
            | Action::CaptureDamageLw
            | Action::CaptureCut
            | Action::ThrownF
            | Action::ThrownB
            | Action::ThrownHi
            | Action::ThrownLw
    )
}

pub(crate) fn holder_action(action: Action) -> bool {
    matches!(
        action,
        Action::CatchPull
            | Action::CatchDashPull
            | Action::CatchWait
            | Action::CatchAttack
            | Action::ThrowF
            | Action::ThrowB
            | Action::ThrowHi
            | Action::ThrowLw
    )
}

pub(crate) fn update_fighter_animation(fighter: &mut Fighter, data: &FighterData) -> bool {
    let Some(parameters) = &data.grab else {
        return false;
    };
    if fighter.action == Action::CatchAttack
        && fighter.action_frame as usize >= parameters.pummel.poses.len()
    {
        fighter.grab.pummel_hit = false;
        simulation::enter(fighter, Action::CatchWait);
        return true;
    }
    if let Some(poses) = capture_damage_poses(parameters, fighter.action)
        && fighter.action_frame as usize >= poses.len()
    {
        simulation::enter(fighter, capture_wait_action(fighter.action).unwrap());
        return true;
    }
    let cut_complete = match fighter.action {
        Action::CatchCut => {
            fighter.action_frame as usize >= parameters.escape.catch_cut_poses.len()
        }
        Action::CaptureCut => {
            fighter.action_frame as usize >= parameters.escape.capture_cut_poses.len()
        }
        _ => false,
    };
    if cut_complete {
        simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
        return true;
    }
    let complete = match fighter.action {
        Action::Catch | Action::CatchDash => catch_for_action(parameters, fighter.action)
            .is_some_and(|catch| fighter.action_frame as usize >= catch.frames.len()),
        action
            if throw_for_action(&parameters.throws, action)
                .is_some_and(|throw| fighter.action_frame as usize >= throw.poses.len()) =>
        {
            true
        }
        _ => false,
    };
    if complete {
        simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
    }
    complete
}

pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    controller: Controller,
) -> bool {
    let pressed = controller.buttons & !fighter.previous_input.buttons;
    if owns_action(fighter.action) {
        if fighter.action == Action::CatchWait {
            if input::pummel_pressed(pressed) {
                fighter.grab.pummel_hit = false;
                simulation::enter(fighter, Action::CatchAttack);
                return true;
            }
            let Some(rules) = rules else { return true };
            let action = input::direction(
                controller.stick,
                fighter.previous_input.stick,
                controller.cstick,
                fighter.previous_input.cstick,
                fighter.facing,
                [
                    rules.horizontal_threshold,
                    rules.up_threshold,
                    rules.down_threshold,
                ],
            )
            .map(|direction| match direction {
                input::ThrowDirection::Forward => Action::ThrowF,
                input::ThrowDirection::Backward => Action::ThrowB,
                input::ThrowDirection::Up => Action::ThrowHi,
                input::ThrowDirection::Down => Action::ThrowLw,
            });
            if let Some(action) = action {
                simulation::enter(fighter, action);
            }
        }
        return true;
    }
    // ftCo_Catch_CheckInput and ftCo_800D8A38 need a fresh logical A press
    // with the logical shoulder held; Fighter_procInput folds physical Z into
    // both bits.
    let a_pressed = logical_a(controller.buttons) && !logical_a(fighter.previous_input.buttons);
    let shoulder_held = controller.shield_held() || controller.buttons & super::BUTTON_Z != 0;
    if fighter.grounded && a_pressed && shoulder_held && data.grab.is_some() && rules.is_some() {
        let action = match fighter.action {
            Action::Dash | Action::Run => Some(Action::CatchDash),
            Action::Turn => {
                // Turn_IASA temporarily applies facing_after before checking Catch.
                if !fighter.locomotion.turn_has_turned {
                    fighter.facing = -fighter.facing;
                }
                Some(Action::Catch)
            }
            // ftCo_Catch_CheckInput runs from Wait, Walk, Squat and KneeBend;
            // SquatWait and SquatRv chains never reach it.
            Action::Wait | Action::Walk | Action::Squat | Action::JumpSquat => Some(Action::Catch),
            // ftCo_AppealS_IASA reaches ftCo_Catch_CheckInput too.
            _ if matches!(
                super::tilt::interrupt_chain(fighter, data),
                Some(super::tilt::Chain::Wait) | Some(super::tilt::Chain::Taunt)
            ) =>
            {
                Some(Action::Catch)
            }
            _ => None,
        };
        if let Some(action) = action {
            simulation::enter(fighter, action);
            return true;
        }
    }
    false
}

/// `ftCo_80091B9C` arms the guard `x24` buffer when Run, or Dash after the
/// `x4C` frame, raises a shield; `ftCo_800923B4` clears it for other entries.
/// Any stale union residue on a powershield raised elsewhere is not modeled.
pub(crate) fn shield_entry_buffer(fighter: &Fighter, rules: Option<&Rules>) -> f32 {
    let Some(shield_grab) = rules.and_then(|rules| rules.shield_grab.as_ref()) else {
        return 0.0;
    };
    match fighter.action {
        Action::Run => shield_grab.dash_buffer_frames,
        Action::Dash if fighter.action_frame as f32 > shield_grab.dash_buffer_frame_limit => {
            shield_grab.dash_buffer_frames
        }
        _ => 0.0,
    }
}

fn logical_a(buttons: u16) -> bool {
    buttons & (super::BUTTON_A | super::BUTTON_Z) != 0
}

/// `ftCo_800D8B9C` (GuardOn/GuardReflect only) followed by
/// `ftCo_Catch_CheckInput`, at their guard IASA positions after the escapes.
pub(crate) fn update_shield_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    controller: Controller,
) -> bool {
    if data.grab.is_none() || rules.is_none_or(|rules| rules.shield_grab.is_none()) {
        return false;
    }
    // Fighter_procInput folds physical Z into logical A plus a held shoulder.
    let a_pressed = logical_a(controller.buttons) && !logical_a(fighter.previous_input.buttons);
    let shoulder_held = controller.shield_held() || controller.buttons & super::BUTTON_Z != 0;
    if matches!(fighter.action, Action::GuardOn | Action::GuardReflect)
        && input::dash_shield_grab(a_pressed, &mut fighter.shield.dash_grab_buffer)
    {
        start_shield_catch(fighter, Action::CatchDash);
        return true;
    }
    if input::shield_grab(shoulder_held, a_pressed) {
        start_shield_catch(fighter, Action::Catch);
        return true;
    }
    false
}

/// `ftCo_800D8C54` through an ordinary Fighter_ChangeMotionState.
fn start_shield_catch(fighter: &mut Fighter, action: Action) {
    super::shield::leave_guard(fighter);
    simulation::enter(fighter, action);
}

/// Paired priority-1 transitions and the scripted release event.
pub(crate) fn update_pairs(
    data: &MatchData,
    state: &mut MatchState,
    controllers: [Controller; 2],
    active: [bool; 2],
) -> Result<[bool; 2], Error> {
    let mut frozen = [false; 2];
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        if victim != 1 - holder {
            return Err(Error::Physics("invalid capture relationship".into()));
        }
        let parameters = data.fighters[holder]
            .grab
            .as_ref()
            .ok_or_else(|| Error::Data("capture state requires grab resources".into()))?;
        if active[victim]
            && matches!(
                state.fighters[victim].action,
                Action::CaptureWaitHi
                    | Action::CaptureDamageHi
                    | Action::CaptureWaitLw
                    | Action::CaptureDamageLw
            )
        {
            update_escape(data, state, victim, controllers[victim]);
            if capture_waiting(state.fighters[victim].action)
                && state.fighters[victim].grab.escape_timer <= 0.0
            {
                escape_pair(data, state, holder, victim);
                continue;
            }
        }
        if !active[holder] {
            continue;
        }
        let holder_action = state.fighters[holder].action;
        match holder_action {
            Action::CatchPull
                if state.fighters[holder].action_frame >= parameters.catch.pull_frames =>
            {
                simulation::enter(&mut state.fighters[holder], Action::CatchWait);
                let wait = capture_wait_action(state.fighters[victim].action)
                    .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
                simulation::enter(&mut state.fighters[victim], wait);
            }
            Action::CatchDashPull
                if state.fighters[holder].action_frame >= parameters.catch_dash.pull_frames =>
            {
                simulation::enter(&mut state.fighters[holder], Action::CatchWait);
                let wait = capture_wait_action(state.fighters[victim].action)
                    .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
                simulation::enter(&mut state.fighters[victim], wait);
            }
            Action::CatchAttack
                if state.fighters[holder].action_frame == parameters.pummel.hit_frame
                    && !state.fighters[holder].grab.pummel_hit =>
            {
                state.fighters[holder].grab.pummel_hit = true;
                apply_pummel(data, state, holder, victim, parameters.pummel.damage)?;
                frozen[holder] = true;
                frozen[victim] = true;
            }
            action
                if throw_for_action(&parameters.throws, action).is_some_and(|throw| {
                    state.fighters[holder].action_frame == throw.release_frame
                }) =>
            {
                let throw = throw_for_action(&parameters.throws, action).unwrap();
                let hit = throw.hit;
                let staled = super::staling::hit(
                    &state.fighters[holder].staling,
                    hit.damage,
                    data.rules.staling.as_ref(),
                )?;
                detach(state, holder, victim);
                super::damage::apply_hit(
                    data,
                    state,
                    holder,
                    &Hitbox {
                        clank: false,
                        rebound: false,
                        element: Default::default(),
                        group: 0,
                        bone: 0,
                        center: [0.0; 3],
                        radius: 0.0,
                        damage: hit.damage,
                        shield_damage: 0,
                        angle_degrees: hit.angle_degrees,
                        growth: hit.growth,
                        fixed: hit.fixed,
                        base: hit.base,
                    },
                    staled,
                    crate::fighter::damage::HurtHeight::Middle,
                    super::damage::HitDirection::Throw,
                )?;
                if data.rules.staling.is_some() {
                    state.fighters[holder]
                        .staling
                        .queue
                        .record(staled.identity, false);
                }
                frozen[holder] = true;
                frozen[victim] = true;
            }
            _ => {}
        }
    }
    Ok(frozen)
}

pub(crate) fn synchronize_actions(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        let holder_action = state.fighters[holder].action;
        let victim_action = match holder_action {
            Action::ThrowF => Action::ThrownF,
            Action::ThrowB => Action::ThrownB,
            Action::ThrowHi => Action::ThrownHi,
            Action::ThrowLw => Action::ThrownLw,
            _ => continue,
        };
        if state.fighters[holder].grab.throw_rate == 0.0 {
            let throw = data.fighters[holder]
                .grab
                .as_ref()
                .and_then(|parameters| throw_for_action(&parameters.throws, holder_action))
                .ok_or_else(|| Error::Data("throw state requires grab resources".into()))?;
            let rate = input::throw_animation_rate(
                throw.weight_independent,
                data.fighters[victim].weight,
                data.rules.grab.as_ref().unwrap().throw_weight_scale,
            );
            if !rate.is_finite() || rate <= 0.0 {
                return Err(Error::Data("invalid weight-dependent throw rate".into()));
            }
            state.fighters[holder].grab.throw_rate = rate;
            state.fighters[victim].grab.throw_rate = rate;
        }
        if state.fighters[victim].action != victim_action {
            simulation::enter(&mut state.fighters[victim], victim_action);
        }
    }
    Ok(())
}

/// Advances paired HSD time. Before release, a fast rate stops at the scripted
/// event frame; the release callback restores ordinary rate-one advancement.
pub(crate) fn paired_throw_release(
    data: &MatchData,
    state: &MatchState,
    player: usize,
) -> Option<u32> {
    let holder = state.fighters[player].grab.captor.unwrap_or(player);
    state.fighters[holder].grab.victim?;
    let parameters = data.fighters[holder].grab.as_ref()?;
    throw_for_action(&parameters.throws, state.fighters[holder].action)
        .map(|throw_| throw_.release_frame)
}

pub(crate) fn advance_action_frame(fighter: &mut Fighter, release_frame: Option<u32>) -> bool {
    let Some(release_frame) = release_frame else {
        return false;
    };
    if fighter.grab.throw_rate == 0.0 {
        return false;
    }
    fighter.grab.throw_elapsed += fighter.grab.throw_rate;
    fighter.action_frame = (fighter.grab.throw_elapsed as u32).min(release_frame);
    true
}

pub(crate) fn release_broken_pairs(state: &mut MatchState) {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        if !holder_action(state.fighters[holder].action) {
            detach(state, holder, victim);
            if captured(state.fighters[victim].action)
                || matches!(
                    state.fighters[victim].action,
                    Action::ThrownF | Action::ThrownB | Action::ThrownHi | Action::ThrownLw
                )
            {
                let grounded = state.fighters[victim].grounded;
                simulation::enter(
                    &mut state.fighters[victim],
                    if grounded { Action::Wait } else { Action::Fall },
                );
            }
        }
    }
}

pub(crate) fn break_for_player(state: &mut MatchState, player: usize) {
    if let Some(victim) = state.fighters[player].grab.victim {
        detach(state, player, victim);
        let grounded = state.fighters[victim].grounded;
        simulation::enter(
            &mut state.fighters[victim],
            if grounded { Action::Wait } else { Action::Fall },
        );
    }
    if let Some(holder) = state.fighters[player].grab.captor {
        detach(state, holder, player);
        let grounded = state.fighters[holder].grounded;
        simulation::enter(
            &mut state.fighters[holder],
            if grounded { Action::Wait } else { Action::Fall },
        );
    }
}

pub(crate) fn attach_all(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        if let Some(victim) = state.fighters[holder].grab.victim {
            attach(data, state, holder, victim, true)?;
        }
    }
    Ok(())
}

pub(crate) fn scan(
    data: &MatchData,
    state: &mut MatchState,
    frozen: [bool; 2],
) -> Result<(), Error> {
    for (holder, &holder_frozen) in frozen.iter().enumerate() {
        let victim = 1 - holder;
        let source = &state.fighters[holder];
        let source_action = source.action;
        let target = &state.fighters[victim];
        let Some(parameters) = &data.fighters[holder].grab else {
            continue;
        };
        let Some(catch) = catch_for_action(parameters, source_action) else {
            continue;
        };
        if holder_frozen
            || source.grab != State::default()
            || target.grab != State::default()
            || target.invincibility > 0
            || target.intangibility > 0
            || !target.body_state.accepts_contact()
            || matches!(
                target.action,
                Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
            )
            || catch.grounded_targets_only && !target.grounded
        {
            continue;
        }
        let source_pose = simulation::pose(source, &data.fighters[holder])?;
        let target_pose = simulation::pose(target, &data.fighters[victim])?;
        let frame = catch
            .frames
            .get(source.action_frame as usize)
            .ok_or_else(|| Error::Physics("catch frame is outside supplied animation".into()))?;
        let mut collided = false;
        for grabbox in &frame.grabboxes {
            let grab = grabbox
                .physics()
                .transform(&source_pose, 1.0)
                .map_err(physics)?;
            let grab = WorldCapsule {
                start: grab.start,
                end: grab.end,
                radius: grab.radius,
            };
            for (index, hurtbox) in data.fighters[victim].hurtboxes.iter().enumerate() {
                if !hurtbox.grabbable
                    || !simulation::hurtbox_state(target, &data.fighters[victim], index)?
                        .accepts_contact()
                {
                    continue;
                }
                let hurt = hurtbox
                    .physics()
                    .transform(&target_pose, 1.0)
                    .map_err(physics)?;
                let hurt = WorldCapsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let matrix = target_pose.world_matrix(hurtbox.bone).map_err(physics)?;
                let mut contact = body_collision::Contact::default();
                if body_collision::capsule_matrix(&grab, &hurt, matrix, 3.0, &mut contact)
                    .map_err(physics)?
                {
                    collided = true;
                    break;
                }
            }
            if collided {
                break;
            }
        }
        if collided {
            let escape = &data.rules.grab.as_ref().unwrap().escape;
            state.fighters[holder].grab.victim = Some(victim);
            state.fighters[victim].grab.captor = Some(holder);
            state.fighters[victim].grab.escape_timer =
                escape.timer_base + state.fighters[victim].percent * escape.timer_percent_scale;
            state.fighters[victim].grab.mash = input::MashState::default();
            state.fighters[victim].facing = state.fighters[holder].facing;
            state.fighters[victim].velocity = [0.0; 2];
            state.fighters[victim].knockback = [0.0; 2];
            state.fighters[victim].ground_knockback = 0.0;
            state.fighters[victim].ground_velocity = 0.0;
            let pull = if source_action == Action::CatchDash {
                Action::CatchDashPull
            } else {
                Action::CatchPull
            };
            let victim_action = capture_pulled_action(state.fighters[victim].grounded);
            simulation::enter(&mut state.fighters[holder], pull);
            simulation::enter(&mut state.fighters[victim], victim_action);
            state.events.push(Event::Grabbed { holder, victim });
            attach(data, state, holder, victim, true)?;
        }
    }
    Ok(())
}

pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let parameters = data.grab.as_ref()?;
    match fighter.action {
        Action::Catch | Action::CatchDash => catch_for_action(parameters, fighter.action)?
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CatchAttack => parameters
            .pummel
            .poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureDamageHi => parameters
            .capture_damage
            .high
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureDamageLw => parameters
            .capture_damage
            .low
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CatchCut => parameters
            .escape
            .catch_cut_poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureCut => parameters
            .escape
            .capture_cut_poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        action if throw_for_action(&parameters.throws, action).is_some() => {
            throw_for_action(&parameters.throws, action)?
                .poses
                .get(fighter.action_frame as usize)
                .map(Vec::as_slice)
        }
        _ => None,
    }
}

fn catch_for_action(parameters: &Parameters, action: Action) -> Option<&Catch> {
    match action {
        Action::Catch => Some(&parameters.catch),
        Action::CatchDash => Some(&parameters.catch_dash),
        _ => None,
    }
}

fn attach(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    allow_lift: bool,
) -> Result<(), Error> {
    let attachment = data.fighters[holder]
        .grab
        .as_ref()
        .ok_or_else(|| Error::Data("capture state requires grab resources".into()))?
        .attachment;
    let holder_pose = simulation::pose(&state.fighters[holder], &data.fighters[holder])?;
    let holder_anchor = BoneCapsule::sphere(attachment.holder_bone, attachment.holder_point, 0.0)
        .transform(&holder_pose, 1.0)
        .map_err(physics)?
        .start;
    let victim_pose = simulation::pose(&state.fighters[victim], &data.fighters[victim])?;
    let victim_anchor = BoneCapsule::sphere(attachment.victim_bone, attachment.victim_point, 0.0)
        .transform(&victim_pose, 1.0)
        .map_err(physics)?
        .start;
    let position = [
        state.fighters[victim].position[0],
        state.fighters[victim].position[1],
        state.fighters[victim].depth,
    ];
    let (position, lifted) = input::capture_alignment(
        position,
        holder_anchor,
        victim_anchor,
        data.rules.grab.as_ref().unwrap().capture_lift_threshold,
        data.fighters[victim].bones[0].scale[1],
    );
    state.fighters[victim].position = [position[0], position[1]];
    state.fighters[victim].depth = position[2];
    if allow_lift && lifted && transfer_capture_family(&mut state.fighters[victim], true) {
        attach(data, state, holder, victim, false)?;
    }
    Ok(())
}

fn detach(state: &mut MatchState, holder: usize, victim: usize) {
    state.fighters[holder].grab = State::default();
    state.fighters[victim].grab = State::default();
}

fn update_escape(data: &MatchData, state: &mut MatchState, victim: usize, controller: Controller) {
    let rules = &data.rules.grab.as_ref().unwrap().escape;
    let target = &mut state.fighters[victim];
    target.grab.escape_timer -= rules.timer_decrement;
    let pressed = controller.buttons & !target.previous_input.buttons;
    let logical_pressed = u32::from(pressed)
        | if controller.shield_held() && !target.previous_input.shield_held() {
            1 << 31
        } else {
            0
        };
    input::mash(
        &mut target.grab.escape_timer,
        &mut target.grab.mash,
        logical_pressed,
        controller.stick,
        rules.mash_penalty,
        rules.stick_threshold,
    );
}

fn escape_pair(data: &MatchData, state: &mut MatchState, holder: usize, victim: usize) {
    let speed = data.rules.grab.as_ref().unwrap().escape.release_speed;
    let facing = state.fighters[holder].facing;
    detach(state, holder, victim);
    simulation::enter(&mut state.fighters[holder], Action::CatchCut);
    simulation::enter(&mut state.fighters[victim], Action::CaptureCut);
    release_velocity(&mut state.fighters[holder], -facing * speed);
    release_velocity(&mut state.fighters[victim], facing * speed);
    state.events.push(Event::GrabEscaped { holder, victim });
}

fn release_velocity(fighter: &mut Fighter, velocity: f32) {
    if fighter.grounded {
        fighter.ground_velocity = velocity;
    } else {
        fighter.velocity[0] = velocity;
    }
}

fn apply_pummel(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    damage: u32,
) -> Result<(), Error> {
    let staled = super::staling::hit(
        &state.fighters[holder].staling,
        damage,
        data.rules.staling.as_ref(),
    )?;
    let hitlag = combat::hitlag(
        staled.damage as i32,
        false,
        1.0,
        &data.rules.hitlag.physics(),
    )
    .map_err(physics)?;
    if !hitlag.is_finite() || hitlag < 0.0 {
        return Err(Error::Physics("pummel produced invalid hitlag".into()));
    }
    state.fighters[holder].hitlag = state.fighters[holder].hitlag.max(hitlag);
    let target = &mut state.fighters[victim];
    let damage_action = capture_damage_action(target.action)
        .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
    target.percent = (target.percent + staled.damage).min(999.0);
    target.hitlag = target.hitlag.max(hitlag);
    target.di_pending = false;
    simulation::enter(target, damage_action);
    super::combat_history::record_hit(
        state,
        holder,
        victim,
        staled.identity.move_id,
        &data.rules.damage.combo,
    );
    state.events.push(Event::Hit {
        attacker: holder,
        victim,
        damage: staled.damage,
        knockback: 0.0,
    });
    if data.rules.staling.is_some() {
        state.fighters[holder]
            .staling
            .queue
            .record(staled.identity, false);
    }
    Ok(())
}

pub(crate) fn move_id(
    parameters: Option<&Parameters>,
    action: Action,
) -> Result<Option<u16>, Error> {
    let move_id = match action {
        Action::CatchAttack => parameters.map(|parameters| parameters.pummel.move_id),
        Action::ThrowF => parameters.map(|parameters| parameters.throws.forward.move_id),
        Action::ThrowB => parameters.map(|parameters| parameters.throws.backward.move_id),
        Action::ThrowHi => parameters.map(|parameters| parameters.throws.up.move_id),
        Action::ThrowLw => parameters.map(|parameters| parameters.throws.down.move_id),
        _ => return Ok(None),
    };
    move_id
        .flatten()
        .map(Some)
        .ok_or_else(|| Error::Data("staling requires an explicit grab move_id".into()))
}

fn throw_for_action(throws: &Throws, action: Action) -> Option<&Throw> {
    match action {
        Action::ThrowF => Some(&throws.forward),
        Action::ThrowB => Some(&throws.backward),
        Action::ThrowHi => Some(&throws.up),
        Action::ThrowLw => Some(&throws.down),
        _ => None,
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
