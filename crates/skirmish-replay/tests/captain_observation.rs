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
        (game::Action::SpecialLw, 357, 311),
        (game::Action::SpecialLwGroundEnd, 358, 312),
        (game::Action::SpecialLwEndAir, 362, 315),
        (game::Action::SpecialAirLw, 359, 313),
        (game::Action::SpecialAirLwLandingEnd, 360, 314),
        (game::Action::SpecialAirLwEndAir, 361, 316),
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

fn captain_at_frame(
    replay: &skirmish_replay::slippi::Replay,
    frame_id: i32,
) -> peppi_adapter::Actor {
    let index = replay
        .frame_indices(skirmish_replay::slippi::Timeline::LastRecorded)
        .unwrap()
        .iter()
        .copied()
        .find(|&index| replay.frame(index).unwrap().id == frame_id)
        .unwrap_or_else(|| panic!("recorded frame {frame_id} is missing"));
    replay
        .frame(index)
        .unwrap()
        .actors
        .into_iter()
        .find(|actor| actor.port == peppi_adapter::Port::P4 && !actor.follower)
        .unwrap_or_else(|| panic!("Captain Falcon leader missing at frame {frame_id}"))
}

#[test]
fn captain_real_slippi_states_match_bundled_definition_metadata() {
    let replay = skirmish_replay::slippi::Replay::read(std::io::Cursor::new(include_bytes!(
        "../../../tests/fixtures/slippi/01-marth-dr-mario-yoshi-captain-falcon-battlefield.slp"
    )))
    .unwrap();

    for (frame_id, action, state, animation) in [
        (522, game::Action::SpecialAirHi, 354, 308),
        (6594, game::Action::SpecialAirLw, 359, 313),
        (6623, game::Action::SpecialAirLwEndAir, 361, 316),
    ] {
        let actor = captain_at_frame(&replay, frame_id);
        assert_eq!(
            definition::builtin_slippi_ids(Some(CAPTAIN_EXTERNAL_ID), action),
            Some((state, animation)),
            "bundled Captain metadata for frame {frame_id}"
        );
        assert_eq!(
            actor.post.character, 2,
            "Captain internal ID at frame {frame_id}"
        );
        assert_eq!(
            u32::from(actor.post.state),
            state,
            "recorded state at frame {frame_id}"
        );
        // This fixture is Slippi 3.9, before post.animation_index was
        // recorded. The definition assertion above still checks the expected
        // 308/313/316 animation paired with each raw source state.
    }
}

#[test]
fn captain_real_slippi_special_state_spans_are_preserved() {
    let replay = skirmish_replay::slippi::Replay::read(std::io::Cursor::new(include_bytes!(
        "../../../tests/fixtures/slippi/01-marth-dr-mario-yoshi-captain-falcon-battlefield.slp"
    )))
    .unwrap();

    let mut spans = Vec::new();
    for index in replay
        .frame_indices(skirmish_replay::slippi::Timeline::LastRecorded)
        .unwrap()
        .iter()
        .copied()
    {
        let frame = replay.frame(index).unwrap();
        let actor = frame
            .actors
            .iter()
            .find(|actor| actor.port == peppi_adapter::Port::P4 && !actor.follower)
            .unwrap_or_else(|| panic!("Captain Falcon leader missing at frame {}", frame.id));
        assert_eq!(
            actor.post.character, 2,
            "Captain Falcon must remain the internal-ID-2 actor at frame {}",
            frame.id
        );

        let state = u32::from(actor.post.state);
        if !(347..=363).contains(&state) {
            continue;
        }
        if let Some((last_state, _start, end)) = spans.last_mut()
            && *last_state == state
            && *end + 1 == frame.id
        {
            *end = frame.id;
        } else {
            spans.push((state, frame.id, frame.id));
        }
    }

    assert_eq!(
        spans,
        vec![
            (354, 522, 585),
            (354, 2066, 2128),
            (354, 2397, 2460),
            (354, 3929, 3992),
            (359, 6594, 6622),
            (361, 6623, 6651),
        ]
    );
}
