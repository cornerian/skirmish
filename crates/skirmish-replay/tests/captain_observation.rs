//! Runtime-backed Captain Falcon action-state mapping.
//!
//! This test asserts Captain Falcon's bundled state and animation metadata.

use skirmish::game::{self, script::definition};
use skirmish_replay::observation;

const CAPTAIN_EXTERNAL_ID: u8 = 0;

#[test]
fn captain_runtime_resolves_recorded_states_and_animation_metadata() {
    let data = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .expect("integration game fixture");
    let game = game::Match::new(data, 1).expect("integration game");
    let mut fighter = game.state().fighters[0].clone();

    for (action, state, animation) in [
        (game::Action::SpecialNStart, 347, 301),
        (game::Action::SpecialAirNStart, 348, 302),
        (game::Action::SpecialSStart, 349, 303),
        (game::Action::SpecialS, 350, 304),
        (game::Action::SpecialAirSStart, 351, 305),
        (game::Action::SpecialAirS, 352, 306),
        (game::Action::SpecialHi, 353, 307),
        (game::Action::SpecialAirHi, 354, 308),
        (game::Action::SpecialLwEnd, 360, 314),
        (game::Action::SpecialAirLw, 359, 313),
        (game::Action::SpecialAirLwEnd, 361, 316),
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
            Some((state, animation)),
            "bundled Captain metadata pair for {action:?}"
        );
        assert_eq!(
            observation::animation_index(&fighter, Some(CAPTAIN_EXTERNAL_ID)),
            Some(animation),
            "observation animation metadata for {action:?}"
        );
    }
}
