//! Owner identity keeps animation-loop metadata separate for moves that share
//! one native action and animation track.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{Action, BUTTON_A, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, register)

class Finite(Move):
    action = action("attack_air_b", animation_loop=False)

class Looping(Move):
    action = action("attack_air_b", animation_loop=True)

finite = Finite()
looping = Looping()
ordinary = Move()

@register
class TestFighter(Fighter):
    name = "owner_animation_test"
    attributes = Attributes
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(finite, looping, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn data() -> skirmish::game::data::MatchData {
    let mut data = aerial::data();
    let moves = &mut data.fighters[0].aerials.as_mut().unwrap().moves;
    for mov in moves.iter_mut().take(2) {
        let hitbox = |y| {
            serde_json::from_value(serde_json::json!({
                "group": 0, "bone": 0, "center": [0.0, y, 0.0], "radius": 1.0,
                "damage": 1, "angle_degrees": 45.0, "growth": 1, "fixed": 1, "base": 1
            }))
            .unwrap()
        };
        for (index, frame) in mov.attack.frames.iter_mut().enumerate() {
            frame.hitboxes = vec![hitbox(if index % 2 == 0 { 1.0 } else { 9.0 })];
        }
    }
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    data
}

fn input(forward: bool) -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            cstick: if forward { [1.0, 0.0] } else { [0.0, 0.0] },
            ..Default::default()
        },
        Controller::default(),
    ]
}

#[test]
fn shared_native_action_uses_each_move_owner_for_looping_and_checkpoint_replay() {
    let mut finite = Match::new(data(), 0).expect("owner fixture registers");
    let state = finite.step(input(false)).unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    for _ in 0..7 {
        finite.step([Controller::default(); 2]).unwrap();
    }
    let state = finite
        .step([Controller::default(); 2])
        .expect("finite owner reaches the native aerial boundary");
    assert_eq!(state.fighters[0].action, Action::Fall);

    let mut looping = Match::new(data(), 0).expect("owner fixture registers");
    let state = looping.step(input(true)).unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert!((state.fighters[0].hitboxes[0].current[1] - 1.0).abs() < 2e-3);
    for _ in 0..7 {
        looping.step([Controller::default(); 2]).unwrap();
    }
    let state = looping.step([Controller::default(); 2]).unwrap();
    assert!((state.fighters[0].hitboxes[0].current[1] - 1.0).abs() < 2e-3);

    let checkpoint = looping.checkpoint();
    let expected = serde_json::to_vec(looping.step([Controller::default(); 2]).unwrap()).unwrap();
    looping.restore_checkpoint(&checkpoint).unwrap();
    let replay = looping.step([Controller::default(); 2]).unwrap();
    assert_eq!(serde_json::to_vec(replay).unwrap(), expected);
}
