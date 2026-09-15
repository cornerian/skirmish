#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/escape.rs"]
mod escape;

use skirmish::game::{Action, BUTTON_L, Controller, Match, script::LocalValue, script::Program};

fn shield_profile(mut data: skirmish::game::data::MatchData) -> skirmish::game::data::MatchData {
    #[derive(serde::Deserialize)]
    struct Fixture {
        rules: skirmish::fighter::shield::Rules,
        attributes: skirmish::fighter::shield::Attributes,
    }
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    let mut rules = fixture.rules;
    rules.powershield_input_window = 3;
    rules.powershield_reflect_frames = 1.0;
    rules.powershield_frames = 1.0;
    data.rules.shield = Some(rules);
    for fighter in &mut data.fighters {
        fighter.shield = Some(fixture.attributes.clone());
    }
    data
}

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class Roll(Move):
    action = "escape_f"
    @hook.enter("escape_f")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class Spot(Move):
    action = "special_n_start"
    @hook.enter("special_n_start")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class Shield(Move):
    action = "guard_on"
    @hook.enter("guard_on", "guard_reflect")
    def entered(self, fighter, ctx):
        fighter.action_state.entered = fighter.action_state.entered + 1

class ActionState:
    entered: int = 0

ordinary = Move()
roll = Roll()
spot = Spot()
shield = Shield()
@register
class DefenseTest(Fighter):
    name = "defense_test"
    attributes = Attributes
    action_state = ActionState
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(shield, spot, roll, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

#[test]
fn registered_roll_preserves_native_action_and_owner() {
    let mut data = escape::profile(conformance::data());
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    game.step([
        Controller {
            buttons: BUTTON_L,
            stick: [1.0, 0.0],
            ..Default::default()
        },
        Controller::default(),
    ])
    .unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_L,
                stick: [1.0, 0.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::EscapeF);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn custom_spot_dodge_action_is_authoritative() {
    let mut data = escape::profile(conformance::data());
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_L,
                stick: [0.0, -1.0],
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn registered_shield_uses_native_powershield_variant_once() {
    let mut data = shield_profile(conformance::data());
    data.fighters[0].script = Some(Program::new(SOURCE).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_L,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::GuardReflect);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}

#[test]
fn custom_shield_action_skips_native_guard_setup() {
    let mut data = shield_profile(conformance::data());
    let source = SOURCE
        .replace("action = \"guard_on\"", "action = \"special_n_start\"")
        .replace(
            "@hook.enter(\"guard_on\", \"guard_reflect\")",
            "@hook.enter(\"special_n_start\")",
        );
    data.fighters[0].script = Some(Program::new(&source).unwrap());
    let mut game = Match::new(data, 0).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_L,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    assert_eq!(
        state.fighters[0].action_state["entered"],
        LocalValue::Integer(1)
    );
}
