#![allow(dead_code)]

use skirmish::game::{data::MatchData, idle::IdleAnimations, idle::IdleEntry};

/// Invented idle table: Wait1 (sub-motion 2) restarts every `wait1_length`
/// frame; once entries are present, a draw always fires at length and (since
/// the only nonzero-weight entry the default two-entry table lists is
/// Wait1_0 itself, weight 60) is accepted immediately from Wait1_0 without a
/// re-draw (`inlineA0`'s "current == 2" exemption). `TWO_ENTRY` mirrors the
/// design note's own two-entry example (60/40).
pub const WAIT1_LENGTH: f32 = 3.0;
pub const TWO_ENTRY: [IdleEntry; 2] = [
    IdleEntry {
        animation: 2,
        weight: 60,
        length: WAIT1_LENGTH,
    },
    IdleEntry {
        animation: 3,
        weight: 40,
        length: 5.0,
    },
];

/// A single always-current-animation entry: whatever the current animation
/// is, picking again from a one-row 100-weight table always re-selects it,
/// so this fires exactly one `HSD_Randi` draw per restart regardless of the
/// draw's value (`inlineA0` accepts every repeat from Wait1_0/31; this table
/// only ever contains Wait1_0).
pub const SINGLE_ENTRY: [IdleEntry; 1] = [IdleEntry {
    animation: 2,
    weight: 100,
    length: WAIT1_LENGTH,
}];

pub fn animations(entries: &[IdleEntry]) -> IdleAnimations {
    IdleAnimations {
        wait1_length: WAIT1_LENGTH,
        entries: entries.to_vec(),
    }
}

/// Install `idle` (the two-entry table) on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    for fighter in &mut data.fighters {
        fighter.idle = Some(animations(&TWO_ENTRY));
    }
    data
}
