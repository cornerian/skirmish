//! Match-level coverage for immutable class parameters exposed to lifecycle callbacks.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{Action, BUTTON_A, Controller, Match, script::LocalValue, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
from dataclasses import dataclass, field

@dataclass(frozen=True, slots=True)
class Parameters(Attributes):
    typed_int: int = 17
    nested: dict = field(default_factory=lambda: {"float_value": 1.25})

class ActionState:
    seen_int: int = 0
    seen_float: float = 0.0

class ReadParameters(Move):
    action = "attack_air_b"
    @hook.enter("attack_air_b")
    def entered(self, fighter, ctx):
        fighter.action_state.seen_int = ctx.parameters.typed_int
        fighter.action_state.seen_float = ctx.parameters.nested.float_value

binding = ReadParameters()
ordinary = Move()

@register
class ParameterFighter(Fighter):
    name = "parameter_test"
    attributes = Parameters
    action_state = ActionState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, binding, ordinary, ordinary)
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

fn input() -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ]
}

#[test]
fn action_entered_callback_reads_typed_and_nested_parameters() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("parameter fixture compiles"));
    let mut game = Match::new(data, 0).expect("parameter match resources load");

    let state = game.step(input()).expect("aerial action enters");
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert_eq!(
        state.fighters[0].action_state["seen_int"],
        LocalValue::Integer(17)
    );
    assert_eq!(
        state.fighters[0].action_state["seen_float"],
        LocalValue::Number(1.25)
    );
}

#[test]
fn parameter_assignment_is_rejected_and_state_remains_uncommitted() {
    let source = SOURCE.replace(
        "fighter.action_state.seen_int = ctx.parameters.typed_int",
        "ctx.parameters.typed_int = 99",
    );
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(source).expect("readonly fixture compiles"));
    let mut game = Match::new(data, 0).expect("readonly match resources load");

    let error = game
        .step(input())
        .expect_err("parameter mutation must fail");
    assert!(
        error.to_string().contains("lifecycle hook ActionEntered"),
        "unexpected error: {error}"
    );
    assert!(
        !game.state().fighters[0]
            .action_state
            .contains_key("seen_int")
    );
}
