//! Ground input resolves class move ownership while retaining native variants.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/tilt.rs"]
mod tilt;

use skirmish::game::{Action, BUTTON_A, Controller, Match, script::LocalValue, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, hook, register)

class Forward(Move):
    action = "attack_s3_s"
    @hook.enter("attack_s3_s", "attack_s3_hi")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class Custom(Move):
    action = "special_n_start"
    @hook.enter("special_n_start")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class ActionState:
    entered: int = 0

ordinary = Move()
forward = Forward()
custom = Custom()

@register
class TestFighter(Fighter):
    name = "ground_test"
    attributes = Attributes
    action_state = ActionState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(forward, ordinary, ordinary)
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
            stick: [0.7, 0.3],
            ..Default::default()
        },
        Controller::default(),
    ]
}

#[test]
fn registered_forward_tilt_preserves_native_angled_action_and_routes_owner() {
    let mut data = tilt::profile(conformance::data());
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    let state = game.step(input()).unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackS3Hi);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn custom_forward_tilt_entry_action_is_authoritative() {
    let mut data = tilt::profile(conformance::data());
    let source = SOURCE
        .replace("action = \"attack_s3_s\"", "action = \"special_n_start\"")
        .replace(
            "@hook.enter(\"attack_s3_s\", \"attack_s3_hi\")",
            "@hook.enter(\"special_n_start\")",
        );
    data.fighters[0].script = Some(Program::new(&source).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    let state = game.step(input()).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}
