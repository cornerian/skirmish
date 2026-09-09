//! Wholly synthetic Slippi fixtures; these are format tests, not recorded games.
//!
//! Peppi's mutable Arrow containers and `.slp` writer generate the actual UBJSON
//! envelope, payload-size table, versioned events and end block. The small
//! big-endian payloads below fill its public `read_push` builders. Their fields
//! follow Peppi 2.1.2's `frame/immutable/slippi.rs`; all values are invented.
//! Character/stage numbers only exercise format identifiers. No game resources,
//! executable, console state or complete simulator checkpoint are embedded.
#![allow(dead_code)] // Individual integration-test targets use different helpers.

use peppi::{
    frame::mutable,
    game::{self, Port, immutable::Game, shift_jis::MeleeString},
    io::slippi::{self, Version},
};

pub const ROLLBACK_FRAMES: [i32; 5] = [-123, -122, -121, -122, -121];
pub const JOYSTICK_X_BITS: u32 = 0x8000_0000;
pub const JOYSTICK_Y_BITS: u32 = 0x3eaa_aaab;
pub const CSTICK_X_BITS: u32 = 0x0000_0001;
pub const LOGICAL_BUTTONS: u32 = 0x0001_0100;
pub const PHYSICAL_BUTTONS: u16 = 0x0100;

#[derive(Clone, Debug)]
pub struct Fixture {
    pub version: Version,
    /// Event order, including repeated IDs when testing rollback resolution.
    pub frame_ids: Vec<i32>,
    /// A second character on P3 (zero-based port 2); P1 never has a follower.
    pub follower: bool,
    /// Whether a complete GameEnd event is emitted. This is distinct from
    /// truncation: `false` still produces a complete UBJSON file envelope.
    pub ended: bool,
    /// Per-event-row bookend watermarks, when present in the format. Defaults
    /// to `frame_id - 7`, leaving short rollback fixtures unfinalized.
    pub finalized_frames: Option<Vec<i32>>,
    /// Event-row indexes on which the declared P3 follower is absent.
    pub absent_followers: Vec<usize>,
    /// Optional synthetic Gecko bytes. The writer adds splitter padding.
    pub gecko_codes: Option<Vec<u8>>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            version: Version(3, 18, 0),
            frame_ids: vec![-123, -122, -121],
            follower: false,
            ended: true,
            finalized_frames: None,
            absent_followers: Vec::new(),
            gecko_codes: None,
        }
    }
}

/// Distinguishes event rows even when frame IDs repeat after a rollback.
pub fn sample_value(row: usize, port: u8, follower: bool) -> f32 {
    row as f32 * 100.0 + f32::from(port) * 10.0 + if follower { 1000.0 } else { 0.0 }
}

pub fn pre_position(row: usize, port: u8, follower: bool) -> [f32; 2] {
    let value = sample_value(row, port, follower);
    [value + 0.5, -value - 0.5]
}

pub fn post_position(row: usize, port: u8, follower: bool) -> [f32; 2] {
    let value = sample_value(row, port, follower);
    [value + 0.125, -value - 0.25]
}

/// Generate genuine `.slp` bytes using Peppi's public writer. Supported fixture
/// versions are 2.0 through 3.18, including implicit frames before 2.2.
pub fn replay_bytes(fixture: &Fixture) -> Vec<u8> {
    replay_bytes_with(fixture, |_, _| {})
}

