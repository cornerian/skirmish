//! Acceptance tests for Pon's typed native fighter and hit proxies.

use skirmish::game::script::{Error, FighterView, HitView, Hook, LocalValue, Program};

pub const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class FoxAttributes(Attributes):
    weight = 100
class State:
    counter: int = 2
    frames: int = 3
    token: int = 4
class ActionState:
    phase: int = 5
    frames: int = 6
class Ordinary(Move): pass
class CombatMove(Move):
    @hook.before_hit
    def before_hit(self, fighter, hit):
        fighter.state.counter = fighter.state.counter + 1
        fighter.state.frames = fighter.state.frames + 1
        fighter.state.token = fighter.state.token + 1
        fighter.action_state.phase = fighter.action_state.phase + 2
        fighter.action_state.frames = fighter.action_state.frames + 1
        hit.damage = hit.damage + 1.5
        hit.angle = hit.angle + 10.0
        hit.apply_damage = False
        hit.cancelled = True
ordinary = Ordinary()
combat = CombatMove()
@register
class Fox(Fighter):
    name = "fox"
    attributes = FoxAttributes
    state = State
    action_state = ActionState
    specials = SpecialMoves(combat, ordinary, ordinary, ordinary)
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

fn fighter() -> FighterView {
    FighterView {
        id: 1,
        ..Default::default()
    }
}

fn hit() -> HitView {
    HitView {
        damage: 5.0,
        angle: 20.0,
        ..Default::default()
    }
}

#[test]
fn typed_state_action_state_and_hit_fields_round_trip_through_native_host() {
    let program = Program::new(SOURCE).expect("typed Fighter must compile");
    let result = program
        .dispatch(
            Hook::BeforeHit,
            &fighter(),
            Some(&hit()),
            &Default::default(),
        )
        .expect("typed callback must run");
    assert_eq!(result.locals.get("counter"), Some(&LocalValue::Integer(3)));
    assert_eq!(result.locals.get("frames"), Some(&LocalValue::Integer(4)));
    assert_eq!(result.locals.get("token"), Some(&LocalValue::Integer(5)));
    assert!(!result.locals.contains_key("phase"));
    assert!(!result.locals.contains_key("action_state"));
    assert_eq!(
        result.action_state.get("phase"),
        Some(&LocalValue::Integer(7))
    );
    assert_eq!(
        result.action_state.get("frames"),
        Some(&LocalValue::Integer(7))
    );
    assert!(!result.action_state.contains_key("counter"));
    assert!(!result.action_state.contains_key("token"));
    let patch = result.hit.expect("hit patch");
    assert_eq!(patch.damage, 6.5);
    assert_eq!(patch.angle, 30.0);
    assert!(patch.cancelled);
    assert!(!patch.apply_damage);
}

#[test]
fn undeclared_state_write_is_rejected_without_mutating_input_state() {
    let source = SOURCE.replace(
        "fighter.state.counter = fighter.state.counter + 1",
        "fighter.state.missing = 1",
    );
    let program = Program::new(source).expect("fixture must compile");
    let locals = std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(9))]);
    let error = program
        .dispatch(Hook::BeforeHit, &fighter(), Some(&hit()), &locals)
        .expect_err("undeclared state write must fail");
    assert!(matches!(error, Error::Runtime(_)));
    assert_eq!(locals.get("counter"), Some(&LocalValue::Integer(9)));
}

#[test]
fn readonly_fighter_assignment_is_rejected_without_mutating_input_hit() {
    let source = SOURCE.replace("hit.damage = hit.damage + 1.5", "fighter.id = 9");
    let program = Program::new(source).expect("fixture must compile");
    let original = hit();
    let error = program
        .dispatch(
            Hook::BeforeHit,
            &fighter(),
            Some(&original),
            &Default::default(),
        )
        .expect_err("readonly fighter assignment must fail");
    assert!(matches!(error, Error::Runtime(_)));
    assert_eq!(original.damage, 5.0);
}
