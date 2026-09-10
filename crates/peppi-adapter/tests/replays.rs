mod support;

use peppi_adapter::{Error, Inputs, Port, Replay, Timeline, Version};
use replay_validation::{
    Checkpoint, FrameStepper, Transition, ValidationError, compare_f32_bits, validate_fallible,
};
use std::convert::Infallible;
use support::*;

#[test]
fn parser_preserves_peppi_types_ports_versions_and_float_bits() {
    for version in [
        Version(2, 0, 0),
        Version(2, 0, 1),
        Version(2, 1, 0),
        Version(2, 2, 0),
        Version(3, 0, 0),
        Version(3, 18, 0),
    ] {
        let fixture = Fixture {
            version,
            follower: true,
            ..Default::default()
        };
        let bytes = replay_bytes(&fixture);
        let replay = Replay::read(bytes.as_slice()).unwrap();
        let summary = replay.summary(Timeline::LastRecorded).unwrap();
        assert_eq!(summary.ports, [Port::P1, Port::P3]);
        assert_eq!(summary.version, version);
        assert_eq!(summary.physical_frames, 3);
        assert_eq!(summary.first_frame, Some(-123));
        assert_eq!(summary.last_frame, Some(-121));
        assert_eq!(summary.bytes, bytes.len());
        assert_eq!(replay.game().start.random_seed, 0x1234_5678);
        let frame = replay.frame(1).unwrap();
        assert_eq!(frame.actors.len(), 3);
        for actor in &frame.actors {
            assert_eq!(actor.pre.joystick.x.to_bits(), JOYSTICK_X_BITS);
            assert_eq!(actor.pre.joystick.y.to_bits(), JOYSTICK_Y_BITS);
            assert_eq!(actor.pre.cstick.x.to_bits(), CSTICK_X_BITS);
            assert_eq!(actor.pre.buttons, LOGICAL_BUTTONS);
            assert_eq!(actor.pre.buttons_physical, PHYSICAL_BUTTONS);
            assert_eq!(
                [actor.pre.position.x, actor.pre.position.y],
                pre_position(1, actor.port as u8, actor.follower)
            );
            assert_eq!(
                [actor.post.position.x, actor.post.position.y],
                post_position(1, actor.port as u8, actor.follower)
            );
            assert_eq!(actor.pre.raw_analog_y.is_some(), version.gte(3, 15));
            assert_eq!(actor.post.velocities.is_some(), version.gte(3, 5));
        }
        assert_eq!(frame.end.is_some(), version.gte(3, 0));
        assert_eq!(frame.start.is_some(), version.gte(2, 2));
        assert_eq!(
            frame.actors[0].post.hurtbox_state.is_some(),
            version.gte(2, 1)
        );
        assert_eq!(frame.items.is_some(), version.gte(3, 0));
        assert_eq!(frame.fod_platforms.is_some(), version.gte(3, 18));
        assert!(replay.frame(3).is_err());
    }
}

#[test]
fn legacy_pre_events_define_frames_without_start_or_bookend_events() {
    for version in [Version(2, 0, 0), Version(2, 0, 1), Version(2, 1, 0)] {
        let fixture = Fixture {
            version,
            ..Default::default()
        };
        let bytes = replay_bytes(&fixture);
        assert!(
            event_offsets(&bytes)
                .iter()
                .all(|(code, _, _)| ![0x3a, 0x3c].contains(code))
        );
        let replay = Replay::read(bytes.as_slice()).unwrap();
        assert_eq!(
            replay.frame_indices(Timeline::LastRecorded).unwrap(),
            [0, 1, 2]
        );
        for index in 0..3 {
            let frame = replay.frame(index).unwrap();
            assert_eq!(frame.id, -123 + index as i32);
            assert!(frame.start.is_none() && frame.end.is_none());
            assert_eq!(
                frame.actors[0].post.position.x,
                post_position(index, 0, false)[0]
            );
        }
        // A post event from the previous frame must be complete before a new
        // Pre ID can implicitly open the following frame.
        let (_, start, end) = event_offsets(&bytes)
            .into_iter()
            .find(|(code, _, _)| *code == 0x38)
            .unwrap();
        let mut missing_post = bytes.clone();
        missing_post.drain(start..end);
        let length = u32::from_be_bytes(bytes[11..15].try_into().unwrap()) - (end - start) as u32;
        missing_post[11..15].copy_from_slice(&length.to_be_bytes());
        assert!(Replay::read(missing_post.as_slice()).is_err());
        for frame_ids in [vec![-123, -121], vec![-123, -122, -123], vec![-122]] {
            let fixture = Fixture {
                version,
                frame_ids,
                ..Default::default()
            };
            assert!(Replay::read(replay_bytes(&fixture).as_slice()).is_err());
        }
    }
}

