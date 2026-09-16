//! Regression observations from the checked-in four-player Captain Falcon game.
//!
//! These are parser/input/action-state facts from the recording. They are
//! deliberately not simulator expectations: this test provides stable replay
//! evidence for a future native-parity harness.

use peppi_adapter::{Port, Replay, Timeline, Version};

const REPLAY: &[u8] = include_bytes!(
    "../../../tests/fixtures/slippi/01-marth-dr-mario-yoshi-captain-falcon-battlefield.slp"
);

fn captain(replay: &Replay, index: usize) -> peppi_adapter::Actor {
    replay
        .frame(index)
        .unwrap()
        .actors
        .into_iter()
        .find(|actor| actor.port == Port::P4 && !actor.follower)
        .unwrap_or_else(|| panic!("Captain Falcon leader missing at frame index {index}"))
}

#[test]
fn captain_fixture_preserves_metadata_inputs_and_action_observations() {
    let replay = Replay::read(std::io::Cursor::new(REPLAY)).unwrap();
    let summary = replay.summary(Timeline::LastRecorded).unwrap();

    assert_eq!(summary.version, Version(3, 9, 0));
    assert_eq!(summary.stage, 31); // Battlefield
    assert_eq!(summary.ports, [Port::P1, Port::P2, Port::P3, Port::P4]);
    assert_eq!(summary.physical_frames, 8_638);
    assert_eq!(summary.first_frame, Some(-123));
    assert_eq!(summary.last_frame, Some(8_514));
    assert_eq!(summary.latest_finalized_frame, Some(8_514));
    assert_eq!(summary.end_method, peppi_adapter::peppi::game::EndMethod::Resolved);

    // GameStart uses the external CSS character ID. Captain Falcon is P4
    // (external ID 0); frame Post uses the corresponding internal ID 2.
    let players = &replay.game().start.players;
    assert_eq!(players.len(), 4);
    assert_eq!(players.iter().find(|p| p.port == Port::P4).unwrap().character, 0);

    // Frame 522 is an actual recorded Captain action-state transition. Keep
    // both controller observations and the resulting state so future parity
    // work can consume this as a real replay input/post-state pair.
    let actor = captain(&replay, 645);
    assert_eq!(replay.frame(645).unwrap().id, 522);
    assert_eq!(actor.pre.state, 27);
    assert_eq!(actor.pre.joystick.x.to_bits(), (-0.6625_f32).to_bits());
    assert_eq!(actor.pre.joystick.y.to_bits(), 0.7375_f32.to_bits());
    assert_eq!(actor.pre.buttons, 66_048);
    assert_eq!(actor.pre.buttons_physical, 512);
    assert_eq!(actor.post.character, 2);
    assert_eq!(actor.post.state, 354);
    assert_eq!(actor.post.state_age.unwrap().to_bits(), 1.0_f32.to_bits());
    assert_eq!(actor.post.airborne, Some(1));
    assert_eq!(actor.post.jumps, Some(0));
}
