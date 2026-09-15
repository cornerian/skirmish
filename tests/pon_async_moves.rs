#![cfg(feature = "experimental-continuations")]

#[path = "support/aerial.rs"]
mod aerial;
use skirmish::game::{BUTTON_A, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, register)

class Timed(Move):
    action = "attack_air_b"
    async def run(self, action: MoveContext):
        action.fighter.action_state.counter = action.fighter.action_state.counter + 1
        await action.wait(2)
        action.fighter.action_state.counter = action.fighter.action_state.counter + 1

class Ordinary(Move):
    pass

class State:
    counter: int = 0

ordinary = Ordinary(); timed = Timed()
@register
class AsyncFighter(Fighter):
    name = "async_fighter"
    attributes = Attributes; action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, timed, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

const MULTI_SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, register)

class Timed(Move):
    action = "attack_air_b"
    async def run(self, action: MoveContext):
        value = 10
        action.fighter.action_state.counter = action.fighter.action_state.counter + 1
        await action.wait(1)
        action.fighter.action_state.counter = action.fighter.action_state.counter + value
        value = value + 5
        await action.wait(1)
        action.fighter.action_state.counter = action.fighter.action_state.counter + value

class Ordinary(Move):
    pass

class State:
    counter: int = 0

ordinary = Ordinary(); timed = Timed()
@register
class AsyncFighter(Fighter):
    name = "async_fighter_multi"
    attributes = Attributes; action_state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, timed, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

#[test]
fn full_fighter_exports_qualified_async_move_run() {
    let program = Program::new(SOURCE).expect("full Fighter must compile");
    let callbacks = program
        .compiled()
        .expect("Pon program is linked")
        .callback_keys()
        .expect("callback table is exported");
    assert!(callbacks.iter().any(|key| key == "move_1.run"));
}

