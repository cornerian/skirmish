//! Match-level contracts for multi-surface ECB response. Exact squeeze helper
//! arithmetic remains covered against the selected original C bodies.

#[path = "support/conformance.rs"]
mod support;

use skirmish::{
    collision::{ecb, stage},
    game::{
        BUTTON_X, Controller, Event, Match,
        data::{CollisionBox, MatchData, StageGeometry},
        stage_motion::{Rules, Track, Transform},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn transform(x: f32, y: f32) -> Transform {
    Transform {
        matrix: [[1.0, 0.0, x], [0.0, 1.0, y]],
    }
}

fn line(start: [f32; 2], end: [f32; 2], kind: u32) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: stage::ENABLED | kind,
        ..stage::Line::default()
    }
}

fn boxed_data() -> MatchData {
    let mut data = support::data();
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -100.0, 200.0];
    data.stage.spawns = [[0.0, 0.0], [30.0, 0.0]];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            line([-100.0, 0.0], [100.0, 0.0], stage::FLOOR),
            line([100.0, 4.0], [-100.0, 4.0], stage::CEILING),
            line([-2.0, 10.0], [-2.0, -10.0], stage::RIGHT_WALL),
            line([2.0, -10.0], [2.0, 10.0], stage::LEFT_WALL),
        ],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-110.0, -20.0],
            bounds_max: [110.0, 20.0],
            floor: 0..1,
            ceiling: 1..2,
            right_wall: 2..3,
            left_wall: 3..4,
            dynamic: 0..0,
        }],
    });
    data.stage.motion = Some(Rules {
        tracks: vec![
            Track {
                lines: 1..2,
                frames: vec![
                    Transform::IDENTITY,
                    transform(0.0, -2.0),
                    Transform::IDENTITY,
                ],
            },
            Track {
                lines: 2..3,
                frames: vec![
                    Transform::IDENTITY,
                    transform(1.0, 0.0),
                    Transform::IDENTITY,
                ],
            },
            Track {
                lines: 3..4,
                frames: vec![
                    Transform::IDENTITY,
                    transform(-1.0, 0.0),
                    Transform::IDENTITY,
                ],
            },
        ],
    });
    for fighter in &mut data.fighters {
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
    data
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0003, "{actual} != {expected}");
}

#[test]
fn inward_stage_motion_squeezes_all_four_contacts_then_restores_the_ecb() {
    let mut game = Match::new(boxed_data(), 31).unwrap();
    let checkpoint = game.checkpoint();
    let initial = game.state().fighters[0].ecb.current;

    let squeezed = game.step(IDLE).unwrap().clone();
    let fighter = &squeezed.fighters[0];
    assert_eq!(squeezed.stage.frame, 1);
    assert_eq!(fighter.contacts, [Some(0), Some(1), Some(3), Some(2)]);
    assert_eq!(fighter.ground_line, Some(0));
    assert!(fighter.grounded);
    assert!(fighter.ecb.restore_unsqueezed);
    assert_eq!(fighter.ecb.unsqueezed, initial);
    assert!(!fighter.ecb.stop);
    near(fighter.position[0], 0.0);
    near(fighter.position[1], 0.0001);
    near(fighter.ecb.current.left[0], -1.0);
    near(fighter.ecb.current.right[0], 1.0);
    near(fighter.ecb.current.bottom[1], 0.0);
    near(fighter.ecb.current.top[1], 1.9998);

    let restored = game.step(IDLE).unwrap().clone();
    let fighter = &restored.fighters[0];
    assert_eq!(restored.stage.frame, 2);
    assert_eq!(fighter.contacts, [Some(0), None, None, None]);
    assert_eq!(fighter.ecb.current, initial);
    assert_eq!(fighter.ecb.desired, initial);
    assert!(!fighter.ecb.restore_unsqueezed);

    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.step(IDLE).unwrap(), &squeezed);
    assert_eq!(game.step(IDLE).unwrap(), &restored);
}

fn rising_floor(platform: bool) -> MatchData {
    let mut data = support::data();
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -100.0, 200.0];
    data.stage.spawns = [[0.0, 5.0], [30.0, 0.0]];
    let mut floor = line([-20.0, 0.0], [20.0, 0.0], stage::FLOOR);
    if platform {
        floor.material_flags = stage::PLATFORM as u16;
    }
    data.stage.geometry = Some(StageGeometry {
        lines: vec![floor, line([-100.0, -20.0], [100.0, -20.0], stage::FLOOR)],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-110.0, -30.0],
            bounds_max: [110.0, 20.0],
            floor: 0..2,
            ..stage::Joint::default()
        }],
    });
    data.stage.motion = Some(Rules {
        tracks: vec![Track {
            lines: 0..1,
            frames: vec![Transform::IDENTITY, transform(0.0, 6.0)],
        }],
    });
    for fighter in &mut data.fighters {
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
    data
}

#[test]
fn rising_solid_floor_lands_an_airborne_fighter_without_self_motion() {
    let mut data = rising_floor(false);
    data.fighters[0].movement.gravity = 0.0;
    let mut game = Match::new(data, 32).unwrap();
    assert!(!game.state().fighters[0].grounded);
    let state = game.step(IDLE).unwrap();
    let fighter = &state.fighters[0];
    assert_eq!(fighter.ground_line, Some(0));
    assert_eq!(fighter.contacts[0], Some(0));
    assert!(fighter.grounded);
    near(fighter.position[1], 6.0001);
    assert!(state.events.contains(&Event::Landed { player: 0 }));
}

#[test]
fn rising_one_way_floor_does_not_catch_an_upward_fighter() {
    let mut data = rising_floor(true);
    let movement = &mut data.fighters[0].movement;
    movement.gravity = 0.0;
    movement.jump_vertical_velocity = 0.25;
    movement.short_hop_vertical_velocity = 0.25;
    let mut game = Match::new(data, 33).unwrap();
    let mut jump = IDLE;
    jump[0].buttons = BUTTON_X;
    let state = game.step(jump).unwrap();
    let fighter = &state.fighters[0];
    assert!(fighter.velocity[1] > 0.0);
    assert!(fighter.position[1] < 6.0);
    assert!(!fighter.grounded);
    assert_ne!(fighter.contacts[0], Some(0));
    assert!(!state.events.contains(&Event::Landed { player: 0 }));
}

#[test]
fn floor_sliding_overhead_does_not_land_a_fighter_already_below_its_plane() {
    let mut data = rising_floor(false);
    data.fighters[0].movement.gravity = 0.0;
    let geometry = data.stage.geometry.as_mut().unwrap();
    geometry.lines[0].start = [10.0, 6.0];
    geometry.lines[0].end = [30.0, 6.0];
    data.stage.motion.as_mut().unwrap().tracks[0].frames =
        vec![Transform::IDENTITY, transform(-20.0, 0.0)];
    let mut game = Match::new(data, 34).unwrap();
    assert!(!game.state().fighters[0].grounded);
    let state = game.step(IDLE).unwrap();
    let fighter = &state.fighters[0];
    assert!(!fighter.grounded);
    assert_eq!(fighter.ground_line, None);
    assert_eq!(fighter.contacts[0], None);
    assert!(!state.events.contains(&Event::Landed { player: 0 }));
}
