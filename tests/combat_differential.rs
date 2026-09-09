//! Native comparisons against selected upstream C bodies. Coefficients below
//! are test inputs, not claimed character/common-data defaults.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use physics::combat::*;
use proptest::prelude::*;
use std::sync::Mutex;

static RULES_LOCK: Mutex<()> = Mutex::new(());

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_combat_capsule(values: *const f32, closest: *mut f32) -> i32;
    fn oracle_combat_knockback(
        rules: *const f32,
        hit: *const u32,
        damage: *const f32,
        modifiers: *const f32,
        attack_damage: u32,
        count_override: i32,
        override_mode: i32,
    ) -> f32;
    fn oracle_combat_hitlag(damage: i32, crouching: i32, multiplier: f32, rules: *const f32)
    -> f32;
    fn oracle_combat_initial_hitstun(knockback: f32, scale: f32) -> i32;
}

fn same(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn collision(values: [f32; 11]) {
    let capsule = Capsule {
        start: values[..3].try_into().unwrap(),
        end: values[3..6].try_into().unwrap(),
        radius: values[9],
    };
    let mut actual_closest = [99.0; 3];
    let mut expected_closest = actual_closest;
    let actual = capsule_sphere(
        &capsule,
        values[6..9].try_into().unwrap(),
        values[10],
        &mut actual_closest,
    );
    // SAFETY: the adapter reads exactly 11 scalars and writes three initialized
    // output scalars. All vectors are copied into correctly typed C objects.
    let expected = unsafe { oracle_combat_capsule(values.as_ptr(), expected_closest.as_mut_ptr()) };
    assert_eq!(actual, expected != 0, "values: {values:?}");
    for (a, b) in actual_closest.into_iter().zip(expected_closest) {
        same(a, b);
    }
}

fn compare_knockback(
    r: [f32; 8],
    h: [u32; 3],
    d: [f32; 2],
    m: [f32; 4],
    attack_damage: u32,
    mode: i32,
    count: i32,
) {
    let rules = KnockbackRules {
        weight_scale: r[0],
        weight_base: r[1],
        maximum: r[2],
        percent_scale: r[3],
        damage_percent_scale: r[4],
        fixed_damage: r[5],
        growth_scale: r[6],
        growth_base: r[7],
    };
    let hit = KnockbackHit {
        growth: h[0],
        fixed: h[1],
        base: h[2],
    };
    let damage = DamageState {
        percent: d[0],
        pending_damage: d[1],
        count_override: (mode != 0).then_some(count),
    };
    let modifiers = KnockbackModifiers {
        stage: m[0],
        attack: m[1],
        defense: m[2],
        weight: m[3],
    };
    let actual = knockback(&rules, hit, damage, attack_damage, modifiers).unwrap();
    let _guard = RULES_LOCK.lock().unwrap();
    // SAFETY: all arrays meet the adapter's lengths; Rust checked the reference's
    // float-to-integer domain first. The lock protects C's common-data pointer.
    let expected = unsafe {
        oracle_combat_knockback(
            r.as_ptr(),
            h.as_ptr(),
            d.as_ptr(),
            m.as_ptr(),
            attack_damage,
            count,
            mode,
        )
    };
    same(actual, expected);
}

#[test]
fn capsule_boundaries_degeneracy_and_nonfinite_inputs_match_c() {
    for length in [0.0, 0.001, 0.003, 0.0032, 1.0, 2.0] {
        for radius in [-1.0, -0.0, 0.0, 1.0, 2.0] {
            for height in [radius - f32::EPSILON, radius, radius + f32::EPSILON] {
                collision([
                    0.0, 0.0, 0.0, length, 0.0, 0.0, length, height, 0.0, radius, 0.0,
                ]);
            }
        }
    }
    for special in [
        -0.0,
        f32::from_bits(1),
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        for index in 0..11 {
            let mut values = [1.0; 11];
            values[index] = special;
            collision(values);
        }
    }
}

#[test]
fn fixed_knockback_count_overrides_and_cap_match_c() {
    let rules = [0.125, 2.0, 1000.0, 0.2, 0.05, 8.0, 1.5, 16.0];
    for fixed in [0, 1, 50, u32::MAX] {
        for mode in 0..=2 {
            for count in [i32::MIN, -1, 0, 1, i32::MAX] {
                compare_knockback(
                    rules,
                    [100, fixed, 20],
                    [99.75, 10.5],
                    [1.0, 1.0, 1.0, 100.0],
                    10,
                    mode,
                    count,
                );
            }
        }
    }
    for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        // The fixed branch never casts percent, including a nonfinite percent.
        compare_knockback(rules, [100, 20, 20], [special, 10.0], [1.0; 4], 10, 0, 0);
    }
    for index in 0..8 {
        let mut with_nan = rules;
        with_nan[index] = f32::NAN;
        compare_knockback(with_nan, [100, 0, 20], [10.75, 10.0], [1.0; 4], 10, 0, 0);
    }
}

#[test]
fn undefined_integer_conversions_are_rejected_before_c() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2_147_483_648.0] {
        assert!(initial_hitstun(value, 1.0).is_err());
        assert!(
            hitlag(
                1,
                false,
                1.0,
                &HitlagRules {
                    damage_scale: value,
                    base: 0.0,
                    crouch_multiplier: 1.0
                }
            )
            .is_err()
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn capsule_sphere_matches_c(values in prop::array::uniform11(-10_f32..10_f32)) {
        collision(values);
    }

    #[test]
    fn knockback_matches_c(
        mut rules in prop::array::uniform8(-10_f32..10_f32),
        growth in any::<u32>(), fixed in prop_oneof![Just(0_u32),any::<u32>()], base in any::<u32>(),
        damage in prop::array::uniform2(-1000_f32..1000_f32),
        modifiers in prop::array::uniform4(-10_f32..10_f32),
        attack_damage in any::<u32>(), mode in 0..=2_i32, count in any::<i32>(),
    ) {
        rules[2] *= 10_000_000.0;
        compare_knockback(rules,[growth,fixed,base],damage,modifiers,attack_damage,mode,count);
    }

    #[test]
    fn hitlag_and_initial_hitstun_match_c(
        damage in -1000..1000_i32, multiplier in -4_f32..4_f32,
        rules in prop::array::uniform3(-4_f32..4_f32), crouching in any::<bool>(),
        knockback in -1000_f32..1000_f32, scale in -4_f32..4_f32,
    ) {
        let actual = hitlag(damage,crouching,multiplier,&HitlagRules {
            damage_scale:rules[0],base:rules[1],crouch_multiplier:rules[2]}).unwrap();
        let _guard = RULES_LOCK.lock().unwrap();
        // SAFETY: the three coefficients are present, casts are in i32 range,
        // and the mutex protects the oracle's temporary common-data pointer.
        let expected = unsafe { oracle_combat_hitlag(damage,i32::from(crouching),multiplier,rules.as_ptr()) };
        same(actual,expected);
        // SAFETY: products are finite and within i32 range; same lock as above.
        let expected = unsafe { oracle_combat_initial_hitstun(knockback,scale) };
        prop_assert_eq!(initial_hitstun(knockback,scale).unwrap(),expected);
    }
}
