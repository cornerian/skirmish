//! Finite runtime events exposed to fighter policy.
//!
//! This module deliberately has no frame/tick event. Input, action,
//! animation, combat hook, contact, and explicit deadline events are the
//! complete runtime event vocabulary. Dispatch returns subscription
//! identities rather than invoking user code, so a handler cannot mutate the
//! scheduler while an event is being delivered.
//!
//! The Python frontend should treat [`EventKind::ALL`] and
//! [`EventKind::from_name`] as its complete subscription allowlist. The
//! [`Dispatcher`] is a native host table, not a script-facing object: scripts
//! must not receive `subscribe`, `cancel`, or a scheduler handle. A compiled
//! definition may bind a finite event kind, but cannot install a new binding
//! while dispatching an event.

use super::move_selection::MoveLifetimeId;
use crate::{
    collision::stage::Surface,
    game::{
        Action, Controller,
        script::{
            motion::{MotionBinding, MotionProfileId, MotionState},
            scheduler::{ActionGeneration, OwnerId, SchedulerState, TimerId},
        },
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The runtime crate owns the finite hook registry shared with the Python
/// compiler. This module owns payloads and routing, never a second allowlist.
pub use skirmish_script_runtime::hooks::HookKind as EventKind;

pub const MAX_SUBSCRIPTIONS: usize = 256;
pub const MAX_PENDING_TRANSITIONS: usize = 8;
pub const MAX_PENDING_DEADLINES: usize = 64;
/// Maximum number of post-commit native events that may be drained in one
/// phase. This budget is shared by every transition cascade; it is separate
/// from scheduler timer delivery limits and cannot be used as a frame poll.
pub const MAX_EVENT_CASCADE: usize = 32;

/// Native transition metadata staged by the action entry path.  It is
/// deliberately a small value record: callbacks are delivered only after the
/// native transition commits, and never from inside the VM host mutex.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingActionTransition {
    pub from: Option<Action>,
    pub to: Action,
    pub generation: ActionGeneration,
    pub preserve_state: bool,
    pub keep_frame: bool,
    #[serde(default)]
    pub behavior_index: Option<usize>,
    #[serde(default)]
    pub move_lifetime: Option<super::move_selection::MoveLifetimeId>,
    #[serde(default)]
    pub retain_move: bool,
}

/// Generic native eligibility gates. These describe state supplied by the
/// host and do not choose a character action for the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    GroundOpen,
    AirOpen,
    JumpAvailable,
}

/// Small rollback state owned by the native fighter lifecycle dispatcher.
/// The parent fighter state should clone/checkpoint this alongside action and
/// script state; it contains no evaluator or callback data.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEventState {
    pub action_generation: ActionGeneration,
    /// Full selected-move owner staged by a selector. Unlike the legacy
    /// behavior index this preserves canonical/actionless ownership and its
    /// independent logical lifetime until the transition boundary.
    #[serde(default)]
    pub pending_move_selection: Option<super::move_selection::PendingMoveSelection>,
    #[serde(default)]
    pub next_move_lifetime: u64,
    /// A native selection failure is carried through the transactional
    /// simulation state so Match::step can report it without committing the
    /// partially evaluated frame.
    #[serde(default)]
    pub native_error: Option<String>,
    #[serde(default)]
    pub active_move: Option<super::move_selection::SelectedMove>,
    /// Copied async `Move.run` continuation. The disposable Pon image and
    /// host handles remain in the prepared thread runtime; rollback stores
    /// this value record only.
    #[cfg(feature = "experimental-continuations")]
    #[serde(default)]
    pub pending_move: Option<skirmish_script_runtime::async_move::PendingMove>,
    /// Bit 0 = ground open, bit 1 = air open, bit 2 = jump available.
    pub availability_mask: u8,
    /// The generation for which the terminal animation notification has been
    /// committed. Equality on action frame is insufficient because native
    /// animation owners can hold their terminal frame.
    #[serde(default)]
    pub animation_delivered_generation: Option<ActionGeneration>,
    /// Last action animation sample whose authored command row was applied.
    /// This is keyed by the native generation and frame rather than mutable
    /// command state: callbacks may consume a command value during dispatch,
    /// and that consumption must not make the same animation sample reemit it.
    #[serde(default)]
    pub command_trace_sample: Option<(ActionGeneration, crate::game::Action, u32)>,
    /// Forward-filled source values from the preceding animation sample.
    /// Nullable rows represent the absence of a SetCmdVar event; retaining
    /// this source cursor keeps callback consumption from creating a write.
    #[serde(default)]
    pub command_trace_values: [Option<u32>; 4],
    /// Transition records are drained by the native event phase after the
    /// action callback transaction. A bounded queue makes same-frame action
    /// cascades finite and rollback-safe.
    #[serde(default)]
    pub pending_transitions: Vec<PendingActionTransition>,
    /// Delivered deadline events retain the scheduler timer that produced
    /// them. Legacy script countdowns use `None`; scheduled timers use
    /// `Some(timer)`, so identical tokens cannot steal one another.
    #[serde(default)]
    pub pending_deadline_events: Vec<(u64, Option<TimerId>)>,
    #[cfg(feature = "experimental-continuations")]
    #[serde(default)]
    pub pending_move_timer: Option<TimerId>,
    /// Native scheduler state is rollback data, while its timer records remain
    /// declarative and bounded. It contains no evaluator or callback handles.
    #[serde(default)]
    pub scheduler: SchedulerState,
    #[serde(default)]
    pub motion_profile_id: Option<MotionProfileId>,
    #[serde(default)]
    pub motion_state: MotionState,
    #[serde(default)]
    pub motion_binding: MotionBinding,
}

