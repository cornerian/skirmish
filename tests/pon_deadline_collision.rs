#![cfg(feature = "experimental-continuations")]

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{
    BUTTON_A, Controller, Match,
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, hook, register)

class Timed(Move):
    action = "attack_air_b"
    async def run(self, action: MoveContext):
        action.fighter.action_state.counter = action.fighter.action_state.counter + 1
        await action.wait(2)
        action.fighter.action_state.counter = action.fighter.action_state.counter + 1

    @hook.deadline(4, "attack_air_b")
    def a_authored_deadline(self, fighter, context):
        fighter.action_state.deadline_hits = fighter.action_state.deadline_hits + 1

    @hook.deadline(1, "attack_air_b")
    def z_unrelated_deadline(self, fighter, context):
        fighter.action_state.early_hits = fighter.action_state.early_hits + 1

class Ordinary(Move):
    pass

class State:
    counter: int = 0
    deadline_hits: int = 0
    early_hits: int = 0

ordinary = Ordinary(); timed = Timed()
@register
class AsyncFighter(Fighter):
    name = "deadline_collision_fighter"
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

fn attack_air_b() -> Controller {
    Controller {
        buttons: BUTTON_A,
        stick: [-1.0, 0.0],
        ..Default::default()
    }
}

const CANCELLATION_SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, hook, register)

class Timed(Move):
    action = "attack_air_b"
    @hook.enter("attack_air_b")
    def entered(self, fighter, context):
        fighter.state.entered = fighter.state.entered + 1

    async def run(self, action: MoveContext):
        action.fighter.state.counter = action.fighter.state.counter + 1
        await action.wait(2)
        action.fighter.state.counter = action.fighter.state.counter + 1

    @hook.deadline(2, "attack_air_b")
    def b_cancel_deadline(self, fighter, context):
        fighter.state.pre_cancel = fighter.state.pre_cancel + 1
        fighter.state.cancel_hits = fighter.state.cancel_hits + 1
        fighter.change_action("fall")
        fighter.state.post_cancel = fighter.state.post_cancel + 1

    @hook.deadline(4, "attack_air_b")
    def a_late_deadline(self, fighter, context):
        fighter.action_state.deadline_hits = fighter.action_state.deadline_hits + 1

class Ordinary(Move):
    pass

class PersistentState:
    counter: int = 0
    entered: int = 0
    cancel_hits: int = 0
    pre_cancel: int = 0
    post_cancel: int = 0

class ActionState:
    deadline_hits: int = 0

ordinary = Ordinary(); timed = Timed()
@register
class AsyncFighter(Fighter):
    name = "deadline_cancellation_fighter"
    attributes = Attributes; state = PersistentState; action_state = ActionState
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
fn internal_wait_timer_collision_keeps_authored_callback_independent() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("collision program compiles"));
    let mut game = Match::new(data, 0).expect("aerial match resources load");

    let state = game
        .step([attack_air_b(), Controller::default()])
        .expect("enter aerial");
    let fighter = &state.fighters[0];
    assert_eq!(fighter.action_state["counter"], LocalValue::Integer(1));
    assert_eq!(
        fighter.action_state["deadline_hits"],
        LocalValue::Integer(0)
    );
    assert_eq!(fighter.action_state["early_hits"], LocalValue::Integer(0));
    let pending_timer = fighter
        .script_events
        .pending_move_timer
        .expect("async timer");
    let timers: Vec<_> = fighter.script_events.scheduler.timers().collect();
    let internal = timers
        .iter()
        .find(|(id, _)| *id == pending_timer)
        .expect("internal timer record");
    let authored = timers
        .iter()
        .find(|(_, timer)| timer.deadline() == 4)
        .expect("authored deadline record");
    assert_eq!(
        internal.1.token(),
        authored.1.token(),
        "test fixture must exercise numeric token collision"
    );

    let at_wait = game.step([Controller::default(); 2]).expect("wait frame");
    assert_eq!(
        at_wait.fighters[0].action_state["counter"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        at_wait.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(0)
    );
    assert_eq!(
        at_wait.fighters[0].action_state["early_hits"],
        LocalValue::Integer(1)
    );

    let resumed = game.step([Controller::default(); 2]).expect("resume frame");
    assert_eq!(
        resumed.fighters[0].action_state["counter"],
        LocalValue::Integer(2)
    );
    assert_eq!(
        resumed.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(0)
    );
    assert_eq!(
        resumed.fighters[0].action_state["early_hits"],
        LocalValue::Integer(1)
    );
    assert!(resumed.fighters[0].script_events.pending_move.is_none());

    let before_authored = game
        .step([Controller::default(); 2])
        .expect("pre-authored deadline frame");
    assert_eq!(
        before_authored.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(0)
    );
    assert_eq!(
        before_authored.fighters[0].action_state["early_hits"],
        LocalValue::Integer(1)
    );
    let authored_frame = game
        .step([Controller::default(); 2])
        .expect("authored deadline frame");
    assert_eq!(
        authored_frame.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        authored_frame.fighters[0].action_state["early_hits"],
        LocalValue::Integer(1)
    );
    let after_authored = game
        .step([Controller::default(); 2])
        .expect("post-authored deadline frame");
    assert_eq!(
        after_authored.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        after_authored.fighters[0].action_state["early_hits"],
        LocalValue::Integer(1)
    );
}