#[test]
fn match_enters_async_move_and_stages_pre_await_state() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).expect("match resources load");
    for _ in 0..5 {
        game.step([Controller::default(), Controller::default()])
            .expect("neutral setup frame");
    }
    let state = game
        .step([
            Controller {
                buttons: BUTTON_A,
                stick: [-1.0, 0.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect("first async move frame");
    assert_eq!(
        state.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    assert!(state.fighters[0].script_events.pending_move.is_some());
    let checkpoint = game.checkpoint();
    let before = game
        .step([Controller::default(), Controller::default()])
        .unwrap();
    assert_eq!(
        before.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    let after_bytes = {
        let after = game
            .step([Controller::default(), Controller::default()])
            .unwrap();
        assert_eq!(
            after.fighters[0].action_state["counter"],
            skirmish::game::script::LocalValue::Integer(2)
        );
        assert!(after.fighters[0].script_events.pending_move.is_none());
        serde_json::to_vec(after).expect("serialize post-resume state")
    };
    game.restore_checkpoint(&checkpoint).unwrap();
    let replay_before = game
        .step([Controller::default(), Controller::default()])
        .unwrap();
    assert_eq!(
        replay_before.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    let replay_after_bytes = {
        let replay_after = game
            .step([Controller::default(), Controller::default()])
            .unwrap();
        assert_eq!(
            replay_after.fighters[0].action_state["counter"],
            skirmish::game::script::LocalValue::Integer(2)
        );
        serde_json::to_vec(replay_after).expect("serialize replayed post-resume state")
    };
    assert_eq!(after_bytes, replay_after_bytes);
}

#[test]
fn match_runs_multiple_waits_and_replays_both_checkpoints_byte_identically() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(MULTI_SOURCE).unwrap());
    let mut game = Match::new(data, 0).expect("match resources load");
    for _ in 0..5 {
        game.step([Controller::default(), Controller::default()])
            .expect("neutral setup frame");
    }
    let input = [
        Controller {
            buttons: BUTTON_A,
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ];
    let first = game.step(input).expect("first async move frame").clone();
    assert_eq!(
        first.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(1)
    );
    let first_checkpoint = game.checkpoint();
    assert!(first.fighters[0].script_events.pending_move_timer.is_some());
    assert!(
        first.fighters[0]
            .script_events
            .pending_move
            .as_ref()
            .is_some_and(|pending| pending.continuation.await_token.deadline_frame
                >= u64::from(first.next_frame))
    );

    let delivery_frames = 1;
    let middle = game
        .step([Controller::default(), Controller::default()])
        .expect("first resume")
        .clone();
    assert_eq!(
        middle.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(11)
    );
    let second_checkpoint = game.checkpoint();
    assert!(
        middle.fighters[0]
            .script_events
            .pending_move_timer
            .is_some()
    );
    assert!(
        middle.fighters[0]
            .script_events
            .pending_move
            .as_ref()
            .is_some_and(|pending| pending.continuation.await_token.deadline_frame
                >= u64::from(middle.next_frame))
    );
    let complete = game
        .step([Controller::default(), Controller::default()])
        .expect("second resume")
        .clone();
    let complete_bytes = serde_json::to_vec(&complete).unwrap();
    assert_eq!(
        game.state().fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(26)
    );

    game.restore_checkpoint(&first_checkpoint).unwrap();
    for _ in 0..delivery_frames {
        game.step([Controller::default(), Controller::default()])
            .expect("replay deadline delivery frame");
    }
    assert_eq!(
        serde_json::to_vec(game.state()).unwrap(),
        serde_json::to_vec(&middle).unwrap(),
    );
    let replay_complete_from_first = serde_json::to_vec(
        game.step([Controller::default(), Controller::default()])
            .expect("replay second resume from first checkpoint"),
    )
    .unwrap();
    assert_eq!(replay_complete_from_first, complete_bytes);

    game.restore_checkpoint(&second_checkpoint).unwrap();
    let replay_complete_from_second = serde_json::to_vec(
        game.step([Controller::default(), Controller::default()])
            .expect("replay second resume from second checkpoint"),
    )
    .unwrap();
    assert_eq!(replay_complete_from_second, complete_bytes);
}

#[test]
fn immediate_async_run_commits_effect_once_without_pending_timer() {
    let immediate_source = SOURCE.replace("        await action.wait(2)\n", "");
    assert!(!immediate_source.contains("await action.wait"));
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(immediate_source).unwrap());
    let mut game = Match::new(data, 0).expect("match resources load");
    for _ in 0..5 {
        game.step([Controller::default(), Controller::default()])
            .unwrap();
    }
    let checkpoint = game.checkpoint();
    let input = [
        Controller {
            buttons: BUTTON_A,
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ];
    let state = game.step(input).expect("immediate async move frame");
    assert_eq!(
        state.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(2)
    );
    assert!(state.fighters[0].script_events.pending_move.is_none());
    assert!(state.fighters[0].script_events.pending_move_timer.is_none());
    let state_bytes = serde_json::to_vec(state).expect("serialize immediate state");

    game.restore_checkpoint(&checkpoint).unwrap();
    let replay = game
        .step(input)
        .expect("replayed immediate async move frame");
    assert_eq!(serde_json::to_vec(replay).unwrap(), state_bytes);
    let next_neutral = game
        .step([Controller::default(), Controller::default()])
        .expect("neutral frame after immediate move");
    assert_eq!(
        next_neutral.fighters[0].action_state["counter"],
        skirmish::game::script::LocalValue::Integer(2)
    );
    assert!(
        next_neutral.fighters[0]
            .script_events
            .pending_move
            .is_none()
    );
    assert!(
        next_neutral.fighters[0]
            .script_events
            .pending_move_timer
            .is_none()
    );
}

#[test]
fn failed_immediate_async_run_rolls_back_host_effects() {
    let failing_source = SOURCE.replace(
        "        await action.wait(2)\n",
        "        action.fighter.action_state.counter = 1 / 0\n",
    );
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(failing_source).unwrap());
    let mut game = Match::new(data, 0).expect("match resources load");
    for _ in 0..5 {
        game.step([Controller::default(), Controller::default()])
            .expect("neutral setup frame");
    }
    let before = game.state().clone();
    let error = game
        .step([
            Controller {
                buttons: BUTTON_A,
                stick: [-1.0, 0.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect_err("failed async move must abort its host transaction");
    assert!(error.to_string().contains("division") || error.to_string().contains("zero"));
    assert_eq!(game.state(), &before);
}
