//! Acceptance coverage for lifecycle callback ordering and rollback.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::{
    BUTTON_B, Controller, Match,
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, Button, DefenseMoves, Fighter,
    GetupMoves, GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, hook, register)

class State:
    count: int = 0
class InputMove(Move):
    @hook.input_pressed(Button.B)
    def first(self, fighter, context):
        fighter.state.count = fighter.state.count + 1
        return True
ordinary = InputMove()
class LaterMove(Move):
    @hook.input_pressed(Button.B)
    def later(self, fighter, context):
        1 / 0
later = LaterMove()
@register
class LifecycleFighter(Fighter):
    name = "lifecycle_input"
    attributes = Attributes
    state = State
    specials = SpecialMoves(ordinary, later, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
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

#[test]
fn first_true_skips_later_lifecycle_callback() {
    let mut data = conformance::data();
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([
        Controller {
            buttons: BUTTON_B,
            ..Default::default()
        },
        Controller::default(),
    ])
    .expect("first affirmative callback must short-circuit later callback");
    assert_eq!(
        game.state().fighters[0].script_state["count"],
        LocalValue::Integer(1)
    );
}

#[test]
fn invalid_lifecycle_return_does_not_commit_staged_state() {
    let mut data = conformance::data();
    data.fighters[0].script =
        Some(Program::new(SOURCE.replace("return True", "return 7")).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    let error = game
        .step([
            Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect_err("invalid lifecycle return must fail");
    assert!(error.to_string().contains("invalid return"));
    assert!(!game.state().fighters[0].script_state.contains_key("count"));
}
