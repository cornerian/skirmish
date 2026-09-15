//! Bounded deterministic one-shot deadline scheduler.
//!
//! Timers are declarative records, never callbacks.  Action-relative records
//! carry an action generation; every native transition advances that
//! generation and invalidates the old records, including an exit/re-entry of
//! the same action.  Advancing is transactional and delivers by
//! `(deadline, timer_id)`. World deadlines and action-phase deadlines use
//! separate clocks: a paused/hitlag world step cannot advance an action timer.
//!
//! [`SchedulerState`] is an internal native host state machine. Its methods
//! are intentionally not script primitives: the frontend/compiler may emit
//! only static [`TimerSpec`] records attached to an event/action table. A
//! script receives a delivered deadline and can request an ordinary gameplay
//! command, but it cannot call `schedule`, `cancel`, `enter_action`, or
//! `advance`, and it cannot provide a callback that runs inside those methods.
//!
//! Action transitions can legitimately schedule a fresh one shot record for
//! the new generation on every transition. That is bounded native behavior,
//! not a hidden timer loop; prohibiting a script from manufacturing a
//! transition loop belongs to command validation and the compiled event
//! table, not this generic scheduler. There is no interval timer or synthetic
//! per-frame event that would make polling convenient.

use super::events::Event;
use crate::game::Action;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_TIMERS: usize = 256;
pub const MAX_DELIVERIES_PER_ADVANCE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OwnerId(u32);

impl OwnerId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimerId(u64);

impl TimerId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ActionGeneration(u64);

