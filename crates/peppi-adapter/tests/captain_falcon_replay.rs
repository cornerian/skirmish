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
    assert_eq!(
        summary.end_method,
        peppi_adapter::peppi::game::EndMethod::Resolved
    );

    let roster = replay.roster();
    assert!(roster.is_teams);
    assert_eq!(
        roster
            .players
            .iter()
            .map(|player| player.port)
            .collect::<Vec<_>>(),
        [Port::P1, Port::P2, Port::P3, Port::P4]
    );
    assert_eq!(
        roster
            .players
            .iter()
            .map(|player| player.character)
            .collect::<Vec<_>>(),
        [9, 22, 17, 0]
    );
    assert_eq!(
        roster
            .players
            .iter()
            .map(|player| player.team)
            .collect::<Vec<_>>(),
        [
            Some(peppi_adapter::peppi::game::Team { color: 0, shade: 0 }),
            Some(peppi_adapter::peppi::game::Team { color: 1, shade: 0 }),
            Some(peppi_adapter::peppi::game::Team { color: 1, shade: 0 }),
            Some(peppi_adapter::peppi::game::Team { color: 0, shade: 0 }),
        ]
    );
    assert!(
        roster
            .players
            .iter()
            .all(|player| { player.player_type == peppi_adapter::peppi::game::PlayerType::Human })
    );
    assert!(roster.players.iter().all(|player| player.stocks == 4));
    assert_eq!(
        roster
            .players
            .iter()
            .map(|player| player.costume)
            .collect::<Vec<_>>(),
        [1, 2, 2, 2]
    );

    // These settings come from the authoritative GameStart parse, rather than
    // being inferred from the frame stream.
    let start = &replay.game().start;
    assert_eq!(start.timer, 480);
    assert_eq!(start.item_spawn_frequency, -1);
    assert_eq!(start.stage, 31);

    // GameStart uses the external CSS character ID. Captain Falcon is P4
    // (external ID 0); frame Post uses the corresponding internal ID 2.
    let players = &replay.game().start.players;
    assert_eq!(players.len(), 4);
    assert_eq!(
        players
            .iter()
            .find(|p| p.port == Port::P4)
            .unwrap()
            .character,
        0
    );

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
        assert_eq!(
            last.post.state, final_post_state,
            "window end post at {end}"
        );
        assert_eq!(last.post.airborne, Some(1), "airborne at {end}");
    }

    // Each recorded aerial Dive entry includes B plus a distinct snapshot of
    // Captain's pre-state and stick direction. Positions and facing are
    // retained as observations of the movement that begins at each entry.
    for (frame_id, pre_state, stick_x, stick_y, direction, position) in [
        (
            522,
            27,
            -0.6625_f32,
            0.7375_f32,
            -1.0_f32,
            [155.83063_f32, -18.104641_f32],
        ),
        (
            2066,
            32,
            -0.65_f32,
            0.75_f32,
            -1.0_f32,
            [126.28996_f32, -29.326918_f32],
        ),
        (
            2397,
            32,
            -0.6375_f32,
            0.75_f32,
            -1.0_f32,
            [175.08943_f32, -38.935917_f32],
        ),
        (
            3929,
            27,
            0.7_f32,
            0.7_f32,
            1.0_f32,
            [-137.69496_f32, 36.903496_f32],
        ),
    ] {
        let actor = captain(&replay, (frame_id + 123) as usize);
        assert_eq!(actor.pre.state, pre_state, "Dive pre-state at {frame_id}");
        assert_eq!(actor.pre.buttons, 66_048, "Dive B input at {frame_id}");
        assert_eq!(
            actor.pre.buttons_physical, 512,
            "Dive physical B input at {frame_id}"
        );
        assert_eq!(actor.pre.joystick.x.to_bits(), stick_x.to_bits());
        assert_eq!(actor.pre.joystick.y.to_bits(), stick_y.to_bits());
        assert_eq!(actor.post.state, 354, "Dive post-state at {frame_id}");
        assert_eq!(actor.post.state_age.unwrap().to_bits(), 1.0_f32.to_bits());
        assert_eq!(actor.post.airborne, Some(1));
        assert_eq!(
            actor.post.direction.to_bits(),
            direction.to_bits(),
            "Dive facing at {frame_id}"
        );
        assert_eq!(actor.post.position.x.to_bits(), position[0].to_bits());
        assert_eq!(actor.post.position.y.to_bits(), position[1].to_bits());
    }

    // The recording also contains an aerial down-special entry at 6594,
    // followed by its state-359 and state-361 aerial phases. These labels are
    // Captain Falcon's aerial down-special states in ftCaptain/forward.h, not
    // a grounded-to-air transition.
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
    let before_end = captain(&replay, (6651 + 123) as usize);
    assert_eq!(before_end.pre.state, 361);
    assert_eq!(before_end.post.state, 361);
    assert_eq!(
        before_end.post.state_age.unwrap().to_bits(),
        28.0_f32.to_bits()
    );
    let end = captain(&replay, (6652 + 123) as usize);
    assert_eq!(end.pre.state, 29);
    assert_eq!(end.post.state, 29);
    assert_eq!(end.post.state_age.unwrap().to_bits(), 0.0_f32.to_bits());
    assert_eq!(end.post.airborne, Some(1));
    assert_eq!(end.post.direction.to_bits(), (-1.0_f32).to_bits());
    assert_eq!(end.post.position.x.to_bits(), 121.696594_f32.to_bits());
    assert_eq!(end.post.position.y.to_bits(), 8.720186_f32.to_bits());
    let air = captain(&replay, (6623 + 123) as usize);
    assert_eq!(air.pre.state, 361);
    assert_eq!(air.pre.buttons, 262_144);
    assert_eq!(air.pre.joystick.x.to_bits(), (-0.9875_f32).to_bits());
    assert_eq!(air.pre.joystick.y.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn captain_two_player_fixture_records_roster_and_special_entries() {
    let replay = Replay::read(std::io::Cursor::new(include_bytes!(
        "../../../tests/fixtures/slippi/11-captain-falcon-marth-final-destination.slp"
    )))
    .unwrap();
    let summary = replay.summary(Timeline::LastRecorded).unwrap();
    assert_eq!(summary.version, Version(2, 0, 1));
    assert_eq!(summary.stage, 32); // Final Destination
    assert_eq!(summary.ports, [Port::P1, Port::P4]);
    assert_eq!(summary.physical_frames, 8_962);
    assert_eq!(summary.first_frame, Some(-123));
    assert_eq!(summary.last_frame, Some(8_838));
    assert_eq!(summary.latest_finalized_frame, None);
    assert_eq!(
        summary.end_method,
        peppi_adapter::peppi::game::EndMethod::Game
    );

    let roster = replay.roster();
    assert!(!roster.is_teams);
    assert_eq!(
        roster
            .players
            .iter()
            .map(|player| (
                player.port,
                player.character,
                player.player_type,
                player.team
            ))
            .collect::<Vec<_>>(),
        [
            (
                Port::P1,
                0,
                peppi_adapter::peppi::game::PlayerType::Human,
                None
            ),
            (
                Port::P4,
                9,
                peppi_adapter::peppi::game::PlayerType::Human,
                None
            ),
        ]
    );

    // These are the complete Captain Falcon special-action Post states
    // observed in this recording. They are replay observations, not claims
    // about native simulation parity.
    let mut observed = std::collections::BTreeSet::new();
    for index in 0..summary.selected_frames {
        observed.insert(captain(&replay, index).post.state);
    }
    assert!([349, 351, 358, 368]
        .into_iter()
        .all(|state| observed.contains(&state)));

    // Representative recorded entries, retaining controller input and both
    // sides of each action-state transition for future adapter consumers.
    for (frame, pre_state, post_state, buttons, stick) in [
        (1328, 28, 368, 66_048, (-0.675_f32, 0.725_f32)),
        (1822, 25, 358, 262_656, (-0.9875_f32, 0.0_f32)),
        (2411, 20, 349, 262_656, (-0.9875_f32, 0.0_f32)),
        (2422, 349, 351, 262_656, (-0.9875_f32, 0.0_f32)),
        (5214, 29, 368, 66_048, (0.6875_f32, 0.7125_f32)),
    ] {
        let actor = captain(&replay, (frame + 123) as usize);
        assert_eq!(actor.pre.state, pre_state, "entry pre-state at {frame}");
        assert_eq!(actor.post.state, post_state, "entry post-state at {frame}");
        assert_eq!(actor.pre.buttons, buttons, "entry buttons at {frame}");
        assert_eq!(actor.pre.joystick.x.to_bits(), stick.0.to_bits());
        assert_eq!(actor.pre.joystick.y.to_bits(), stick.1.to_bits());
    }
}
