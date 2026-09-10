//! Bone-driven catch, paired capture, and forward-throw scheduling.
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
    collision::{bones::BoneCapsule, sweep},
    fighter::{combat::Capsule as WorldCapsule, grab as input},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub horizontal_threshold: f32,
    pub up_threshold: f32,
    /// Signed negative common-data threshold.
    pub down_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub catch: Catch,
    pub attachment: Attachment,
    pub throws: Throws,
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
pub struct Throw {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub victim: Option<usize>,
    pub captor: Option<usize>,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
) -> Result<(), Error> {
    if ![
        rules.horizontal_threshold,
        rules.up_threshold,
        rules.down_threshold,
    ]
    .into_iter()
    .all(f32::is_finite)
        || !(0.0..=1.0).contains(&rules.horizontal_threshold)
        || rules.horizontal_threshold == 0.0
        || !(0.0..=1.0).contains(&rules.up_threshold)
        || rules.up_threshold == 0.0
        || !(-1.0..0.0).contains(&rules.down_threshold)
        || parameters.catch.frames.is_empty()
        || parameters.catch.frames.len() > 4096
        || parameters.catch.pull_frames == 0
        || parameters.catch.pull_frames >= 1_000_000
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
    let mut active = false;
    for frame in &parameters.catch.frames {
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
        {
            return Err(Error::Data("invalid explicit throw parameters".into()));
        }
        for pose in &throw.poses {
            super::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

pub(crate) fn valid_relationship(fighters: &[Fighter; 2], player: usize) -> bool {
    let other = 1 - player;
    let fighter = &fighters[player];
    let partner = &fighters[other];
    match (fighter.grab.victim, fighter.grab.captor) {
        (None, None) => true,
        (Some(victim), None) => {
            victim == other
                && partner.grab
                    == (State {
                        victim: None,
                        captor: Some(player),
                    })
                && pair_actions(fighter.action, partner.action)
        }
        (None, Some(holder)) => {
            holder == other
                && partner.grab
                    == (State {
                        victim: Some(player),
                        captor: None,
                    })
                && pair_actions(partner.action, fighter.action)
        }
        (Some(_), Some(_)) => false,
    }
}

fn pair_actions(holder: Action, victim: Action) -> bool {
    matches!(
        (holder, victim),
        (Action::CatchPull, Action::CapturePulled)
            | (Action::CatchWait, Action::CaptureWait)
            | (Action::ThrowF, Action::ThrownF)
            | (Action::ThrowB, Action::ThrownB)
            | (Action::ThrowHi, Action::ThrownHi)
            | (Action::ThrowLw, Action::ThrownLw)
    )
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::Catch
            | Action::CatchPull
            | Action::CatchWait
            | Action::ThrowF
            | Action::ThrowB
            | Action::ThrowHi
            | Action::ThrowLw
            | Action::CapturePulled
            | Action::CaptureWait
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
            | Action::CatchWait
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
    let complete = match fighter.action {
        Action::Catch => fighter.action_frame as usize >= parameters.catch.frames.len(),
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
    if owns_action(fighter.action) {
        if fighter.action == Action::CatchWait {
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
    let pressed = controller.buttons & !fighter.previous_input.buttons;
    if fighter.grounded
        && matches!(fighter.action, Action::Wait | Action::Walk)
        && pressed & super::BUTTON_Z != 0
        && data.grab.is_some()
        && rules.is_some()
    {
        simulation::enter(fighter, Action::Catch);
        return true;
    }
    false
}

/// Paired priority-1 transitions and the scripted release event.
pub(crate) fn update_pairs(data: &MatchData, state: &mut MatchState) -> Result<[bool; 2], Error> {
    let mut released = [false; 2];
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
        let holder_action = state.fighters[holder].action;
        match holder_action {
            Action::CatchPull
                if state.fighters[holder].action_frame >= parameters.catch.pull_frames =>
            {
                simulation::enter(&mut state.fighters[holder], Action::CatchWait);
                simulation::enter(&mut state.fighters[victim], Action::CaptureWait);
            }
            action
                if throw_for_action(&parameters.throws, action).is_some_and(|throw| {
                    state.fighters[holder].action_frame == throw.release_frame
                }) =>
            {
                let hit = throw_for_action(&parameters.throws, action).unwrap().hit;
                detach(state, holder, victim);
                super::damage::apply_hit(
                    data,
                    state,
                    holder,
                    &Hitbox {
                        clank: false,
                        rebound: false,
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
                    super::staling::Hit {
                        identity: state.fighters[holder].staling.identity,
                        group: 0,
                        base_damage: hit.damage,
                        damage: hit.damage as f32,
                    },
                    crate::fighter::damage::HurtHeight::Middle,
                )?;
                released[holder] = true;
                released[victim] = true;
            }
            _ => {}
        }
    }
    Ok(released)
}

pub(crate) fn synchronize_actions(state: &mut MatchState) {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        let victim_action = match state.fighters[holder].action {
            Action::ThrowF => Some(Action::ThrownF),
            Action::ThrowB => Some(Action::ThrownB),
            Action::ThrowHi => Some(Action::ThrownHi),
            Action::ThrowLw => Some(Action::ThrownLw),
            _ => None,
        };
        if let Some(action) = victim_action
            && state.fighters[victim].action != action
        {
            simulation::enter(&mut state.fighters[victim], action);
        }
    }
}

pub(crate) fn release_broken_pairs(state: &mut MatchState) {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        if !holder_action(state.fighters[holder].action) {
            detach(state, holder, victim);
            if matches!(
                state.fighters[victim].action,
                Action::CapturePulled
                    | Action::CaptureWait
                    | Action::ThrownF
                    | Action::ThrownB
                    | Action::ThrownHi
                    | Action::ThrownLw
            ) {
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
            attach(data, state, holder, victim)?;
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
        let target = &state.fighters[victim];
        let Some(parameters) = &data.fighters[holder].grab else {
            continue;
        };
        if holder_frozen
            || source.action != Action::Catch
            || source.grab != State::default()
            || target.grab != State::default()
            || target.invincibility > 0
            || matches!(
                target.action,
                Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
            )
            || parameters.catch.grounded_targets_only && !target.grounded
        {
            continue;
        }
        let source_pose = simulation::pose(source, &data.fighters[holder])?;
        let target_pose = simulation::pose(target, &data.fighters[victim])?;
        let frame = parameters
            .catch
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
            for hurtbox in &data.fighters[victim].hurtboxes {
                let hurt = hurtbox
                    .physics()
                    .transform(&target_pose, 1.0)
                    .map_err(physics)?;
                let hurt = WorldCapsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let mut closest = sweep::ClosestPair::default();
                if sweep::capsule_capsule(&grab, &hurt, &mut closest) {
                    collided = true;
                    break;
                }
            }
            if collided {
                break;
            }
        }
        if collided {
            state.fighters[holder].grab.victim = Some(victim);
            state.fighters[victim].grab.captor = Some(holder);
            state.fighters[victim].facing = state.fighters[holder].facing;
            state.fighters[victim].velocity = [0.0; 2];
            state.fighters[victim].knockback = [0.0; 2];
            state.fighters[victim].ground_velocity = 0.0;
            simulation::enter(&mut state.fighters[holder], Action::CatchPull);
            simulation::enter(&mut state.fighters[victim], Action::CapturePulled);
            state.events.push(Event::Grabbed { holder, victim });
            attach(data, state, holder, victim)?;
        }
    }
    Ok(())
}

pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let parameters = data.grab.as_ref()?;
    match fighter.action {
        Action::Catch => parameters
            .catch
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        action if throw_for_action(&parameters.throws, action).is_some() => {
            throw_for_action(&parameters.throws, action)?
                .poses
                .get(fighter.action_frame as usize)
                .map(Vec::as_slice)
        }
        _ => None,
    }
}

fn attach(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
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
    let mut local_victim = state.fighters[victim].clone();
    local_victim.position = [0.0; 2];
    local_victim.depth = 0.0;
    let victim_pose = simulation::pose(&local_victim, &data.fighters[victim])?;
    let victim_anchor = BoneCapsule::sphere(attachment.victim_bone, attachment.victim_point, 0.0)
        .transform(&victim_pose, 1.0)
        .map_err(physics)?
        .start;
    state.fighters[victim].position = [
        holder_anchor[0] - victim_anchor[0],
        holder_anchor[1] - victim_anchor[1],
    ];
    state.fighters[victim].depth = holder_anchor[2] - victim_anchor[2];
    Ok(())
}

fn detach(state: &mut MatchState, holder: usize, victim: usize) {
    state.fighters[holder].grab.victim = None;
    state.fighters[victim].grab.captor = None;
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