#[test]
fn same_frame_authored_deadline_cancels_async_move_and_replays() {
    let mut data = aerial::data();
    data.fighters[0].script =
        Some(Program::new(CANCELLATION_SOURCE).expect("cancellation program compiles"));
    let mut game = Match::new(data, 0).expect("aerial match resources load");
    let entered = game
        .step([attack_air_b(), Controller::default()])
        .expect("enter aerial");
    assert_eq!(
        entered.fighters[0].script_state["counter"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        entered.fighters[0].script_state["entered"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        entered.fighters[0].script_state["cancel_hits"],
        LocalValue::Integer(0)
    );
    let pending_timer = entered.fighters[0]
        .script_events
        .pending_move_timer
        .expect("async timer");
    let timers: Vec<_> = entered.fighters[0]
        .script_events
        .scheduler
        .timers()
        .collect();
    let internal = timers
        .iter()
        .find(|(id, _)| *id == pending_timer)
        .expect("internal timer");
    let authored = timers
        .iter()
        .find(|(_, timer)| timer.deadline() == 2)
        .expect("same-frame authored timer");
    assert_ne!(
        internal.0, authored.0,
        "same-frame records must remain distinct timers"
    );
    assert!(
        authored.0 < internal.0,
        "authored timer must be delivered first in the same frame"
    );

    let checkpoint = game.checkpoint();
    let before_cancel = game
        .step([Controller::default(); 2])
        .expect("pre-cancellation frame");
    assert_eq!(
        before_cancel.fighters[0].action,
        skirmish::game::Action::AttackAirB
    );
    assert_eq!(
        before_cancel.fighters[0].script_state["counter"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        before_cancel.fighters[0].script_state["cancel_hits"],
        LocalValue::Integer(0)
    );
    assert_eq!(
        before_cancel.fighters[0].script_state["pre_cancel"],
        LocalValue::Integer(0)
    );
    let cancelled = game
        .step([Controller::default(); 2])
        .expect("authored cancellation frame");
    assert_eq!(cancelled.fighters[0].action, skirmish::game::Action::Fall);
    assert_eq!(
        cancelled.fighters[0].script_state["counter"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        cancelled.fighters[0].script_state["cancel_hits"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        cancelled.fighters[0].script_state["entered"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        cancelled.fighters[0].script_state["pre_cancel"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        cancelled.fighters[0].script_state["post_cancel"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        cancelled.fighters[0].action_state["deadline_hits"],
        LocalValue::Integer(0)
    );
    assert!(cancelled.fighters[0].script_events.pending_move.is_none());
    assert!(
        cancelled.fighters[0]
            .script_events
            .pending_move_timer
            .is_none()
    );
    let after_cancel = game
        .step([Controller::default(); 2])
        .expect("post-cancellation stale-event frame");
    assert_eq!(
        after_cancel.fighters[0].action,
        skirmish::game::Action::Fall
    );
    assert_eq!(
        after_cancel.fighters[0].script_state["counter"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        after_cancel.fighters[0].script_state["cancel_hits"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        after_cancel.fighters[0].script_state["pre_cancel"],
        LocalValue::Integer(1)
    );
    assert_eq!(
        after_cancel.fighters[0].script_state["post_cancel"],
        LocalValue::Integer(1)
    );
    assert!(
        after_cancel.fighters[0]
            .script_events
            .pending_move
            .is_none()
    );
    assert!(
        after_cancel.fighters[0]
            .script_events
            .pending_move_timer
            .is_none()
    );
    let expected = serde_json::to_vec(after_cancel).expect("serialize post-cancellation state");

    game.restore_checkpoint(&checkpoint)
        .expect("restore before cancellation");
    game.step([Controller::default(); 2])
        .expect("replay pre-cancellation frame");
    game.step([Controller::default(); 2])
        .expect("replay cancellation frame");
    let replay_after_cancel = game
        .step([Controller::default(); 2])
        .expect("replay post-cancellation stale-event frame");
    assert_eq!(
        expected,
        serde_json::to_vec(replay_after_cancel).expect("serialize replayed state")
    );
}
