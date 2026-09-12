//! Per-character move sets: resource declarations, the Slippi state/animation
//! id tables, dispatch state machines, and the pure per-character scalar
//! arithmetic those state machines call.
//!
//! Each character is one subtree implementing `common::SpecialMove` for its
//! special moves — adding a character means adding its subtree, a `Specials`
//! variant below, and a registry arm in [`moves`]/[`slippi_ids`]; shared
//! special-move machinery lives in [`common`] and per-character arithmetic in
//! [`arithmetic`].

pub mod arithmetic;
pub mod common;
pub mod fox;

use crate::game::Action;
use common::SpecialMove;
use serde::{Deserialize, Serialize};

/// A fighter's special-move resources, tagged by which character's move set
/// they belong to. Each variant carries every one of that character's moves
/// as its own `Option`. `neutral` on both variants is Fox's own dedicated
/// Blaster resource (`characters::fox::neutral::NeutralSpecial`), not the
/// generic shared shell (`characters::common`'s retired `neutral` shell -- see
/// `docs/fox-neutral-special.md`); Falco's own Laser reuses the identical
/// type (same source file, same attribute shape, see below), but stays
/// `None` in every export so far.
///
/// `Falco` carries the exact same field types as `Fox` rather than its own
/// resource shapes: `ftFc_Init_MotionStateTable` (`ftfalco.c:23-370`) points
/// every one of Falco's own `ftFx_MS_Special*` entries at the identical
/// `ftFx_Special*_{Anim,IASA,Phys,Coll}` callbacks Fox's own table uses, and
/// `ftFc_Init_LoadSpecialAttrs` (`ftfalco.c`) forwards straight to
/// `ftFx_Init_LoadSpecialAttrs` (`ftfox.c:503-506`), and `ftFc_Init_OnLoad`
/// calls `ftFx_Init_OnLoadForFalco` (`ftfox.c:481-484`), which load Falco's
/// own `PlFc.dat` attributes through the same `ftFox_DatAttrs` shape Fox's
/// own load uses. The only `FTKIND_FALCO`
/// branch in any of the three moves' source
/// (`ftfoxspecials.c:247-262`, `ftFox_SpecialS_CreateGhostItem`) just picks
/// the cosmetic ghost-item kind for the side special's already-unmodeled
/// ghost item (see `fox::side`'s own doc); the up special (Fire Bird,
/// `ftfoxspecialhi.c`) and down special (Reflector, `ftfoxspeciallw.c`)
/// have no Falco-kind branch at all, so this is a genuinely data-only
/// difference, not a behavioral one, and duplicating `fox::side::
/// SideSpecial`/`fox::up::UpSpecial`/`fox::down::DownSpecial` as
/// Falco-specific types would just be copies with no distinct fields.
/// `ftfoxspecialn.c` (Blaster) has an equivalent `FTKIND_FALCO` branch
/// (`foxSFX`/`falcoSFX` in `ftFox_SpecialN_FireBlasterShot`) picking only
/// cosmetic SFX, so `fox::neutral::NeutralSpecial` is reused verbatim here
/// too, for the same reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "character")]
pub enum Specials {
    /// `#[serde(alias = "fox")]`: the standalone `fighters/fox.json` sample
    /// writes `specials.character` capitalized (`"Fox"`), but the composed
    /// pairing exports that also carry a Falco fighter (`fox-falco-fd/`,
    /// `falco-fox-fd/match-data.json`) write every `character` tag
    /// lowercase instead, `"fox"` included -- so this tag accepts both
    /// spellings, matching `Falco`'s own `"falco"` alias below.
    #[serde(alias = "fox")]
    Fox {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        neutral: Option<fox::neutral::NeutralSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        side: Option<fox::side::SideSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        up: Option<fox::up::UpSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        down: Option<fox::down::DownSpecial>,
    },
    /// `#[serde(alias = "falco")]`: every export that carries a Falco
    /// fighter (`fighters/falco.json`, `fox-falco-fd/`, `falco-fox-fd/
    /// match-data.json`) writes `specials.character` lowercase (`"falco"`),
    /// so the tag accepts that spelling without the pack being re-exported.
    #[serde(alias = "falco")]
    Falco {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        neutral: Option<fox::neutral::NeutralSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        side: Option<fox::side::SideSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        up: Option<fox::up::UpSpecial>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        down: Option<fox::down::DownSpecial>,
    },
}

impl Specials {
    pub(crate) fn fox_neutral(&self) -> Option<&fox::neutral::NeutralSpecial> {
        match self {
            Specials::Fox { neutral, .. } | Specials::Falco { neutral, .. } => neutral.as_ref(),
        }
    }

    /// Named for the module that owns this move's dispatch/validation code
    /// (`fox::side`); also serves `Specials::Falco`, which reuses that same
    /// module (see the enum's own doc).
    pub(crate) fn fox_side(&self) -> Option<&fox::side::SideSpecial> {
        match self {
            Specials::Fox { side, .. } | Specials::Falco { side, .. } => side.as_ref(),
        }
    }

    /// See `fox_side`'s doc: also serves `Specials::Falco`.
    pub(crate) fn fox_up(&self) -> Option<&fox::up::UpSpecial> {
        match self {
            Specials::Fox { up, .. } | Specials::Falco { up, .. } => up.as_ref(),
        }
    }

    /// See `fox_side`'s doc: also serves `Specials::Falco`.
    pub(crate) fn fox_down(&self) -> Option<&fox::down::DownSpecial> {
        match self {
            Specials::Fox { down, .. } | Specials::Falco { down, .. } => down.as_ref(),
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
        Some(Specials::Fox { .. } | Specials::Falco { .. }) => fox::MOVES,
        None => &[],
    }
}

/// The Slippi action-state id and animation index for one of a character's
/// own motion states, keyed by the recording's external character id
/// (`character` in the observation layer's own inputs; Fox is 2, Falco 20,
/// `crates/cli/src/initialization.rs`'s `CHARACTER_EXTERNAL_IDS`). `None`
/// when `character` plays no registered character-specific moves, or
/// `action` is not one of them.
pub fn slippi_ids(character: Option<u8>, action: Action) -> Option<(u32, u32)> {
    let id = character?;
    if fox::CHARACTER_IDS.contains(&id) {
        return fox::slippi_ids(action);
    }
    None
}
