//! Match-level special angle-362 contact direction and launch composition.
use skirmish::game::{
    BUTTON_A, Controller, Event, Match, State,
    data::{Hurtbox, HurtboxState, MatchData},
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data(center: [f32; 3]) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.spawns = [[0.0, 0.0]; 2];
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.fighters[1].hurtboxes = vec![Hurtbox {
        bone: 0,
        start: [0.0; 3],
        end: [0.0; 3],
        radius: 1.0,
        state: HurtboxState::Enabled,
        grabbable: true,
    }];
    for hit in data.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.bone = 0;
        hit.center = center;
        hit.radius = 0.5;
        hit.angle_degrees = 362.0;
    }
    data
}

fn connect(data: MatchData) -> State {
    let mut game = Match::new(data, 37).unwrap();
    let checkpoint = game.checkpoint();
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..6 {
        if game.state().events.iter().any(|event| {
            matches!(
                event,
                Event::Hit {
                    attacker: 0,
                    victim: 1,
                    ..
                }
            )
        }) {
            let expected = game.state().clone();
            game.restore_checkpoint(&checkpoint).unwrap();
            game.step(attack).unwrap();
            for _ in 0..6 {
                if game
                    .state()
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Hit { .. }))
                {
                    assert_eq!(game.state(), &expected);
                    return expected;
                }
                game.step(IDLE).unwrap();
            }
            panic!("restored jab did not connect");
        }
        game.step(IDLE).unwrap();
    }
    panic!("jab did not connect: {:?}", game.state())
}

#[test]
fn contact_quadrants_select_signed_angle_and_damage_facing() {
    for (center, facing, signs) in [
        ([-1.0, -1.0, 0.0], -1.0, [1.0, 1.0]),
        ([1.0, -1.0, 0.0], 1.0, [-1.0, 1.0]),
        ([-1.0, 1.0, 0.0], -1.0, [1.0, -1.0]),
    ] {
        let state = connect(data(center));
        let victim = &state.fighters[1];
        assert_eq!(victim.facing, facing);
        assert_eq!(victim.knockback.map(f32::signum), signs);
        assert!((victim.knockback[0].abs() - victim.knockback[1].abs()).abs() < 0.0001);
    }
}

#[test]
fn vertical_contact_uses_zero_angle_and_the_source_direction_tie() {
    let state = connect(data([0.0, -1.0, 0.0]));
    let victim = &state.fighters[1];
    assert_eq!(victim.facing, -1.0);
    assert!(victim.knockback[0] > 0.0);
    assert_eq!(victim.knockback[1], 0.0);
}