#[test]
fn legacy_actor_absence_cannot_silently_shift_later_rows() {
    for version in [Version(2, 0, 0), Version(2, 1, 0)] {
        let trailing = Fixture {
            version,
            follower: true,
            absent_followers: vec![1, 2],
            ..Default::default()
        };
        let replay = Replay::read(replay_bytes(&trailing).as_slice()).unwrap();
        assert_eq!(replay.frame(0).unwrap().actors.len(), 3);
        assert_eq!(replay.frame(1).unwrap().actors.len(), 2);
        assert_eq!(replay.frame(2).unwrap().actors.len(), 2);
        for absent_followers in [vec![0], vec![1]] {
            let intermittent = Fixture {
                absent_followers,
                ..trailing.clone()
            };
            let error = Replay::read(replay_bytes(&intermittent).as_slice()).unwrap_err();
            assert!(error.to_string().contains("intermittently absent"));
        }
    }
}

#[test]
fn rollback_replaces_the_entire_abandoned_tail_without_rebasing_ids() {
    for (ids, selected) in [
        (ROLLBACK_FRAMES.to_vec(), vec![0, 3, 4]),
        (vec![-123, -122, -121, -122], vec![0, 3]),
    ] {
        let bytes = replay_bytes(&Fixture {
            frame_ids: ids.clone(),
            ..Default::default()
        });
        let replay = Replay::read(bytes.as_slice()).unwrap();
        assert_eq!(
            replay.frame_indices(Timeline::LastRecorded).unwrap(),
            selected
        );
        let transitions: Vec<_> = replay
            .transitions(Timeline::LastRecorded)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (offset, transition) in transitions.iter().enumerate() {
            assert_eq!(transition.frame, -123 + offset as i32);
            let row = selected[offset];
            let actor = &transition.expected.actors[0];
            assert_eq!(actor.pre, transition.input.actors[0].pre);
            assert_eq!(actor.post.position.x, post_position(row, 0, false)[0]);
        }
        assert_eq!(
            replay
                .summary(Timeline::LastRecorded)
                .unwrap()
                .discarded_frames,
            ids.len() - selected.len()
        );
    }
}

#[test]
fn finalized_prefix_is_explicit_and_game_end_does_not_confirm_predictions() {
    let fixture = Fixture {
        finalized_frames: Some(vec![-124, -123, -122]),
        ..Default::default()
    };
    let replay = Replay::read(replay_bytes(&fixture).as_slice()).unwrap();
    assert_eq!(
        replay.frame_indices(Timeline::FinalizedOnly).unwrap(),
        [0, 1]
    );
    assert_eq!(
        replay.frame_indices(Timeline::LastRecorded).unwrap(),
        [0, 1, 2]
    );
    assert_eq!(
        replay.summary(Timeline::FinalizedOnly).unwrap().last_frame,
        Some(-122)
    );
    let no_finalized = Replay::read(replay_bytes(&Fixture::default()).as_slice()).unwrap();
    assert!(
        no_finalized
            .frame_indices(Timeline::FinalizedOnly)
            .unwrap()
            .is_empty()
    );
    let old = Replay::read(
        replay_bytes(&Fixture {
            version: Version(2, 2, 0),
            ..Default::default()
        })
        .as_slice(),
    )
    .unwrap();
    assert!(old.frame_indices(Timeline::FinalizedOnly).is_err());
    for fixture in [
        Fixture {
            frame_ids: vec![-123, -122, -123],
            finalized_frames: Some(vec![-124, -123, -124]),
            ..Default::default()
        },
        Fixture {
            finalized_frames: Some(vec![-122, -122, -121]),
            ..Default::default()
        },
        Fixture {
            frame_ids: vec![-123, -121],
            ..Default::default()
        },
        Fixture {
            frame_ids: vec![-124],
            ..Default::default()
        },
    ] {
        assert!(Replay::read(replay_bytes(&fixture).as_slice()).is_err());
    }
}

#[test]
fn absent_follower_remains_absent_and_gecko_bytes_are_retained() {
    let fixture = Fixture {
        follower: true,
        absent_followers: vec![1],
        gecko_codes: Some(vec![0x5a; 700]),
        ..Default::default()
    };
    let replay = Replay::read(replay_bytes(&fixture).as_slice()).unwrap();
    assert_eq!(replay.frame(0).unwrap().actors.len(), 3);
    assert_eq!(replay.frame(1).unwrap().actors.len(), 2);
    assert_eq!(replay.frame(2).unwrap().actors.len(), 3);
    assert_eq!(
        replay.game().gecko_codes.as_ref().unwrap().bytes[..700],
        [0x5a; 700]
    );
}

