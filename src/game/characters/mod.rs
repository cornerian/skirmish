//! Per-character special-move registries. `Specials` is the resource shape
//! a fighter's data carries (one variant per playable character, each
//! holding its own moves' `Option`al parameters); the modules underneath
//! list what those moves are and how their Slippi ids resolve.
//!
//! Adding a whole new character means adding a `Specials` variant here, a
//! `characters::<name>` module with its own registry entry (mirroring
//! `fox::MOVES`), and wiring its external character id into [`moves`] and
//! [`slippi_ids`]. Adding a move to a character that already exists here
//! only touches that character's own module (see `fox::MOVES`'s doc).

pub mod fox;

use super::{Action, specials::SpecialMove, specials::neutral};
use serde::{Deserialize, Serialize};

/// A fighter's special-move resources, tagged by which character's move set
/// they belong to. Each variant carries every one of that character's moves
/// as its own `Option`, including the shared neutral shell (`neutral`),
/// since even the shared shell's ground/air poses are per-character data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "character")]
pub enum Specials {
    Fox {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        neutral: Option<neutral::Parameters>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        side: Option<fox::side::SideSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        down: Option<fox::down::DownSpecial>,
    },
}

impl Specials {
    pub(crate) fn neutral(&self) -> Option<&neutral::Parameters> {
        match self {
            Specials::Fox { neutral, .. } => neutral.as_ref(),
        }
    }

    pub(crate) fn fox_side(&self) -> Option<&fox::side::SideSpecial> {
        match self {
            Specials::Fox { side, .. } => side.as_ref(),
        }
    }

    pub(crate) fn fox_down(&self) -> Option<&fox::down::DownSpecial> {
        match self {
            Specials::Fox { down, .. } => down.as_ref(),
        }
    }
}

/// Every move any registered character can play, independent of which
/// fighter's resources enable it this match. Used by bare `Action`-keyed
/// queries (collision mode, ledge catchability, phase ownership) that only
/// need to know which move a state belongs to.
pub(crate) const ALL_MOVES: &[&dyn SpecialMove] = fox::MOVES;

/// The moves a fighter's own `specials` resource makes available this
/// match, in dispatch priority order.
pub(crate) fn moves(specials: Option<&Specials>) -> &'static [&'static dyn SpecialMove] {
    match specials {
        Some(Specials::Fox { .. }) => fox::MOVES,
        None => &[],
    }
}

/// The Slippi action-state id and animation index for one of a character's
/// own motion states, keyed by the recording's external character id
/// (`character` in the observation layer's own inputs). `None` when
/// `character` plays no registered character-specific moves, or `action`
/// is not one of them.
pub fn slippi_ids(character: Option<u8>, action: Action) -> Option<(u32, u32)> {
    let id = character?;
    if fox::CHARACTER_IDS.contains(&id) {
        return fox::slippi_ids(action);
    }
    None
}
