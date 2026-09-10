//! Match-level wall-jump contacts, motion, stage speed and deterministic state.

#[path = "support/aerial.rs"]
mod aerial_resources;
#[path = "support/special.rs"]
mod special_resources;
use aerial_resources::conformance as support;

use skirmish::{
    collision::{ecb, stage},
    game::{
        Action, BUTTON_A, BUTTON_B, BUTTON_X, Controller, Event, Match, State,
        data::{CollisionBox, MatchData, StageGeometry},
        stage_motion::{Rules as MotionRules, Track, Transform},
        wall_jump::{Attributes, Rules},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn line(start: [f32; 2], end: [f32; 2], kind: u32) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: stage::ENABLED | kind,
        ..stage::Line::default()
    }
}

fn data() -> MatchData {
    let mut data = support::data();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.wall_jump = Some(Rules {
        tilt_deadzone: 0.3,
        input_window: 5.0,
        stick_threshold: 0.7,
        tilt_window: 3.0,
        startup_frames: 2,
        vertical_velocity_base: 0.5,
    });
    data.stage.floor.left = -20.0;
    data.stage.floor.right = 20.0;
    data.stage.blast = [-30.0, 30.0, -20.0, 80.0];
    data.stage.spawns = [[0.0, 10.0], [0.0, 0.0]];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            line([-20.0, 0.0], [20.0, 0.0], stage::FLOOR),
            line([5.0, -10.0], [5.0, 50.0], stage::LEFT_WALL),
            line([-5.0, 50.0], [-5.0, -10.0], stage::RIGHT_WALL),
        ],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-25.0, -15.0],
            bounds_max: [25.0, 55.0],
            floor: 0..1,
            left_wall: 1..2,
            right_wall: 2..3,
            ..stage::Joint::default()
        }],
    });
    for fighter in &mut data.fighters {
        fighter.movement.gravity = 0.0;
        fighter.collision_box = CollisionBox::Fixed {
            source: ecb::FixedSource {
                up: 3.0,
                down: 0.0,
                front: 1.5,
                back: 1.5,
                angle: 0.0,
            },
        };
        let mut frames = vec![fighter.bones.clone(); 8];
        for (frame, pose) in frames.iter_mut().enumerate() {
            pose[0].translation[1] += frame as f32 * 0.01;
        }
        fighter.wall_jump = Some(Attributes {
            can_walljump: true,
            minimum_approach_speed: 0.2,
            horizontal_velocity: 3.0,
            vertical_velocity: 4.0,
            frames,
        });
    }
    data
}

fn input(stick_x: f32) -> [Controller; 2] {
    let mut result = IDLE;
    result[0].stick[0] = stick_x;
    result
}

fn step(game: &mut Match, stick_x: f32) -> State {
    game.step(input(stick_x)).unwrap().clone()
}

fn approach(game: &mut Match, direction: f32) -> State {
    for _ in 0..80 {
        let fighter = &game.state().fighters[0];
        let boundary = 3.5 * direction;
        let near = if direction > 0.0 {
            fighter.position[0] + fighter.velocity[0] >= boundary - 0.8
        } else {
            fighter.position[0] + fighter.velocity[0] <= boundary + 0.8
        };
        let state = step(game, if near { -direction } else { direction });
        if state
            .events
            .iter()
            .any(|event| matches!(event, Event::WallJumped { player: 0, .. }))
        {
            return state;
        }
    }
    panic!("wall jump was not reached: {:?}", game.state().fighters[0]);
}

#[test]
fn both_wall_sides_launch_after_a_fresh_away_flick_and_decay_repeated_height() {
    let mut game = Match::new(data(), 7).unwrap();
    let first = approach(&mut game, 1.0);
    let fighter = &first.fighters[0];
    assert!(
        first
            .events
            .contains(&Event::WallJumped { player: 0, line: 1 })
    );
    assert_eq!(fighter.action, Action::PassiveWallJump);
    assert_eq!(fighter.facing, -1.0);
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert_eq!(fighter.wall_jump.startup_timer, 2);
    assert_eq!(fighter.wall_jump.used, 1);

    step(&mut game, -1.0);
    let launched = step(&mut game, -1.0);
    assert_eq!(launched.fighters[0].velocity[1], 4.0);
    assert!(launched.fighters[0].velocity[0] < 0.0);

    let second = approach(&mut game, -1.0);
    assert!(
        second
            .events
            .contains(&Event::WallJumped { player: 0, line: 2 })
    );
    assert_eq!(second.fighters[0].facing, 1.0);
    assert_eq!(second.fighters[0].wall_jump.used, 2);
    step(&mut game, 1.0);
    let launched = step(&mut game, 1.0);
    assert_eq!(launched.fighters[0].velocity[1], 2.0);
    assert!(launched.fighters[0].velocity[0] > 0.0);
}

#[test]
fn startup_and_sampled_motion_replay_bit_exactly_from_a_checkpoint() {
    let mut game = Match::new(data(), 11).unwrap();
    approach(&mut game, 1.0);
    let checkpoint = game.checkpoint();
    let expected = [
        step(&mut game, -1.0),
        step(&mut game, -1.0),
        step(&mut game, 0.0),
    ];
    game.restore_checkpoint(&checkpoint).unwrap();
    let actual = [
        step(&mut game, -1.0),
        step(&mut game, -1.0),
        step(&mut game, 0.0),
    ];
    assert_eq!(actual, expected);
    assert_eq!(actual[0].fighters[0].wall_jump.startup_timer, 1);
    assert_eq!(actual[1].fighters[0].velocity[1], 4.0);
}

