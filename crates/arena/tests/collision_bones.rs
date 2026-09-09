//! Synthetic integration checks for animation-driven environmental collision.
use arena::{
    BUTTON_A, BUTTON_X, Controller, Error, Match,
    data::{CollisionBox, MatchData, StageGeometry},
};
use physics::{ecb, stage};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    for fighter in &mut data.fighters {
        for frame in &mut fighter.jab.frames {
            frame.hitboxes.clear();
        }
    }
    data
}

fn wall(data: &mut MatchData, start: [f32; 2], end: [f32; 2]) {
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            stage::Line {
                start: [-8.0, 0.0],
                end: [8.0, 0.0],
                flags: stage::ENABLED | stage::FLOOR,
                ..Default::default()
            },
            stage::Line {
                start,
                end,
                flags: stage::ENABLED | stage::LEFT_WALL,
                ..Default::default()
            },
        ],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-8.0, -10.0],
            bounds_max: [8.0, 20.0],
            floor: 0..1,
            left_wall: 1..2,
            ..Default::default()
        }],
    });
}

fn sample_bones(data: &mut MatchData, extension: f32) {
    let fighter = &mut data.fighters[0];
    fighter.collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    fighter.jab.frames[1].bones[1].translation[0] = extension;
}

fn attack() -> [Controller; 2] {
    let mut input = IDLE;
    input[0].buttons = BUTTON_A;
    input
}

#[test]
fn bone_animation_alone_moves_the_collision_box_into_a_wall() {
    let mut animated = data();
    sample_bones(&mut animated, 5.0);
    wall(&mut animated, [2.0, -10.0], [2.0, 20.0]);
    let mut fixed = animated.clone();
    fixed.fighters[0].collision_box = data().fighters[0].collision_box.clone();
    let mut animated = Match::new(animated, 1).unwrap();
    let mut fixed = Match::new(fixed, 1).unwrap();
    for game in [&mut animated, &mut fixed] {
        game.step(attack()).unwrap();
        assert_eq!(game.state().fighters[0].position[0], -2.0);
        game.step(IDLE).unwrap();
    }
    let f = &animated.state().fighters[0];
    assert_eq!(f.ecb.current.right[0], 5.0);
    assert_eq!(f.contacts[2], Some(1));
    assert!(f.position[0] < -2.9);
    assert!((f.position[0] + f.ecb.current.right[0] - 2.0).abs() < 0.002);
    assert_eq!(f.velocity, [0.0; 2]);
    assert_eq!(fixed.state().fighters[0].position[0], -2.0);
    assert_eq!(fixed.state().fighters[0].contacts[2], None);
}

#[test]
fn animation_subdivision_error_rolls_back_ecb_action_and_frame_state() {
    let mut resource = data();
    sample_bones(&mut resource, 100_000.0);
    let mut game = Match::new(resource, 2).unwrap();
    game.step(attack()).unwrap();
    let checkpoint = game.checkpoint();
    // JSON serialization retains finite float sign bits and all public state,
    // including the five ECB history shapes and load/interpolation flags.
    let before = serde_json::to_vec(game.state()).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            game.step(IDLE),
            Err(Error::Physics(message)) if message.contains("subdivision")
        ));
        assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    game.reset(2);
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[0].ecb.current.right[0], 2.0);
}

#[test]
fn sloped_wall_response_must_not_accept_a_penetrating_jump() {
    let mut resource = data();
    resource.stage.spawns[0] = [-3.0, 0.0];
    wall(&mut resource, [0.0, 0.0], [-5.0, 10.0]);
    let mut game = Match::new(resource, 3).unwrap();
    let mut jumping = IDLE;
    jumping[0].buttons = BUTTON_X;
    for _ in 0..3 {
        game.step(jumping).unwrap();
    }
    let f = &game.state().fighters[0];
    let [side_x, side_y] = [
        f.position[0] + f.ecb.current.right[0],
        f.position[1] + f.ecb.current.right[1],
    ];
    assert_eq!(f.contacts[2], Some(1));
    assert!(side_x <= -0.5 * side_y + 0.002, "{f:?}");
}
