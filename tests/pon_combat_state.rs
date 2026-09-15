//! Combat dispatch keeps persistent and action scoped state independent.

use skirmish::game::script::{
    FighterView, HitPatch, HitView, Hook, LocalState, LocalValue, Program,
};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class FoxAttributes(Attributes):
    weight = 100
class State:
    counter: int = 10
class ActionState:
    counter: int = 100
class CombatMove(Move):
    @hook.before_hit
    def before_hit(self, fighter, hit):
        fighter.state.counter = fighter.state.counter + 1
        fighter.action_state.counter = fighter.action_state.counter + 2
    @hook.before_receive_hit
    def before_receive_hit(self, fighter, hit):
        fighter.state.counter = fighter.state.counter + 1
        fighter.action_state.counter = fighter.action_state.counter + 2
        hit.cancelled = False
        hit.apply_damage = True
        hit.apply_knockback = True
        hit.apply_hitlag = True
        hit.apply_hitstun = True
combat = CombatMove()
ordinary = Move()
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

#[test]
fn same_named_fields_remain_separate_across_dispatches() {
    let program = Program::new(SOURCE).expect("fixture compiles");
    let fighter = FighterView {
        id: 1,
        ..Default::default()
    };
    let persistent =
        std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(10))]);
    let action = std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(100))]);

    let first = program
        .dispatch_with_states(
            Hook::BeforeHit,
            &fighter,
            Some(&Default::default()),
            &persistent,
            &action,
        )
        .expect("first dispatch");
    assert_eq!(first.locals["counter"], LocalValue::Integer(11));
    assert_eq!(first.action_state["counter"], LocalValue::Integer(102));

    let second = program
        .dispatch_with_states(
            Hook::BeforeHit,
            &fighter,
            Some(&Default::default()),
            &first.locals,
            &first.action_state,
        )
        .expect("second dispatch");
    assert_eq!(second.locals["counter"], LocalValue::Integer(12));
    assert_eq!(second.action_state["counter"], LocalValue::Integer(104));
}

#[test]
fn no_callback_hit_fast_path_preserves_native_knockback() {
    let program = Program::new(SOURCE).expect("fixture compiles");
    let fighter = FighterView {
        id: 1,
        ..Default::default()
    };
    let hit = HitView {
        damage: 8.0,
        angle: 45.0,
        knockback: 37.5,
        ..Default::default()
    };

    let result = program
        .dispatch(Hook::AfterHit, &fighter, Some(&hit), &LocalState::new())
        .expect("unhandled hook uses the fast path");
    assert_eq!(result.hit.expect("hit patch returned").knockback, 37.5);
}

#[test]
fn patch_context_preserves_monotonic_baseline_gates() {
    let program = Program::new(SOURCE).expect("fixture compiles");
    let baseline = HitPatch {
        cancelled: false,
        apply_damage: false,
        apply_knockback: false,
        apply_hitlag: false,
        apply_hitstun: false,
        ..HitPatch::default()
    };
    let persistent =
        std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(10))]);
    let action = std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(100))]);
    let error = program
        .dispatch_with_states_with_patch(
            Hook::BeforeReceiveHit,
            &FighterView::default(),
            &HitView::default(),
            &baseline,
            &persistent,
            &action,
        )
        .expect_err("defender cannot re-enable a disabled gate");
    assert!(
        error.to_string().contains("cannot be re-enabled"),
        "{error}"
    );
    assert_eq!(persistent["counter"], LocalValue::Integer(10));
    assert_eq!(action["counter"], LocalValue::Integer(100));
    assert!(!baseline.cancelled);
    assert!(!baseline.apply_damage);
    assert!(!baseline.apply_knockback);
    assert!(!baseline.apply_hitlag);
    assert!(!baseline.apply_hitstun);
}

#[test]
fn failed_receive_hit_callback_rolls_back_both_state_domains() {
    let source = SOURCE.replace(
        "        hit.apply_damage = True\n",
        "        1 / 0\n        hit.apply_damage = True\n",
    );
    let program = Program::new(source).expect("fixture compiles");
    let persistent =
        std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(10))]);
    let action = std::collections::BTreeMap::from([("counter".into(), LocalValue::Integer(100))]);
    let baseline = HitPatch {
        apply_damage: false,
        apply_knockback: false,
        apply_hitlag: false,
        apply_hitstun: false,
        ..HitPatch::default()
    };
    let error = program
        .dispatch_with_states_with_patch(
            Hook::BeforeReceiveHit,
            &FighterView::default(),
            &HitView::default(),
            &baseline,
            &persistent,
            &action,
        )
        .expect_err("division by zero aborts callback transaction");
    assert!(
        error.to_string().contains("division") || error.to_string().contains("zero"),
        "{error}"
    );
    assert_eq!(persistent["counter"], LocalValue::Integer(10));
    assert_eq!(action["counter"], LocalValue::Integer(100));
}
