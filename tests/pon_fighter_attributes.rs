//! Class-defined native attributes are applied at match registration.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::{BUTTON_X, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, register)
from dataclasses import dataclass, field
@dataclass(frozen=True, slots=True)
class NativeAttributes(Attributes):
    weight: float = 123
    movement: dict = field(default_factory=lambda: {"gravity": 0.25, "jump_vertical_velocity": 12.0})
@register
class NativeFighter(Fighter):
    name = "native_attributes"
    attributes = NativeAttributes
    ordinary = Move()
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

#[test]
fn typed_attributes_override_native_values_and_change_jump_motion() {
    let mut overridden = conformance::data();
    let original = overridden.fighters[0].movement.clone();
    overridden.fighters[0].script = Some(Program::new(SOURCE).expect("program compiles"));
    let mut game = Match::new(overridden, 0).expect("attributes are valid");
    assert_eq!(game.data().fighters[0].weight, 123.0);
    assert_eq!(game.data().fighters[0].movement.gravity, 0.25);
    assert_eq!(
        game.data().fighters[0].movement.jump_vertical_velocity,
        12.0
    );
    assert_eq!(
        game.data().fighters[0].movement.air_max_horizontal_velocity,
        original.air_max_horizontal_velocity
    );

    let baseline_source = SOURCE
        .replace("123", "100")
        .replace("0.25", "0.2")
        .replace("12.0", "2.4");
    let mut baseline_data = conformance::data();
    baseline_data.fighters[0].script =
        Some(Program::new(baseline_source).expect("program compiles"));
    let mut baseline = Match::new(baseline_data, 0).expect("baseline is valid");
    let input = [
        Controller {
            buttons: BUTTON_X,
            ..Default::default()
        },
        Controller::default(),
    ];
    for _ in 0..3 {
        baseline.step(input).expect("baseline input");
        game.step(input).expect("overridden input");
    }
    assert_ne!(
        baseline.state().fighters[0].position,
        game.state().fighters[0].position
    );
    assert_ne!(
        baseline.state().fighters[0].velocity,
        game.state().fighters[0].velocity
    );
}

#[test]
fn invalid_typed_attributes_reject_match_before_construction() {
    let mut data = conformance::data();
    data.fighters[0].script = Some(
        Program::new(SOURCE.replace("\"gravity\": 0.25", "\"unknown\": 1.0"))
            .expect("program compiles"),
    );
    let error = Match::new(data, 0).expect_err("unknown movement key must reject");
    assert!(error.to_string().contains("unknown movement attribute"));
}
