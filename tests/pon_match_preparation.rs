//! Match preparation is explicit when a loaded match crosses threads.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::{
    BUTTON_B, Controller, Match,
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class EntryMove(Move):
    @hook.press("B")
    def entered(self, fighter, context):
        fighter.state.counter = fighter.state.counter + 1
        return True
class State:
    counter: int = 0
ordinary = EntryMove()
@register
class NativeFighter(Fighter):
    name = "native_preparation"
    attributes = Attributes
    state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
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

fn loaded_match() -> Match {
    let mut data = conformance::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("preparation fixture compiles"));
    Match::new(data, 0).expect("preparation fixture loads")
}

#[test]
fn moved_match_requires_preparation_and_preserves_state_on_failure() {
    let game = loaded_match();
    let checkpoint_state = game.state().clone();
    let checkpoint_resource_id = game.resource_id();
    let worker = std::thread::spawn(move || {
        let mut game = game;
        let error = game
            .step([
                Controller {
                    buttons: BUTTON_B,
                    ..Default::default()
                },
                Controller::default(),
            ])
            .expect_err("unprepared gameplay thread must fail");
        assert!(
            error
                .to_string()
                .contains("not prepared for current thread")
        );
        assert_eq!(game.state(), &checkpoint_state);
        assert_eq!(game.resource_id(), checkpoint_resource_id);
        game.prepare_for_current_thread()
            .expect("prepare worker thread");
        game.step([
            Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect("prepared gameplay thread dispatches");
        assert_eq!(
            game.state().fighters[0].script_state["counter"],
            LocalValue::Integer(1)
        );
    });
    worker.join().expect("worker test should complete");
}

#[test]
fn cached_program_prepares_when_registered_on_another_thread() {
    let data = {
        let mut data = conformance::data();
        data.fighters[0].script = Some(Program::new(SOURCE).expect("fixture compiles"));
        data
    };
    let first = Match::new(data.clone(), 0).expect("first registration");
    drop(first);
    std::thread::spawn(move || Match::new(data, 1).expect("cached registration"))
        .join()
        .expect("registration worker should complete");
}
