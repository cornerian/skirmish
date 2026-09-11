#![allow(dead_code)] // Each integration target uses a different subset.
use skirmish::game::{data::MatchData, taunt::Taunt};

/// Install the invented `tests/fixtures/game/taunt.json` right/left motions
/// on both fighters. Its bones match the synthetic integration skeleton.
pub fn profile(mut data: MatchData) -> MatchData {
    let taunt: Taunt = serde_json::from_str(include_str!("../fixtures/game/taunt.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.taunt = Some(taunt.clone());
    }
    data
}

/// The right taunt only (no left motion supplied): facing left still picks
/// AppealSR.
pub fn right_only(mut data: MatchData) -> MatchData {
    let taunt: Taunt = serde_json::from_str(include_str!("../fixtures/game/taunt.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.taunt = Some(Taunt {
            right: taunt.right.clone(),
            left: None,
        });
    }
    data
}
