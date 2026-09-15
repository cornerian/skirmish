#![cfg(feature = "experimental-continuations")]

//! Callback driven specials retain the behavior selected by the input event.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_B, BUTTON_L, Controller, Match,
    script::{
        LocalValue, Program,
        resources::{Resources, Specials},
    },
};

const SOURCE: &str = r#"
from dataclasses import dataclass
from skirmish import (AerialMoves, Action, Attributes, Button, DefenseMoves, Fighter,
    GetupMoves, GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves,
    SpecialMove, SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, action, hook,
    register)

class First(SpecialMove):
    resource = "neutral"
    phase = action("special_n_start")
    loop = action("special_n_loop")

    @hook.input_pressed(Button.B)
    def press(self, fighter, ctx):
        if fighter.action in (self.phase, self.loop):
            fighter.change_action(self.loop, preserve_state=True)
            return True
        fighter.change_action(self.phase)
        return True

    @hook.input_pressed(Button.L, actions=(phase, loop))
    def cancel(self, fighter, ctx):
        fighter.change_action(Action.WAIT)
        return True

    @hook.enter(phase, loop)
    def entered(self, fighter, ctx):
        fighter.state.first_enters = fighter.state.first_enters + 1

class Second(SpecialMove):
    # This behavior deliberately shares First's canonical native phases.
    resource = "neutral"
    phase = action("special_n_start")
    loop = action("special_n_loop")

    @hook.input_pressed(Button.A, actions=(phase, loop))
    def press(self, fighter, ctx):
        fighter.change_action(self.phase, preserve_state=True)
        return True

    @hook.enter(phase, loop)
    def entered(self, fighter, ctx):
        fighter.state.second_enters = fighter.state.second_enters + 1

@dataclass(frozen=True)
class ExtendedSpecialMoves(SpecialMoves):
    first: SpecialMove
    second: SpecialMove

class State:
    first_enters: int = 0
    second_enters: int = 0

first = First(); second = Second(); ordinary = Move()
@register
class OwnerFighter(Fighter):
    name = "special_owner_test"
    attributes = Attributes; state = State; action_state = State
    specials = ExtendedSpecialMoves(first, ordinary, ordinary, ordinary, ordinary, second)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary); smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary); throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary); getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn input(buttons: u16) -> [Controller; 2] {
    [
        Controller {
            buttons,
            ..Default::default()
        },
        Controller::default(),
    ]
}

fn loaded() -> Match {
    let mut data = conformance::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("special owner fixture compiles"));
    let mut attack = data.fighters[0].jab.clone();
    attack.frames.truncate(1);
    let resources = Resources::new(std::collections::BTreeMap::from([(
        "neutral".to_owned(),
        serde_json::to_value(attack).expect("neutral resource serializes"),
    )]))
    .expect("neutral resource validates");
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials {
            character: "test_fighter".into(),
            resources: resources.clone(),
        });
    }
    Match::new(data, 0).expect("special owner fixture loads")
}

#[test]
fn selected_special_owner_survives_phases_cancel_and_checkpoint_replay() {
    let mut game = loaded();
    let checkpoint = game.checkpoint();

    let entered = game
        .step(input(BUTTON_B))
        .expect("initial special input")
        .clone();
    let fighter = &entered.fighters[0];
    assert_eq!(fighter.action, Action::SpecialNStart);
    assert_eq!(fighter.script_state["first_enters"], LocalValue::Integer(1));
    assert_eq!(
        fighter.script_state["second_enters"],
        LocalValue::Integer(0)
    );
    let owner = fighter
        .script_events
        .active_move
        .expect("initial callback owner");

    let checkpoint_after_enter = game.checkpoint();
    game.step(input(0)).expect("release initial special input");
    let switched = game
        .step(input(BUTTON_A))
        .expect("second owner reselects shared phase")
        .clone();
    assert_eq!(switched.fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        switched.fighters[0].script_state["second_enters"],
        LocalValue::Integer(1)
    );
    let switched_owner = switched.fighters[0]
        .script_events
        .active_move
        .expect("second callback owner");
    assert_ne!(switched_owner.behavior_index, owner.behavior_index);
    assert_ne!(switched_owner.lifetime, owner.lifetime);

    game.restore_checkpoint(&checkpoint_after_enter)
        .expect("restore before same owner loop");
    game.step(input(0)).expect("release before same owner loop");
    let looped = game
        .step(input(BUTTON_B))
        .expect("same owner loop transition")
        .clone();
    assert_eq!(looped.fighters[0].action, Action::SpecialNLoop);
    assert_eq!(
        looped.fighters[0].script_state["first_enters"],
        LocalValue::Integer(2)
    );
    assert_eq!(
        looped.fighters[0].script_state["second_enters"],
        LocalValue::Integer(0)
    );
    assert_eq!(
        looped.fighters[0]
            .script_events
            .active_move
            .expect("retained owner")
            .behavior_index,
        owner.behavior_index
    );

    let canceled = game.step(input(BUTTON_L)).expect("special cancel").clone();
    assert_eq!(canceled.fighters[0].action, Action::Wait);
    assert_eq!(canceled.fighters[0].script_events.active_move, None);

    game.restore_checkpoint(&checkpoint_after_enter)
        .expect("restore special checkpoint");
    game.step(input(0)).expect("release before replay loop");
    let replay = game
        .step(input(BUTTON_B))
        .expect("replay loop transition")
        .clone();
    assert_eq!(
        serde_json::to_vec(&replay).unwrap(),
        serde_json::to_vec(&looped).unwrap()
    );

    game.restore_checkpoint(&checkpoint)
        .expect("restore initial checkpoint");
    let replay_enter = game
        .step(input(BUTTON_B))
        .expect("replay initial special input")
        .clone();
    assert_eq!(
        serde_json::to_vec(&replay_enter).unwrap(),
        serde_json::to_vec(&entered).unwrap()
    );
}
