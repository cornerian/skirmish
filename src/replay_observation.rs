//! Explicit input and observation policies for the experimental native match.
//! No recorded position, action state or random seed initializes the simulator.

use crate::{game, slippi};
use serde::Serialize;
use slippi::Port;
use std::fmt;

pub const FIELDS: &[&str] = &[
    "position.x",
    "position.y",
    "direction",
    "percent",
    "stocks",
    "airborne",
];

pub const INPUT_POLICY: &str = "processed main-stick XY and physical A/X/Y; neutral C-stick and triggers; main-stick direction flags allowed; no replay state or RNG overrides";

const BUTTONS: u16 = game::BUTTON_A | game::BUTTON_X | game::BUTTON_Y;
// HSD_PadADConvert in the pinned controller.c derives these four flags from the
// main stick. Bits 20..23 describe the C-stick; bit 31 is actionable HSD_PAD_LR
// (Fighter_Spaghetti_8006AD10), so those are deliberately unsupported here.
const MAIN_STICK_FLAGS: u32 = 0x000f_0000;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FighterObservation {
    pub port: Port,
    pub position: [f32; 2],
    pub direction: f32,
    pub percent: f32,
    pub stocks: u8,
    pub airborne: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Observation {
    pub fighters: [FighterObservation; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    pub port: Port,
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}: expected bits {}, got {}",
            self.port, self.field, self.expected, self.actual
        )
    }
}

fn actors(frame: &slippi::Frame, ports: [Port; 2]) -> Result<[&slippi::Actor; 2], String> {
    if ports[0] == ports[1] {
        return Err("native player ports must be distinct".into());
    }
    if frame.actors.len() != 2 || frame.actors.iter().any(|actor| actor.follower) {
        return Err(format!(
            "frame {} requires exactly two present leader actors without followers",
            frame.id
        ));
    }
    let find = |port| {
        frame
            .actors
            .iter()
            .find(|actor| actor.port == port)
            .ok_or_else(|| format!("frame {} is missing leader {port}", frame.id))
    };
    Ok([find(ports[0])?, find(ports[1])?])
}

pub fn controllers(
    frame: &slippi::Frame,
    ports: [Port; 2],
) -> Result<[game::Controller; 2], String> {
    let convert = |actor: &slippi::Actor| {
        let pre = &actor.pre;
        let stick = [pre.joystick.x, pre.joystick.y];
        let unsupported_physical = pre.buttons_physical & !BUTTONS;
        let unsupported_logical = pre.buttons & !(u32::from(BUTTONS) | MAIN_STICK_FLAGS);
        if unsupported_physical != 0 || unsupported_logical != 0 {
            return Err(format!(
                "{} unsupported button bits: physical {unsupported_physical:#06x}, processed {unsupported_logical:#010x}",
                actor.port
            ));
        }
        if [
            pre.cstick.x,
            pre.cstick.y,
            pre.triggers,
            pre.triggers_physical.l,
            pre.triggers_physical.r,
        ]
        .iter()
        .any(|&value| value != 0.0)
        {
            return Err(format!(
                "{} requires neutral C-stick and triggers",
                actor.port
            ));
        }
        if stick
            .iter()
            .any(|axis| !axis.is_finite() || !(-1.0..=1.0).contains(axis))
        {
            return Err(format!(
                "{} main-stick values must be finite and within [-1, 1]",
                actor.port
            ));
        }
        Ok(game::Controller {
            buttons: pre.buttons_physical,
            stick,
        })
    };
    let [first, second] = actors(frame, ports)?;
    Ok([convert(first)?, convert(second)?])
}

pub fn expected(frame: &slippi::Frame, ports: [Port; 2]) -> Result<Observation, String> {
    let convert = |actor: &slippi::Actor| {
        let post = &actor.post;
        let airborne = match post.airborne {
            Some(0) => false,
            Some(1) => true,
            _ => {
                return Err(format!(
                    "{} post.airborne must be present and either 0 or 1",
                    actor.port
                ));
            }
        };
        Ok(FighterObservation {
            port: actor.port,
            position: [post.position.x, post.position.y],
            direction: post.direction,
            percent: post.percent,
            stocks: post.stocks,
            airborne,
        })
    };
    let [first, second] = actors(frame, ports)?;
    Ok(Observation {
        fighters: [convert(first)?, convert(second)?],
    })
}