impl NativeEventState {
    pub const GROUND_OPEN: u8 = 1 << 0;
    pub const AIR_OPEN: u8 = 1 << 1;
    pub const JUMP_AVAILABLE: u8 = 1 << 2;

    /// Advance the action generation and stage its post-commit event.  The
    /// generation changes even on same-action re-entry, matching scheduler
    /// invalidation semantics. Callers treat a full queue as a transactional
    /// native error before mutating gameplay state.
    pub fn begin_action(
        &mut self,
        from: Option<Action>,
        to: Action,
    ) -> Result<ActionGeneration, &'static str> {
        // An armed owner belongs to this attempted entry only. Consume it
        // even when staging fails so a later unrelated transition cannot
        // inherit the selection.
        let staged = self.pending_move_selection.take();
        let behavior_index = staged.as_ref().map(|selection| selection.behavior_index);
        if self.pending_transitions.len() >= MAX_PENDING_TRANSITIONS {
            return Err("native action transition queue is full");
        }
        let generation = self
            .action_generation
            .next()
            .ok_or("native action generation exhausted")?;
        self.action_generation = generation;
        self.pending_transitions.push(PendingActionTransition {
            from,
            to,
            generation,
            preserve_state: false,
            keep_frame: false,
            behavior_index,
            move_lifetime: staged.as_ref().map(|selection| selection.lifetime),
            retain_move: false,
        });
        Ok(generation)
    }

    /// Allocate a bounded logical move lifetime and stage its explicit owner.
    /// The selection is promoted only by the transition/event boundary.
    pub fn stage_move_selection(
        &mut self,
        entry: super::move_registry::MoveEntry,
        _native_action: Action,
    ) -> Result<MoveLifetimeId, &'static str> {
        let lifetime = self
            .next_move_lifetime
            .checked_add(1)
            .ok_or("native move lifetime exhausted")?;
        self.next_move_lifetime = lifetime;
        let lifetime = super::move_selection::MoveLifetimeId::from_raw(lifetime);
        self.pending_move_selection = Some(super::move_selection::PendingMoveSelection {
            behavior_index: entry.behavior_index,
            canonical: entry.canonical,
            lifetime,
        });
        Ok(lifetime)
    }

    pub fn record_native_error(&mut self, error: &'static str) {
        if self.native_error.is_none() {
            self.native_error = Some(error.into());
        }
    }

    pub fn take_pending_move_selection(
        &mut self,
    ) -> Option<super::move_selection::PendingMoveSelection> {
        self.pending_move_selection.take()
    }

    pub fn set_last_transition_flags(&mut self, preserve_state: bool, keep_frame: bool) {
        if let Some(transition) = self.pending_transitions.last_mut() {
            transition.preserve_state = preserve_state;
            transition.keep_frame = keep_frame;
        }
    }

    /// Authorize an explicitly declared internal phase to retain the active
    /// move owner across its native action transition.
    pub fn set_last_transition_move_retention(&mut self, retain: bool) {
        if let Some(transition) = self.pending_transitions.last_mut() {
            transition.retain_move = retain;
        }
    }

    pub fn take_pending_transitions(&mut self) -> Vec<PendingActionTransition> {
        std::mem::take(&mut self.pending_transitions)
    }

    pub fn push_deadline_token(&mut self, token: u64) -> Result<(), &'static str> {
        if self.pending_deadline_events.len() >= MAX_PENDING_DEADLINES {
            return Err("native deadline event queue is full");
        }
        self.pending_deadline_events.push((token, None));
        Ok(())
    }

    pub fn push_deadline_event(
        &mut self,
        token: u64,
        timer: Option<TimerId>,
    ) -> Result<(), &'static str> {
        if self.pending_deadline_events.len() >= MAX_PENDING_DEADLINES {
            return Err("native deadline event queue is full");
        }
        self.pending_deadline_events.push((token, timer));
        Ok(())
    }

    pub fn take_deadline_events(&mut self) -> Vec<(u64, Option<TimerId>)> {
        std::mem::take(&mut self.pending_deadline_events)
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn take_pending_move(
        &mut self,
    ) -> Option<skirmish_script_runtime::async_move::PendingMove> {
        self.pending_move.take()
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn set_pending_move(
        &mut self,
        pending: skirmish_script_runtime::async_move::PendingMove,
    ) -> Result<(), &'static str> {
        if self.pending_move.is_some() {
            return Err("native async move is already pending");
        }
        self.pending_move = Some(pending);
        Ok(())
    }

    pub fn animation_due(&self) -> bool {
        self.animation_delivered_generation != Some(self.action_generation)
    }

    pub fn mark_animation_delivered(&mut self) {
        self.animation_delivered_generation = Some(self.action_generation);
    }

    pub fn gate_available(&self, gate: GateKind) -> bool {
        let bit = match gate {
            GateKind::GroundOpen => Self::GROUND_OPEN,
            GateKind::AirOpen => Self::AIR_OPEN,
            GateKind::JumpAvailable => Self::JUMP_AVAILABLE,
        };
        self.availability_mask & bit != 0
    }

    pub fn set_gate(&mut self, gate: GateKind, available: bool) {
        let bit = match gate {
            GateKind::GroundOpen => Self::GROUND_OPEN,
            GateKind::AirOpen => Self::AIR_OPEN,
            GateKind::JumpAvailable => Self::JUMP_AVAILABLE,
        };
        if available {
            self.availability_mask |= bit;
        } else {
            self.availability_mask &= !bit;
        }
    }
}

