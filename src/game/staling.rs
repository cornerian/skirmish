//! Native action/slot integration for the original stale-move primitives.
//! Frame samples cache damage while a hitbox remains active. Creation, group
//! changes and explicit damage changes reevaluate staling; a contact records
//! its captured attack identity after all same-frame contacts are collected.
use super::{
    Action, Error, Fighter,
    data::{AttackFrame, FighterData},
};
use crate::fighter::stale::{Entry, InstanceCounter, Queue, Rules};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Hit {
    pub identity: Entry,
    pub group: u8,
    /// Integer damage before staling, ftColl_8007ABD0's unk_count.
    pub base_damage: u32,
    /// Stored HitCapsule::damage; percent accumulation uses this value.
    pub damage: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub queue: Queue,
    pub identity: Entry,
    pub hits: [Option<Hit>; 4],
    /// Deferred only within one native callback turn. Flushed before contacts
    /// and before publishing a frame/checkpoint, in scheduler order.
    pub(crate) transitions: Vec<Action>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            queue: Queue::default(),
            identity: Entry::INACTIVE,
            hits: [None; 4],
            transitions: vec![],
        }
    }
}

pub(crate) fn validate(rules: &Rules) -> Result<(), Error> {
    let mut factor = 1.0;
    for value in rules.penalties {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Data("invalid stale-move penalty".into()));
        }
        factor -= value;
    }
    if factor < 0.0 {
        return Err(Error::Data(
            "stale-move penalties produce negative damage".into(),
        ));
    }
    Ok(())
}

pub(crate) fn transition(fighter: &mut Fighter, action: Action) {
    fighter.staling.hits = [None; 4];
    fighter.staling.transitions.push(action);
}

/// Supported attacks use their explicit resource identity; other motions use
/// source sentinel 1. Every caller uses the same attack lookup as pose sampling.
pub(crate) fn flush(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    counter: &mut InstanceCounter,
) -> Result<(), Error> {
    for action in fighter.staling.transitions.drain(..) {
        if rules.is_some() {
            let move_id = match data.attack(action, fighter.prone) {
                Some(attack) => attack.move_id.ok_or_else(|| {
                    Error::Data("staling requires an explicit attack move_id".into())
                })?,
                None => 1,
            };
            fighter.staling.identity.change_move(move_id, counter);
        }
    }
    Ok(())
}

pub(crate) fn sample(
    state: &mut State,
    frame: Option<&AttackFrame>,
    rules: Option<&Rules>,
) -> Result<(), Error> {
    let mut next = [None; 4];
    if let Some(frame) = frame {
        for (slot, hit) in frame.hitboxes.iter().enumerate() {
            let old = state.hits[slot];
            let cached = old.filter(|old| {
                old.identity == state.identity
                    && old.group == hit.group
                    && old.base_damage == hit.damage
            });
            next[slot] = Some(cached.unwrap_or_else(|| Hit {
                identity: state.identity,
                group: hit.group,
                base_damage: hit.damage,
                damage: rules.map_or(hit.damage as f32, |rules| {
                    state
                        .queue
                        .damage(i32::from(state.identity.move_id), hit.damage as f32, rules)
                }),
            }));
            if next[slot].is_some_and(|hit| !hit.damage.is_finite() || hit.damage < 0.0) {
                return Err(Error::NonFinite);
            }
        }
    }
    state.hits = next;
    Ok(())
}