#[test]
fn ordinary_wall_jump_dispatches_supported_air_actions_after_startup() {
    for (buttons, expected) in [
        (BUTTON_B, Action::SpecialAirN),
        (BUTTON_A, Action::AttackAirN),
        (BUTTON_X, Action::JumpAerial),
        (BUTTON_A | BUTTON_X, Action::AttackAirN),
        (BUTTON_A | BUTTON_B, Action::SpecialAirN),
    ] {
        let mut resource = data();
        let mut aerial = aerial_resources::data();
        for player in 0..2 {
            resource.fighters[player].aerials = aerial.fighters[player].aerials.take();
        }
        let resource = special_resources::profile(resource);
        let mut game = Match::new(resource, 23).unwrap();
        approach(&mut game, 1.0);
        while game.state().fighters[0].wall_jump.startup_timer != 0 {
            step(&mut game, -1.0);
        }
        let mut input = IDLE;
        input[0].buttons = buttons;
        assert_eq!(game.step(input).unwrap().fighters[0].action, expected);
    }
}

#[test]
fn translating_wall_uses_relative_speed_to_arm_a_stationary_fighter() {
    let mut resource = data();
    resource.stage.spawns[0] = [0.0, 10.0];
    resource.fighters[0].movement.air_drift_stick_mul = 0.0;
    resource.fighters[0].movement.aerial_drift_base = 0.0;
    resource.stage.motion = Some(MotionRules {
        tracks: vec![Track {
            lines: 1..2,
            frames: vec![
                Transform::IDENTITY,
                Transform {
                    matrix: [[1.0, 0.0, -3.6], [0.0, 1.0, 0.0]],
                },
            ],
        }],
    });
    let mut game = Match::new(resource, 13).unwrap();
    let state = step(&mut game, -1.0);
    assert!(
        state
            .events
            .contains(&Event::WallJumped { player: 0, line: 1 }),
        "{state:?}"
    );
    assert!((state.fighters[0].position[0] + 0.1).abs() < 0.000_01);
    assert_eq!(state.fighters[0].wall_jump.used, 1);
}

#[test]
fn sampled_wall_jump_bones_drive_headless_ecb_updates() {
    let mut resource = data();
    resource.fighters[0].movement.air_drift_stick_mul = 0.0;
    resource.fighters[0].movement.aerial_drift_base = 0.0;
    resource.fighters[0].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    resource.fighters[0].wall_jump.as_mut().unwrap().frames[1][1].translation[0] = -6.0;
    resource.stage.motion = Some(MotionRules {
        tracks: vec![Track {
            lines: 1..2,
            frames: vec![
                Transform::IDENTITY,
                Transform {
                    matrix: [[1.0, 0.0, -3.1], [0.0, 1.0, 0.0]],
                },
                Transform {
                    matrix: [[1.0, 0.0, 50.0], [0.0, 1.0, 0.0]],
                },
            ],
        }],
    });
    let mut game = Match::new(resource, 19).unwrap();
    assert!(
        step(&mut game, -1.0)
            .events
            .contains(&Event::WallJumped { player: 0, line: 1 })
    );
    assert_eq!(step(&mut game, 0.0).fighters[0].ecb.desired.right[0], 6.0);
}

#[test]
fn landing_restores_repeated_wall_jump_height() {
    let mut resource = data();
    resource.fighters[0].movement.gravity = 0.1;
    resource.fighters[0]
        .wall_jump
        .as_mut()
        .unwrap()
        .vertical_velocity = 2.0;
    let mut game = Match::new(resource, 17).unwrap();
    approach(&mut game, 1.0);
    assert_eq!(game.state().fighters[0].wall_jump.used, 1);
    for _ in 0..80 {
        let state = step(&mut game, 0.0);
        if state.fighters[0].grounded {
            assert_eq!(state.fighters[0].wall_jump.used, 0);
            assert_eq!(state.fighters[0].wall_jump.input_timer, 254);
            return;
        }
    }
    panic!("wall jumper did not land");
}

#[test]
fn common_and_fighter_wall_jump_resources_are_paired_and_validated() {
    let mut missing_fighter = data();
    missing_fighter.fighters[0].wall_jump = None;
    assert!(Match::new(missing_fighter, 0).is_err());

    let mut missing_rules = data();
    missing_rules.rules.wall_jump = None;
    assert!(Match::new(missing_rules, 0).is_err());

    for invalidate in [0, 1, 2] {
        let mut invalid = data();
        match invalidate {
            0 => invalid.rules.wall_jump.as_mut().unwrap().stick_threshold = f32::NAN,
            1 => {
                invalid.fighters[0]
                    .wall_jump
                    .as_mut()
                    .unwrap()
                    .minimum_approach_speed = -1.0
            }
            _ => invalid.fighters[0]
                .wall_jump
                .as_mut()
                .unwrap()
                .frames
                .clear(),
        }
        assert!(Match::new(invalid, 0).is_err());
    }

    let mut incapable = data();
    incapable.fighters[0]
        .wall_jump
        .as_mut()
        .unwrap()
        .can_walljump = false;
    let mut game = Match::new(incapable, 0).unwrap();
    for _ in 0..30 {
        step(&mut game, 1.0);
    }
    assert_eq!(game.state().fighters[0].wall_jump.used, 0);
}