#[cfg(test)]
mod native_state_tests {
    use super::*;

    #[test]
    fn transitions_are_generation_scoped_and_flags_are_staged() {
        let mut state = NativeEventState::default();
        let first = state
            .begin_action(Some(Action::Wait), Action::Walk)
            .unwrap();
        state.set_last_transition_flags(true, true);
        let second = state
            .begin_action(Some(Action::Walk), Action::Walk)
            .unwrap();
        assert!(second.get() > first.get());
        assert_eq!(state.pending_transitions[0].generation, first);
        assert!(state.pending_transitions[0].preserve_state);
        assert!(state.pending_transitions[0].keep_frame);
        assert_eq!(state.pending_transitions[1].generation, second);
    }

    #[test]
    fn animation_delivery_is_once_per_generation() {
        let mut state = NativeEventState::default();
        assert!(state.animation_due());
        state.mark_animation_delivered();
        assert!(!state.animation_due());
        state
            .begin_action(Some(Action::Wait), Action::Walk)
            .unwrap();
        assert!(state.animation_due());
    }
}

/// Event identities for the existing mutable combat hook context. The native
/// host resolves these identities to its transactional hit context; the event
/// does not duplicate `HitPatch` or pretend that before/after phases have the
/// same mutability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitIdentity {
    pub attacker: u8,
    pub victim: u8,
    pub hitbox_group: u8,
    pub projectile: bool,
}

