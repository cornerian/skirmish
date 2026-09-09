//! Synthetic integration contracts for the partial native ECB response policy.
//! Original stage helpers have separate C differential tests; these tests do
//! not certify Melee's complete corner, squeeze or moving-platform scheduler.
use serde_json::Value;
use skirmish::collision::{ecb, stage};
use skirmish::game::{
    Action, BUTTON_A, BUTTON_X, Controller, Error, Event, Match, State,
    data::{Bone, CollisionBox, Floor, MatchData, StageGeometry},
};

const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    let mut resource: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    resource.rules.countdown_frames = 0;
    resource.stage.floor = Floor {
        left: -40.0,
        right: 40.0,
        y: 0.0,
    };
    resource.stage.blast = [-60.0, 60.0, -30.0, 100.0];
    resource.stage.spawns = [[0.0, 0.0], [-20.0, 0.0]];
    for fighter in &mut resource.fighters {
        fighter.collision_box = CollisionBox::Fixed {
            source: ecb::FixedSource {
                up: 3.0,
                down: 0.0,
                front: 1.5,
                back: 1.5,
                angle: 0.0,
            },
        };
    }
    resource
}

fn line(start: [f32; 2], end: [f32; 2], kind: u32) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: kind | stage::ENABLED,
        ..Default::default()
    }
}

fn geometry(lines: Vec<stage::Line>) -> StageGeometry {
    // Each fixture keeps lines of a given kind contiguous, like MapJoint ranges.
    let range = |kind| {
        let begin = lines
            .iter()
            .position(|line| line.flags & kind != 0)
            .unwrap_or(0);
        let end = lines
            .iter()
            .rposition(|line| line.flags & kind != 0)
            .map_or(0, |i| i + 1);
        begin..end
    };
    let joint = stage::Joint {
        id: 0,
        flags: stage::ENABLED,
        bounds_min: [-50.0, -20.0],
        bounds_max: [50.0, 80.0],
        floor: range(stage::FLOOR),
        ceiling: range(stage::CEILING),
        left_wall: range(stage::LEFT_WALL),
        right_wall: range(stage::RIGHT_WALL),
        dynamic: 0..0,
    };
    StageGeometry {
        lines,
        joints: vec![joint],
    }
}

fn input(buttons: u16, horizontal: f32) -> [Controller; 2] {
    [
        Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            buttons,
            stick: [horizontal, 0.0],
        },
        IDLE[1],
    ]
}

fn floor_height(line: &stage::Line, x: f32) -> f32 {
    line.start[1]
        + (line.end[1] - line.start[1]) * (x - line.start[0]) / (line.end[0] - line.start[0])
}

fn fast_jump(resource: &mut MatchData) {
    let movement = &mut resource.fighters[0].movement;
    movement.jump_vertical_velocity = 12.0;
    movement.short_hop_vertical_velocity = 12.0;
    movement.gravity = 2.0;
    movement.terminal_velocity = 12.0;
    movement.fast_fall_velocity = 12.0;
}

#[test]
fn walks_up_connected_slopes_then_loses_support_past_the_edge() {
    let mut resource = data();
    resource.stage.spawns = [[-6.0, 1.0], [-7.0, 0.5]];
    let mut lines = vec![
        line([-8.0, 0.0], [0.0, 4.0], stage::FLOOR),
        line([0.0, 4.0], [8.0, 6.0], stage::FLOOR),
    ];
    lines[0].next[0] = Some(1);
    lines[1].previous[0] = Some(0);
    resource.stage.geometry = Some(geometry(lines.clone()));
    let mut game = Match::new(resource, 1).unwrap();
    assert!(game.state().fighters[0].grounded);
    assert_eq!(game.state().fighters[0].ground_line, Some(0));
    let mut crossed = false;
    let mut fell = false;
    for _ in 0..40 {
        let before_x = game.state().fighters[0].position[0];
        let fighter = &game.step(input(0, 1.0)).unwrap().fighters[0];
        assert!(fighter.position[0] > before_x);
        if let Some(id) = fighter.ground_line {
            crossed |= id == 1;
            assert!(fighter.grounded);
            assert_eq!(fighter.contacts[0], Some(id));
            let expected_y = floor_height(&lines[id], fighter.position[0]);
            assert!(
                (fighter.position[1] - expected_y).abs() < 0.002,
                "{fighter:?}"
            );
            assert!(fighter.floor_normal[0] < 0.0 && fighter.floor_normal[1] > 0.0);
        } else {
            assert!(crossed);
            assert!(!fighter.grounded);
            assert_eq!(fighter.action, Action::Fall);
            assert!(fighter.position[0] > 8.0);
            fell = true;
            break;
        }
    }
    assert!(crossed && fell);
}

