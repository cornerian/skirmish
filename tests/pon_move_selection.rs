//! Native controller input selects the class move identity before entering a
//! native action. The destination aerial owns the command stream and landing
//! flags, even when the input slot's ordinary resource is different.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{Action, BUTTON_A, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class NeutralBinding(Move):
    action = "attack_air_b"
    @hook.enter("attack_air_b")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class ActionState:
    entered: int = 0

binding = NeutralBinding()
ordinary = Move()

@register
class TestFighter(Fighter):
    name = "test_fighter"
    attributes = Attributes
    action_state = ActionState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(binding, ordinary, ordinary, ordinary, ordinary)
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
fn neutral_controller_aerial_uses_registered_back_aerial_resources() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("move definition compiles"));

    // Make source neutral and destination back observably different. The
    // selected action must use the destination's command flags after entry.
    let moves = &mut data.fighters[0].aerials.as_mut().unwrap().moves;
    moves[0].flags[0].landing_lag = false;
    moves[2].flags[0].landing_lag = true;

    let mut game = Match::new(data, 0).expect("registered resources validate");
    let state = game
        .step([
            Controller {
                buttons: BUTTON_A,
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect("neutral aerial input executes");
    let fighter = &state.fighters[0];
    assert_eq!(fighter.action, Action::AttackAirB);
    assert!(fighter.aerial.landing_lag_enabled);
    assert_eq!(
        fighter.action_state["entered"],
        skirmish::game::script::LocalValue::Integer(1)
    );

    let state = game
        .step([Controller::default(); 2])
        .expect("destination aerial continues");
    assert_eq!(
        state.fighters[0].action_state["entered"],
        skirmish::game::script::LocalValue::Integer(1)
    );
}
