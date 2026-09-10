//! End-to-end fighter-contact direction, facing and grounded projection.
use skirmish::{
    fighter::damage::HurtHeight,
    game::{
        BUTTON_A, Controller, Event, Match, State,
        damage::{DamageMotionRules, DamagePoseAttributes, GroundLaunchRules},
        data::{Bone, MatchData},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data(attacker: usize, attacker_x: f32, victim_x: f32) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.spawns[attacker] = [attacker_x, 0.0];
    data.stage.spawns[1 - attacker] = [victim_x, 0.0];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    for hit in data.fighters[attacker]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.angle_degrees = 0.0;
        hit.radius = 30.0;
    }
    data
}

fn connect(game: &mut Match, attacker: usize) -> State {
    let mut attack = IDLE;
    attack[attacker].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..8 {
        if game.state().events.iter().any(|event| {
            matches!(
                event,
                Event::Hit {
                    attacker: source,
                    victim,
                    ..
                } if *source == attacker && *victim == 1 - attacker
            )
        }) {
            return game.state().clone();
        }
        game.step(IDLE).unwrap();
    }
    panic!("jab did not connect: {:?}", game.state())
}

fn poses(base: &[Bone]) -> DamagePoseAttributes {
    let motion = vec![base.to_vec()];
    DamagePoseAttributes {
        hurtbox_heights: vec![HurtHeight::Middle],
        ground: core::array::from_fn(|_| core::array::from_fn(|_| motion.clone())),
        air: core::array::from_fn(|_| motion.clone()),
        fly: core::array::from_fn(|_| motion.clone()),
    }
}

#[test]
fn either_fighter_launches_the_victim_away_by_position_and_reorients_them() {
    for (attacker, attacker_x, victim_x, facing, launch_sign) in
        [(0, 2.0, -2.0, 1.0, -1.0), (1, -2.0, 2.0, -1.0, 1.0)]
    {
        let mut game = Match::new(data(attacker, attacker_x, victim_x), 19).unwrap();
        let checkpoint = game.checkpoint();
        let expected = connect(&mut game, attacker);
        let victim = &expected.fighters[1 - attacker];
        assert_eq!(victim.facing, facing);
        assert_eq!(victim.knockback[0].signum(), launch_sign);
        assert_eq!(victim.knockback[1], 0.0);

        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(connect(&mut game, attacker), expected);
    }
}

#[test]
fn equal_x_uses_the_source_tie_direction() {
    let mut game = Match::new(data(0, 0.0, 0.0), 23).unwrap();
    let state = connect(&mut game, 0);
    assert_eq!(state.fighters[1].facing, 1.0);
    assert!(state.fighters[1].knockback[0] < 0.0);
}

#[test]
fn position_direction_composes_with_grounded_tangent_launch() {
    let mut resource = data(0, 2.0, -2.0);
    resource.rules.damage.damage_motion = Some(DamageMotionRules {
        thresholds: [100.0, 200.0, 300.0],
    });
    resource.rules.damage.ground_launch = Some(GroundLaunchRules {
        fly_bounce_angle_radians: 0.2,
        fly_bounce_vertical_multiplier: 0.5,
        ground_knockback_friction_multiplier: 2.0,
    });
    for fighter in &mut resource.fighters {
        fighter.damage_poses = Some(poses(&fighter.bones));
    }
    let mut game = Match::new(resource, 29).unwrap();
    let state = connect(&mut game, 0);
    let victim = &state.fighters[1];
    assert_eq!(victim.facing, 1.0);
    assert!(victim.grounded);
    assert!(victim.ground_knockback < 0.0);
    assert_eq!(victim.knockback[0], victim.ground_knockback);
    assert_eq!(victim.knockback[1].to_bits(), (-0.0_f32).to_bits());
}
