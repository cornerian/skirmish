//! Check Arrow shape and null masks before using Peppi's unchecked transpose.

use peppi::{frame::immutable as column, game::immutable::Game, io::slippi::Version};

use crate::{Actor, Error, Frame};

fn length(actual: usize, expected: usize, name: &str) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Invalid(format!(
            "{name} has {actual} rows; expected {expected}"
        )));
    }
    Ok(())
}

fn present<T>(value: &Option<T>, required: bool, name: &str) -> Result<(), Error> {
    if value.is_some() != required {
        return Err(Error::Invalid(format!(
            "{name} presence does not match the replay version"
        )));
    }
    Ok(())
}

// The inferred bitmap type keeps Arrow out of this crate's direct dependencies.
// `None` and a bitmap containing only set bits both mean every row is present.
macro_rules! mask {
    ($rows:expr, $expected:expr, $actual:expr) => {{
        let (expected, actual) = ($expected, $actual);
        if let Some(bits) = actual {
            length(bits.len(), $rows, stringify!($actual))?;
        }
        let different = match (expected, actual) {
            (Some(a), Some(b)) => a.iter().ne(b.iter()),
            (Some(bits), None) | (None, Some(bits)) => bits.unset_bits() != 0,
            (None, None) => false,
        };
        if different {
            return Err(Error::Invalid(format!(
                "{} has inconsistent row presence",
                stringify!($actual)
            )));
        }
    }};
}

macro_rules! columns {
    ($rows:expr, $validity:expr; $($value:expr),+ $(,)?) => {{
        $(
            let value = &$value;
            length(value.len(), $rows, stringify!($value))?;
            mask!($rows, $validity, value.validity());
        )+
    }};
}

macro_rules! optional_columns {
    ($rows:expr, $validity:expr, $version:expr; $($value:expr => ($major:literal, $minor:literal)),+ $(,)?) => {{
        $(
            present(&$value, $version.gte($major, $minor), stringify!($value))?;
            if let Some(value) = &$value {
                columns!($rows, $validity; value);
            }
        )+
    }};
}

// A flat event array is fully covered by one offset range per recorded frame.
macro_rules! offsets {
    ($rows:expr, $events:expr, $offsets:expr) => {{
        let offsets = &$offsets;
        length(offsets.len(), $rows + 1, stringify!($offsets))?;
        if offsets.first().copied() != Some(0)
            || offsets
                .last()
                .copied()
                .and_then(|x| usize::try_from(x).ok())
                != Some($events)
        {
            return Err(Error::Invalid(format!(
                "{} does not cover the event rows",
                stringify!($offsets)
            )));
        }
    }};
}

fn actor(data: &column::Data, rows: usize, version: Version) -> Result<(), Error> {
    let valid = data.validity.as_ref();
    mask!(rows, valid, valid);
    let pre = &data.pre;
    let post = &data.post;
    mask!(rows, valid, pre.validity.as_ref());
    mask!(rows, valid, post.validity.as_ref());
    columns!(rows, valid;
        pre.random_seed, pre.state, pre.direction, pre.triggers,
        pre.buttons, pre.buttons_physical,
        post.character, post.state, post.direction, post.percent, post.shield,
        post.last_attack_landed, post.combo_count, post.last_hit_by, post.stocks,
    );
    for position in [&pre.position, &pre.joystick, &pre.cstick, &post.position] {
        mask!(rows, valid, position.validity.as_ref());
        columns!(rows, valid; position.x, position.y);
    }
    mask!(rows, valid, pre.triggers_physical.validity.as_ref());
    columns!(rows, valid; pre.triggers_physical.l, pre.triggers_physical.r);
    optional_columns!(rows, valid, version;
        pre.raw_analog_x => (1, 2),
        pre.percent => (1, 4),
        pre.raw_analog_y => (3, 15),
        pre.raw_analog_cstick_x => (3, 17),
        pre.raw_analog_cstick_y => (3, 17),
        post.state_age => (0, 2),
        post.misc_as => (2, 0),
        post.airborne => (2, 0),
        post.ground => (2, 0),
        post.jumps => (2, 0),
        post.l_cancel => (2, 0),
        post.hurtbox_state => (2, 1),
        post.hitlag => (3, 8),
        post.animation_index => (3, 11),
        post.last_hit_by_instance => (3, 16),
        post.instance_id => (3, 16),
    );
    present(&post.state_flags, version.gte(2, 0), "post.state_flags")?;
    if let Some(flags) = &post.state_flags {
        columns!(rows, valid; flags.0, flags.1, flags.2, flags.3, flags.4);
    }
    present(&post.velocities, version.gte(3, 5), "post.velocities")?;
    if let Some(velocities) = &post.velocities {
        mask!(rows, valid, velocities.validity.as_ref());
        columns!(rows, valid;
            velocities.self_x_air, velocities.self_y, velocities.knockback_x,
            velocities.knockback_y, velocities.self_x_ground,
        );
    }
    Ok(())
}