/// Events are routing records. Mutable combat data remains in the existing
/// native lifecycle transaction selected by [`HitIdentity`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    InputPressed {
        frame: u32,
        player: u8,
        buttons: u16,
    },
    InputReleased {
        frame: u32,
        player: u8,
        buttons: u16,
    },
    StickChanged {
        frame: u32,
        player: u8,
        stick: [f32; 2],
        cstick: [f32; 2],
        trigger: f32,
    },
    ActionAvailabilityChanged {
        frame: u32,
        player: u8,
        gate: GateKind,
        available: bool,
    },
    /// Post-commit transition notification. Entry initialization has already
    /// applied the two native flags carried by this event.
    ActionEntered {
        frame: u32,
        player: u8,
        from: Option<Action>,
        to: Action,
        generation: ActionGeneration,
        /// Whether action-local script state was carried into the destination
        /// action by the committed native transition.
        preserve_state: bool,
        /// Whether the destination action retained the source action frame.
        keep_frame: bool,
    },
    /// Post-commit transition notification. `from` and `to` are snapshots;
    /// the callback observes the destination lifecycle state rather than a
    /// mutable pre-transition fighter view.
    ActionExited {
        frame: u32,
        player: u8,
        from: Action,
        to: Action,
        generation: ActionGeneration,
    },
    AnimationEnded {
        frame: u32,
        player: u8,
        action: Action,
    },
    ScheduledDeadline {
        frame: u32,
        owner: OwnerId,
        timer: TimerId,
        token: u64,
    },
    CommandTraceChanged {
        frame: u32,
        player: u8,
        action: Action,
        command_index: u8,
        value: Option<u32>,
    },
    BeforeHit {
        frame: u32,
        identity: HitIdentity,
    },
    BeforeReceiveHit {
        frame: u32,
        identity: HitIdentity,
    },
    AfterHit {
        frame: u32,
        identity: HitIdentity,
    },
    AfterReceiveHit {
        frame: u32,
        identity: HitIdentity,
    },
    ProjectileContact {
        frame: u32,
        player: u8,
        projectile_owner: Option<u8>,
    },
    Landed {
        frame: u32,
        player: u8,
    },
    SurfaceContact {
        frame: u32,
        player: u8,
        surface: Surface,
        line: usize,
        normal: [f32; 3],
    },
    GroundAirChanged {
        frame: u32,
        player: u8,
        grounded: bool,
        ground_line: Option<usize>,
        floor_normal: [f32; 3],
    },
    PlatformDropDecision {
        frame: u32,
        player: u8,
        on_platform: bool,
        input: Controller,
    },
}