// This artificial transform tests the parser/validator protocol only; it is not
// a Melee simulator. The deliberately small observation policy checks XY bits.
struct PositionProtocol {
    steps: usize,
    corrupt_at: Option<usize>,
}
impl FrameStepper for PositionProtocol {
    type Checkpoint = usize;
    type Input = Inputs;
    type Observation = Vec<[f32; 2]>;
    type Error = Infallible;
    fn restore(&mut self, checkpoint: &usize) -> Result<(), Infallible> {
        self.steps = *checkpoint;
        Ok(())
    }
    fn advance(&mut self, input: &Inputs) -> Result<Self::Observation, Infallible> {
        let mut result: Vec<_> = input
            .actors
            .iter()
            .map(|a| [a.pre.position.x - 0.375, a.pre.position.y + 0.25])
            .collect();
        if self.corrupt_at == Some(self.steps) {
            result[0][0] += 1.0;
        }
        self.steps += 1;
        Ok(result)
    }
}

#[test]
fn parsed_transitions_feed_the_validator_and_report_first_divergence() {
    let fixture = Fixture {
        frame_ids: ROLLBACK_FRAMES.to_vec(),
        ..Default::default()
    };
    let replay = Replay::read(replay_bytes(&fixture).as_slice()).unwrap();
    for corrupt_at in [None, Some(1)] {
        let mut stepper = PositionProtocol {
            steps: 99,
            corrupt_at,
        };
        let transitions = replay
            .transitions(Timeline::LastRecorded)
            .unwrap()
            .map(|next| {
                next.map(|t| Transition {
                    frame: t.frame,
                    input: t.input,
                    expected: t
                        .expected
                        .actors
                        .iter()
                        .map(|a| [a.post.position.x, a.post.position.y])
                        .collect(),
                })
            });
        let result = validate_fallible(
            &mut stepper,
            &Checkpoint {
                next_frame: -123,
                state: 0,
            },
            transitions,
            |expected: &Vec<[f32; 2]>, actual: &Vec<[f32; 2]>| {
                expected
                    .iter()
                    .flatten()
                    .zip(actual.iter().flatten())
                    .find_map(|(a, b)| compare_f32_bits(a, b))
            },
        );
        if corrupt_at.is_some() {
            assert!(matches!(
                result,
                Err(ValidationError::Mismatch {
                    frame: -122,
                    checked_frames: 1,
                    ..
                })
            ));
        } else {
            let report = result.unwrap();
            assert_eq!(
                (report.first_frame, report.last_frame, report.checked_frames),
                (-123, -121, 3)
            );
        }
    }
}

/// Routing mutations target complete generated events, not accidental byte patterns.
fn event_offsets(bytes: &[u8]) -> Vec<(u8, usize, usize)> {
    let raw_end = 15 + u32::from_be_bytes(bytes[11..15].try_into().unwrap()) as usize;
    let table_end = 16 + bytes[16] as usize;
    let mut sizes = [0_usize; 256];
    for record in bytes[17..table_end].as_chunks::<3>().0 {
        sizes[record[0] as usize] = u16::from_be_bytes([record[1], record[2]]) as usize;
    }
    let mut offset = table_end;
    let mut events = Vec::new();
    while offset < raw_end {
        let code = bytes[offset];
        let end = offset + 1 + sizes[code as usize];
        events.push((code, offset, end));
        offset = end;
    }
    events
}

#[test]
fn malformed_and_incomplete_replays_never_become_validation_input() {
    let complete = replay_bytes(&Fixture::default());
    for bytes in [
        vec![],
        vec![0; 32],
        truncated_bytes(&Fixture::default()),
        replay_bytes(&Fixture {
            ended: false,
            ..Default::default()
        }),
    ] {
        assert!(Replay::read(bytes.as_slice()).is_err());
    }
    for index in 0..complete.len() {
        assert!(
            Replay::read(&complete[..index]).is_err(),
            "accepted truncation at {index}"
        );
    }
    let mut trailing = complete.clone();
    trailing.push(0);
    assert!(Replay::read(trailing.as_slice()).is_err());
    let mut future = complete.clone();
    let (_, start, _) = event_offsets(&future)
        .into_iter()
        .find(|(code, _, _)| *code == 0x36)
        .unwrap();
    future[start + 1] = 4;
    assert!(matches!(
        Replay::read(future.as_slice()),
        Err(Error::UnsupportedVersion(Version(4, 18, 0)))
    ));
    for (relative, value) in [(5, 1), (5, 4), (6, 1)] {
        let mut bytes = complete.clone();
        let (_, pre, _) = event_offsets(&bytes)
            .into_iter()
            .find(|(code, _, _)| *code == 0x37)
            .unwrap();
        bytes[pre + relative] = value;
        assert!(Replay::read(bytes.as_slice()).is_err());
    }
    let mut duplicate = complete.clone();
    let (_, start, end) = event_offsets(&duplicate)
        .into_iter()
        .find(|(code, _, _)| *code == 0x37)
        .unwrap();
    let copy = duplicate[start..end].to_vec();
    duplicate.splice(end..end, copy.iter().copied());
    let raw_len = u32::from_be_bytes(duplicate[11..15].try_into().unwrap()) + copy.len() as u32;
    duplicate[11..15].copy_from_slice(&raw_len.to_be_bytes());
    assert!(Replay::read(duplicate.as_slice()).is_err());
}
