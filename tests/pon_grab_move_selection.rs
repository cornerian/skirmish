#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/grab.rs"]
mod grab;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, Controller, Match, script::LocalValue, script::Program,
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class CatchMove(Move):
    action = "catch"
    @hook.enter("catch")
    def entered(self, fighter, ctx):
        fighter.state.entered = fighter.state.entered + 1
class ThrowMove(Move):
    action = "throw_f"
    @hook.enter("throw_f")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1
class ActionState:
    entered: int = 0
class PersistentState:
    entered: int = 0
ordinary = Move(); catch_move = CatchMove(); throw_move = ThrowMove()
@register
class GrabTest(Fighter):
    name = "grab_test"
    attributes = Attributes
    action_state = ActionState
    state = PersistentState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(catch_move, ordinary, ordinary)
    throws = ThrowMoves(throw_move, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn fighter_input(buttons: u16, stick: [f32; 2]) -> [Controller; 2] {
    [
        Controller {
            buttons,
            stick,
            ..Default::default()
        },
        Controller::default(),
    ]
}

fn setup() -> Match {
    let mut data = grab::profile(conformance::data());
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    Match::new(data, 0).unwrap()
}

#[test]
fn registered_standing_grab_enters_native_action_and_owner_once() {
    let mut game = setup();
    let state = game
        .step(fighter_input(BUTTON_A | BUTTON_L, [0.0, 0.0]))
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::CatchPull);
    assert_eq!(
        state.fighters[0].script_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn registered_forward_throw_owns_real_capture_transition() {
    let mut game = setup();
    game.step(fighter_input(BUTTON_A | BUTTON_L, [0.0, 0.0]))
        .unwrap();
    let mut waiting = false;
    for _ in 0..20 {
        let state = game.step(fighter_input(0, [0.0, 0.0])).unwrap();
        if state.fighters[0].action == Action::CatchWait {
            waiting = true;
            break;
        }
    }
    assert!(waiting, "grab must reach CatchWait before throw input");
    let state = game.step(fighter_input(0, [1.0, 0.0])).unwrap();
    assert_eq!(state.fighters[0].action, Action::ThrowF);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}
