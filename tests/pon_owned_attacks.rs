//! A canonical native action may have different physical attack resources per
//! selected class move owner.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::script::resources::{Resources, Specials};
use skirmish::game::{Action, BUTTON_A, Controller, Match, script::Program};

const SOURCE: &str = r#"
from dataclasses import dataclass
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, register)

class First(Move):
    action = action("attack_air_b", attack="first")

class Second(Move):
    action = action("attack_air_b", attack="second")

class Unowned(Move):
    action = action("attack_air_b")

first = First()
second = Second()
unowned = Unowned()
ordinary = Move()

@dataclass(frozen=True)
class ExtendedSpecialMoves(SpecialMoves):
    first: Move
    second: Move

@register
class TestFighter(Fighter):
    name = "owner_attack_test"
    attributes = Attributes
    specials = ExtendedSpecialMoves(ordinary, ordinary, ordinary, ordinary, first, second)
    aerials = AerialMoves(first, second, unowned, ordinary, ordinary)
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

const SOURCE_UNOWNED_NEUTRAL: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, register)

class First(Move):
    action = action("attack_air_b", attack="first")

class Second(Move):
    action = action("attack_air_b", attack="second")

class Unowned(Move):
    action = action("attack_air_b")

first = First()
second = Second()
unowned = Unowned()
ordinary = Move()

@register
class TestFighter(Fighter):
    name = "owner_attack_test"
    attributes = Attributes
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(unowned, first, second, ordinary, ordinary)
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

// Keep the moveset slot identities fixed while changing the order in which
// the behavior classes are declared.  Runtime ownership must follow the
// stable move ids rather than the source declaration order.
const SOURCE_REVERSED_BEHAVIORS: &str = r#"
from dataclasses import dataclass
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, register)

class First(Move):
    action = action("attack_air_b", attack="first")

class Second(Move):
    action = action("attack_air_b", attack="second")

class Unowned(Move):
    action = action("attack_air_b")

unowned = Unowned()
second = Second()
first = First()
ordinary = Move()

@dataclass(frozen=True)
class ExtendedSpecialMoves(SpecialMoves):
    first: Move
    second: Move

@register
class TestFighter(Fighter):
    name = "owner_attack_test"
    attributes = Attributes
    specials = ExtendedSpecialMoves(ordinary, ordinary, ordinary, ordinary, second, first)
    aerials = AerialMoves(first, second, unowned, ordinary, ordinary)
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

fn attack(
    base: &skirmish::game::data::Attack,
    move_id: u16,
    y: f32,
) -> skirmish::game::data::Attack {
    let mut attack = base.clone();
    attack.move_id = Some(move_id);
    for frame in &mut attack.frames {
        frame.hitboxes.clear();
        frame.hitboxes.push(
            serde_json::from_value(serde_json::json!({
                "group": 0, "bone": 0, "center": [0.0, y, 0.0], "radius": 1.0,
                "damage": 1, "angle_degrees": 45.0, "growth": 1, "fixed": 1, "base": 1
            }))
            .unwrap(),
        );
    }
    attack
}

fn data(unowned_first: bool) -> skirmish::game::data::MatchData {
    let mut data = aerial::data();
    data.fighters[0].jab.move_id = Some(11);
    data.fighters[1].jab.move_id = Some(11);
    data.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.0; 9],
        debug_bypass: false,
    });
    let first = attack(&data.fighters[0].jab, 71, 2.0);
    let second = attack(&data.fighters[0].jab, 92, 8.0);
    let resources = Resources::new(std::collections::BTreeMap::from([
        ("first".into(), serde_json::to_value(first).unwrap()),
        ("second".into(), serde_json::to_value(second).unwrap()),
    ]))
    .unwrap();
    for fighter in &mut data.fighters {
        let source = if unowned_first {
            SOURCE_UNOWNED_NEUTRAL
        } else {
            SOURCE
        };
        fighter.script = Some(Program::new(source).unwrap());
        fighter.specials = Some(Specials {
            character: "owner_attack_test".into(),
            resources: resources.clone(),
        });
    }
    data
}

fn reversed_data() -> skirmish::game::data::MatchData {
    let mut data = aerial::data();
    data.fighters[0].jab.move_id = Some(11);
    data.fighters[1].jab.move_id = Some(11);
    data.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.0; 9],
        debug_bypass: false,
    });
    let first = attack(&data.fighters[0].jab, 71, 2.0);
    let second = attack(&data.fighters[0].jab, 92, 8.0);
    let resources = Resources::new(std::collections::BTreeMap::from([
        ("first".into(), serde_json::to_value(first).unwrap()),
        ("second".into(), serde_json::to_value(second).unwrap()),
    ]))
    .unwrap();
    for fighter in &mut data.fighters {
        fighter.script = Some(Program::new(SOURCE_REVERSED_BEHAVIORS).unwrap());
        fighter.specials = Some(Specials {
            character: "owner_attack_test".into(),
            resources: resources.clone(),
        });
    }
    data
}

