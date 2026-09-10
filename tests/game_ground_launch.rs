//! End-to-end grounded launch projection and scalar knockback decay.
use skirmish::{
    collision::stage,
    fighter::damage::{DamageMotion, HurtHeight},
    game::{
        Action, BUTTON_A, Controller, Error, Event, Match,
        damage::{DamageMotionRules, DamagePoseAttributes, GroundLaunchRules},
        data::{Bone, MatchData, StageGeometry},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn repeated_pose(base: &[Bone]) -> Vec<Vec<Bone>> {
    vec![base.to_vec()]
}

fn poses(base: &[Bone]) -> DamagePoseAttributes {
    let motion = repeated_pose(base);
    DamagePoseAttributes {
        hurtbox_heights: vec![HurtHeight::Middle],
        ground: core::array::from_fn(|_| core::array::from_fn(|_| motion.clone())),
        air: core::array::from_fn(|_| motion.clone()),
        fly: core::array::from_fn(|_| motion.clone()),
    }
}

fn data(thresholds: [f32; 3], angle: f32) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.damage.damage_motion = Some(DamageMotionRules { thresholds });
    data.rules.damage.ground_launch = Some(GroundLaunchRules {
        fly_bounce_angle_radians: 0.2,
        fly_bounce_vertical_multiplier: 0.5,
        ground_knockback_friction_multiplier: 2.0,
    });
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    for fighter in &mut data.fighters {
        fighter.damage_poses = Some(poses(&fighter.bones));
    }
    for hit in data.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.angle_degrees = angle;
        hit.radius = 30.0;
    }
    data
}

fn hit(data: MatchData) -> Match {
    let mut game = Match::new(data, 42).unwrap();
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..5 {
        game.step(IDLE).unwrap();
        if game
            .state()
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
        {
            return game;
        }
    }
    panic!("jab did not connect")
}

fn event_knockback(game: &Match) -> f32 {
    game.state()
        .events
        .iter()
        .find_map(|event| match event {
            Event::Hit { knockback, .. } => Some(*knockback),
            _ => None,
        })
        .unwrap()
}

fn finish_hitlag(game: &mut Match) {
    while game.state().fighters[1].hitlag > 0.0 {
        game.step(IDLE).unwrap();
    }
}

#[test]
fn low_horizontal_hit_stays_grounded_freezes_then_decays_the_scalar() {
    let mut game = hit(data([100.0, 200.0, 300.0], 0.0));
    let fighter = &game.state().fighters[1];
    assert_eq!(fighter.action, Action::Damage);
    assert_eq!(
        fighter.damage_motion,
        Some(DamageMotion::Ground {
            level: 0,
            height: HurtHeight::Middle,
        })
    );
    assert!(fighter.grounded);
    assert!(fighter.ground_line.is_some());
    assert_eq!(fighter.locomotion.jumps_used, 0);
    assert!(fighter.ground_knockback > 0.0);
    assert_eq!(fighter.knockback[0], fighter.ground_knockback);
    assert_eq!(
        fighter.knockback[1].to_bits(),
        (-fighter.floor_normal[0] * fighter.ground_knockback).to_bits()
    );
    assert!(
        serde_json::to_string(game.state())
            .unwrap()
            .contains("\"ground_knockback\"")
    );

    let frozen_position = fighter.position;
    let frozen_scalar = fighter.ground_knockback;
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].position, frozen_position);
    assert_eq!(game.state().fighters[1].ground_knockback, frozen_scalar);

    finish_hitlag(&mut game);
    let checkpoint = game.checkpoint();
    let decrement = game.data().fighters[1].movement.ground_friction
        * game
            .data()
            .rules
            .damage
            .ground_launch
            .as_ref()
            .unwrap()
            .ground_knockback_friction_multiplier;
    let expected = frozen_scalar - decrement;
    game.step(IDLE).unwrap();
    let after = &game.state().fighters[1];
    assert_eq!(after.ground_knockback.to_bits(), expected.to_bits());
    assert_eq!(after.knockback[0].to_bits(), expected.to_bits());
    assert_eq!(
        after.knockback[1].to_bits(),
        (-after.floor_normal[0] * expected).to_bits()
    );
    let expected_state = serde_json::to_vec(game.state()).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    game.step(IDLE).unwrap();
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), expected_state);
}

