//! Synthetic integration checks for animation-driven environmental collision.
use skirmish::collision::{ecb, stage};
use skirmish::game::{
    BUTTON_A, BUTTON_X, Controller, Error, Match,
    data::{CollisionBox, MatchData, StageGeometry},
};

const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
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

/// `docs/ecb-load-flags.md`: `game::collision::sample`'s `load_flags` picks
/// the per-path flags from `f.grounded`, not from the resource's own (now
/// unused) `CollisionBox::Bones.flags`. These two tests exercise the default
/// idle/fall pose (`data.bones`, sampled whenever no attack/special/movement
/// pose applies) directly, so no attack input or animation frame stepping is
/// needed -- unlike the wall tests above, which route through Jab specifically
/// to get an animated bone.
fn bones_ecb(data: &mut MatchData, root: [f32; 3], child: [f32; 3]) {
    let fighter = &mut data.fighters[0];
    fighter.bones[0].translation = root;
    fighter.bones[1].translation = child;
    fighter.collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        // Deliberately wrong (mode 0x12's narrow+two-unit-height bits, never
        // the correct 5/6): proves this field is read nowhere.
        flags: 0x12,
    };
}

/// `mpColl_LoadECB_JObj`'s `!(flags & CollisionFlagAir_CanGrabLedge)` padding
/// branch (`mpcoll.c:392-397`): airborne (`load_flags(false) == 6`) always
/// has bit 0x4 set, so the padding is never applied. A right-side joint at
/// x=0 is already forced to the +2 floor by `load_joints`'s own minimum-width
/// clamp regardless of padding, so this pins the distinguishing side: left
/// stays at the raw -5 sample; the old resource-flags bug (every pack's
/// `flags` was 0, meaning padding *was* applied) would have produced -7.
#[test]
fn airborne_bones_ecb_has_no_two_unit_padding() {
    let mut resource = data();
    resource.stage.floor.y = -7.0;
    bones_ecb(&mut resource, [0.0, 0.0, 0.0], [-5.0, 0.0, 0.0]);
    let game = Match::new(resource, 4).unwrap();
    let f = &game.state().fighters[0];
    assert!(
        !f.grounded,
        "fixture must spawn airborne for this to be mode 6"
    );
    assert_eq!(f.ecb.current.left[0], -5.0);
    assert_eq!(f.ecb.current.right[0], 2.0);
}

/// Two mechanisms from the same evidence (`docs/ecb-load-flags.md`'s entry
/// fall) in one synthetic fall: the root/child sample (offsets +3/+7) gives a
/// raw ECB bottom entirely *above* position, like the real recording's
/// falling pose. Landing must (1) rest `position` itself on the floor rather
/// than the elevated raw bottom (`ecb_unlocked` in `mpColl_80046904`, ported
/// in `game::collision::resolve`'s floor-contact branch), and (2) once
/// grounded, `load_flags(true) == 5` anchors the bottom to exactly 0
/// (`mpColl_LoadECB_JObj`'s `if (flags & 1) { bottom_y = 0.0F; ... }`,
/// mpcoll.c:425-429) even though the raw sample never changes.
#[test]
fn falling_bones_ecb_lands_on_position_then_anchors_the_bottom_to_zero() {
    let mut resource = data();
    resource.stage.spawns[0] = [-2.0, 5.0];
    bones_ecb(&mut resource, [0.0, 3.0, 0.0], [0.0, 4.0, 0.0]);
    let mut game = Match::new(resource, 6).unwrap();
    assert!(
        !game.state().fighters[0].grounded,
        "spawn must be well above the floor for this to fall and land"
    );
    for _ in 0..200 {
        game.step(IDLE).unwrap();
        if game.state().fighters[0].grounded {
            break;
        }
    }
    let f = game.state().fighters[0].clone();
    assert!(f.grounded, "expected to land within 200 frames");
    assert!(
        (f.position[1] - 0.0).abs() < 0.01,
        "position should rest at the floor (0), not the elevated raw bottom \
         (would be -3 without the ecb_unlocked floor-snap fix): {}",
        f.position[1]
    );
    // One more frame: `sample` now reads the settled `grounded == true`.
    game.step(IDLE).unwrap();
    let f = &game.state().fighters[0];
    assert!(f.grounded);
    assert_eq!(f.ecb.current.bottom[1], 0.0);
    assert_eq!(f.ecb.current.top[1], 7.0);
}
