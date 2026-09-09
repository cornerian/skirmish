//! Strict event-envelope validation before Peppi parses frame payloads.
//!
//! Peppi remains responsible for game settings, frame fields and UBJSON. This
//! pass only checks declared boundaries and routing: its parser otherwise maps
//! unoccupied ports to port zero, tolerates duplicate actor events, and asserts
//! some event-order invariants. Rollback IDs are permitted between frames.
use crate::Error;
use peppi::{
    game::Game as _,
    io::slippi::{self, Version, de::Event},
};
use std::{io::Cursor, panic::catch_unwind};

pub(crate) fn check(bytes: &[u8]) -> Result<(), Error> {
    let mut header = Cursor::new(bytes);
    let raw_len = slippi::de::parse_header(&mut header, None)
        .map_err(|error| Error::Invalid(format!("Slippi header: {error}")))?
        as usize;
    if raw_len == 0 {
        return invalid("in-progress replay has no declared raw length");
    }
    let raw_start = header.position() as usize;
    let raw_end = raw_start
        .checked_add(raw_len)
        .ok_or_else(|| Error::Invalid("raw length overflows address space".into()))?;
    let raw = bytes
        .get(raw_start..raw_end)
        .ok_or_else(|| Error::Invalid("declared raw length exceeds file size".into()))?;
    if !matches!(bytes.get(raw_end), Some(0x55 | 0x7d)) {
        return invalid("raw events are not followed by a UBJSON metadata/end delimiter");
    }

    let (sizes, mut offset) = payload_sizes(raw)?;
    let (code, payload) = event(raw, &mut offset, &sizes)?;
    if code != Event::GameStart {
        return invalid("GameStart must immediately follow the payload-size table");
    }
    let version = match payload.get(..3) {
        Some([major, minor, patch]) => Version(*major, *minor, *patch),
        _ => return invalid("GameStart lacks a format version"),
    };
    if !(Version(2, 0, 0)..=slippi::MAX_SUPPORTED_VERSION).contains(&version) {
        return Err(Error::UnsupportedVersion(version));
    }
    for code in [Event::FramePre, Event::FramePost, Event::GameEnd] {
        if sizes[code as usize].is_none() {
            return invalid(format!("missing {code:?} payload size"));
        }
    }
    if version.gte(2, 2) && sizes[Event::FrameStart as usize].is_none() {
        return invalid("missing FrameStart payload size");
    }
    if version.gte(3, 0) && sizes[Event::FrameEnd as usize].is_none() {
        return invalid("missing FrameEnd payload size");
    }

    // Bounded input and the version check happen before invoking Peppi's
    // settings parser. No independently interpreted player-settings offsets.
    let parsed = catch_unwind(|| slippi::de::parse_start(raw, None))
        .map_err(|_| Error::ParserPanic)?
        .map_err(|error| Error::Invalid(format!("Slippi GameStart: {error}")))?;
    if parsed.bytes_read() != offset {
        return invalid("GameStart and payload-size boundaries disagree");
    }
    let mut occupied = [false; 4];
    let mut followers = [false; 4];
    for player in &parsed.start().players {
        occupied[player.port as usize] = true;
        followers[player.port as usize] = player.character == peppi::game::ICE_CLIMBERS;
    }

    let mut frame = None::<Frame>;
    let mut legacy_frames = 0;
    let mut legacy_actor_rows = [0; 8];
    let mut seen_frame = false;
    let mut gecko_started = false;
    let mut gecko_complete = false;
    while offset < raw.len() {
        let (code, payload) = event(raw, &mut offset, &sizes)?;
        if gecko_started && !gecko_complete && code != Event::MessageSplitter {
            return invalid("unfinished split GeckoCodes event");
        }
        match code {
            Event::Payloads | Event::GameStart => {
                return invalid("duplicate payload-size table or GameStart event");
            }
            Event::GameEnd => {
                if version.gte(3, 0) && frame.is_some() {
                    return invalid("GameEnd arrived before FrameEnd");
                }
                close(frame.take())?;
                if offset != raw.len() {
                    return invalid(
                        "GameEnd must be the final raw event; duplicates are unsupported",
                    );
                }
                return Ok(());
            }
            Event::FrameStart => {
                if version.lt(2, 2) {
                    return invalid("FrameStart is unavailable before Slippi 2.2");
                }
                if version.gte(3, 0) && frame.is_some() {
                    return invalid("FrameStart arrived before the previous FrameEnd");
                }
                close(frame.take())?;
                frame = Some(Frame {
                    id: frame_id(payload)?,
                    actors: [0; 8],
                });
                seen_frame = true;
            }
            Event::FramePre | Event::FramePost => {
                // Before 2.2 a new FramePre ID opens the next frame. No rollback
                // records or explicit frame-start events exist in this format.
                if version.lt(2, 2) && code == Event::FramePre {
                    let id = frame_id(payload)?;
                    if frame.as_ref().is_none_or(|current| current.id != id) {
                        let next = frame
                            .as_ref()
                            .map_or(Some(peppi::frame::FIRST_INDEX), |current| {
                                current.id.checked_add(1)
                            });
                        if next != Some(id) {
                            return invalid(
                                "legacy FramePre IDs must start at -123 and advance contiguously",
                            );
                        }
                        close(frame.take())?;
                        frame = Some(Frame { id, actors: [0; 8] });
                        legacy_frames += 1;
                        seen_frame = true;
                    }
                }
                let frame = current(&mut frame, payload)?;
                let routing = payload.get(4..6).ok_or_else(|| {
                    Error::Invalid("actor event lacks port/follower routing".into())
                })?;
                let (port, follower) = (usize::from(routing[0]), routing[1]);
                if port >= 4 || !occupied[port] {
                    return invalid("actor event references an unoccupied or invalid port");
                }
                if follower > 1 || follower == 1 && !followers[port] {
                    return invalid("actor event references an invalid Ice Climbers follower");
                }
                let actor_index = port * 2 + usize::from(follower);
                let actor = &mut frame.actors[actor_index];
                if code == Event::FramePre {
                    if *actor != 0 {
                        return invalid("duplicate or out-of-order FramePre for an actor");
                    }
                    // Peppi 2.1.2 pads missing legacy actors only at GameEnd.
                    // A later reappearance would otherwise shift rows left.
                    // Trailing absence is safe; intermittent absence is not.
                    if version.lt(2, 2) {
                        if legacy_actor_rows[actor_index] != legacy_frames - 1 {
                            return invalid(
                                "intermittently absent actors before Slippi 2.2 cannot be aligned by Peppi",
                            );
                        }
                        legacy_actor_rows[actor_index] += 1;
                    }
                    *actor = 1;
                } else {
                    if *actor != 1 {
                        return invalid(
                            "duplicate FramePost or FramePost without a matching FramePre",
                        );
                    }
                    *actor = 3;
                }
            }
            Event::FrameEnd => {
                if !version.gte(3, 0) {
                    return invalid("FrameEnd is unavailable before Slippi 3.0");
                }
                current(&mut frame, payload)?;
                close(frame.take())?;
            }
            Event::Item => {
                if !version.gte(3, 0) {
                    return invalid("Item events are unavailable before Slippi 3.0");
                }
                current(&mut frame, payload)?;
            }
            Event::FodPlatform | Event::DreamlandWhispy | Event::StadiumTransformation => {
                if !version.gte(3, 18) {
                    return invalid("stage update events are unavailable before Slippi 3.18");
                }
                current(&mut frame, payload)?;
            }
            Event::MessageSplitter => {
                if !version.gte(3, 3) || seen_frame || gecko_complete {
                    return invalid("split GeckoCodes must precede all frames and occur once");
                }
                if payload.len() != 516
                    || payload[514] != Event::GeckoCodes as u8
                    || payload[515] > 1
                {
                    return invalid("only the 516-byte GeckoCodes splitter envelope is supported");
                }
                let size = u16::from_be_bytes([payload[512], payload[513]]);
                if size > 512 || payload[515] == 0 && size != 512 {
                    return invalid("invalid split GeckoCodes chunk length");
                }
                if sizes[Event::GeckoCodes as usize].is_none() {
                    return invalid("split GeckoCodes has no declared payload size");
                }
                gecko_started = true;
                gecko_complete = payload[515] == 1;
            }
            Event::GeckoCodes => {
                return invalid("GeckoCodes must use the supported MessageSplitter envelope");
            }
        }
    }
    invalid("raw event block has no complete GameEnd")
}