/// Edit complete synthetic columns before Peppi writes the actual file. Tests
/// can inject independently produced observations or malformed recorded fields
/// without introducing another `.slp` serializer.
pub fn replay_bytes_with(
    fixture: &Fixture,
    edit: impl FnOnce(&mut game::Start, &mut mutable::Frame),
) -> Vec<u8> {
    assert!(fixture.version >= Version(2, 0, 0));
    assert!(fixture.version <= Version(3, 18, 0));
    if let Some(finalized) = &fixture.finalized_frames {
        assert_eq!(finalized.len(), fixture.frame_ids.len());
    }
    let version = fixture.version;
    let mut start = game_start(fixture);
    let mut frames = mutable::Frame::with_capacity(
        fixture.frame_ids.len(),
        version,
        &game::port_occupancy(&start),
    );
    for (row, &id) in fixture.frame_ids.iter().enumerate() {
        frames.id.push(Some(id));
        let mut start = (0xaabb_0000 + row as u32).to_be_bytes().to_vec();
        if version.gte(3, 10) {
            start.extend((700 + row as u32).to_be_bytes());
        }
        if let Some(frame_start) = &mut frames.start {
            frame_start
                .read_push(&mut start.as_slice(), version)
                .unwrap();
        }
        for port in &mut frames.ports {
            push_character(&mut port.leader, row, port.port as u8, false, version);
            if let Some(follower) = &mut port.follower {
                if fixture.absent_followers.contains(&row) {
                    follower.push_null(version);
                } else {
                    push_character(follower, row, port.port as u8, true, version);
                }
            }
        }
        if let Some(end) = &mut frames.end {
            let finalized = fixture
                .finalized_frames
                .as_ref()
                .map_or_else(|| id.saturating_sub(7), |values| values[row]);
            end.read_push(&mut finalized.to_be_bytes().as_slice(), version)
                .unwrap();
        }
        for offsets in [
            &mut frames.item_offset,
            &mut frames.fod_platform_offset,
            &mut frames.dreamland_whispy_offset,
            &mut frames.stadium_transformation_offset,
        ]
        .into_iter()
        .flatten()
        {
            offsets.try_push(0).unwrap();
        }
    }
    let end = fixture.ended.then(|| game::End {
        method: game::EndMethod::Game,
        bytes: game::Bytes(vec![0; if version.gte(3, 13) { 6 } else { 2 }]),
        lras_initiator: Some(None),
        players: version.gte(3, 13).then(|| {
            vec![
                game::PlayerEnd {
                    port: Port::P1,
                    placement: 0,
                },
                game::PlayerEnd {
                    port: Port::P3,
                    placement: 1,
                },
            ]
        }),
    });
    edit(&mut start, &mut frames);
    let game = Game {
        start,
        end,
        frames: frames.into(),
        metadata: None,
        gecko_codes: fixture.gecko_codes.as_ref().map(|codes| {
            assert!(version.gte(3, 3) && !codes.is_empty());
            let mut padded = codes.clone();
            padded.resize(codes.len().next_multiple_of(512), 0);
            game::GeckoCodes {
                bytes: padded,
                actual_size: codes.len().try_into().unwrap(),
            }
        }),
        hash: None,
        quirks: None,
    };
    let mut bytes = Vec::new();
    slippi::write(&mut bytes, &game).unwrap();
    bytes
}

/// Leave a partial GameEnd payload, retaining the original advertised length.
pub fn truncated_bytes(fixture: &Fixture) -> Vec<u8> {
    let mut complete = fixture.clone();
    complete.ended = true;
    let mut bytes = replay_bytes(&complete);
    bytes.truncate(bytes.len() - 3);
    bytes
}

fn game_start(fixture: &Fixture) -> game::Start {
    let version = fixture.version;
    // Versions 2.0 through 3.6 have 418-byte payloads. The writer edits fields and
    // preserves unmapped ones. Absent player slots must retain type 3, not the
    // zero that denotes a human player. Offsets exclude the event-code byte.
    let size = 418
        + usize::from(version.gte(3, 7)) * 2
        + usize::from(version.gte(3, 9)) * 164
        + usize::from(version.gte(3, 11)) * 116
        + usize::from(version.gte(3, 12))
        + usize::from(version.gte(3, 14)) * 59;
    let mut raw = vec![0; size];
    for slot in 0..6 {
        raw[0x65 + slot * 36] = 3;
    }
    game::Start {
        slippi: slippi::Slippi { version },
        bitfield: [0; 4],
        is_raining_bombs: false,
        is_teams: false,
        item_spawn_frequency: -1,
        self_destruct_score: -1,
        stage: 31,
        timer: 480,
        item_spawn_bitfield: [0; 5],
        damage_ratio: 1.0,
        players: [Port::P1, Port::P3]
            .into_iter()
            .map(|port| player(port, fixture))
            .collect(),
        random_seed: 0x1234_5678,
        bytes: game::Bytes(raw),
        is_pal: Some(false),
        is_frozen_ps: Some(false),
        scene: version
            .gte(3, 7)
            .then_some(game::Scene { minor: 2, major: 2 }),
        language: version.gte(3, 12).then_some(game::Language::English),
        r#match: version.gte(3, 14).then(|| game::Match {
            id: "synthetic-format-test".into(),
            game: 1,
            tiebreaker: 0,
        }),
    }
}