impl Event {
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::InputPressed { .. } => EventKind::InputPressed,
            Self::InputReleased { .. } => EventKind::InputReleased,
            Self::StickChanged { .. } => EventKind::StickChanged,
            Self::ActionAvailabilityChanged { .. } => EventKind::ActionAvailabilityChanged,
            Self::ActionEntered { .. } => EventKind::ActionEntered,
            Self::ActionExited { .. } => EventKind::ActionExited,
            Self::AnimationEnded { .. } => EventKind::AnimationEnded,
            Self::ScheduledDeadline { .. } => EventKind::ScheduledDeadline,
            Self::CommandTraceChanged { .. } => EventKind::CommandTraceChanged,
            Self::BeforeHit { .. } => EventKind::BeforeHit,
            Self::BeforeReceiveHit { .. } => EventKind::BeforeReceiveHit,
            Self::AfterHit { .. } => EventKind::AfterHit,
            Self::AfterReceiveHit { .. } => EventKind::AfterReceiveHit,
            Self::ProjectileContact { .. } => EventKind::ProjectileContact,
            Self::Landed { .. } => EventKind::Landed,
            Self::SurfaceContact { .. } => EventKind::SurfaceContact,
            Self::GroundAirChanged { .. } => EventKind::GroundAirChanged,
            Self::PlatformDropDecision { .. } => EventKind::PlatformDropDecision,
        }
    }

    pub const fn frame(&self) -> u32 {
        match self {
            Self::InputPressed { frame, .. }
            | Self::InputReleased { frame, .. }
            | Self::StickChanged { frame, .. }
            | Self::ActionAvailabilityChanged { frame, .. }
            | Self::ActionEntered { frame, .. }
            | Self::ActionExited { frame, .. }
            | Self::AnimationEnded { frame, .. }
            | Self::ScheduledDeadline { frame, .. }
            | Self::CommandTraceChanged { frame, .. }
            | Self::BeforeHit { frame, .. }
            | Self::BeforeReceiveHit { frame, .. }
            | Self::AfterHit { frame, .. }
            | Self::AfterReceiveHit { frame, .. }
            | Self::ProjectileContact { frame, .. }
            | Self::Landed { frame, .. }
            | Self::SurfaceContact { frame, .. }
            | Self::GroundAirChanged { frame, .. }
            | Self::PlatformDropDecision { frame, .. } => *frame,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A fixed native event table. There is no wildcard key and no callback value
/// to invoke, which keeps dispatch structurally unable to self-register work.
/// Frontends should compile their event declarations into this table once at
/// resource load and expose only the resulting event payload to script code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispatcher {
    subscriptions: BTreeMap<EventKind, BTreeSet<SubscriptionId>>,
    owners: BTreeMap<SubscriptionId, OwnerId>,
    next_id: u64,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self {
            subscriptions: BTreeMap::new(),
            owners: BTreeMap::new(),
            next_id: 1,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DispatchError {
    #[error("event subscription limit of {MAX_SUBSCRIPTIONS} exceeded")]
    TooManySubscriptions,
    #[error("subscription ID space exhausted")]
    IdExhausted,
}

impl Dispatcher {
    pub fn subscribe(
        &mut self,
        owner: OwnerId,
        kind: EventKind,
    ) -> Result<SubscriptionId, DispatchError> {
        if self.owners.len() >= MAX_SUBSCRIPTIONS {
            return Err(DispatchError::TooManySubscriptions);
        }
        let id = SubscriptionId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(DispatchError::IdExhausted)?;
        self.subscriptions.entry(kind).or_default().insert(id);
        self.owners.insert(id, owner);
        Ok(id)
    }

    pub fn cancel(&mut self, id: SubscriptionId) -> bool {
        if self.owners.remove(&id).is_none() {
            return false;
        }
        for ids in self.subscriptions.values_mut() {
            ids.remove(&id);
        }
        true
    }

    pub fn cancel_owner(&mut self, owner: OwnerId) {
        let ids = self
            .owners
            .iter()
            .filter_map(|(id, candidate)| (*candidate == owner).then_some(*id))
            .collect::<Vec<_>>();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Return matching subscription IDs in stable registration-ID order.
    pub fn dispatch(&self, event: &Event) -> Vec<SubscriptionId> {
        self.subscriptions
            .get(&event.kind())
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.owners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_names_match_the_python_allowlist() {
        assert_eq!(
            EventKind::from_name("before_hit"),
            Some(EventKind::BeforeHit)
        );
        assert_eq!(EventKind::from_name("combat"), None);
        assert_eq!(EventKind::from_name("frame"), None);
        assert_eq!(EventKind::ALL.len(), 18);
    }

    #[test]
    fn dispatcher_only_routes_explicit_kinds() {
        let owner = OwnerId::new(7);
        let mut dispatcher = Dispatcher::default();
        let input = dispatcher
            .subscribe(owner, EventKind::InputPressed)
            .unwrap();
        let action = dispatcher
            .subscribe(owner, EventKind::ActionEntered)
            .unwrap();
        let event = Event::InputPressed {
            frame: 1,
            player: 0,
            buttons: 1,
        };
        assert_eq!(dispatcher.dispatch(&event), vec![input]);
        assert!(
            dispatcher
                .dispatch(&Event::Landed {
                    frame: 1,
                    player: 0
                })
                .is_empty()
        );
        assert!(dispatcher.cancel(action));
        assert_eq!(dispatcher.len(), 1);
    }

    #[test]
    fn availability_state_is_a_small_native_gate_mask() {
        let mut state = NativeEventState::default();
        assert!(!state.gate_available(GateKind::GroundOpen));
        state.set_gate(GateKind::GroundOpen, true);
        state.set_gate(GateKind::JumpAvailable, true);
        assert!(state.gate_available(GateKind::GroundOpen));
        assert!(state.gate_available(GateKind::JumpAvailable));
        assert!(!state.gate_available(GateKind::AirOpen));
    }
}
