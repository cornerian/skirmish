//! Damage-direction assignments checked against pinned source statements.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::{fighter_hit_direction, throw_hit_direction};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_fighter_hit_direction(victim_x: f32, attacker_x: f32) -> f32;
    fn oracle_throw_hit_direction(attacker_facing: f32) -> f32;
}

fn compare_fighter(victim_x: f32, attacker_x: f32) {
    // SAFETY: the oracle accepts two scalars and has no external state.
    let expected = unsafe { oracle_fighter_hit_direction(victim_x, attacker_x) };
    assert_eq!(
        fighter_hit_direction(victim_x, attacker_x).to_bits(),
        expected.to_bits()
    );
}

fn compare_throw(attacker_facing: f32) {
    // SAFETY: the oracle accepts one scalar and has no external state.
    let expected = unsafe { oracle_throw_hit_direction(attacker_facing) };
    assert_eq!(
        throw_hit_direction(attacker_facing).to_bits(),
        expected.to_bits()
    );
}

#[test]
fn adapter_statements_are_verbatim_in_the_pinned_sources() {
    let adapter = include_str!("oracle/hit_direction.c");
    for (begin, end, source) in [
        (
            "/* BEGIN VERBATIM FIGHTER DIRECTION */\n",
            "/* END VERBATIM FIGHTER DIRECTION */",
            include_str!("oracle/original/combat_knockback.c"),
        ),
        (
            "/* BEGIN VERBATIM THROW DIRECTION */\n",
            "/* END VERBATIM THROW DIRECTION */",
            include_str!("oracle/original/throw_input.c"),
        ),
    ] {
        let block = adapter
            .split(begin)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap();
        assert!(source.contains(block));
    }
}

#[test]
fn equality_signed_zero_nan_and_infinity_match() {
    for victim_x in [
        f32::NEG_INFINITY,
        -1.0,
        -0.0,
        0.0,
        1.0,
        f32::INFINITY,
        f32::NAN,
    ] {
        for attacker_x in [-0.0, 0.0, f32::INFINITY, f32::NAN] {
            compare_fighter(victim_x, attacker_x);
        }
        compare_throw(victim_x);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_inputs_match(victim in any::<u32>(), attacker in any::<u32>()) {
        compare_fighter(f32::from_bits(victim), f32::from_bits(attacker));
        compare_throw(f32::from_bits(attacker));
    }
}