struct Frame {
    id: i32,
    /// Each actor is absent (0), pre-only (1), or a completed pre/post pair (3).
    actors: [u8; 8],
}

fn close(frame: Option<Frame>) -> Result<(), Error> {
    if frame.is_some_and(|frame| frame.actors.contains(&1)) {
        return invalid("frame ended with an actor FramePre lacking FramePost");
    }
    Ok(())
}

fn current<'a>(frame: &'a mut Option<Frame>, payload: &[u8]) -> Result<&'a mut Frame, Error> {
    let frame = frame
        .as_mut()
        .ok_or_else(|| Error::Invalid("frame event occurred outside an open frame".into()))?;
    if frame.id != frame_id(payload)? {
        return invalid("event frame ID differs from the open frame");
    }
    Ok(frame)
}

fn frame_id(payload: &[u8]) -> Result<i32, Error> {
    let id: [u8; 4] = payload
        .get(..4)
        .and_then(|id| id.try_into().ok())
        .ok_or_else(|| Error::Invalid("frame event lacks its signed frame ID".into()))?;
    Ok(i32::from_be_bytes(id))
}

type Sizes = [Option<usize>; 256];

fn payload_sizes(raw: &[u8]) -> Result<(Sizes, usize), Error> {
    if raw.first() != Some(&(Event::Payloads as u8)) {
        return invalid("raw events must begin with the payload-size table");
    }
    let width = usize::from(
        *raw.get(1)
            .ok_or_else(|| Error::Invalid("missing payload-size table length".into()))?,
    );
    if width % 3 != 1 {
        return invalid("invalid payload-size table length");
    }
    let end = width + 1;
    let table = raw
        .get(2..end)
        .ok_or_else(|| Error::Invalid("payload-size table extends beyond raw events".into()))?;
    let mut sizes = [None; 256];
    for entry in table.as_chunks::<3>().0 {
        let code = usize::from(entry[0]);
        let size = usize::from(u16::from_be_bytes([entry[1], entry[2]]));
        if size == 0 || sizes[code].is_some() || code == Event::Payloads as usize {
            return invalid("zero, duplicate, or recursive payload-size declaration");
        }
        sizes[code] = Some(size);
    }
    Ok((sizes, end))
}

fn event<'a>(raw: &'a [u8], offset: &mut usize, sizes: &Sizes) -> Result<(Event, &'a [u8]), Error> {
    let code = *raw
        .get(*offset)
        .ok_or_else(|| Error::Invalid("missing event code".into()))?;
    let size = sizes[usize::from(code)]
        .ok_or_else(|| Error::Invalid(format!("event {code:#04x} has no declared payload size")))?;
    let start = *offset + 1;
    let end = start
        .checked_add(size)
        .ok_or_else(|| Error::Invalid("event payload length overflow".into()))?;
    let payload = raw
        .get(start..end)
        .ok_or_else(|| Error::Invalid("event payload crosses the declared raw boundary".into()))?;
    let event = Event::try_from(code)
        .map_err(|_| Error::Invalid(format!("unsupported event code {code:#04x}")))?;
    *offset = end;
    Ok((event, payload))
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::Invalid(message.into()))
}
