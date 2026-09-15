#![cfg(feature = "experimental-continuations")]

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{
    Action, BUTTON_A, Controller, Match,
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, register)

class Timed(Move):
    action = "attack_air_b"
    async def run(self, action: MoveContext):
        action.fighter.state.counter = action.fighter.state.counter + 100
        await action.wait(20)
        action.fighter.state.counter = action.fighter.state.counter + 1

class State:
    counter: int = 0

timed = Timed(); ordinary = Move()
@register
class AsyncFighter(Fighter):
    name = "async_cancellation_fighter"
    attributes = Attributes; state = State; action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, timed, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn attack_air_b() -> Controller {
    Controller {
        buttons: BUTTON_A,
        stick: [-1.0, 0.0],
        ..Default::default()
    }
}

fn setup() -> Match {
    let mut data = aerial::data();
    data.fighters[0].script =
        Some(Program::new(SOURCE).expect("async cancellation program compiles"));
    Match::new(data, 0).expect("aerial match resources load")
}

fn enter_async_move(game: &mut Match, expected_counter: i64) {
    let state = game
        .step([attack_air_b(), Controller::default()])
        .expect("native aerial entry succeeds");
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert_eq!(
        state.fighters[0].script_state["counter"],
        LocalValue::Integer(expected_counter)
    );
    assert!(state.fighters[0].script_events.pending_move.is_some());
}

#[test]
fn native_animation_exit_cancels_async_move_and_reentry_gets_new_generation() {
    let mut game = setup();
    enter_async_move(&mut game, 100);
    let first = &game.state().fighters[0].script_events;
    let first_generation = first.action_generation;
    let first_timer = first.pending_move_timer.expect("move has a deadline timer");

    let mut exited = false;
    for _ in 0..12 {
        game.step([Controller::default(); 2])
            .expect("native animation frame");
        if game.state().fighters[0].action == Action::Fall {
            exited = true;
            break;
        }
    }
    assert!(
        exited,
        "the native eight-frame aerial must exit before the await deadline"
    );
    let fighter = &game.state().fighters[0];
    assert_eq!(fighter.script_state["counter"], LocalValue::Integer(100));
    assert!(fighter.script_events.pending_move.is_none());
    assert!(fighter.script_events.pending_move_timer.is_none());
    assert!(
        !fighter
            .script_events
            .scheduler
            .timers()
            .any(|(id, _)| id == first_timer)
    );

    for _ in 0..20 {
        game.step([Controller::default(); 2])
            .expect("stale deadline drain frame");
        let fighter = &game.state().fighters[0];
        assert_eq!(fighter.script_state["counter"], LocalValue::Integer(100));
        assert!(fighter.script_events.pending_move.is_none());
    }

    enter_async_move(&mut game, 200);
    let fighter = &game.state().fighters[0];
    let second_generation = fighter.script_events.action_generation;
    let second_timer = fighter
        .script_events
        .pending_move_timer
        .expect("reentered move has a deadline timer");
    assert_ne!(second_generation, first_generation);
    assert_ne!(second_timer, first_timer);
    assert_eq!(fighter.script_state["counter"], LocalValue::Integer(200));

    for _ in 0..12 {
        game.step([Controller::default(); 2])
            .expect("second native animation frame");
        if game.state().fighters[0].action == Action::Fall {
            break;
        }
    }
    let fighter = &game.state().fighters[0];
    assert_eq!(fighter.action, Action::Fall);
    assert_eq!(fighter.script_state["counter"], LocalValue::Integer(200));
    assert!(fighter.script_events.pending_move.is_none());
    assert!(fighter.script_events.pending_move_timer.is_none());
}

#[test]
fn cancelled_async_move_checkpoint_replays_byte_identically() {
    let mut game = setup();
    enter_async_move(&mut game, 100);
    let checkpoint = game.checkpoint();
    for _ in 0..12 {
        game.step([Controller::default(); 2])
            .expect("native animation frame");
        if game.state().fighters[0].action == Action::Fall {
            break;
        }
    }
    let expected = serde_json::to_vec(game.state()).expect("serialize cancelled state");
    game.restore_checkpoint(&checkpoint)
        .expect("restore midwait checkpoint");
    for _ in 0..12 {
        game.step([Controller::default(); 2])
            .expect("replayed native animation frame");
        if game.state().fighters[0].action == Action::Fall {
            break;
        }
    }
    assert_eq!(
        serde_json::to_vec(game.state()).expect("serialize replayed state"),
        expected
    );
}
