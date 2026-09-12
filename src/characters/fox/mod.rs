//! Fox's registry entry: which special moves he plays, in dispatch
//! priority order, and the Slippi state/animation ids his phases report.
//! Falco plays the exact same moves through the exact same registry entry
//! (`CHARACTER_IDS`, `slippi_ids`) -- see `characters::Specials`'s own
//! doc for why the decomp makes this a data-only difference, not a
//! behavioral one.
//!
//! To add one of Fox's queued specials: write its own file next to `side.rs`
//! implementing `specials::SpecialMove`, add it to [`MOVES`] below in the
//! source's own dispatch priority, and add its state/animation ids to
//! [`slippi_ids`]. Nothing outside this file needs to change.

pub mod down;
pub mod neutral;
pub mod side;
pub mod up;

use super::common as specials;
use crate::game::Action;

/// Fox's specials, in the same priority the grounded/aerial dispatch chains
/// check them: the side special first (grounded: `ftCo_Attack100_
/// CheckInput` is checked after `SpecialS`; aerial: the side branch already
/// defers to the up special whenever the stick clears the vertical
/// threshold, so this order reproduces both dispatch priorities), then the
/// up special, then Blaster (`neutral`, Fox's own dedicated module -- the
/// shared generic shell is retired for Fox, see `docs/
/// fox-neutral-special.md`), then the down special (the source's own
/// grounded chain checks SpecialS, SpecialHi, SpecialN, then SpecialLw in
/// that fixed order; down.rs's own module doc explains why the aerial
/// dispatcher's different real order does not require a different Rust
/// iteration order here).
pub(crate) const MOVES: &[&dyn specials::SpecialMove] =
    &[&side::MOVE, &up::MOVE, &neutral::MOVE, &down::MOVE];

/// External Slippi CSS character ids that play this move set: Fox (2) and
/// Falco (20, `crates/cli/src/initialization.rs`'s `CHARACTER_EXTERNAL_IDS`).
/// Falco's own internal fighter kind is a different number, `FTKIND_FALCO`
/// (22, `ft/forward.h:112`) -- not to be confused with the external CSS id
/// this table keys on -- and shares Fox's own source file for these moves
/// (`ftFc_Init_MotionStateTable`, `ftfalco.c:23-370`, points every one of
/// Falco's `ftFx_MS_Special*` entries at the identical Fox callbacks) while
/// keeping its own attributes (`characters::Specials::Falco`'s doc).
pub(crate) const CHARACTER_IDS: [u8; 2] = [2, 20];

/// The Slippi action-state id and its (possibly extrapolated) animation
/// index for one of Fox's own motion states, or `None` when `action` is not
/// one of them (the caller falls back to the common table). Also used for
/// Falco (`CHARACTER_IDS`'s own doc): `ftFc_Init_MotionStateTable` gives
/// Falco the identical `ftFx_MS_Special*` state ids used below, so no
/// separate Falco table exists; the animation indices carry the same
/// unverified extrapolation this module already flags for Fox, now assumed
/// (not separately confirmed) to extend to Falco's own figatree too.
pub(crate) fn slippi_ids(action: Action) -> Option<(u32, u32)> {
    use Action::*;
    Some(match action {
        // `ftFx_MS_SpecialNStart == ftCo_MS_Count`, confirmed by counting
        // backward from the side special's own confirmed `ftCo_MS_Count +
        // 6 == 347`; Loop/End and the air trio follow in source
        // declaration order (341..346). Animation indices 296/297/299/300
        // continue the same unverified -46 offset the 295/298 endpoints
        // already established; see `docs/fox-neutral-special.md`.
        SpecialNStart => (341, 295),
        SpecialNLoop => (342, 296),
        SpecialNEnd => (343, 297),
        SpecialAirNStart => (344, 298),
        SpecialAirNLoop => (345, 299),
        SpecialAirNEnd => (346, 300),
        // `ftFx_MS_SpecialSStart` is `ftCo_MS_Count + 6`; Start/Dash/End
        // follow it in source declaration order (347..352). The animation
        // indices are an unverified extrapolation: see docs/fox-side-
        // special.md's "Resource and state shape" section for the two
        // confirmed data points (341->295, 344->298) this assumes a
        // constant -46 offset continues from.
        SpecialSStart => (347, 301),
        SpecialS => (348, 302),
        SpecialSEnd => (349, 303),
        SpecialAirSStart => (350, 304),
        SpecialAirS => (351, 305),
        SpecialAirSEnd => (352, 306),
        // `ftFx_MS_SpecialHiHold` follows `ftFx_MS_SpecialAirSEnd` in source
        // declaration order (353..359, `ftFox/forward.h:64-70`). The
        // animation indices continue the same unverified -46 extrapolation
        // as the side special's own; see docs/fox-up-special.md.
        SpecialHiHold => (353, 307),
        SpecialHiHoldAir => (354, 308),
        SpecialHi => (355, 309),
        SpecialAirHi => (356, 310),
        SpecialHiLanding => (357, 311),
        SpecialHiFall => (358, 312),
        SpecialHiBound => (359, 313),
        // `ftFx_MS_SpecialLwStart` is confirmed `ftCo_MS_Count + 6 + 4`
        // (`ftFox/forward.h:71-80`: it directly follows the side special's
        // own five states in source declaration order), so 360..369 are
        // exact, not extrapolated. Their animation indices reuse the same
        // unverified -46 offset from the state id the side special's own
        // two confirmed data points established (see the module doc there);
        // `ftFx_SM_SpecialLwTurn`/`SpecialAirLwTurn` have no entry of their
        // own in forward.h's SM_ list (only Start/Loop/Hit/End do), so Turn
        // reuses Loop's own animation index, consistent with Turn being a
        // brief interruption of the Loop cycle rather than a separate asset.
        SpecialLwStart => (360, 314),
        SpecialLw => (361, 315),
        SpecialLwHit => (362, 316),
        SpecialLwEnd => (363, 317),
        SpecialLwTurn => (364, 315),
        SpecialAirLwStart => (365, 319),
        SpecialAirLw => (366, 320),
        SpecialAirLwHit => (367, 321),
        SpecialAirLwEnd => (368, 322),
        SpecialAirLwTurn => (369, 320),
        _ => return None,
    })
}
