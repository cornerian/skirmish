//! Rollback-safe identity for a selected class move.
//!
//! Native actions are canonical engine states and therefore are not unique
//! move identities.  A moveset entry supplies the behavior index that owns
//! the callbacks; this record travels with the action generation so callback
//! routing cannot fall back to behavior declaration order.

use super::{move_registry::MoveEntry, scheduler::ActionGeneration};
use crate::game::Action;
use serde::{Deserialize, Serialize};

/// Logical lifetime of a selected class move. This is independent of the
/// native action generation: an owned move may cross several declared native
/// phases while retaining this value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MoveLifetimeId(u64);

impl MoveLifetimeId {
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingMoveSelection {
    pub behavior_index: usize,
    pub canonical: Option<Action>,
    pub lifetime: MoveLifetimeId,
}

/// The behavior selected for the current native action generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedMove {
    pub behavior_index: usize,
    pub action: Action,
    pub generation: ActionGeneration,
    #[serde(default)]
    pub lifetime: MoveLifetimeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeMoveSelection {
    Entered(Action),
    CallbackDriven { behavior_index: usize },
    Unbound(Action),
}

/// Select a registered move and stage its owner before native entry. The
/// pending owner is consumed atomically by `NativeEventState::begin_action`.
pub(crate) fn select_native_move(
    fighter: &mut crate::game::Fighter,
    data: &crate::game::data::FighterData,
    group: super::move_registry::MoveGroup,
    slot: super::move_registry::MoveSlot,
    fallback: Action,
) -> NativeMoveSelection {
    select_native_move_variant(fighter, data, group, slot, fallback, fallback)
}

/// Select a move while preserving a native input variant. A registration
/// whose canonical action equals `default_entry` owns the callbacks but uses
/// `native_variant` for the actual transition (for example forward/back
/// native variants sharing one class entry).
pub(crate) fn select_native_move_variant(
    fighter: &mut crate::game::Fighter,
    data: &crate::game::data::FighterData,
    group: super::move_registry::MoveGroup,
    slot: super::move_registry::MoveSlot,
    default_entry: Action,
    native_variant: Action,
) -> NativeMoveSelection {
    let Some(program) = super::definition::cached_program(data) else {
        crate::game::simulation::enter(fighter, native_variant);
        return NativeMoveSelection::Unbound(native_variant);
    };
    let Some(entry) = program.moves().resolve_typed(&group, &slot) else {
        crate::game::simulation::enter(fighter, native_variant);
        return NativeMoveSelection::Unbound(native_variant);
    };
    let Some(action) = entry.canonical else {
        if let Err(error) = fighter
            .script_events
            .stage_move_selection(entry, fighter.action)
        {
            fighter.script_events.record_native_error(error);
            return NativeMoveSelection::Unbound(native_variant);
        }
        return NativeMoveSelection::CallbackDriven {
            behavior_index: entry.behavior_index,
        };
    };
    if let Err(error) = fighter
        .script_events
        .stage_move_selection(entry, native_variant)
    {
        fighter.script_events.record_native_error(error);
        return NativeMoveSelection::Unbound(native_variant);
    }
    let destination = if action == default_entry {
        native_variant
    } else {
        action
    };
    crate::game::simulation::enter(fighter, destination);
    NativeMoveSelection::Entered(destination)
}

impl SelectedMove {
    /// Construct an owner record after the native transition generation has
    /// been staged.  Callback dispatch must use this record only for the
    /// matching generation.
    pub const fn new(entry: MoveEntry, generation: ActionGeneration) -> Option<Self> {
        let Some(action) = entry.canonical else {
            return None;
        };
        Some(Self {
            behavior_index: entry.behavior_index,
            action,
            generation,
            lifetime: MoveLifetimeId::from_raw(0),
        })
    }

    pub const fn with_lifetime(
        entry: MoveEntry,
        action: Action,
        generation: ActionGeneration,
        lifetime: MoveLifetimeId,
    ) -> Self {
        Self {
            behavior_index: entry.behavior_index,
            action,
            generation,
            lifetime,
        }
    }

    pub fn matches(self, action: Action, generation: ActionGeneration) -> bool {
        self.action == action && self.generation == generation
    }
}
