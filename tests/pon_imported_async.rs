#![cfg(feature = "experimental-continuations")]

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{
    BUTTON_A, Controller, Match,
    script::LocalValue,
    script::definition::{AssetStore, Definition},
};

const ROOT: &str = r#"
from helper import ImportedMove
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, register)

class Ordinary(Move):
    pass

class State:
    counter: int = 0

ordinary = Ordinary(); imported = ImportedMove()
@register
class ImportedFighter(Fighter):
    name = "imported_async_fighter"
    attributes = Attributes
    action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, imported, ordinary, ordinary)
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

const HELPER: &str = r#"
from skirmish import Move, MoveContext

MODULE_INCREMENT = 9

class BaseMove(Move):
    action = "attack_air_b"
    async def run(self, action: MoveContext):
        action.fighter.action_state.counter = action.fighter.action_state.counter + MODULE_INCREMENT
        await action.wait(1)
        action.fighter.action_state.counter = action.fighter.action_state.counter + MODULE_INCREMENT

class ImportedMove(BaseMove):
    pass
"#;

fn setup() -> Match {
    let mut data = aerial::data();
    let mut assets = AssetStore::builtins();
    assets.register("helper.py", HELPER);
    let definition = Definition::load_registered(ROOT, &assets).expect("imported fighter loads");
    data.fighters[0].script = Some((*definition.program).clone());
    Match::new(data, 0).expect("imported async match loads")
}

fn attack_air_b() -> Controller {
    Controller {
        buttons: BUTTON_A,
        stick: [-1.0, 0.0],
        ..Default::default()
    }
}

#[test]
fn imported_inherited_run_uses_helper_globals_and_replays_checkpoint() {
    let mut game = setup();
    let first = game
        .step([attack_air_b(), Controller::default()])
        .expect("enter imported async move")
        .clone();
    assert_eq!(
        first.fighters[0].action_state["counter"],
        LocalValue::Integer(9)
    );
    let checkpoint = game.checkpoint();
    let completed = game
        .step([Controller::default(); 2])
        .expect("resume imported async move")
        .clone();
    assert_eq!(
        completed.fighters[0].action_state["counter"],
        LocalValue::Integer(18)
    );
    let expected = serde_json::to_vec(&completed).expect("serialize completion");
    game.restore_checkpoint(&checkpoint)
        .expect("restore imported checkpoint");
    let replayed = game
        .step([Controller::default(); 2])
        .expect("replay imported async move");
    assert_eq!(
        serde_json::to_vec(replayed).expect("serialize replay"),
        expected
    );
}
