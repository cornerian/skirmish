//! Ordinary grounded non-Slash clash scan and sampled Rebound callbacks.
//! Source: ftColl_80078C70/8007699C, Fighter_ProcessHit and ftCo_Rebound.c.
//! Two native players, no capture/items/bypass hit flags, uniform supplied floor
//! friction. The complete original entity and animation-command graph is external.
use super::{
    Action, Error, Event, Fighter,
    data::{AttackFrame, Bone, FighterData, MatchData},
};
use crate::{collision::sweep, fighter::combat};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseRules {
    pub damage_gap: i32,
    pub duration_scale: f32,
    pub duration_base: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Victim {
    pub id: u32,
    pub remaining: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Victims {
    entries: [Victim; 12],
    next: u8,
}

impl Victims {
    pub fn from_parts(entries: [Victim; 12], next: u8) -> Option<Self> {
        (next < 12).then_some(Self { entries, next })
    }
    pub fn entries(&self) -> &[Victim; 12] {
        &self.entries
    }
    pub fn next(&self) -> u8 {
        self.next
    }
    pub fn contains(&self, id: u32) -> bool {
        id != 0 && self.entries.iter().any(|entry| entry.id == id)
    }
    pub fn record(&mut self, id: u32) -> Result<bool, ClankError> {
        if id == 0 {
            return Err(ClankError::InvalidIdentity);
        }
        if self.contains(id) {
            return Ok(false);
        }
        let index = match self.entries.iter().position(|entry| entry.id == 0) {
            Some(index) => index,
            None => {
                let index = usize::from(self.next);
                self.next = (self.next + 1) % 12;
                index
            }
        };
        self.entries[index] = Victim { id, remaining: 0 };
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Hit {
    pub enabled: bool,
    pub group: u32,
    pub damage: f32,
    pub clank: bool,
    pub rebound: bool,
    pub hits_grounded: bool,
    pub victims: Victims,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Response {
    pub damage: i32,
    pub rebound_duration: f32,
    pub towards: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClashFighter {
    pub id: u32,
    pub grounded: bool,
    pub x: f32,
    pub hits: [Hit; 4],
    pub response: Response,
}

pub fn eligible(fighters: &[ClashFighter; 2], slots: [usize; 2]) -> bool {
    fighters[0].id != 0
        && fighters[1].id != 0
        && fighters[0].id != fighters[1].id
        && fighters.iter().enumerate().all(|(side, f)| {
            f.grounded
                && f.hits.get(slots[side]).is_some_and(|hit| {
                    hit.enabled
                        && hit.clank
                        && hit.hits_grounded
                        && !hit.victims.contains(fighters[1 - side].id)
                })
        })
}

pub fn resolve_pair(
    fighters: &mut [ClashFighter; 2],
    slots: [usize; 2],
    second_candidates: &mut [bool; 4],
    rules: &ResponseRules,
) -> Result<bool, ClankError> {
    if fighters.iter().any(|f| f.id == 0) || fighters[0].id == fighters[1].id {
        return Err(ClankError::InvalidIdentity);
    }
    if slots.into_iter().any(|slot| slot >= 4) {
        return Err(ClankError::InvalidSlot);
    }
    if rules.damage_gap < 0
        || ![rules.duration_scale, rules.duration_base]
            .into_iter()
            .all(f32::is_finite)
        || fighters.iter().any(|f| {
            f.response.damage < 0
                || ![f.x, f.response.rebound_duration, f.response.towards]
                    .into_iter()
                    .all(f32::is_finite)
        })
    {
        return Err(ClankError::InvalidParameters);
    }
    let damage = [
        fighters[0].hits[slots[0]].damage,
        fighters[1].hits[slots[1]].damage,
    ];
    let integers = [integer_damage(damage[0])?, integer_damage(damage[1])?];
    let mut next = fighters.clone();
    let mut candidates = *second_candidates;
    for side in [1, 0] {
        if damage_beats(integers[side], integers[1 - side], rules.damage_gap) {
            let hit = next[side].hits[slots[side]];
            let opponent = next[1 - side].id;
            for (slot, same_group) in next[side].hits.iter_mut().enumerate() {
                if same_group.enabled && same_group.group == hit.group {
                    let inserted = same_group.victims.record(opponent)?;
                    if side == 1 && inserted {
                        candidates[slot] = false;
                    }
                }
            }
            let amount = if damage[side] != 0.0 && integers[side] == 0 {
                1
            } else {
                integers[side]
            };
            if amount > next[side].response.damage {
                next[side].response.damage = amount;
                if hit.rebound && next[side].grounded {
                    let duration = amount as f32 * rules.duration_scale + rules.duration_base;
                    if !duration.is_finite() {
                        return Err(ClankError::InvalidParameters);
                    }
                    next[side].response.rebound_duration = duration;
                    next[side].response.towards = if next[side].x < next[1 - side].x {
                        1.0
                    } else {
                        -1.0
                    };
                }
            }
        }
    }
    *fighters = next;
    *second_candidates = candidates;
    Ok(damage_beats(integers[0], integers[1], rules.damage_gap))
}

/// The decomp compares the cached integer damages with a strict subtraction.
/// Keep that ordering in a wider type so a valid maximum gap cannot overflow
/// the host Rust arithmetic before the comparison is made.
fn damage_beats(damage: i32, opponent: i32, gap: i32) -> bool {
    i64::from(damage) - i64::from(gap) < i64::from(opponent)
}

fn integer_damage(damage: f32) -> Result<i32, ClankError> {
    if (0.0..2_147_483_648.0).contains(&damage) {
        Ok(damage as i32)
    } else {
        Err(ClankError::InvalidDamage)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReboundRules {
    pub animation_length: f32,
    pub push_scale: f32,
    pub push_base: f32,
    pub surface_friction_multiplier: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rebound {
    pub animation_rate: f32,
    pub impulse: f32,
    pub ground_acceleration: f32,
}

pub fn rebound(duration: f32, towards: f32, rules: &ReboundRules) -> Result<Rebound, ClankError> {
    if duration <= 0.0
        || ![
            duration,
            towards,
            rules.animation_length,
            rules.push_scale,
            rules.push_base,
            rules.surface_friction_multiplier,
        ]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(ClankError::InvalidParameters);
    }
    let impulse = -towards * (duration * rules.push_scale + rules.push_base);
    let ground_acceleration = if rules.surface_friction_multiplier < 1.0 {
        impulse * rules.surface_friction_multiplier
    } else {
        impulse
    };
    let animation_rate = (rules.animation_length + 0.1) / duration;
    if ![impulse, ground_acceleration, animation_rate]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(ClankError::InvalidParameters);
    }
    Ok(Rebound {
        animation_rate,
        impulse,
        ground_acceleration,
    })
}

pub fn apply_rebound_friction(impulse: &mut f32) -> bool {
    if *impulse != 0.0 {
        *impulse = 0.0;
        false
    } else {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClankError {
    #[error("clank requires distinct nonzero native fighter identities")]
    InvalidIdentity,
    #[error("clank hitbox slot is outside 0..4")]
    InvalidSlot,
    #[error("cached clank damage is outside the nonnegative C int conversion range")]
    InvalidDamage,
    #[error("invalid or nonfinite clank/rebound parameters")]
    InvalidParameters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    OrdinaryGroundedNonSlash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub profile: Profile,
    pub response: ResponseRules,
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
    #[serde(default)]
    pub poses_blend_frames: u8,
    #[serde(default)]
    pub poses_dynamics_variant: u8,
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
            let response = rebound(
                duration,
                1.0,
                &ReboundRules {
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
    pub victims: Victims,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub slots: [Slot; 4],
    /// Original pending response is cleared after ProcessHit; recovery state persists.
    pub response: Response,
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
                .map_or_else(Victims::default, |s| s.victims);
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
            .attack_for(f)
            .and_then(|a| a.frames.get(f.action_frame as usize));
        ClashFighter {
            id: player as u32 + 1,
            grounded: f.grounded,
            x: f.position[0],
            response: f.clank.response,
            hits: core::array::from_fn(|slot| {
                frame
                    .and_then(|frame| frame.hitboxes.get(slot))
                    .map_or_else(Hit::default, |hit| Hit {
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
                if !candidates[inner] || !eligible(&pair, [outer, inner]) {
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
                    let stop =
                        resolve_pair(&mut pair, [outer, inner], &mut candidates, &rules.response)
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
                    .attack_for(f)
                    .and_then(|a| a.frames.get(f.action_frame as usize))
                    .map_or(&fd.bones, |frame| &frame.bones)
                    .clone();
                let rebound = rebound(
                    response.rebound_duration,
                    response.towards,
                    &ReboundRules {
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
        f.clank.response = Response::default();
        super::staling::flush(
            f,
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
            &mut state.action_instances,
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

#[cfg(test)]
mod tests {
    use super::{Animation, ClashFighter, Hit, Response, ResponseRules, resolve_pair};

    #[test]
    fn animation_metadata_defaults_for_older_json() {
        let animation: Animation =
            serde_json::from_str(r#"{"animation_length":3.5,"poses":[]}"#).unwrap();
        assert_eq!(animation.poses_blend_frames, 0);
        assert_eq!(animation.poses_dynamics_variant, 0);
    }

    #[test]
    fn animation_metadata_round_trips() {
        let animation = Animation {
            animation_length: 3.5,
            poses: vec![],
            poses_blend_frames: 7,
            poses_dynamics_variant: 9,
        };
        let json = serde_json::to_value(&animation).unwrap();
        assert_eq!(json["poses_blend_frames"], 7);
        assert_eq!(json["poses_dynamics_variant"], 9);
        assert_eq!(
            serde_json::from_value::<Animation>(json).unwrap(),
            animation
        );
    }

    #[test]
    fn maximum_damage_gap_keeps_source_ordering_without_integer_overflow() {
        let mut fighters = [
            ClashFighter {
                id: 1,
                grounded: true,
                x: 0.0,
                hits: [Hit::default(); 4],
                response: Response::default(),
            },
            ClashFighter {
                id: 2,
                grounded: true,
                x: 1.0,
                hits: [Hit::default(); 4],
                response: Response::default(),
            },
        ];
        let mut candidates = [true; 4];
        let rules = ResponseRules {
            damage_gap: i32::MAX,
            duration_scale: 0.0,
            duration_base: 0.0,
        };

        // Source ftColl_8007699C uses (int)dmg - x3CC < (int)other.
        assert!(resolve_pair(&mut fighters, [0, 0], &mut candidates, &rules).unwrap());
    }
}
