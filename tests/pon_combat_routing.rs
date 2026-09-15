//! Native combat callback routing through the resolved Pon binding table.

use skirmish::game::script::{Error, FighterView, HitView, Hook, Program};

const SOURCE: &str = r#"
from skirmish import (Action, AerialMoves, Attributes, DefenseMoves, Fighter,
    GetupMoves, GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves,
    SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, action, hook, register)

class Ordinary(Move): pass
class Filtered(Move):
    start = action(Action.SPECIAL_N_START)
    @hook.before_hit(actions=(start,))
    def behavior(self, fighter, hit):
        hit.damage = hit.damage * 2.0
    @hook.landed(start)
    def only_start(self, fighter, ctx):
        1 / 0

ordinary = Ordinary()
filtered = Filtered()
@register
class Fox(Fighter):
    name = "fox"
    attributes = Attributes
    specials = SpecialMoves(filtered, ordinary, ordinary, ordinary)
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
    def root(self, fighter, hit):
        hit.damage = hit.damage + 1.0
"#;

fn fighter(action: &str) -> FighterView {
    FighterView {
        action: action.into(),
        ..Default::default()
    }
}

fn hit(damage: f32) -> HitView {
    HitView {
        damage,
        ..Default::default()
    }
}

#[test]
fn combat_routes_root_then_behavior_in_declaration_order() {
    let program = Program::new(SOURCE).expect("fixture should compile");
    let result = program
        .dispatch(
            Hook::BeforeHit,
            &fighter("SpecialNStart"),
            Some(&hit(5.0)),
            &Default::default(),
        )
        .expect("matching combat callbacks should run");
    assert_eq!(result.hit.expect("patch").damage, 12.0);

    let result = program
        .dispatch(
            Hook::BeforeHit,
            &fighter("SpecialNLoop"),
            Some(&hit(5.0)),
            &Default::default(),
        )
        .expect("root callback should still run");
    assert_eq!(result.hit.expect("patch").damage, 6.0);
}

#[test]
fn combat_callback_failure_does_not_commit_staged_patch() {
    let source = SOURCE.replace(
        "hit.damage = hit.damage * 2.0",
        "hit.damage = hit.damage * 2.0\n        1 / 0",
    );
    let program = Program::new(source).expect("fixture should compile");
    let original = hit(5.0);
    let error = program
        .dispatch(
            Hook::BeforeHit,
            &fighter("SpecialNStart"),
            Some(&original),
            &Default::default(),
        )
        .expect_err("failed callback must abort transaction");
    assert!(matches!(error, Error::Runtime(_)));
    assert!(error.to_string().contains("division") || error.to_string().contains("zero"));
    assert_eq!(original.damage, 5.0);
}

#[test]
fn combat_action_filter_skips_behavior_for_unknown_action() {
    let program = Program::new(SOURCE).expect("fixture should compile");
    let result = program
        .dispatch(
            Hook::BeforeHit,
            &fighter("unregistered_action"),
            Some(&hit(5.0)),
            &Default::default(),
        )
        .expect("unknown action must skip filtered behavior");
    assert_eq!(result.hit.expect("patch").damage, 6.0);
}
