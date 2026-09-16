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


    // Four recorded state-354 windows. The final frame of the second window
    // transitions to state 252 in Post while still reporting state 354 in
    // Pre; retaining both sides catches off-by-one handling in future replay
    // consumers.
    for (start, end, final_post_state) in [
        (522, 585, 354),
        (2066, 2129, 252),
        (2397, 2460, 354),
        (3929, 3992, 354),
    ] {
        for frame_id in start..end {
            let actor = captain(&replay, (frame_id + 123) as usize);
            assert_eq!(actor.post.state, 354, "state-354 window at {frame_id}");
            assert_eq!(actor.post.airborne, Some(1), "airborne at {frame_id}");
        }
        let last = captain(&replay, (end + 123) as usize);
        assert_eq!(last.pre.state, 354, "state-354 window end pre at {end}");
        assert_eq!(last.post.state, final_post_state, "window end post at {end}");
        assert_eq!(last.post.airborne, Some(1), "airborne at {end}");
    }

    // The recording also contains a down-special entry at 6594, followed by
    // its state-359 ground/air segment and state-361 continuation.
    let entry = captain(&replay, (6594 + 123) as usize);
    assert_eq!(entry.pre.state, 27);
    assert_eq!(entry.pre.buttons, 131_584);
    assert_eq!(entry.pre.joystick.x.to_bits(), (-0.6375_f32).to_bits());
    assert_eq!(entry.pre.joystick.y.to_bits(), (-0.7625_f32).to_bits());
    assert_eq!(entry.post.state, 359);
    assert_eq!(entry.post.airborne, Some(1));
    for frame_id in 6594..=6622 {
        let actor = captain(&replay, (frame_id + 123) as usize);
        assert_eq!(actor.post.state, 359, "state-359 segment at {frame_id}");
        assert_eq!(actor.post.airborne, Some(1), "airborne at {frame_id}");
    }
    for frame_id in 6623..=6651 {
        let actor = captain(&replay, (frame_id + 123) as usize);
        assert_eq!(actor.post.state, 361, "state-361 segment at {frame_id}");
        assert_eq!(actor.post.airborne, Some(1), "airborne at {frame_id}");
    }
    let air = captain(&replay, (6623 + 123) as usize);
    assert_eq!(air.pre.state, 361);
    assert_eq!(air.pre.buttons, 262_144);
    assert_eq!(air.pre.joystick.x.to_bits(), (-0.9875_f32).to_bits());
    assert_eq!(air.pre.joystick.y.to_bits(), 0.0_f32.to_bits());
}
