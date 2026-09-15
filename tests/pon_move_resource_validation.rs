//! Canonical Pon move links are checked against native destination resources
//! when a match is registered.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)

class Shared(Move):
    action = "attack_air_b"

ordinary = Move()
shared = Shared()

@register
class TestFighter(Fighter):
    name = "test_fighter"
    attributes = Attributes
    specials = SpecialMoves(shared, ordinary, ordinary, ordinary)
    aerials = AerialMoves(shared, ordinary, ordinary, ordinary, ordinary)
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
fn aerial_and_cross_group_links_share_the_destination_resource() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("move definition compiles"));
    Match::new(data, 0).expect("aerial destination is available to both groups");
}

#[test]
fn canonical_aerial_link_without_destination_resource_is_rejected_at_registration() {
    let mut data = aerial::data();
    data.fighters[0].script = Some(Program::new(SOURCE).expect("move definition compiles"));
    data.fighters[0].aerials = None;

    let error = Match::new(data, 0).expect_err("missing canonical destination must fail early");
    assert!(error.to_string().contains("invalid move destinations"));
    assert!(error.to_string().contains("no native resource"));
}
