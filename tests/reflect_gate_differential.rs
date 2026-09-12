//! The Reflector hand-off's own eligibility gate (`ftColl_80077464`,
//! already pinned whole-file as `combat_knockback.c`): a laser reflects
//! only when its damage does not exceed `down::Reflect.max_damage`. A
//! verbatim excerpt (like `hit_direction_differential.rs`'s own pattern),
//! not a full extraction -- see `tests/oracle/reflect_gate.c`'s own header
//! note for why. `game::projectile::step` implements this same comparison
//! (`(damage as i32) <= down.reflect.max_damage`); this test proves that
//! comparison's direction and inclusivity against the pinned source's own
//! exact branch, and the fractional-damage-rounds-up-to-1 idiom the source
//! uses (inapplicable to this codebase's own integer `Hitbox::damage`, but
//! confirmed here rather than silently assumed).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_reflect_eligible(hit_damage_in: f32, max_damage_in: i32, out_damage: *mut i32) -> i32;
}

fn expected_damage(hit_damage: f32) -> i32 {
    if hit_damage != 0.0 {
        let truncated = hit_damage as i32;
        if truncated != 0 { truncated } else { 1 }
    } else {
        0
    }
}

fn compare(hit_damage: f32, max_damage: i32) {
    let mut damage = 0;
    let eligible = unsafe { oracle_reflect_eligible(hit_damage, max_damage, &mut damage) };
    let expected = expected_damage(hit_damage);
    assert_eq!(damage, expected, "damage derivation for hit_damage={hit_damage}");
    assert_eq!(
        eligible != 0,
        expected <= max_damage,
        "eligibility for damage={expected}, max_damage={max_damage}"
    );
}

#[test]
fn adapter_statements_are_verbatim_in_the_pinned_sources() {
    let adapter = include_str!("oracle/reflect_gate.c");
    let source = include_str!("oracle/original/combat_knockback.c");
    let block = adapter
        .split("/* BEGIN VERBATIM REFLECT DAMAGE AND GATE */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM REFLECT DAMAGE AND GATE */")
        .next()
        .unwrap();
    assert!(source.contains(block));
}

#[test]
fn known_values_match() {
    compare(3.0, 20);
    compare(3.0, 1);
    compare(3.0, 3);
    compare(0.0, 0);
    compare(0.5, 0);
    compare(0.5, 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_damage_and_cap_match(
        hit_damage in 0.0f32..1000.0,
        max_damage in -10i32..1000,
    ) {
        compare(hit_damage, max_damage);
    }
}
