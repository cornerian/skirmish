//! Integration coverage for the class based Pon fighter boundary.

use serde::Deserialize;
use skirmish::game::script::{Error, FighterView, HitView, Hook, Program};

const PON_FIGHTER_SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves, GrabMoves,
    GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves, TauntMoves,
    ThrowMoves, TiltMoves, hook, register)
class Ordinary(Move): pass
class FoxAttributes(Attributes):
    weight = 100
class DamageMove(Move):
    @hook.before_hit
    def before_hit(self, fighter, hit):
        hit.damage = hit.damage + 7.0
ordinary = Ordinary()
damage = DamageMove()
@register
class Fox(Fighter):
    name = "fox"
    attributes = FoxAttributes
    specials = SpecialMoves(damage, ordinary, ordinary, ordinary)
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
    @hook.before_hit
    def root_before_hit(self, fighter, hit):
        pass
"#;

fn pon_fighter() -> FighterView {
    FighterView {
        id: 1,
        action: "special.neutral".into(),
        grounded: true,
        ..Default::default()
    }
}

fn pon_hit(damage: f32) -> HitView {
    HitView {
        attacker: 0,
        defender: 1,
        damage,
        ..Default::default()
    }
}

#[test]
fn pon_complete_fighter_dispatches_before_hit_through_native_host() {
    let program = Program::new(PON_FIGHTER_SOURCE).expect("complete Fighter must compile");
    let original = pon_hit(5.0);
    let result = program
        .dispatch(
            Hook::BeforeHit,
            &pon_fighter(),
            Some(&original),
            &Default::default(),
        )
        .expect("before_hit callback must run");
    assert_eq!(result.hit.expect("hit result").damage, 12.0);
    assert_eq!(original.damage, 5.0);
}

#[test]
fn pon_failing_callback_leaves_input_hit_unchanged() {
    let source = PON_FIGHTER_SOURCE.replace(
        "hit.damage = hit.damage + 7.0",
        "hit.damage = hit.damage + 7.0\n        1 / 0",
    );
    let program = Program::new(source).expect("fixture should compile");
    let original = pon_hit(5.0);
    let error = program
        .dispatch(
            Hook::BeforeHit,
            &pon_fighter(),
            Some(&original),
            &Default::default(),
        )
        .expect_err("invalid host write must fail callback");
    assert!(matches!(error, Error::Runtime(_)));
    assert!(error.to_string().contains("division") || error.to_string().contains("zero"));
    assert_eq!(original.damage, 5.0);
}

#[test]
fn pon_malformed_source_is_rejected_with_compile_error() {
    let error = Program::new("this is not valid source").expect_err("malformed source");
    assert!(matches!(error, Error::Compile(_)));
}

#[test]
fn pon_serialized_complete_fighter_source_loads_and_dispatches() {
    let program = Program::deserialize(serde_json::json!(PON_FIGHTER_SOURCE))
        .expect("serialized Fighter source must load");
    let result = program
        .dispatch(
            Hook::BeforeHit,
            &pon_fighter(),
            Some(&pon_hit(3.0)),
            &Default::default(),
        )
        .expect("deserialized callback must run");
    assert_eq!(result.hit.expect("hit result").damage, 10.0);
}