#[test]
fn rising_low_hit_and_horizontal_fly_hit_both_leave_ground() {
    let rising = hit(data([100.0, 200.0, 300.0], 30.0));
    let fighter = &rising.state().fighters[1];
    assert!(!fighter.grounded);
    assert_eq!(fighter.ground_line, None);
    assert_eq!(fighter.ground_knockback, 0.0);
    assert!(fighter.knockback[1] > 0.0);
    assert_eq!(
        fighter.damage_motion,
        Some(DamageMotion::Ground {
            level: 0,
            height: HurtHeight::Middle,
        })
    );

    let fly = hit(data([0.01, 0.02, 0.03], 0.0));
    let fighter = &fly.state().fighters[1];
    assert!(!fighter.grounded);
    assert_eq!(fighter.ground_line, None);
    assert_eq!(fighter.ground_knockback, 0.0);
    assert_eq!(fighter.locomotion.jumps_used, 1);
    assert_eq!(
        fighter.damage_motion,
        Some(DamageMotion::Fly {
            height: HurtHeight::Middle,
        })
    );
    assert_eq!(fighter.knockback[1], 0.0);
}

#[test]
fn downward_fly_launch_reverses_and_scales_vertical_knockback() {
    let game = hit(data([0.01, 0.02, 0.03], 330.0));
    let fighter = &game.state().fighters[1];
    let speed = event_knockback(&game) * game.data().rules.knockback_speed;
    let radians = 330.0_f32 * f32::from_bits(0x3c8e_fa35);
    let expected = [
        speed * libm::cosf(radians),
        -(speed * libm::sinf(radians))
            * game
                .data()
                .rules
                .damage
                .ground_launch
                .as_ref()
                .unwrap()
                .fly_bounce_vertical_multiplier,
    ];
    assert!(!fighter.grounded);
    assert_eq!(fighter.ground_knockback, 0.0);
    for (actual, expected) in fighter.knockback.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000001);
    }
    assert!(fighter.knockback[1] > 0.0);
}

#[test]
fn low_launch_uses_the_supporting_slope_tangent() {
    let mut data = data([100.0, 200.0, 300.0], 0.0);
    data.stage.floor.y = -100.0;
    data.stage.spawns = [[-0.5, -0.375], [0.5, 0.375]];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![stage::Line {
            start: [-100.0, -75.0],
            end: [100.0, 75.0],
            flags: stage::FLOOR | stage::ENABLED,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-110.0, -90.0],
            bounds_max: [110.0, 90.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    let game = hit(data);
    let fighter = &game.state().fighters[1];
    assert!(fighter.grounded);
    assert!((fighter.floor_normal[0] + 0.6).abs() < 0.000001);
    assert!((fighter.floor_normal[1] - 0.8).abs() < 0.000001);
    assert_eq!(
        fighter.knockback[0].to_bits(),
        (fighter.floor_normal[1] * fighter.ground_knockback).to_bits()
    );
    assert_eq!(
        fighter.knockback[1].to_bits(),
        (-fighter.floor_normal[0] * fighter.ground_knockback).to_bits()
    );
}

#[test]
fn grounded_launch_resources_are_explicit_validated_and_roundtrip() {
    let resource = data([100.0, 200.0, 300.0], 0.0);
    let encoded = serde_json::to_string(&resource).unwrap();
    assert!(encoded.contains("\"ground_launch\""));
    assert_eq!(
        serde_json::from_str::<MatchData>(&encoded).unwrap(),
        resource
    );
    Match::new(resource.clone(), 1).unwrap();

    let mut orphan = resource.clone();
    orphan.rules.damage.damage_motion = None;
    assert!(
        matches!(Match::new(orphan, 1), Err(Error::Data(message)) if message.contains("requires damage-motion"))
    );

    for invalid in [
        GroundLaunchRules {
            fly_bounce_angle_radians: -0.1,
            ..resource.rules.damage.ground_launch.clone().unwrap()
        },
        GroundLaunchRules {
            fly_bounce_vertical_multiplier: f32::NAN,
            ..resource.rules.damage.ground_launch.clone().unwrap()
        },
        GroundLaunchRules {
            ground_knockback_friction_multiplier: -1.0,
            ..resource.rules.damage.ground_launch.clone().unwrap()
        },
    ] {
        let mut bad = resource.clone();
        bad.rules.damage.ground_launch = Some(invalid);
        assert!(
            matches!(Match::new(bad, 1), Err(Error::Data(message)) if message.contains("grounded-launch"))
        );
    }
}