fn registry_owners(data: &skirmish::game::data::MatchData) -> [usize; 3] {
    use skirmish::game::script::move_registry::{MoveGroup, MoveSlot};

    let moves = data.fighters[0].script.as_ref().unwrap().moves();
    let first = moves
        .resolve_typed(&MoveGroup::Aerials, &MoveSlot::Forward)
        .unwrap();
    let second = moves
        .resolve_typed(&MoveGroup::Aerials, &MoveSlot::Back)
        .unwrap();
    let neutral_entry = moves
        .resolve_typed(&MoveGroup::Aerials, &MoveSlot::Neutral)
        .unwrap();
    assert_eq!(first.canonical, Some(Action::AttackAirB));
    assert_eq!(second.canonical, Some(Action::AttackAirB));
    assert_ne!(first.behavior_index, second.behavior_index);
    assert_eq!(neutral_entry.canonical, Some(Action::AttackAirB));
    [
        neutral_entry.behavior_index,
        first.behavior_index,
        second.behavior_index,
    ]
}

fn input(forward: bool) -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            cstick: if forward { [1.0, 0.0] } else { [0.0, 0.0] },
            ..Default::default()
        },
        Controller::default(),
    ]
}

#[test]
fn same_native_action_samples_the_selected_owners_attack_geometry() {
    let fixture = data(false);
    let owners = registry_owners(&fixture);
    assert_ne!(owners[0], owners[1]);
    let mut samples = Vec::new();
    for forward in [false, true] {
        let mut game = Match::new(fixture.clone(), 0).unwrap();
        let state = game.step(input(forward)).unwrap();
        assert_eq!(state.fighters[0].action, Action::AttackAirB);
        let expected_y = state.fighters[0].position[1] + if forward { 8.0 } else { 2.0 };
        let actual = state.fighters[0].hitboxes[0].current[1];
        assert!(
            (actual - expected_y).abs() < 1e-3,
            "expected {expected_y}, got {actual}"
        );
        assert_eq!(
            state.fighters[0].staling.identity.move_id,
            if forward { 92 } else { 71 }
        );
        samples.push(actual);
    }
    assert!(samples.iter().all(|sample| sample.is_finite()));
    assert_ne!(samples[0], samples[1]);
}

#[test]
fn owner_without_attack_uses_native_geometry_and_does_not_inherit_other_owner() {
    let fixture = data(true);
    let owners = registry_owners(&fixture);
    assert_ne!(owners[0], owners[1]);
    let mut game = Match::new(fixture, 0).unwrap();
    let state = game.step(input(false)).unwrap();
    assert_eq!(state.fighters[0].action, Action::AttackAirB);
    assert_eq!(state.fighters[0].staling.identity.move_id, 22);
    assert_eq!(state.fighters[0].hitboxes[0].current, [0.0; 3]);
}

#[test]
fn owner_links_survive_checkpoint_restore_byte_identically() {
    let mut game = Match::new(data(false), 0).unwrap();
    game.step(input(true)).unwrap();
    let checkpoint = game.checkpoint();
    let expected = serde_json::to_vec(game.step([Controller::default(); 2]).unwrap()).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    let replay = serde_json::to_vec(game.step([Controller::default(); 2]).unwrap()).unwrap();
    assert_eq!(replay, expected);
}

#[test]
fn reversed_behavior_declaration_keeps_slot_owner_geometry() {
    let fixture = reversed_data();
    let owners = registry_owners(&fixture);
    let original = registry_owners(&data(false));
    assert_eq!(owners[0], original[1]);
    assert_eq!(owners[1], original[0]);
    let mut samples = Vec::new();
    for forward in [false, true] {
        let mut game = Match::new(fixture.clone(), 0).unwrap();
        let state = game.step(input(forward)).unwrap();
        assert_eq!(state.fighters[0].action, Action::AttackAirB);
        let expected_y = state.fighters[0].position[1] + if forward { 8.0 } else { 2.0 };
        let actual = state.fighters[0].hitboxes[0].current[1];
        assert!((actual - expected_y).abs() < 1e-3);
        assert_eq!(
            state.fighters[0].staling.identity.move_id,
            if forward { 92 } else { 71 }
        );
        samples.push(actual);
    }
    assert_ne!(samples[0], samples[1]);
}

#[test]
fn resources_round_trip_rebuilds_typed_attack_index_without_serializing_cache() {
    let resources = Resources::new(std::collections::BTreeMap::from([(
        "x".into(),
        serde_json::json!({
            "move_id": 3, "frames": [{"bones": [], "hitboxes": [], "hurtbox_states": []}]
        }),
    )]))
    .unwrap();
    let encoded = serde_json::to_value(&resources).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({"x": {"move_id": 3, "frames": [{"bones": [], "hitboxes": [], "hurtbox_states": []}]}})
    );
    let decoded: Resources = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.attack("x").unwrap().move_id, Some(3));
}