fn player(port: Port, fixture: &Fixture) -> game::Player {
    game::Player {
        port,
        character: if port == Port::P3 && fixture.follower {
            game::ICE_CLIMBERS
        } else {
            2
        },
        r#type: game::PlayerType::Human,
        stocks: 4,
        costume: 0,
        team: None,
        handicap: 9,
        bitfield: 0,
        cpu_level: None,
        damage_start: 0,
        damage_spawn: 0,
        offense_ratio: 1.0,
        defense_ratio: 1.0,
        model_scale: 1.0,
        ucf: Some(game::Ucf {
            dash_back: None,
            shield_drop: None,
        }),
        name_tag: Some(MeleeString("TEST".into())),
        netplay: fixture.version.gte(3, 9).then(|| game::Netplay {
            name: MeleeString("Synthetic".into()),
            code: MeleeString("TEST#0".into()),
            suid: fixture.version.gte(3, 11).then(String::new),
        }),
    }
}

fn push_character(
    data: &mut mutable::Data,
    row: usize,
    port: u8,
    follower: bool,
    version: Version,
) {
    if let Some(validity) = &mut data.validity {
        validity.push(true);
    }
    data.pre
        .read_push(&mut pre(row, port, follower, version).as_slice(), version)
        .unwrap();
    data.post
        .read_push(&mut post(row, port, follower, version).as_slice(), version)
        .unwrap();
}

fn float(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend(value.to_bits().to_be_bytes());
}

fn pre(row: usize, port: u8, follower: bool, version: Version) -> Vec<u8> {
    let mut bytes = (0x7654_0000 + row as u32).to_be_bytes().to_vec();
    bytes.extend((10 + row as u16).to_be_bytes());
    for value in pre_position(row, port, follower).into_iter().chain([
        if port == 0 { 1.0 } else { -1.0 },
        f32::from_bits(JOYSTICK_X_BITS),
        f32::from_bits(JOYSTICK_Y_BITS),
        f32::from_bits(CSTICK_X_BITS),
        -0.25,
        0.5,
    ]) {
        float(&mut bytes, value);
    }
    bytes.extend(LOGICAL_BUTTONS.to_be_bytes());
    bytes.extend(PHYSICAL_BUTTONS.to_be_bytes());
    float(&mut bytes, 0.25);
    float(&mut bytes, 0.75);
    bytes.push((-127_i8) as u8);
    float(&mut bytes, sample_value(row, port, follower) + 0.5);
    if version.gte(3, 15) {
        bytes.push(127);
    }
    if version.gte(3, 17) {
        bytes.extend([(-42_i8) as u8, 42]);
    }
    bytes
}

fn post(row: usize, port: u8, follower: bool, version: Version) -> Vec<u8> {
    let mut bytes = vec![if follower { 11 } else { 2 }];
    bytes.extend((14 + row as u16).to_be_bytes());
    for value in post_position(row, port, follower).into_iter().chain([
        if port == 0 { 1.0 } else { -1.0 },
        sample_value(row, port, follower) + 1.0,
        59.75,
    ]) {
        float(&mut bytes, value);
    }
    bytes.extend([1, 2, 0xff, 4]); // last attack, combo, last hitter, stocks
    float(&mut bytes, row as f32 + 0.25);
    bytes.extend([1, 2, 3, 4, 5]); // state flags
    float(&mut bytes, 6.25); // miscellaneous action-state value
    bytes.push(u8::from(port == 2)); // airborne
    bytes.extend(17_u16.to_be_bytes());
    bytes.extend([2, 0]); // jumps, L-cancel
    if version.gte(2, 1) {
        bytes.push(0); // hurtbox state
    }
    if version.gte(3, 5) {
        for value in [0.125, -0.25, 0.5, -1.0, 2.0] {
            float(&mut bytes, value);
        }
    }
    if version.gte(3, 8) {
        float(&mut bytes, 3.0);
    }
    if version.gte(3, 11) {
        bytes.extend((100 + row as u32).to_be_bytes());
    }
    if version.gte(3, 16) {
        bytes.extend(7_u16.to_be_bytes());
        bytes.extend((8 + u16::from(port)).to_be_bytes());
    }
    bytes
}

