#![cfg(feature = "experimental-continuations")]

//! Match-level coverage for the lifetime of a selected Pon move.
//!
//! These scenarios deliberately observe state and timer delivery through
//! `Match::step`; they do not assert the shape of the continuation records.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{
    Action, BUTTON_A, Controller, Match,
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, action, hook, register)

class Actionless(Move):
    async def run(self, action: MoveContext):
        action.fighter.state.started = action.fighter.state.started + 1
        await action.wait(1)
        action.fighter.state.resumed = action.fighter.state.resumed + 1
        await action.wait(1)
        action.fighter.state.finished = action.fighter.state.finished + 1

class Canonical(Move):
    phase = action("attack_air_b")
    next_phase = action("attack_air_hi")
    action = "attack_air_b"
    @hook.deadline(1, "attack_air_b")
    def phase_change(self, fighter, context):
        fighter.change_action(self.next_phase, preserve_state=True)
    async def run(self, action: MoveContext):
        action.fighter.state.started = action.fighter.state.started + 1
        await action.wait(1)
        action.fighter.hitlag = 3
        await action.wait(2)
        action.fighter.state.resumed = action.fighter.state.resumed + 1

class Long(Move):
    action = "attack_air_hi"
    async def run(self, action: MoveContext):
        action.fighter.state.started = action.fighter.state.started + 1
        await action.wait(20)
        action.fighter.state.resumed = action.fighter.state.resumed + 1

class State:
    started: int = 0
    resumed: int = 0
    finished: int = 0

actionless = Actionless(); canonical = Canonical(); long = Long(); ordinary = Move()
@register
class LifetimeFighter(Fighter):
    name = "move_lifetime_test"
    attributes = Attributes; state = State; action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, actionless, long, canonical)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
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

fn setup() -> Match {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("lifetime fixture compiles"));
    for flag in &mut data.fighters[0].aerials.as_mut().unwrap().moves[2].flags {
        flag.allow_interrupt = true;
    }
    for flag in &mut data.fighters[0].aerials.as_mut().unwrap().moves[4].flags {
        flag.allow_interrupt = true;
    }
    Match::new(data, 0).expect("aerial match resources load")
}