#[test]
fn high_speed_jump_stops_at_ceiling_without_tunneling() {
    let mut resource = data();
    fast_jump(&mut resource);
    resource.stage.geometry = Some(geometry(vec![
        line([-30.0, 0.0], [30.0, 0.0], stage::FLOOR),
        line([15.0, 8.0], [-15.0, 8.0], stage::CEILING),
    ]));
    let mut game = Match::new(resource, 2).unwrap();
    let mut touched = false;
    for _ in 0..8 {
        let fighter = &game.step(input(BUTTON_X, 0.0)).unwrap().fighters[0];
        assert!(
            fighter.position[1] + fighter.ecb.current.top[1] <= 8.002,
            "{fighter:?}"
        );
        if fighter.contacts[1] == Some(1) {
            touched = true;
            assert!(fighter.velocity[1] <= 0.0);
        }
    }
    assert!(touched, "the 12-unit launch must contact the ceiling");
}

#[test]
fn high_speed_horizontal_movement_stops_on_both_wall_orientations() {
    for direction in [-1.0_f32, 1.0] {
        let mut resource = data();
        let movement = &mut resource.fighters[0].movement;
        movement.walk_acceleration_mul = 12.0;
        movement.walk_acceleration_base = 0.0;
        movement.walk_max_velocity = 12.0;
        movement.ground_max_horizontal_velocity = 12.0;
        let wall = if direction > 0.0 {
            line([5.0, -10.0], [5.0, 30.0], stage::LEFT_WALL)
        } else {
            line([-5.0, 30.0], [-5.0, -10.0], stage::RIGHT_WALL)
        };
        resource.stage.geometry = Some(geometry(vec![
            line([-30.0, 0.0], [30.0, 0.0], stage::FLOOR),
            wall,
        ]));
        let mut game = Match::new(resource, 3).unwrap();
        let slot = if direction > 0.0 { 2 } else { 3 };
        for _ in 0..3 {
            let fighter = &game.step(input(0, direction)).unwrap().fighters[0];
            let side = if direction > 0.0 {
                fighter.ecb.current.right[0]
            } else {
                fighter.ecb.current.left[0]
            };
            assert!(
                (fighter.position[0] + side) * direction <= 5.002,
                "{fighter:?}"
            );
            assert_eq!(fighter.contacts[slot], Some(1));
            assert_eq!(fighter.ground_velocity, 0.0);
        }
    }
}

#[test]
fn platform_allows_upward_pass_then_catches_descending_ecb_bottom() {
    let mut resource = data();
    fast_jump(&mut resource);
    let mut platform = line([-10.0, 6.0], [10.0, 6.0], stage::FLOOR);
    platform.material_flags = stage::PLATFORM as u16;
    resource.stage.geometry = Some(geometry(vec![
        line([-30.0, 0.0], [30.0, 0.0], stage::FLOOR),
        platform,
    ]));
    let mut game = Match::new(resource, 4).unwrap();
    let mut passed_above = false;
    let mut landed = false;
    for _ in 0..30 {
        let snapshot = game.step(input(BUTTON_X, 0.0)).unwrap();
        let fighter = &snapshot.fighters[0];
        let foot = fighter.position[1] + fighter.ecb.current.bottom[1];
        if foot > 6.002 && fighter.velocity[1] > 0.0 {
            passed_above = true;
            assert!(!fighter.grounded);
            assert_ne!(fighter.contacts[0], Some(1));
        }
        if fighter.ground_line == Some(1) {
            assert!(passed_above);
            assert!(snapshot.events.contains(&Event::Landed { player: 0 }));
            assert!(fighter.grounded);
            assert!((foot - 6.0).abs() < 0.002);
            landed = true;
            break;
        }
    }
    assert!(passed_above && landed);
}