#[cfg(test)]
mod fixture_checks {
    use super::*;
    use peppi::game::Game as _;
    use std::io::Cursor;

    #[test]
    fn generated_bytes_parse_with_versioned_fields_and_rollback_rows_intact() {
        for version in [Version(2, 2, 0), Version(3, 0, 0), Version(3, 18, 0)] {
            let fixture = Fixture {
                version,
                frame_ids: ROLLBACK_FRAMES.to_vec(),
                follower: true,
                ..Fixture::default()
            };
            let game = slippi::read(Cursor::new(replay_bytes(&fixture)), None).unwrap();
            assert_eq!(game.start.slippi.version, version);
            assert_eq!(game.frames.id.values().as_slice(), ROLLBACK_FRAMES);
            assert!(game.end.is_some());
            for row in 0..fixture.frame_ids.len() {
                let frame = game.frame(row);
                assert_eq!(frame.ports.len(), 2);
                assert_eq!(frame.ports[0].port, Port::P1);
                assert_eq!(frame.ports[1].port, Port::P3);
                assert!(frame.ports[0].follower.is_none());
                let follower = frame.ports[1].follower.as_ref().unwrap();
                assert_eq!(
                    [follower.post.position.x, follower.post.position.y],
                    post_position(row, 2, true)
                );
                let pre = frame.ports[0].leader.pre;
                assert_eq!(pre.joystick.x.to_bits(), JOYSTICK_X_BITS);
                assert_eq!(pre.joystick.y.to_bits(), JOYSTICK_Y_BITS);
                assert_eq!(pre.cstick.x.to_bits(), CSTICK_X_BITS);
                assert_eq!(pre.buttons, LOGICAL_BUTTONS);
                assert_eq!(pre.buttons_physical, PHYSICAL_BUTTONS);
                assert_eq!(pre.raw_analog_y.is_some(), version.gte(3, 15));
                let post = frame.ports[0].leader.post;
                assert_eq!(post.velocities.is_some(), version.gte(3, 5));
                assert_eq!(post.hitlag.is_some(), version.gte(3, 8));
                assert_eq!(frame.end.is_some(), version.gte(3, 0));
            }
        }
    }

    #[test]
    fn missing_end_is_parseable_but_partial_event_is_not() {
        for version in [Version(2, 2, 0), Version(3, 18, 0)] {
            let fixture = Fixture {
                version,
                ended: false,
                ..Fixture::default()
            };
            let game = slippi::read(Cursor::new(replay_bytes(&fixture)), None).unwrap();
            assert!(game.end.is_none());
            assert_eq!(game.frames.id.len(), 3);
            assert!(slippi::read(Cursor::new(truncated_bytes(&fixture)), None).is_err());
        }
    }

    #[test]
    fn optional_absent_followers_watermarks_and_splitter_bytes_survive_parsing() {
        let fixture = Fixture {
            follower: true,
            absent_followers: vec![1],
            finalized_frames: Some(vec![-130, -129, -123]),
            gecko_codes: Some(vec![0xa5; 700]),
            ..Fixture::default()
        };
        let game = slippi::read(Cursor::new(replay_bytes(&fixture)), None).unwrap();
        let follower = game.frames.ports[1].follower.as_ref().unwrap();
        let validity = follower.validity.as_ref().unwrap();
        assert!(validity.get_bit(0));
        assert!(!validity.get_bit(1));
        assert!(validity.get_bit(2));
        assert_eq!(
            game.frames
                .end
                .as_ref()
                .unwrap()
                .latest_finalized_frame
                .as_ref()
                .unwrap()
                .values()
                .as_slice(),
            [-130, -129, -123]
        );
        let codes = game.gecko_codes.unwrap();
        assert_eq!(codes.actual_size, 700);
        assert_eq!(&codes.bytes[..700], &[0xa5; 700]);
    }
}
