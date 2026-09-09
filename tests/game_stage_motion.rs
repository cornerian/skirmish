//! Resource-driven moving collision geometry composed with match physics.

#[path = "support/conformance.rs"]
mod support;

use skirmish::{
    collision::{ecb, stage},
    game::{
        BUTTON_A, BUTTON_X, Controller, Error, Event, Match,
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

fn transform(scale_x: f32, shear_y: f32, x: f32, y: f32) -> Transform {
    Transform {
        matrix: [[scale_x, 0.0, x], [shear_y, 1.0, y]],
    }
}

fn line(start: [f32; 2], end: [f32; 2], material: u16) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: stage::FLOOR | stage::ENABLED,
        material_flags: material,
        ..stage::Line::default()
    }
}

fn data(frames: Vec<Transform>) -> MatchData {
    let mut data = support::data();
    data.stage.floor.left = -150.0;
    data.stage.floor.right = 150.0;
    data.stage.blast = [-200.0, 200.0, -100.0, 200.0];
    data.stage.spawns = [[0.0, 6.0], [-20.0, 0.0]];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            line([-150.0, 0.0], [150.0, 0.0], 0),
            line([-100.0, 6.0], [100.0, 6.0], stage::PLATFORM as u16),
        ],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-160.0, -10.0],
            bounds_max: [160.0, 20.0],
            floor: 0..2,
            ..stage::Joint::default()
        }],
    });
    data.stage.motion = Some(Rules {
        tracks: vec![Track {
            lines: 1..2,
            frames,
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

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0002, "{actual} != {expected}");
}

#[test]
fn translated_and_scaled_platform_remaps_support_and_exposes_current_geometry() {
    let frames = vec![
        Transform::IDENTITY,
        transform(1.0, 0.0, 2.0, 1.0),
        transform(1.2, 0.0, 3.0, 2.0),
        Transform::IDENTITY,
    ];
    let mut game = Match::new(data(frames), 7).unwrap();
    assert_eq!(game.state().stage.frame, 0);
    assert_eq!(game.state().fighters[0].ground_line, Some(1));
    assert_eq!(game.state().fighters[1].ground_line, Some(0));

    let first = game.step(IDLE).unwrap().clone();
    assert_eq!(first.stage.frame, 1);
    near(first.fighters[0].position[0], 2.0);
    near(first.fighters[0].position[1], 7.0001);
    assert_eq!(first.fighters[0].contacts[0], Some(1));
    assert_eq!(first.fighters[1].position[0], -20.0);
    let geometry = game.stage_geometry();
    assert_eq!(geometry.lines[1].start, [-98.0, 7.0]);
    assert_eq!(geometry.lines[1].end, [102.0, 7.0]);
    assert_eq!(geometry.joints[0].bounds_min, [-180.0, -30.0]);
    assert_eq!(geometry.joints[0].bounds_max, [180.0, 37.0]);

    let second = game.step(IDLE).unwrap().clone();
    near(second.fighters[0].position[0], 3.0);
    near(second.fighters[0].position[1], 8.0001);
    let third = game.step(IDLE).unwrap().clone();
    near(third.fighters[0].position[0], 0.0);
    near(third.fighters[0].position[1], 6.0001);
    let wrapped = game.step(IDLE).unwrap();
    near(wrapped.fighters[0].position[0], 0.0);
    near(wrapped.fighters[0].position[1], 6.0001);
}

#[test]
fn platform_remap_runs_after_ground_self_motion() {
    let moving_data = data(vec![Transform::IDENTITY, transform(1.0, 0.0, 2.0, 1.0)]);
    let mut static_data = moving_data.clone();
    static_data.stage.motion = None;
    let mut moving = Match::new(moving_data, 11).unwrap();
    let mut fixed = Match::new(static_data, 11).unwrap();
    let input = [
        Controller {
            stick: [1.0, 0.0],
            ..Controller::default()
        },
        Controller::default(),
    ];
    let moved = moving.step(input).unwrap().fighters[0].position;
    let baseline = fixed.step(input).unwrap().fighters[0].position;
    near(moved[0], baseline[0] + 2.0);
    near(moved[1], baseline[1] + 1.0);
}

#[test]
fn airborne_fighter_stops_inheriting_motion_then_lands_on_current_platform() {
    let frames = (0..64)
        .map(|frame| transform(1.0, 0.0, frame as f32, 0.0))
        .collect();
    let mut game = Match::new(data(frames), 13).unwrap();
    let mut jump = IDLE;
    jump[0].buttons = BUTTON_X;
    game.step(jump).unwrap();
    while game.state().fighters[0].grounded {
        game.step(IDLE).unwrap();
    }
    let airborne_x = game.state().fighters[0].position[0];
    let frame = game.state().stage.frame;
    let next = game.step(IDLE).unwrap();
    assert_eq!(next.stage.frame, frame + 1);
    assert_eq!(next.fighters[0].position[0].to_bits(), airborne_x.to_bits());

    for _ in 0..50 {
        let state = game.step(IDLE).unwrap();
        if state.fighters[0].ground_line == Some(1) {
            let before = state.fighters[0].position[0];
            let carried = game.step(IDLE).unwrap();
            near(carried.fighters[0].position[0], before + 1.0);
            return;
        }
    }
    panic!("fighter never landed on the moving platform");
}

#[test]
fn moving_support_carries_grounded_fighters_during_hitlag() {
    let frames = (0..32)
        .map(|frame| transform(1.0, 0.0, frame as f32, 0.0))
        .collect();
    let mut resource = data(frames);
    resource.stage.spawns = [[-1.0, 6.0], [1.0, 6.0]];
    resource.rules.hitlag.base = 3.0;
    resource.rules.hitlag.damage_scale = 0.0;
    let mut game = Match::new(resource, 15).unwrap();
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..8 {
        let state = game.step(IDLE).unwrap();
        if state
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
        {
            assert!(state.fighters[0].grounded);
            assert!(!state.fighters[1].grounded);
            assert!(state.fighters.iter().all(|fighter| fighter.hitlag > 0.0));
            let before = state.fighters.each_ref().map(|fighter| fighter.position[0]);
            let carried = game.step(IDLE).unwrap();
            near(carried.fighters[0].position[0], before[0] + 1.0);
            assert_eq!(
                carried.fighters[1].position[0].to_bits(),
                before[1].to_bits()
            );
            return;
        }
    }
    panic!("jab never reached the moving-platform opponent");
}

#[test]
fn checkpoint_restores_stage_phase_geometry_and_carry_exactly() {
    let frames = vec![
        Transform::IDENTITY,
        transform(1.0, 0.1, 1.0, 0.5),
        transform(0.9, -0.1, 2.0, 1.0),
    ];
    let mut game = Match::new(data(frames), 17).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let mut expected = Vec::new();
    for frame in 0..12 {
        let mut input = IDLE;
        input[0].stick[0] = if frame % 4 < 2 { 0.75 } else { -0.5 };
        expected.push((input, game.step(input).unwrap().clone()));
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    for (input, state) in expected {
        assert_eq!(game.step(input).unwrap(), &state);
    }
}

#[test]
fn initial_transform_and_countdown_have_explicit_timing() {
    let mut resource = data(vec![transform(1.0, 0.0, 0.0, 2.0), Transform::IDENTITY]);
    resource.stage.spawns[0][1] = 8.0;
    resource.rules.countdown_frames = 2;
    let mut game = Match::new(resource, 19).unwrap();
    assert!(game.state().fighters[0].grounded);
    assert_eq!(game.state().fighters[0].position[1], 8.0);
    assert_eq!(game.state().stage.frame, 0);
    game.step(IDLE).unwrap();
    game.step(IDLE).unwrap();
    assert_eq!(game.state().stage.frame, 0);
    let playing = game.step(IDLE).unwrap();
    assert_eq!(playing.stage.frame, 1);
    near(playing.fighters[0].position[1], 6.0001);
}

#[test]
fn malformed_motion_resources_are_rejected_transactionally() {
    let valid = data(vec![Transform::IDENTITY]);
    let json = serde_json::to_string(&valid).unwrap();
    assert_eq!(serde_json::from_str::<MatchData>(&json).unwrap(), valid);
    let cases: [fn(&mut MatchData); 8] = [
        |data| data.stage.geometry = None,
        |data| data.stage.motion.as_mut().unwrap().tracks.clear(),
        |data| data.stage.motion.as_mut().unwrap().tracks[0].lines = 1..1,
        |data| data.stage.motion.as_mut().unwrap().tracks[0].lines = 1..3,
        |data| data.stage.motion.as_mut().unwrap().tracks[0].frames.clear(),
        |data| data.stage.motion.as_mut().unwrap().tracks[0].frames[0].matrix[0][0] = f32::NAN,
        |data| data.stage.motion.as_mut().unwrap().tracks[0].frames[0].matrix[0][0] = -1.0,
        |data| {
            let duplicate = data.stage.motion.as_ref().unwrap().tracks[0].clone();
            data.stage.motion.as_mut().unwrap().tracks.push(duplicate);
        },
    ];
    for corrupt in cases {
        let mut bad = valid.clone();
        corrupt(&mut bad);
        assert!(matches!(Match::new(bad, 23), Err(Error::Data(_))));
    }

    let mut game = Match::new(valid, 23).unwrap();
    let before = game.state().clone();
    let mut invalid = IDLE;
    invalid[0].stick[0] = f32::NAN;
    assert!(matches!(game.step(invalid), Err(Error::Input(0))));
    assert_eq!(game.state(), &before);
}