#[test]
fn malformed_geometry_rejects_creation_and_invalid_input_preserves_ecb_state() {
    let mut resource = data();
    resource.stage.geometry = Some(geometry(vec![line(
        [-30.0, 0.0],
        [30.0, 0.0],
        stage::FLOOR,
    )]));
    let mut game = Match::new(resource.clone(), 5).unwrap();
    for case in 0..5 {
        let mut bad = resource.clone();
        let geometry = bad.stage.geometry.as_mut().unwrap();
        match case {
            0 => geometry.lines[0].next[0] = Some(10),
            1 => geometry.joints[0].floor = 0..2,
            2 => geometry.lines[0].start[0] = f32::NAN,
            3 => geometry.lines[0].end = geometry.lines[0].start,
            _ => {
                bad.fighters[0].collision_box = CollisionBox::Bones {
                    indices: [999; 6],
                    parameters: ecb::JointParameters {
                        side_y_offset: 0.0,
                        height_threshold: 4.0,
                        width_threshold: 4.0,
                    },
                    flags: 0,
                }
            }
        }
        assert!(matches!(Match::new(bad, 5), Err(Error::Data(_))));
    }
    let before = state_bits(game.state());
    assert!(matches!(
        game.step(input(0, f32::NAN)),
        Err(Error::Input(0))
    ));
    assert_eq!(state_bits(game.state()), before);
}

fn state_bits(state: &State) -> Value {
    fn bits(value: &mut Value) {
        match value {
            Value::Number(number) if number.is_f64() => {
                *value = Value::String(format!("{:016x}", number.as_f64().unwrap().to_bits()));
            }
            Value::Array(values) => values.iter_mut().for_each(bits),
            Value::Object(values) => values.values_mut().for_each(bits),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(state).unwrap();
    bits(&mut value);
    value
}

#[test]
fn checkpoint_restores_all_ecb_history_and_contact_state_bitwise() {
    let mut resource = data();
    resource.stage.geometry = Some(geometry(vec![line(
        [-30.0, 0.0],
        [30.0, 0.0],
        stage::FLOOR,
    )]));
    let fighter = &mut resource.fighters[0];
    let top = Bone {
        parent: Some(0),
        classical_scale: false,
        translation: [0.0, 4.0, 0.0],
        rotation: [0.0; 3],
        scale: [1.0; 3],
    };
    fighter.bones.push(top.clone());
    for (frame, height) in fighter.jab.frames.iter_mut().zip([4.0, 6.0, 9.0, 7.0, 4.0]) {
        frame.bones.push(Bone {
            translation: [0.0, height, 0.0],
            ..top.clone()
        });
    }
    fighter.collision_box = CollisionBox::Bones {
        indices: [0, 1, 2, 0, 1, 2],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    let mut game = Match::new(resource, 6).unwrap();
    game.step(input(BUTTON_A, 0.0)).unwrap();
    let checkpoint = game.checkpoint();
    let saved = state_bits(game.state());
    let old_shape = game.state().fighters[0].ecb.current;
    let script: Vec<_> = (0..25)
        .map(|frame| {
            if (7..11).contains(&frame) {
                input(BUTTON_X, 0.5)
            } else {
                IDLE
            }
        })
        .collect();
    let expected: Vec<_> = script
        .iter()
        .map(|&input| state_bits(game.step(input).unwrap()))
        .collect();
    assert_ne!(old_shape.top[1].to_bits(), 9.0_f32.to_bits());
    assert!(
        expected
            .iter()
            .any(|state| state["fighters"][0]["ecb"]["current"]
                != saved["fighters"][0]["ecb"]["current"])
    );
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(state_bits(game.state()), saved);
    for (&input, expected) in script.iter().zip(expected) {
        assert_eq!(state_bits(game.step(input).unwrap()), expected);
    }
}
