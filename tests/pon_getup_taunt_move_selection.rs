//! Pon taunt registry selection keeps native side variants and lets a custom
//! canonical entry own the transition without applying native taunt setup.

#[path = "support/conformance.rs"]
mod conformance;

#[path = "game_damage_floor.rs"]
mod floor_fixture;

use skirmish::fighter::taunt::{Taunt, TauntAnimation, TauntFrame};
use skirmish::fighter::tilt::GroundFrameFlags;
use skirmish::game::{
    Action, BUTTON_DPAD_UP, Controller, Match,
    data::{BodyState, Bone},
    script::{LocalValue, Program},
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class TauntMove(Move):
    action = "appeal_s_r"
    @hook.enter("appeal_s_r", "appeal_s_l")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class GetupMove(Move):
    action = "down_stand"
    @hook.enter("down_stand")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class ActionState:
    entered: int = 0

ordinary = Move()
taunt = TauntMove()
getup = GetupMove()
@register
class TestFighter(Fighter):
    name = "taunt_test"
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
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(getup, ordinary, ordinary, ordinary)
    taunt = TauntMoves(taunt)
"#;

fn taunt_resource(bones: &[Bone]) -> Taunt {
    let animation = TauntAnimation {
        frames: vec![TauntFrame {
            bones: bones.to_vec(),
            body_state: BodyState::default(),
        }],
        flags: vec![GroundFrameFlags::default()],
        root_translations: None,
    };
    Taunt {
        right: animation.clone(),
        left: Some(animation),
    }
}

#[test]
fn left_facing_taunt_preserves_native_left_variant_and_owner_once() {
    let mut data = conformance::data();
    data.fighters[0].taunt = Some(taunt_resource(&data.fighters[0].bones));
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([
        Controller {
            stick: [-1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ])
    .unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_DPAD_UP,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::AppealSL);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

fn step_taunt(source: &str) -> skirmish::game::State {
    let mut data = conformance::data();
    data.fighters[0].taunt = Some(taunt_resource(&data.fighters[0].bones));
    data.fighters[0].script = Some(Program::new(source).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([
        Controller {
            buttons: BUTTON_DPAD_UP,
            ..Default::default()
        },
        Controller::default(),
    ])
    .unwrap()
    .clone()
}

#[test]
fn native_taunt_uses_registered_owner_once() {
    let state = step_taunt(SOURCE);
    assert_eq!(state.fighters[0].action, Action::AppealSR);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn custom_taunt_entry_is_authoritative() {
    let source = SOURCE
        .replace("action = \"appeal_s_r\"", "action = \"special_n_start\"")
        .replace(
            "@hook.enter(\"appeal_s_r\", \"appeal_s_l\")",
            "@hook.enter(\"special_n_start\")",
        );
    let state = step_taunt(&source);
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn down_wait_getup_native_and_custom_entries_are_authoritative() {
    let mut native = floor_fixture::pon_down_wait_with_script(SOURCE);
    native.step([Controller::default(); 2]).unwrap();
    let native_state = native
        .step([
            Controller::default(),
            Controller {
                buttons: skirmish::game::BUTTON_L,
                ..Default::default()
            },
        ])
        .unwrap();
    assert_eq!(native_state.fighters[1].action, Action::DownStand);

    let source = SOURCE
        .replace(
            "getup = GetupMoves(getup, ordinary, ordinary, ordinary)",
            "getup = GetupMoves(taunt, ordinary, ordinary, ordinary)",
        )
        .replace("action = \"appeal_s_r\"", "action = \"special_n_start\"")
        .replace(
            "@hook.enter(\"appeal_s_r\", \"appeal_s_l\")",
            "@hook.enter(\"special_n_start\")",
        );
    let mut custom = floor_fixture::pon_down_wait_with_script(&source);
    custom.step([Controller::default(); 2]).unwrap();
    let custom_state = custom
        .step([
            Controller::default(),
            Controller {
                buttons: skirmish::game::BUTTON_L,
                ..Default::default()
            },
        ])
        .unwrap();
    assert_eq!(custom_state.fighters[1].action, Action::SpecialNStart);
    assert_eq!(
        custom_state.fighters[1].action_state["entered"],
        LocalValue::Integer(1)
    );
}