pub fn observe(state: &game::State, ports: [Port; 2]) -> Observation {
    Observation {
        fighters: std::array::from_fn(|index| {
            let fighter = &state.fighters[index];
            FighterObservation {
                port: ports[index],
                position: fighter.position,
                direction: fighter.facing,
                percent: fighter.percent,
                stocks: fighter.stocks,
                airborne: !fighter.grounded,
            }
        }),
    }
}

fn difference(
    port: Port,
    field: &'static str,
    expected: u32,
    actual: u32,
    width: usize,
) -> Option<Difference> {
    (expected != actual).then(|| Difference {
        port,
        field,
        expected: format!("0x{expected:0width$x}"),
        actual: format!("0x{actual:0width$x}"),
    })
}

pub fn compare(expected: &Observation, actual: &Observation) -> Option<Difference> {
    for (expected, actual) in expected.fighters.iter().zip(&actual.fighters) {
        let port = expected.port;
        if let Some(difference) =
            difference(port, "port", expected.port as u32, actual.port as u32, 2)
        {
            return Some(difference);
        }
        for (field, expected, actual) in [
            (FIELDS[0], expected.position[0], actual.position[0]),
            (FIELDS[1], expected.position[1], actual.position[1]),
            (FIELDS[2], expected.direction, actual.direction),
            (FIELDS[3], expected.percent, actual.percent),
        ] {
            if let Some(difference) =
                difference(port, field, expected.to_bits(), actual.to_bits(), 8)
            {
                return Some(difference);
            }
        }
        for (field, expected, actual) in [
            (FIELDS[4], expected.stocks, actual.stocks),
            (
                FIELDS[5],
                u8::from(expected.airborne),
                u8::from(actual.airborne),
            ),
        ] {
            if let Some(difference) =
                difference(port, field, u32::from(expected), u32::from(actual), 2)
            {
                return Some(difference);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use slippi::row;

    const PORTS: [Port; 2] = [Port::P3, Port::P1];

    fn frame() -> slippi::Frame {
        slippi::Frame {
            id: -123,
            actors: [Port::P1, Port::P3]
                .map(|port| slippi::Actor {
                    port,
                    follower: false,
                    pre: row::Pre::default(),
                    post: row::Post {
                        airborne: Some(0),
                        stocks: 4,
                        ..Default::default()
                    },
                })
                .into(),
            start: None,
            end: None,
            items: None,
            fod_platforms: None,
            dreamland_whispys: None,
            stadium_transformations: None,
        }
    }

    #[test]
    fn controllers_preserve_stick_bits_and_do_not_copy_recorded_state() {
        let mut frame = frame();
        let pre = &mut frame.actors[1].pre;
        pre.joystick = row::Position {
            x: -0.0,
            y: f32::from_bits(0x3eaa_aaab),
        };
        pre.buttons_physical = game::BUTTON_A | game::BUTTON_X;
        pre.buttons = u32::from(game::BUTTON_A) | MAIN_STICK_FLAGS;
        pre.cstick.x = -0.0;
        pre.triggers_physical.r = -0.0;
        pre.position = row::Position {
            x: f32::NAN,
            y: f32::INFINITY,
        };
        pre.random_seed = u32::MAX;
        pre.state = u16::MAX;
        let controllers = controllers(&frame, PORTS).unwrap();
        assert_eq!(
            controllers[0].stick.map(f32::to_bits),
            [0x8000_0000, 0x3eaa_aaab]
        );
        assert_eq!(controllers[0].buttons, game::BUTTON_A | game::BUTTON_X);
        assert_eq!(controllers[1], game::Controller::default());
    }

    #[test]
    fn unsupported_inputs_and_actor_sets_are_errors() {
        for flag in [0x1, 0x10, 0x20, 0x40, 0x80, 0x200, 0x1000] {
            let mut frame = frame();
            frame.actors[0].pre.buttons_physical = flag;
            assert!(controllers(&frame, PORTS).is_err());
        }
        for flag in [
            0x1,
            0x10,
            0x200,
            0x1000,
            0x0010_0000,
            0x0100_0000,
            0x8000_0000,
        ] {
            let mut frame = frame();
            frame.actors[0].pre.buttons = flag;
            assert!(controllers(&frame, PORTS).is_err());
        }
        for index in 0..5 {
            let mut frame = frame();
            let pre = &mut frame.actors[0].pre;
            let fields = [
                &mut pre.cstick.x,
                &mut pre.cstick.y,
                &mut pre.triggers,
                &mut pre.triggers_physical.l,
                &mut pre.triggers_physical.r,
            ];
            *fields[index] = f32::from_bits(1);
            assert!(controllers(&frame, PORTS).is_err());
        }
        for value in [f32::NAN, f32::INFINITY, -1.001, 1.001] {
            let mut frame = frame();
            frame.actors[0].pre.joystick.x = value;
            assert!(controllers(&frame, PORTS).is_err());
        }
        assert!(controllers(&frame(), [Port::P1; 2]).is_err());
        let mut missing = frame();
        missing.actors.pop();
        assert!(controllers(&missing, PORTS).is_err());
        let mut follower = frame();
        follower.actors[1].follower = true;
        assert!(expected(&follower, PORTS).is_err());
    }

    #[test]
    fn observations_map_ports_and_report_each_selected_field_by_bits() {
        let mut frame = frame();
        frame.actors[1].post.position.x = -0.0;
        let expected = expected(&frame, PORTS).unwrap();
        assert_eq!(expected.fighters[0].port, Port::P3);
        assert_eq!(expected.fighters[0].position[0].to_bits(), 0x8000_0000);
        assert!(compare(&expected, &expected).is_none());
        for &field in FIELDS {
            let mut actual = expected.clone();
            let fighter = &mut actual.fighters[0];
            match field {
                "position.x" => fighter.position[0] = 0.0,
                "position.y" => fighter.position[1] = f32::from_bits(1),
                "direction" => fighter.direction = -1.0,
                "percent" => fighter.percent = 1.0,
                "stocks" => fighter.stocks -= 1,
                "airborne" => fighter.airborne = true,
                _ => unreachable!(),
            }
            let difference = compare(&expected, &actual).unwrap();
            assert_eq!((difference.port, difference.field), (Port::P3, field));
            assert!(difference.to_string().contains(field));
            assert!(serde_json::to_string(&difference).unwrap().contains("0x"));
            if field == "position.x" {
                assert_eq!(difference.expected, "0x80000000");
                assert_eq!(difference.actual, "0x00000000");
            }
        }
        for airborne in [None, Some(2)] {
            frame.actors[0].post.airborne = airborne;
            assert!(super::expected(&frame, PORTS).is_err());
        }
    }

    #[test]
    fn native_observation_uses_only_the_declared_post_fields() {
        let data = serde_json::from_str(include_str!(
            "../crates/arena/tests/fixtures/integration-match.json"
        ))
        .unwrap();
        let game = game::Match::new(data, 1).unwrap();
        let observed = observe(game.state(), PORTS);
        for (index, fighter) in observed.fighters.iter().enumerate() {
            let native = &game.state().fighters[index];
            assert_eq!(fighter.port, PORTS[index]);
            assert_eq!(
                fighter.position.map(f32::to_bits),
                native.position.map(f32::to_bits)
            );
            assert_eq!(fighter.direction.to_bits(), native.facing.to_bits());
            assert_eq!(fighter.percent.to_bits(), native.percent.to_bits());
            assert_eq!(fighter.stocks, native.stocks);
            assert_eq!(fighter.airborne, !native.grounded);
        }
    }
}
