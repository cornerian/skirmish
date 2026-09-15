//! Gameplay callbacks must not retain durable state outside the checkpoint.
//!
//! The generic Pon runtime intentionally permits mutable module globals for
//! ordinary `Program` users.  A gameplay `Match` needs a narrower boundary:
//! state that affects a later callback must belong to the declared fighter
//! state (and therefore be included in a checkpoint), rather than a module
//! list, dict, or class attribute.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{BUTTON_A, Controller, Match, script::LocalValue, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, Button, DefenseMoves, Fighter,
    GetupMoves, GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, hook, register)

# These are deliberately outside Fighter.state.  A gameplay checkpoint must
# not allow any of them to influence the callback after restore.
module_list = []
module_dict = {"count": 0}
class ModuleState:
    count = 0

class State:
    list_count: int = 0
    dict_count: int = 0
    class_count: int = 0

class InputMove(Move):
    @hook.input_pressed(Button.A)
    def pressed(self, fighter, context):
        module_list.append(1)
        module_dict["count"] = module_dict["count"] + 1
        ModuleState.count = ModuleState.count + 1
        fighter.state.list_count = len(module_list)
        fighter.state.dict_count = module_dict["count"]
        fighter.state.class_count = ModuleState.count

binding = InputMove()
ordinary = Move()
@register
class ModuleStateFighter(Fighter):
    name = "module_state_test"
    attributes = Attributes
    state = State
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

fn press_a() -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
        Controller::default(),
    ]
}

fn release() -> [Controller; 2] {
    [Controller::default(); 2]
}

fn setup() -> Match {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("module-state fixture compiles"));
    Match::new(data, 0).expect("module-state match resources load")
}

fn counts(game: &Match) -> [LocalValue; 3] {
    let state = &game.state().fighters[0].script_state;
    [
        state["list_count"].clone(),
        state["dict_count"].clone(),
        state["class_count"].clone(),
    ]
}

#[test]
fn module_heap_mutation_cannot_change_a_post_restore_input_branch() {
    let mut game = setup();
    game.step(press_a()).expect("first input callback succeeds");
    assert_eq!(
        counts(&game),
        [
            LocalValue::Integer(1),
            LocalValue::Integer(1),
            LocalValue::Integer(1),
        ]
    );
    let checkpoint = game.checkpoint();

    game.step(release()).expect("release edge succeeds");
    let expected = game
        .step(press_a())
        .expect("second input callback succeeds");
    let expected_state = serde_json::to_vec(expected).expect("serialize expected branch");
    assert_eq!(
        counts(&game),
        [
            LocalValue::Integer(2),
            LocalValue::Integer(2),
            LocalValue::Integer(2),
        ]
    );

    game.restore_checkpoint(&checkpoint)
        .expect("restore before repeated input branch");
    game.step(release())
        .expect("replayed release edge succeeds");
    let replay = game
        .step(press_a())
        .expect("replayed input callback succeeds");
    assert_eq!(
        serde_json::to_vec(replay).expect("serialize replay branch"),
        expected_state,
        "module list/dict/class mutations must be checkpointed or rejected"
    );
}
