//! Ordinary grounded non-Slash clash scan and sampled Rebound callbacks.
//! Source: ftColl_80078C70/8007699C, Fighter_ProcessHit and ftCo_Rebound.c.
//! Two native players, no capture/items/bypass hit flags, uniform supplied floor
//! friction. The complete original entity and animation-command graph is external.
use super::{
    Action, Error, Event, Fighter,
    data::{AttackFrame, Bone, FighterData, MatchData},
};
use crate::{
    collision::sweep,
    fighter::{clank as math, combat},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    OrdinaryGroundedNonSlash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub profile: Profile,
    pub response: math::Rules,
    pub push_scale: f32,
    pub push_base: f32,
    pub hitlag_maximum: f32,
    pub surface_friction_multiplier: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub animation_length: f32,
    pub poses: Vec<Vec<Bone>>,
}

pub(crate) fn validate(r: &Rules, fighter: &FighterData) -> Result<(), Error> {
    let values = [
        r.response.duration_scale,
        r.response.duration_base,
        r.push_scale,
        r.push_base,
        r.hitlag_maximum,
        r.surface_friction_multiplier,
    ];
    if r.response.damage_gap < 0 || !values.into_iter().all(|v| (0.0..=1_000_000.0).contains(&v)) {
        return Err(Error::Data("invalid ordinary clank coefficients".into()));
    }
    let a = fighter
        .rebound
        .as_ref()
        .ok_or_else(|| Error::Data("ordinary clank profile requires rebound animation".into()))?;
    if !(0.0..=4095.0).contains(&a.animation_length)
        || a.poses.len() > 4096
        || (a.animation_length as usize) >= a.poses.len()
    {
        return Err(Error::Data(
            "rebound requires complete finite animation samples".into(),
        ));
    }
    for pose in &a.poses {
        super::validation::validate_animation_pose(pose, fighter)?;
    }
    // The match accepts at most 999 raw damage. Staling penalties are nonnegative.
    // A positive response must have representable recovery for all supported damage.
    for amount in [1.0, 999.0] {
        let duration = amount * r.response.duration_scale + r.response.duration_base;
        if duration != 0.0 {
            let response = math::rebound(
                duration,
                1.0,
                &math::ReboundRules {
                    animation_length: a.animation_length,
                    push_scale: r.push_scale,
                    push_base: r.push_base,
                    surface_friction_multiplier: r.surface_friction_multiplier,
                },
            )
            .map_err(|e| Error::Data(e.to_string()))?;
            if duration > 1_000_000.0 || response.animation_rate <= 0.0 {
                return Err(Error::Data(
                    "rebound recovery exceeds supported range".into(),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Slot {
    pub group: Option<u8>,
    pub victims: math::Victims,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub slots: [Slot; 4],
    /// Original pending response is cleared after ProcessHit; recovery state persists.
    pub response: math::Response,
    pub clock: f32,
    pub rate: f32,
    pub impulse: f32,
    pub pending_ground_acceleration: f32,
    pub frozen_pose: Vec<Bone>,
}

pub(crate) fn transition(f: &mut Fighter, action: Action) {
    let response = f.clank.response;
    if matches!(action, Action::ReboundStop | Action::Rebound) {
        f.clank.slots = [Slot::default(); 4];
    } else {
        f.clank = State {
            response,
            ..Default::default()
        };
    }
}

/// Sample contract: disable absent/changed slots before creation; new slots copy
/// history from any surviving same-group slot, then other newly created slots.
/// Continuing slots retain history through pose changes and hitlag.
pub(crate) fn sample(state: &mut State, frame: Option<&AttackFrame>) {
    let Some(frame) = frame else {
        state.slots = [Slot::default(); 4];
        return;
    };
    let old = state.slots;
    state.slots = core::array::from_fn(|slot| {
        frame
            .hitboxes
            .get(slot)
            .filter(|hit| old[slot].group == Some(hit.group))
            .map_or(Slot::default(), |_| old[slot])
    });
    for (slot, hit) in frame.hitboxes.iter().enumerate() {
        if state.slots[slot].group != Some(hit.group) {
            let victims = state
                .slots
                .iter()
                .find(|s| s.group == Some(hit.group))
                .map_or_else(math::Victims::default, |s| s.victims);
            state.slots[slot] = Slot {
                group: Some(hit.group),
                victims,
            };
        }
    }
}

pub(crate) fn blocked(f: &Fighter, slot: usize, opponent: usize) -> bool {
    f.clank.slots[slot].victims.contains(opponent as u32 + 1)
}

pub(crate) fn record(f: &mut Fighter, group: u8, opponent: usize) -> Result<(), Error> {
    for slot in &mut f.clank.slots {
        if slot.group == Some(group) {
            slot.victims.record(opponent as u32 + 1).map_err(physics)?;
        }
    }
    Ok(())
}

/// A pair is visited once: later native entity/slot outside earlier candidates.
/// Clash victims are registered before either shield or hurtbox pass; no action
/// changes occur here, preserving damage/shield precedence over pending rebound.
pub(crate) fn scan(
    data: &MatchData,
    state: &mut super::State,
    swept: &[[Option<combat::Capsule>; 4]; 2],
) -> Result<(), Error> {
    let Some(rules) = &data.rules.clank else {
        return Ok(());
    };
    let mut pair = core::array::from_fn(|side| {
        let player = 1 - side;
        let f = &state.fighters[player];
        let frame = data.fighters[player]
            .attack(f.action)
            .and_then(|a| a.frames.get(f.action_frame as usize));
        math::Fighter {
            id: player as u32 + 1,
            grounded: f.grounded,
            x: f.position[0],
            response: f.clank.response,
            hits: core::array::from_fn(|slot| {
                frame
                    .and_then(|frame| frame.hitboxes.get(slot))
                    .map_or_else(math::Hit::default, |hit| math::Hit {
                        enabled: true,
                        group: u32::from(hit.group),
                        damage: f.staling.hits[slot].map_or(hit.damage as f32, |h| h.damage),
                        clank: hit.clank,
                        rebound: hit.rebound,
                        hits_grounded: true,
                        victims: f.clank.slots[slot].victims,
                    })
            }),
        }
    });
    let mut candidates = core::array::from_fn(|slot| {
        let hit = &pair[1].hits[slot];
        hit.enabled && hit.clank && !hit.victims.contains(pair[0].id)
    });
    if pair.iter().all(|f| f.grounded) {
        for outer in 0..4 {
            for inner in 0..4 {
                if !candidates[inner] || !math::eligible(&pair, [outer, inner]) {
                    continue;
                }
                let (Some(first), Some(second)) = (&swept[1][outer], &swept[0][inner]) else {
                    return Err(Error::Data("clank slot missing sweep".into()));
                };
                let mut closest = sweep::ClosestPair::default();
                let collided = sweep::capsule_capsule(first, second, &mut closest);
                if closest
                    .first
                    .into_iter()
                    .chain(closest.second)
                    .any(|v| !v.is_finite())
                {
                    return Err(Error::NonFinite);
                }
                if collided {
                    let stop = math::resolve_pair(
                        &mut pair,
                        [outer, inner],
                        &mut candidates,
                        &rules.response,
                    )
                    .map_err(physics)?;
                    let suppressed = [pair[1].hits[inner].victims.contains(pair[0].id), stop];
                    if suppressed.into_iter().any(|side| side) {
                        state.events.push(Event::Clank {
                            slots: [inner, outer],
                            suppressed,
                        });
                    }
                    if stop {
                        break;
                    }
                }
            }
        }
    }
    for (side, result) in pair.iter().enumerate() {
        let f = &mut state.fighters[1 - side];
        f.clank.response = result.response;
        for (slot, hit) in f.clank.slots.iter_mut().zip(result.hits) {
            slot.victims = hit.victims;
        }
    }
    Ok(())
}

/// Original ProcessHit selects incoming damage or shield stun ahead of clank,
/// then clank ahead of the attacker's dealt-damage hitlag. Motion change clears
/// active hitboxes but retains the current physics skeleton through ReboundStop.
pub(crate) fn finish(
    data: &MatchData,
    state: &mut super::State,
    incoming: [bool; 2],
) -> Result<(), Error> {
    let Some(rules) = &data.rules.clank else {
        return Ok(());
    };
    for (player, incoming) in incoming.into_iter().enumerate() {
        let f = &mut state.fighters[player];
        let response = f.clank.response;
        if response.damage > 0 && !incoming {
            let crouching = matches!(f.action, Action::Squat | Action::SquatWait);
            let lag = combat::hitlag(
                response.damage,
                crouching,
                1.0,
                &data.rules.hitlag.physics(),
            )
            .map_err(physics)?
            .min(rules.hitlag_maximum);
            if response.rebound_duration != 0.0 {
                let fd = &data.fighters[player];
                let animation = fd
                    .rebound
                    .as_ref()
                    .ok_or_else(|| Error::Data("missing rebound animation".into()))?;
                let frozen_pose = fd
                    .attack(f.action)
                    .and_then(|a| a.frames.get(f.action_frame as usize))
                    .map_or(&fd.bones, |frame| &frame.bones)
                    .clone();
                let rebound = math::rebound(
                    response.rebound_duration,
                    response.towards,
                    &math::ReboundRules {
                        animation_length: animation.animation_length,
                        push_scale: rules.push_scale,
                        push_base: rules.push_base,
                        surface_friction_multiplier: rules.surface_friction_multiplier,
                    },
                )
                .map_err(physics)?;
                super::simulation::enter(f, Action::ReboundStop);
                f.clank.clock = 0.0;
                f.clank.rate = rebound.animation_rate;
                f.clank.impulse = rebound.impulse;
                f.clank.pending_ground_acceleration = rebound.ground_acceleration;
                f.clank.frozen_pose = frozen_pose;
            }
            f.hitlag = lag;
        }
        f.clank.response = math::Response::default();
        super::staling::flush(
            f,
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
        )?;
    }
    Ok(())
}

pub(crate) fn update_animation(f: &mut Fighter, data: &FighterData) -> bool {
    if f.action == Action::ReboundStop {
        super::simulation::enter(f, Action::Rebound);
    } else if f.action == Action::Rebound {
        f.clank.clock += f.clank.rate;
        if data
            .rebound
            .as_ref()
            .is_some_and(|a| f.clank.clock >= a.animation_length)
        {
            super::simulation::enter(f, Action::Wait);
        }
    }
    matches!(f.action, Action::ReboundStop | Action::Rebound)
}

pub(crate) fn pose<'a>(f: &'a Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    match f.action {
        Action::ReboundStop => Some(&f.clank.frozen_pose),
        Action::Rebound => data.rebound.as_ref()?.poses.get(f.clank.clock as usize),
        _ => None,
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
