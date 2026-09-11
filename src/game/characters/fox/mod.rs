//! Fox's registry entry: which special moves he plays, in dispatch
//! priority order, and the Slippi state/animation ids his phases report.
//!
//! To add one of Fox's queued specials: write its own file next to `side.rs`
//! implementing `specials::SpecialMove`, add it to [`MOVES`] below (ahead of
//! [`specials::neutral::Move`] if the source also checks it first), and add
//! its state/animation ids to [`slippi_ids`]. Nothing outside this file
//! needs to change.

pub mod down;
pub mod side;

use crate::game::{Action, specials};

/// Fox's specials, in the same priority the grounded/aerial dispatch chains
/// check them: the side special ahead of the shared neutral shell, ahead of
/// the down special (the source's own grounded chain checks SpecialS,
/// SpecialHi (unmodeled), SpecialN, then SpecialLw in that fixed order;
/// down.rs's own module doc explains why the aerial dispatcher's different
/// real order does not require a different Rust iteration order here).
pub(crate) const MOVES: &[&dyn specials::SpecialMove] =
    &[&side::MOVE, &specials::neutral::MOVE, &down::MOVE];

/// External Slippi character ids that play Fox's move set. Falco (22)
/// shares Fox's own source file for these moves but keeps its own
/// attributes; this profile follows the neutral shell's existing precedent
/// of only gating Fox (2) at the observation layer, so 22 is documented
/// here without being included below.
pub(crate) const CHARACTER_IDS: [u8; 1] = [2];

/// The Slippi action-state id and its (possibly extrapolated) animation
/// index for one of Fox's own motion states, or `None` when `action` is not
/// one of them (the caller falls back to the common table).
pub(crate) fn slippi_ids(action: Action) -> Option<(u32, u32)> {
    use Action::*;
    Some(match action {
        SpecialN => (341, 295),
        SpecialAirN => (344, 298),
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
