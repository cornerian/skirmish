#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{Action, BUTTON_A, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class Neutral(Move):
    action = "attack_air_b"
    @hook.enter("attack_air_b")
    def enter(self, fighter, ctx): fighter.state.neutral = fighter.state.neutral + 1
    @hook.exit("attack_air_b")
    def exit(self, fighter, ctx): fighter.state.neutral_exit = fighter.state.neutral_exit + 1

class Forward(Move):
    action = "attack_air_b"
    @hook.enter("attack_air_b")
    def enter(self, fighter, ctx): fighter.state.forward = fighter.state.forward + 1
    @hook.exit("attack_air_b")
    def exit(self, fighter, ctx): fighter.state.forward_exit = fighter.state.forward_exit + 1

class State:
    neutral: int = 0
    forward: int = 0
    neutral_exit: int = 0
    forward_exit: int = 0

n = Neutral(); f = Forward(); ordinary = Move()
@register
class TestFighter(Fighter):
    name = "identity_test"
    attributes = Attributes; state = State; action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(n, f, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

#[test]
fn same_canonical_action_keeps_selected_owner_across_reentry() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    for flag in &mut data.fighters[0].aerials.as_mut().unwrap().moves[2].flags {
        flag.allow_interrupt = true;
    }
    let mut game = Match::new(data, 0).unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_A,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert_eq!(
        state.fighters[0].script_state["neutral"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    assert_eq!(
        state.fighters[0].script_state["forward"],
        skirmish::game::script::LocalValue::Integer(0)
    );
    let checkpoint = game.checkpoint();

    let state = game
        .step([
            Controller {
                buttons: BUTTON_A,
                cstick: [1.0, 0.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert_eq!(
        state.fighters[0].script_state["neutral_exit"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    assert_eq!(
        state.fighters[0].script_state["forward"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    let expected = serde_json::to_vec(state).unwrap();

    game.restore_checkpoint(&checkpoint).unwrap();
    let restored = game
        .step([
            Controller {
                buttons: BUTTON_A,
                cstick: [1.0, 0.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(serde_json::to_vec(restored).unwrap(), expected);
}
