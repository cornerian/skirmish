//! Match-level matrix-aware hurt-capsule collision composition.
use skirmish::game::{
    BUTTON_A, Controller, Event, Match,
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
    data.fighters[1].bones[0].scale = [4.0, 0.5, 1.0];
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
        hit.radius = 0.1;
    }
    data
}

fn connects(data: MatchData) -> bool {
    let mut game = Match::new(data, 31).unwrap();
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
            return true;
        }
        game.step(IDLE).unwrap();
    }
    false
}

#[test]
fn bone_scale_extends_hurt_radius_along_its_long_axis() {
    assert!(connects(data([3.0, 0.0, 0.0])));
}

#[test]
fn bone_scale_reduces_hurt_radius_along_its_short_axis() {
    assert!(!connects(data([0.0, 0.75, 0.0])));
}