pub(crate) fn validate(game: &Game) -> Result<(), Error> {
    let frames = &game.frames;
    let rows = frames.len();
    let version = game.start.slippi.version;
    columns!(rows, None; frames.id);
    let occupancy = peppi::game::port_occupancy(&game.start);
    length(frames.ports.len(), occupancy.len(), "occupied ports")?;
    for (port, occupied) in frames.ports.iter().zip(occupancy) {
        if port.port != occupied.port || port.follower.is_some() != occupied.follower {
            return Err(Error::Invalid("frame ports disagree with GameStart".into()));
        }
        actor(&port.leader, rows, version)?;
        if let Some(follower) = &port.follower {
            actor(follower, rows, version)?;
        }
    }
    present(&frames.start, version.gte(2, 2), "frame.start")?;
    if let Some(start) = &frames.start {
        mask!(rows, None, start.validity.as_ref());
        columns!(rows, None; start.random_seed);
        optional_columns!(rows, None, version; start.scene_frame_counter => (3, 10));
    }
    present(&frames.end, version.gte(3, 0), "frame.end")?;
    if let Some(end) = &frames.end {
        // Before 3.7 no payload columns exist: the bitmap alone counts bookends.
        if version.lt(3, 7) && end.validity.is_none() {
            return Err(Error::Invalid("frame.end has no bookend row count".into()));
        }
        mask!(rows, None, end.validity.as_ref());
        optional_columns!(rows, None, version; end.latest_finalized_frame => (3, 7));
    }
    present(&frames.item, version.gte(3, 0), "frame.item")?;
    present(&frames.item_offset, version.gte(3, 0), "frame.item_offset")?;
    if let Some(item) = &frames.item {
        let events = item.id.len();
        mask!(events, None, item.validity.as_ref());
        mask!(events, None, item.position.validity.as_ref());
        mask!(events, None, item.velocity.validity.as_ref());
        columns!(events, None;
            item.r#type, item.state, item.direction, item.damage, item.timer, item.id,
            item.position.x, item.position.y, item.velocity.x, item.velocity.y,
        );
        optional_columns!(events, None, version;
            item.owner => (3, 6), item.instance_id => (3, 16),
        );
        present(&item.misc, version.gte(3, 2), "item.misc")?;
        if let Some(misc) = &item.misc {
            columns!(events, None; misc.0, misc.1, misc.2, misc.3);
        }
        offsets!(
            rows,
            events,
            frames.item_offset.as_ref().unwrap().as_slice()
        );
    }
    present(
        &frames.fod_platform,
        version.gte(3, 18),
        "frame.fod_platform",
    )?;
    present(
        &frames.fod_platform_offset,
        version.gte(3, 18),
        "frame.fod_platform_offset",
    )?;
    if let Some(platform) = &frames.fod_platform {
        let events = platform.platform.len();
        mask!(events, None, platform.validity.as_ref());
        columns!(events, None; platform.platform, platform.height);
        offsets!(
            rows,
            events,
            frames.fod_platform_offset.as_ref().unwrap().as_slice()
        );
    }
    present(
        &frames.dreamland_whispy,
        version.gte(3, 18),
        "frame.dreamland_whispy",
    )?;
    present(
        &frames.dreamland_whispy_offset,
        version.gte(3, 18),
        "frame.dreamland_whispy_offset",
    )?;
    if let Some(whispy) = &frames.dreamland_whispy {
        let events = whispy.direction.len();
        mask!(events, None, whispy.validity.as_ref());
        columns!(events, None; whispy.direction);
        offsets!(
            rows,
            events,
            frames.dreamland_whispy_offset.as_ref().unwrap().as_slice()
        );
    }
    present(
        &frames.stadium_transformation,
        version.gte(3, 18),
        "frame.stadium_transformation",
    )?;
    present(
        &frames.stadium_transformation_offset,
        version.gte(3, 18),
        "frame.stadium_transformation_offset",
    )?;
    if let Some(transformation) = &frames.stadium_transformation {
        let events = transformation.event.len();
        mask!(events, None, transformation.validity.as_ref());
        columns!(events, None; transformation.event, transformation.r#type);
        offsets!(
            rows,
            events,
            frames
                .stadium_transformation_offset
                .as_ref()
                .unwrap()
                .as_slice()
        );
    }
    Ok(())
}

/// `Replay` validates the immutable game once before exposing frame access.
pub(crate) fn read(game: &Game, index: usize) -> Result<Frame, Error> {
    if index >= game.frames.len() {
        return Err(Error::Invalid(format!(
            "frame index {index} is out of range"
        )));
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let version = game.start.slippi.version;
        let row = game.frames.transpose_one(index, version);
        let mut actors = Vec::with_capacity(game.frames.ports.len() * 2);
        for (source, port) in game.frames.ports.iter().zip(row.ports) {
            for (follower, source, data) in [
                (false, Some(&source.leader), Some(port.leader)),
                (true, source.follower.as_ref(), port.follower),
            ] {
                if let (Some(source), Some(data)) = (source, data)
                    && source
                        .validity
                        .as_ref()
                        .is_none_or(|bits| bits.get_bit(index))
                {
                    actors.push(Actor {
                        port: port.port,
                        follower,
                        pre: data.pre,
                        post: data.post,
                    });
                }
            }
        }
        Frame {
            id: row.id,
            actors,
            start: row.start,
            end: row.end,
            items: row.items,
            fod_platforms: row.fod_platforms,
            dreamland_whispys: row.dreamland_whispys,
            stadium_transformations: row.stadium_transformations,
        }
    }))
    .map_err(|_| Error::ParserPanic)
}

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::*;
    use peppi::frame::mutable;
    use std::io::Cursor;

    fn fixture(version: Version, follower: bool) -> Game {
        let bytes = support::replay_bytes(&support::Fixture {
            version,
            follower,
            ..Default::default()
        });
        peppi::io::slippi::read(Cursor::new(bytes), None).unwrap()
    }

    #[test]
    fn validates_old_and_current_column_layouts() {
        for version in [
            Version(2, 2, 0),
            Version(3, 0, 0),
            Version(3, 6, 0),
            Version(3, 7, 0),
            Version(3, 10, 0),
            Version(3, 18, 0),
        ] {
            let game = fixture(version, true);
            validate(&game).unwrap();
            assert_eq!(read(&game, 0).unwrap().actors.len(), 3);
            assert!(read(&game, game.frames.len()).is_err());
        }
    }

    #[test]
    fn absent_leaders_and_followers_are_not_zero_filled_actors() {
        let version = Version(3, 18, 0);
        let mut game = fixture(version, true);
        let mut absent = mutable::Frame::with_capacity(
            game.frames.len(),
            version,
            &peppi::game::port_occupancy(&game.start),
        );
        for _ in 0..game.frames.len() {
            for port in &mut absent.ports {
                port.leader.push_null(version);
                if let Some(follower) = &mut port.follower {
                    follower.push_null(version);
                }
            }
        }
        let absent: column::Frame = absent.into();
        let mut absent_ports = absent.ports.into_iter();
        game.frames.ports[0].leader = absent_ports.next().unwrap().leader;
        game.frames.ports[1].follower = absent_ports.next().unwrap().follower;
        validate(&game).unwrap();
        let frame = read(&game, 1).unwrap();
        assert_eq!(frame.actors.len(), 1);
        assert_eq!(frame.actors[0].port, peppi::game::Port::P3);
        assert!(!frame.actors[0].follower);

        game.frames.ports[0].leader.post.validity = None;
        assert!(validate(&game).is_err());
    }

    #[test]
    fn rejects_partial_nested_columns_and_inconsistent_validity() {
        let mut game = fixture(Version(3, 18, 0), false);
        game.frames.ports[0].leader.post.position.y.slice(0, 2);
        assert!(validate(&game).is_err());

        let mut game = fixture(Version(3, 18, 0), false);
        game.frames.ports[0].leader.pre.position.validity =
            Some([true, false, true].into_iter().collect());
        assert!(validate(&game).is_err());

        let mut game = fixture(Version(3, 18, 0), false);
        game.frames.ports[0].leader.pre.raw_analog_cstick_x = None;
        assert!(validate(&game).is_err());
    }

    #[test]
    fn rejects_unfinished_bookends_and_uncovered_events() {
        let mut game = fixture(Version(3, 0, 0), false);
        game.frames.end.as_mut().unwrap().validity = Some([true, true].into_iter().collect());
        assert!(validate(&game).is_err());

        let mut game = fixture(Version(3, 18, 0), false);
        game.frames
            .end
            .as_mut()
            .unwrap()
            .latest_finalized_frame
            .as_mut()
            .unwrap()
            .slice(0, 2);
        assert!(validate(&game).is_err());

        let mut game = fixture(Version(3, 18, 0), false);
        game.frames.item_offset = Some(vec![0, 0, 0].try_into().unwrap());
        assert!(validate(&game).is_err());

        let mut game = fixture(Version(3, 18, 0), false);
        game.frames.item_offset = Some(vec![0, 0, 0, 1].try_into().unwrap());
        assert!(validate(&game).is_err());
    }
}