#[test]
fn actionless_run_starts_once_and_resumes_each_wait() {
    let mut game = setup();
    let entered = game.step(input()).expect("actionless move enters");
    // The synthetic aerial fixture starts airborne at y=100; an actionless
    // move keeps the native selection and therefore reaches Fall naturally.
    assert_eq!(entered.fighters[0].action, Action::Fall);
    assert_eq!(
        entered.fighters[0].script_state["started"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        entered.fighters[0].script_state["resumed"],
        LocalValue::Integer(0)
    );
    let first = game
        .step([Controller::default(); 2])
        .expect("first wait boundary");
    assert_eq!(
        first.fighters[0].script_state["started"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        first.fighters[0].script_state["resumed"],
        LocalValue::Integer(1)
    );
    let second = game
        .step([Controller::default(); 2])
        .expect("second wait boundary");
    assert_eq!(
        second.fighters[0].script_state["finished"],
        LocalValue::Integer(1)
    );
    assert!(second.fighters[0].script_events.pending_move.is_none());
}

#[test]
fn wait_boundaries_restore_byte_identically() {
    let mut game = setup();
    game.step(input()).unwrap();
    let first_checkpoint = game.checkpoint();
    let first = game.step([Controller::default(); 2]).unwrap().clone();
    let second_checkpoint = game.checkpoint();
    let complete = game.step([Controller::default(); 2]).unwrap().clone();

    game.restore_checkpoint(&first_checkpoint).unwrap();
    assert_eq!(
        serde_json::to_vec(game.step([Controller::default(); 2]).unwrap()).unwrap(),
        serde_json::to_vec(&first).unwrap()
    );
    game.restore_checkpoint(&second_checkpoint).unwrap();
    assert_eq!(
        serde_json::to_vec(game.step([Controller::default(); 2]).unwrap()).unwrap(),
        serde_json::to_vec(&complete).unwrap()
    );
}

#[test]
fn reselection_of_same_native_move_cancels_old_wait() {
    let mut game = setup();
    let first = game.step(input()).unwrap();
    assert_eq!(
        first.fighters[0].script_state["started"],
        LocalValue::Integer(1)
    );
    game.step([Controller::default(); 2]).unwrap();
    let second = game.step(input()).unwrap();
    assert_eq!(
        second.fighters[0].script_state["started"],
        LocalValue::Integer(2)
    );
    assert_eq!(
        second.fighters[0].script_state["resumed"],
        LocalValue::Integer(1)
    );
    let resumed = game.step([Controller::default(); 2]).unwrap();
    assert_eq!(
        resumed.fighters[0].script_state["resumed"],
        LocalValue::Integer(2)
    );
    assert_eq!(
        resumed.fighters[0].script_state["finished"],
        LocalValue::Integer(0)
    );
    let finished = game.step([Controller::default(); 2]).unwrap();
    assert_eq!(
        finished.fighters[0].script_state["finished"],
        LocalValue::Integer(1)
    );
}

#[test]
fn leaving_native_move_drops_pending_continuation_and_stale_delivery() {
    let mut game = setup();
    let long_input = [
        Controller {
            buttons: BUTTON_A,
            stick: [0.0, 1.0],
            ..Default::default()
        },
        Controller::default(),
    ];
    game.step(long_input).unwrap();
    let mut left = false;
    for _ in 0..12 {
        let state = game.step([Controller::default(); 2]).unwrap();
        if state.fighters[0].action == Action::Fall {
            left = true;
            break;
        }
    }
    assert!(left, "fixture aerial must leave its native action");
    let before = game.state().fighters[0].script_state.clone();
    for _ in 0..24 {
        game.step([Controller::default(); 2]).unwrap();
    }
    assert_eq!(game.state().fighters[0].script_state, before);
    assert!(
        game.state().fighters[0]
            .script_events
            .pending_move
            .is_none()
    );
    assert!(
        game.state().fighters[0]
            .script_events
            .pending_move_timer
            .is_none()
    );
}

#[test]
fn canonical_owner_and_lifetime_survive_hitlag_before_resume() {
    let mut game = setup();
    let canonical_input = [
        Controller {
            buttons: BUTTON_A,
            stick: [0.0, -1.0],
            ..Default::default()
        },
        Controller::default(),
    ];
    let original_generation = game.state().fighters[0]
        .script_events
        .action_generation
        .get();
    let entered = game.step(canonical_input).unwrap();
    let fighter = &entered.fighters[0];
    assert_eq!(fighter.action, Action::AttackAirB);
    assert!(fighter.script_events.action_generation.get() > original_generation);
    assert_eq!(fighter.script_state["started"], LocalValue::Integer(1));
    assert!(fighter.script_events.active_move.is_some());
    assert!(
        fighter
            .script_events
            .active_move
            .expect("canonical owner")
            .lifetime
            .get()
            > 0
    );
    let pending = fighter.script_events.pending_move.as_ref().expect("wait");
    let lifetime_before_transition = fighter
        .script_events
        .active_move
        .expect("canonical owner")
        .lifetime;
    let owner = pending.continuation.await_token.owner;
    let generation = pending.continuation.await_token.generation;
    assert_eq!(owner, 0);
    assert_eq!(generation, fighter.script_events.action_generation.get());
    let changed = game.step([Controller::default(); 2]).unwrap();
    assert_eq!(changed.fighters[0].action, Action::AttackAirHi);
    assert_eq!(
        changed.fighters[0].script_state["resumed"],
        LocalValue::Integer(0)
    );
    assert_eq!(changed.fighters[0].hitlag, 3.0);
    assert_eq!(changed.fighters[0].action_frame, 0);
    let lifetime_after_transition = changed.fighters[0]
        .script_events
        .active_move
        .expect("retained canonical owner")
        .lifetime;
    assert_eq!(lifetime_after_transition, lifetime_before_transition);
    // f2/f3/f4 consume hitlag 2/1/0 without advancing the native phase;
    // f5/f6 advance the retained attack-air phase, and f7 delivers wait(2).
    for (expected_hitlag, expected_action_frame) in
        [(2.0, 0), (1.0, 0), (0.0, 0), (0.0, 1), (0.0, 2)]
    {
        let state = game.step([Controller::default(); 2]).unwrap();
        assert_eq!(state.fighters[0].hitlag, expected_hitlag);
        assert_eq!(state.fighters[0].action_frame, expected_action_frame);
        assert_eq!(
            state.fighters[0].script_state["resumed"],
            LocalValue::Integer(0)
        );
    }
    let resumed = game.step([Controller::default(); 2]).unwrap();
    assert_eq!(
        resumed.fighters[0].script_state["resumed"],
        LocalValue::Integer(1)
    );
    assert_eq!(resumed.fighters[0].action_frame, 3);
}
