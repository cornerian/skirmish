//! Runtime-backed Captain Falcon action-state mapping.
//!
//! This test deliberately goes through the bundled Pon registry rather than
//! reproducing Captain's metadata in Rust.  The state ids are available even
//! while animation ids remain absent from the partial authoring slice.

use skirmish::game::{self, script::definition};
use skirmish_replay::observation;

const CAPTAIN_EXTERNAL_ID: u8 = 0;

#[test]
fn captain_runtime_resolves_recorded_states_without_animation_ids() {
    let data = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .expect("integration game fixture");
    let game = game::Match::new(data, 1).expect("integration game");
    let mut fighter = game.state().fighters[0].clone();

    for (action, state) in [
        (game::Action::SpecialNStart, 347),
        (game::Action::SpecialAirNStart, 348),
        (game::Action::SpecialHi, 353),
        (game::Action::SpecialAirHi, 354),
        (game::Action::SpecialAirLw, 359),
        (game::Action::SpecialAirLwEnd, 361),
    ] {
        fighter.action = action;
        assert_eq!(
            definition::builtin_slippi_state(Some(CAPTAIN_EXTERNAL_ID), action),
            Some(state),
            "bundled Captain state for {action:?}"
        );
        assert_eq!(
            observation::action_state(&fighter, Some(CAPTAIN_EXTERNAL_ID)),
            Some(state as u16),
            "observation state for {action:?}"
        );
        assert_eq!(
            definition::builtin_slippi_ids(Some(CAPTAIN_EXTERNAL_ID), action),
            None,
            "Captain animation pair remains unresolved for {action:?}"
        );
        assert_eq!(
            observation::animation_index(&fighter, Some(CAPTAIN_EXTERNAL_ID)),
            None,
            "observation animation remains unresolved for {action:?}"
        );
    }
}
