//! Program wire format retains the exact dependency bundle used by Pon.

use skirmish::game::script::{
    FighterView, HitView, Hook, Program,
    definition::{AssetStore, Definition},
};

const SOURCE: &str = r#"
from helper import BONUS
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves, GrabMoves,
    GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves, TauntMoves,
    ThrowMoves, TiltMoves, hook, register)
class Ordinary(Move): pass
class DamageMove(Move):
    @hook.before_hit
    def before_hit(self, fighter, hit):
        hit.damage = hit.damage + BONUS
ordinary = Ordinary()
damage = DamageMove()
@register
class Fox(Fighter):
    name = "fox"
    attributes = Attributes
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
"#;

#[test]
fn serialized_program_retains_dependency_source() {
    let mut assets = AssetStore::default();
    assets.register("helper.py", "BONUS = 9.0\n");
    let definition = Definition::load_registered(SOURCE, &assets).expect("fixture loads");
    let wire = serde_json::to_value(definition.program.as_ref()).expect("serialize");
    assert_eq!(wire["version"], "pon-v2");
    assert_eq!(wire["dependencies"]["helper.py"], "BONUS = 9.0\n");
    drop(definition);
    drop(assets);
    let restored: Program = serde_json::from_value(wire).expect("dependency restores");
    let result = restored
        .dispatch(
            Hook::BeforeHit,
            &FighterView {
                action: "special.neutral".into(),
                ..Default::default()
            },
            Some(&HitView {
                damage: 3.0,
                ..Default::default()
            }),
            &Default::default(),
        )
        .expect("restored callback runs");
    assert_eq!(result.hit.expect("patch").damage, 12.0);
}

#[test]
fn unsupported_program_wire_version_fails_closed() {
    let value = serde_json::json!({"version":"pon-v0","source":"pass","dependencies":{}});
    assert!(serde_json::from_value::<Program>(value).is_err());
}
