#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/ledge.rs"]
mod ledge;

use skirmish::game::{Action, Controller, Match, script::LocalValue, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class WaitMove(Move):
    action = "cliff_wait"
    @hook.enter("cliff_wait")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class CustomWait(Move):
    action = "special_n_start"
    @hook.enter("special_n_start")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class ActionState:
    entered: int = 0

wait_move = WaitMove(); custom_wait = CustomWait(); ordinary = Move()
@register
class LedgeTest(Fighter):
    name = "ledge_test"
    attributes = Attributes
    action_state = ActionState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(wait_move, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn setup() -> (skirmish::game::data::MatchData, Match) {
    let mut data = ledge::profile(conformance::data());
    data.stage.floor.left = -2.0;
    data.stage.floor.right = 2.0;
    data.stage.spawns[0] = [-1.9, 1.0];
    data.stage.spawns[1] = [0.0, 0.0];
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let game = Match::new(data.clone(), 0).unwrap();
    (data, game)
}

fn until_wait(game: &mut Match) {
    for _ in 0..30 {
        if game.state().fighters[0].action == Action::CliffWait {
            return;
        }
        game.step([Controller::default(); 2]).unwrap();
    }
    panic!("fighter did not reach ledge wait");
}

#[test]
fn native_ledge_wait_uses_registered_owner_once() {
    let (_, mut game) = setup();
    game.step([
        Controller {
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ])
    .unwrap();
    until_wait(&mut game);
    assert_eq!(game.state().fighters[0].action, Action::CliffWait);
    assert_eq!(
        game.state().fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn custom_ledge_wait_selects_destination_without_native_setup() {
    let (mut data, _) = setup();
    let source = SOURCE.replace(
        "ledge = LedgeMoves(wait_move, ordinary, ordinary, ordinary, ordinary)",
        "ledge = LedgeMoves(custom_wait, ordinary, ordinary, ordinary, ordinary)",
    );
    data.fighters[0].script = Some(Program::new(&source).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([
        Controller {
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ])
    .unwrap();
    for _ in 0..30 {
        if game.state().fighters[0].action == Action::SpecialNStart {
            break;
        }
        game.step([Controller::default(); 2]).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        game.state().fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}