impl ActionGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Native action-entry generation. The scheduler uses checked allocation
    /// internally; fighter rollback state uses this value type without
    /// exposing its representation to scripts.
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionScope {
    pub owner: OwnerId,
    pub action: Action,
    pub generation: ActionGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerSpec {
    /// Delay is strictly positive, so a timer cannot be used as a same-frame
    /// loop. Multiple explicit deadlines on successive frames remain valid.
    AfterFrames(u32),
    AtFrame(u32),
    AtActionFrame {
        action: Action,
        frame: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum TimerScope {
    Absolute,
    Action {
        action: Action,
        generation: ActionGeneration,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timer {
    owner: OwnerId,
    deadline: u32,
    scope: TimerScope,
    token: u64,
}

impl Timer {
    pub const fn owner(self) -> OwnerId {
        self.owner
    }

    pub const fn deadline(self) -> u32 {
        self.deadline
    }

    pub const fn token(self) -> u64 {
        self.token
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ActiveAction {
    action: Action,
    generation: ActionGeneration,
    last_frame: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerState {
    now: u32,
    timers: BTreeMap<TimerId, Timer>,
    actions: BTreeMap<OwnerId, ActiveAction>,
    next_id: u64,
    next_generation: u64,
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScheduleError {
    #[error("scheduler time cannot move backwards")]
    TimeWentBackwards,
    #[error("after-frames delay must be at least one frame")]
    ZeroDelay,
    #[error("scheduler frame counter overflowed")]
    FrameOverflow,
    #[error("action frame cannot move backwards")]
    ActionFrameWentBackwards,
    #[error("deadline {deadline} is before scheduler time {now}")]
    DeadlineInPast { deadline: u32, now: u32 },
    #[error("owner has no active action")]
    NoActiveAction,
    #[error("timer action does not match the owner's active action")]
    ActionMismatch,
    #[error("timer limit of {MAX_TIMERS} exceeded")]
    TooManyTimers,
    #[error("timer ID space exhausted")]
    IdExhausted,
    #[error("action generation space exhausted")]
    GenerationExhausted,
    #[error("advance would deliver more than {MAX_DELIVERIES_PER_ADVANCE} timers")]
    TooManyDeliveries,
}

impl SchedulerState {
    pub const fn new() -> Self {
        Self {
            now: 0,
            timers: BTreeMap::new(),
            actions: BTreeMap::new(),
            next_id: 1,
            next_generation: 1,
        }
    }

    pub const fn now(&self) -> u32 {
        self.now
    }

    pub fn timers(&self) -> impl Iterator<Item = (TimerId, Timer)> + '_ {
        self.timers.iter().map(|(id, timer)| (*id, *timer))
    }

    pub fn action_scope(&self, owner: OwnerId) -> Option<ActionScope> {
        self.actions.get(&owner).map(|active| ActionScope {
            owner,
            action: active.action,
            generation: active.generation,
        })
    }

    /// Native host operation: entering any action invalidates all old
    /// action-relative timers first. The generation changes even when
    /// `action` equals the previous action.
    pub fn enter_action(
        &mut self,
        owner: OwnerId,
        action: Action,
    ) -> Result<ActionScope, ScheduleError> {
        let mut candidate = self.clone();
        candidate.cancel_action_timers(owner);
        let generation = ActionGeneration(candidate.next_generation);
        candidate.next_generation = candidate
            .next_generation
            .checked_add(1)
            .ok_or(ScheduleError::GenerationExhausted)?;
        candidate.actions.insert(
            owner,
            ActiveAction {
                action,
                generation,
                last_frame: 0,
            },
        );
        *self = candidate;
        Ok(ActionScope {
            owner,
            action,
            generation,
        })
    }

    /// Enter a declared internal phase while preserving one move-owned
    /// action timer's remaining action-frame budget. All unrelated action
    /// timers are cancelled exactly as for an ordinary transition.
    pub fn enter_action_retaining(
        &mut self,
        owner: OwnerId,
        action: Action,
        timer_id: Option<TimerId>,
    ) -> Result<ActionScope, ScheduleError> {
        let mut candidate = self.clone();
        let old = candidate.actions.get(&owner).copied();
        let retained = timer_id
            .and_then(|id| candidate.timers.get(&id).copied())
            .filter(|timer| {
                timer.owner == owner
                    && old.is_some_and(|active| {
                        matches!(
                            timer.scope,
                            TimerScope::Action { action, generation }
                                if action == active.action && generation == active.generation
                        )
                    })
            });
        let remaining = retained
            .zip(old)
            .map(|(timer, active)| timer.deadline.saturating_sub(active.last_frame));
        candidate.cancel_action_timers(owner);
        let generation = ActionGeneration(candidate.next_generation);
        candidate.next_generation = candidate
            .next_generation
            .checked_add(1)
            .ok_or(ScheduleError::GenerationExhausted)?;
        candidate.actions.insert(
            owner,
            ActiveAction {
                action,
                generation,
                last_frame: 0,
            },
        );
        if let (Some(id), Some(timer), Some(remaining)) = (timer_id, retained, remaining) {
            candidate.timers.insert(
                id,
                Timer {
                    owner,
                    deadline: remaining,
                    scope: TimerScope::Action { action, generation },
                    token: timer.token,
                },
            );
        }
        *self = candidate;
        Ok(ActionScope {
            owner,
            action,
            generation,
        })
    }

    /// Native host operation: leaving an action invalidates its records before
    /// clearing the scope.
    pub fn exit_action(&mut self, owner: OwnerId) {
        self.cancel_action_timers(owner);
        self.actions.remove(&owner);
    }

    /// Native host operation. Script code must use statically compiled timer
    /// records rather than calling this method dynamically.
    pub fn schedule(
        &mut self,
        owner: OwnerId,
        spec: TimerSpec,
        token: u64,
    ) -> Result<TimerId, ScheduleError> {
        let mut candidate = self.clone();
        let (deadline, scope) = candidate.resolve(owner, spec)?;
        if candidate.timers.len() >= MAX_TIMERS {
            return Err(ScheduleError::TooManyTimers);
        }
        let id = TimerId(candidate.next_id);
        candidate.next_id = candidate
            .next_id
            .checked_add(1)
            .ok_or(ScheduleError::IdExhausted)?;
        candidate.timers.insert(
            id,
            Timer {
                owner,
                deadline,
                scope,
                token,
            },
        );
        *self = candidate;
        Ok(id)
    }

    /// Native host operation; script code cannot cancel a timer by ID.
    pub fn cancel(&mut self, id: TimerId) -> bool {
        self.timers.remove(&id).is_some()
    }

    /// Native host operation used during lifecycle teardown.
    pub fn cancel_owner(&mut self, owner: OwnerId) {
        self.timers.retain(|_, timer| timer.owner != owner);
    }

    /// Native host operation. Advance and drain due timers transactionally. No
    /// callback can run from this method; the returned events are applied by
    /// the host afterward.
    pub fn advance(&mut self, now: u32) -> Result<Vec<Event>, ScheduleError> {
        if now < self.now {
            return Err(ScheduleError::TimeWentBackwards);
        }
        let mut candidate = self.clone();
        candidate.now = now;
        let mut due = candidate
            .timers
            .iter()
            .filter(|(_, timer)| {
                timer.deadline <= now && matches!(timer.scope, TimerScope::Absolute)
            })
            .map(|(id, timer)| (*id, *timer))
            .collect::<Vec<_>>();
        due.sort_by_key(|(id, timer)| (timer.deadline, *id));
        if due.len() > MAX_DELIVERIES_PER_ADVANCE {
            return Err(ScheduleError::TooManyDeliveries);
        }
        let mut events = Vec::with_capacity(due.len());
        for (id, timer) in due {
            candidate.timers.remove(&id);
            events.push(Event::ScheduledDeadline {
                frame: timer.deadline,
                owner: timer.owner,
                timer: id,
                token: timer.token,
            });
        }
        *self = candidate;
        Ok(events)
    }

    /// Native host operation. Advance one owner's action-phase clock and
    /// drain only action-relative timers for its current generation. The
    /// world clock is deliberately ignored, so hitlag and paused animation
    /// cannot cause action deadlines to fire.
    pub fn advance_action(
        &mut self,
        owner: OwnerId,
        action_frame: u32,
    ) -> Result<Vec<Event>, ScheduleError> {
        let active = self
            .actions
            .get(&owner)
            .ok_or(ScheduleError::NoActiveAction)?;
        if action_frame < active.last_frame {
            return Err(ScheduleError::ActionFrameWentBackwards);
        }
        let action = active.action;
        let generation = active.generation;
        let mut candidate = self.clone();
        candidate
            .actions
            .get_mut(&owner)
            .expect("active action was checked above")
            .last_frame = action_frame;
        let mut due = candidate
            .timers
            .iter()
            .filter(|(_, timer)| {
                timer.owner == owner
                    && timer.deadline <= action_frame
                    && matches!(
                        timer.scope,
                        TimerScope::Action {
                            action: timer_action,
                            generation: timer_generation,
                        } if timer_action == action && timer_generation == generation
                    )
            })
            .map(|(id, timer)| (*id, *timer))
            .collect::<Vec<_>>();
        due.sort_by_key(|(id, timer)| (timer.deadline, *id));
        if due.len() > MAX_DELIVERIES_PER_ADVANCE {
            return Err(ScheduleError::TooManyDeliveries);
        }
        let mut events = Vec::with_capacity(due.len());
        for (id, timer) in due {
            candidate.timers.remove(&id);
            events.push(Event::ScheduledDeadline {
                frame: candidate.now,
                owner: timer.owner,
                timer: id,
                token: timer.token,
            });
        }
        *self = candidate;
        Ok(events)
    }

    fn resolve(&self, owner: OwnerId, spec: TimerSpec) -> Result<(u32, TimerScope), ScheduleError> {
        match spec {
            TimerSpec::AfterFrames(delay) => {
                if delay == 0 {
                    return Err(ScheduleError::ZeroDelay);
                }
                let deadline = self
                    .now
                    .checked_add(delay)
                    .ok_or(ScheduleError::FrameOverflow)?;
                Ok((deadline, TimerScope::Absolute))
            }
            TimerSpec::AtFrame(deadline) => {
                if deadline < self.now {
                    return Err(ScheduleError::DeadlineInPast {
                        deadline,
                        now: self.now,
                    });
                }
                Ok((deadline, TimerScope::Absolute))
            }
            TimerSpec::AtActionFrame { action, frame } => {
                let active = self
                    .actions
                    .get(&owner)
                    .ok_or(ScheduleError::NoActiveAction)?;
                if active.action != action {
                    return Err(ScheduleError::ActionMismatch);
                }
                // Action deadlines are authored action-frame targets, not
                // world-time delays. A timer registered mid-action still
                // targets this fixed phase frame.
                let deadline = frame;
                Ok((
                    deadline,
                    TimerScope::Action {
                        action,
                        generation: active.generation,
                    },
                ))
            }
        }
    }

    fn cancel_action_timers(&mut self, owner: OwnerId) {
        self.timers.retain(|_, timer| {
            timer.owner != owner || !matches!(timer.scope, TimerScope::Action { .. })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::script::events::EventKind;

    #[test]
    fn one_shot_deadlines_are_ordered_and_removed() {
        let owner = OwnerId::new(1);
        let mut scheduler = SchedulerState::new();
        let later = scheduler
            .schedule(owner, TimerSpec::AfterFrames(10), 30)
            .unwrap();
        let earlier = scheduler
            .schedule(owner, TimerSpec::AfterFrames(2), 10)
            .unwrap();
        let same_deadline = scheduler
            .schedule(owner, TimerSpec::AtFrame(2), 20)
            .unwrap();
        let events = scheduler.advance(10).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                Event::ScheduledDeadline { timer: first, token: 10, .. },
                Event::ScheduledDeadline { timer: second, token: 20, .. },
                Event::ScheduledDeadline { timer: third, token: 30, .. },
            ] if *first == earlier && *second == same_deadline && *third == later
        ));
        assert!(scheduler.timers().next().is_none());
    }

    #[test]
    fn action_transition_cancels_old_timer_even_on_same_action_reentry() {
        let owner = OwnerId::new(4);
        let mut scheduler = SchedulerState::new();
        let scope = scheduler.enter_action(owner, Action::Wait).unwrap();
        scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: scope.action,
                    frame: 2,
                },
                1,
            )
            .unwrap();
        scheduler.enter_action(owner, Action::Wait).unwrap();
        assert!(scheduler.advance_action(owner, 2).unwrap().is_empty());
    }

    #[test]
    fn zero_delay_and_recurring_forms_are_unrepresentable() {
        let owner = OwnerId::new(2);
        let mut scheduler = SchedulerState::new();
        assert_eq!(
            scheduler.schedule(owner, TimerSpec::AfterFrames(0), 0),
            Err(ScheduleError::ZeroDelay)
        );
        assert_eq!(EventKind::from_name("frame"), None);
    }

    #[test]
    fn world_advance_does_not_fire_action_deadline_during_hitlag() {
        let owner = OwnerId::new(8);
        let mut scheduler = SchedulerState::new();
        scheduler.enter_action(owner, Action::Wait).unwrap();
        scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: Action::Wait,
                    frame: 2,
                },
                44,
            )
            .unwrap();
        assert!(scheduler.advance(100).unwrap().is_empty());
        assert!(scheduler.advance_action(owner, 1).unwrap().is_empty());
        assert!(matches!(
            scheduler.advance_action(owner, 2).unwrap().as_slice(),
            [Event::ScheduledDeadline { token: 44, .. }]
        ));
    }

    #[test]
    fn mid_action_registration_targets_authored_phase_frame() {
        let owner = OwnerId::new(9);
        let mut scheduler = SchedulerState::new();
        scheduler.enter_action(owner, Action::Wait).unwrap();
        scheduler.advance_action(owner, 4).unwrap();
        scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: Action::Wait,
                    frame: 5,
                },
                55,
            )
            .unwrap();
        assert!(scheduler.advance_action(owner, 4).unwrap().is_empty());
        assert!(matches!(
            scheduler.advance_action(owner, 5).unwrap().as_slice(),
            [Event::ScheduledDeadline { token: 55, .. }]
        ));
    }

    #[test]
    fn retained_phase_preserves_remaining_action_frames() {
        let owner = OwnerId::new(10);
        let mut scheduler = SchedulerState::new();
        let first = scheduler.enter_action(owner, Action::Wait).unwrap();
        let timer = scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: Action::Wait,
                    frame: 10,
                },
                77,
            )
            .unwrap();
        scheduler.advance_action(owner, 4).unwrap();
        let second = scheduler
            .enter_action_retaining(owner, Action::Walk, Some(timer))
            .unwrap();
        assert!(second.generation.get() > first.generation.get());
        assert!(scheduler.advance_action(owner, 5).unwrap().is_empty());
        assert!(matches!(
            scheduler.advance_action(owner, 6).unwrap().as_slice(),
            [Event::ScheduledDeadline { timer: id, token: 77, .. }] if *id == timer
        ));
    }

    #[test]
    fn stale_timer_scope_cannot_be_retained() {
        let owner = OwnerId::new(11);
        let mut scheduler = SchedulerState::new();
        let first = scheduler.enter_action(owner, Action::Wait).unwrap();
        let timer = scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: Action::Wait,
                    frame: 3,
                },
                88,
            )
            .unwrap();
        // A normal transition invalidates the old generation and removes the
        // timer; passing its old ID cannot resurrect it.
        let second = scheduler.enter_action(owner, Action::Walk).unwrap();
        assert!(second.generation.get() > first.generation.get());
        let third = scheduler
            .enter_action_retaining(owner, Action::Fall, Some(timer))
            .unwrap();
        assert!(third.generation.get() > second.generation.get());
        assert!(scheduler.advance_action(owner, 100).unwrap().is_empty());
    }

    #[test]
    fn due_timer_is_removed_before_phase_retain_and_cannot_double_resume() {
        let owner = OwnerId::new(12);
        let mut scheduler = SchedulerState::new();
        scheduler.enter_action(owner, Action::Wait).unwrap();
        let timer = scheduler
            .schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action: Action::Wait,
                    frame: 2,
                },
                99,
            )
            .unwrap();
        let due = scheduler.advance_action(owner, 2).unwrap();
        assert!(
            matches!(due.as_slice(), [Event::ScheduledDeadline { timer: id, .. }] if *id == timer)
        );
        scheduler
            .enter_action_retaining(owner, Action::Walk, Some(timer))
            .unwrap();
        assert!(scheduler.advance_action(owner, 1).unwrap().is_empty());
    }
}
