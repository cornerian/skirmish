//! Isolated damage arithmetic checked against pinned upstream C bodies.
//! Rule values are generated/test coefficients, never game-data defaults.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::*;

// `ftCo_Damage_CalcAngle`'s own fused `x148 * ratio + 1` (`docs/math.md`)
// only matches `fighter::damage::launch_angle`'s `f32::mul_add` when
// compiled with FMA contraction, so it lives in its own translation unit
// (`tests/oracle/damage_calc_angle.c`) built into the separate
// `skirmish_oracle_fma` static library (`build.rs`) rather than
// `skirmish_oracle`.
#[link(name = "skirmish_oracle_fma", kind = "static")]
unsafe extern "C" {
    fn oracle_damage_angle_fma(
        angle: i32,
        knockback: f32,
        airborne: i32,
        rules: *const f32,
        bounds: *const u32,
        timer: i32,
        flags: *mut u8,
    ) -> f32;
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_damage_merge(values: *const f32, since_hit: i32, window: i32, output: *mut f32);
    fn oracle_damage_di(values: *const f32, max_degrees: f32, output: *mut f32);
    fn oracle_damage_decay(velocity: *const f32, decay: f32, output: *mut f32);
    fn oracle_damage_armor(knockback: f32, armor: *const f32, minimum: f32) -> f32;
    fn oracle_damage_tilt_timer(timer: u8, current: f32, previous: f32, threshold: f32) -> u8;
    fn oracle_damage_displacement(
        values: *const f32,
        timers: *mut u8,
        allowed: i32,
        window: i32,
        exit: i32,
        output: *mut f32,
    ) -> i32;
}

fn exact(actual: f32, expected: f32) {
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

fn numerical(actual: [f32; 2], expected: [f32; 2], input_scale: f32) {
    for (a, e) in actual.into_iter().zip(expected) {
        // The absolute floor covers `cosf`/`sinf`'s own bounded absolute
        // difference from this crate's real trigonometry oracle
        // (`docs/math.md`'s fused-multiply-add finding) landing near a
        // quadrant boundary the deflected angle crosses, where the
        // input-scaled relative term alone is too tight (confirmed
        // directly).
        assert!(
            (a - e).abs() <= 4e-6 * input_scale.max(a.abs()).max(e.abs()).max(1.0) + 0.05,
            "{a:?} != {e:?}; input scale {input_scale}"
        );
    }
}

fn angle_case(
    angle: i32,
    kb: f32,
    air: bool,
    values: [f32; 4],
    bounds: [u32; 2],
    timer: i32,
    initial: [u8; 2],
) {
    let rules = LaunchAngleRules {
        airborne_radians: values[0],
        grounded_max_degrees: values[1],
        grounded_low_knockback: values[2],
        grounded_high_knockback: values[3],
        special_angle_min: bounds[0],
        special_angle_max: bounds[1],
        special_timer: timer,
    };
    let actual = launch_angle(angle, kb, air, &rules);
    let mut expected_flags = initial;
    // SAFETY: arrays provide four float coefficients, two u32 bounds, and two
    // mutable flag bytes; each call initializes thread-local common data.
    let expected = unsafe {
        oracle_damage_angle_fma(
            angle,
            kb,
            i32::from(air),
            values.as_ptr(),
            bounds.as_ptr(),
            timer,
            expected_flags.as_mut_ptr(),
        )
    };
    exact(actual.radians, expected);
    let actual_flags = actual.special_timer.map_or(initial, |timer| [1, timer]);
    assert_eq!(actual_flags, expected_flags);
}

fn merge_case(values: [f32; 4], since_hit: i32, window: i32) {
    let actual = merge_knockback(
        [values[0], values[1]],
        [values[2], values[3]],
        since_hit,
        window,
    );
    let mut expected = [0.0; 2];
    // SAFETY: four readable and two writable float elements, thread-local rules.
    unsafe { oracle_damage_merge(values.as_ptr(), since_hit, window, expected.as_mut_ptr()) };
    for (a, e) in actual.into_iter().zip(expected) {
        exact(a, e);
    }
}

fn displacement_case(values: [f32; 6], initial: [u8; 2], allowed: bool, window: i32) {
    for exit in [false, true] {
        let mut expected = [0.0; 2];
        let mut expected_timers = initial;
        // SAFETY: six input floats, two mutable timer bytes, two output floats;
        // bounded callback adapter excludes C-stick, LR and collision callbacks.
        let calls = unsafe {
            oracle_damage_displacement(
                values.as_ptr(),
                expected_timers.as_mut_ptr(),
                i32::from(allowed),
                window,
                i32::from(exit),
                expected.as_mut_ptr(),
            )
        };
        let mut actual = [values[0], values[1]];
        let mut timers = initial;
        let moved = if exit {
            automatic_displacement(&mut actual, [values[2], values[3]], values[4], values[5])
        } else {
            smash_displacement(
                &mut actual,
                [values[2], values[3]],
                &mut timers,
                allowed,
                values[4],
                window,
                values[5],
            )
        };
        assert_eq!(i32::from(moved), calls);
        assert_eq!(timers, expected_timers);
        for (a, e) in actual.into_iter().zip(expected) {
            exact(a, e);
        }
    }
}

#[test]
fn sdi_asdi_thresholds_consumed_windows_and_both_coordinates_match_source() {
    for stick in [
        [0.0, -0.0],
        [0.5, 0.0],
        [0.49999997, 0.0],
        [0.3, 0.4],
        [-1.0, -1.0],
    ] {
        for timers in [[0, 254], [254, 0], [2, 254], [3, 3], [254, 254], [255, 0]] {
            for allowed in [false, true] {
                displacement_case(
                    [-0.0, -0.0, stick[0], stick[1], 0.5, 2.0],
                    timers,
                    allowed,
                    3,
                );
            }
        }
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0] {
        for index in 0..6 {
            let mut values = [1.0, 2.0, 0.5, 0.5, 0.5, 2.0];
            values[index] = value;
            displacement_case(values, [0, 254], true, 3);
        }
    }
}

#[test]
fn armor_max_channel_clamp_and_zero_early_return_match_source() {
    for kb in [0.0, -0.0, 0.5, 5.0, 10.0, f32::NAN, f32::INFINITY] {
        for armor in [
            [0.0, 0.0],
            [8.0, 2.0],
            [2.0, 8.0],
            [8.0, 8.0],
            [f32::NAN, 2.0],
            [2.0, f32::NAN],
        ] {
            for minimum in [0.0, 1.0, 20.0] {
                // SAFETY: two readable float armor channels; adapter sets every
                // excluded modifier to its ordinary inactive state.
                let expected = unsafe { oracle_damage_armor(kb, armor.as_ptr(), minimum) };
                exact(subtract_armor(kb, armor, minimum), expected);
            }
        }
    }
}

#[test]
fn tilt_timer_original_byte_wrap_and_inclusive_excursions_match() {
    for timer in [0, 1, 253, 254, 255] {
        for current in [-1.0, -0.5, -0.49999997, 0.0, 0.49999997, 0.5, 1.0, f32::NAN] {
            for previous in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                // SAFETY: scalar arguments and return, no shared state.
                let expected = unsafe { oracle_damage_tilt_timer(timer, current, previous, 0.5) };
                assert_eq!(tilt_timer(timer, current, previous, 0.5), expected);
            }
        }
    }
    let wrapper = include_str!("oracle/damage_fighter.c");
    let block = wrapper
        .split("/* BEGIN VERBATIM TILT TIMER */\n")
        .nth(1)
        .unwrap()
        .split("\n/* END VERBATIM TILT TIMER */")
        .next()
        .unwrap();
    assert!(include_str!("oracle/original/damage_fighter.c").contains(block));
    assert!(block.contains("x670_timer_lstick_tilt_x"));
}

#[test]
fn ordinary_angles_361_thresholds_and_conditional_timer_writes_match() {
    let rules = [0.6, 40.0, 20.0, 30.0];
    for angle in [i32::MIN, -1, 0, 359, 360, 361, 362, 363, 400, i32::MAX] {
        for kb in [0.0, 19.999998, 20.0, 20.000002, 29.999998, 30.0, 100.0] {
            for air in [false, true] {
                angle_case(angle, kb, air, rules, [360, 362], -257, [7, 99]);
            }
        }
    }
    let at_threshold = launch_angle(
        361,
        20.0,
        false,
        &LaunchAngleRules {
            airborne_radians: 0.6,
            grounded_max_degrees: 40.0,
            grounded_low_knockback: 20.0,
            grounded_high_knockback: 30.0,
            special_angle_min: 0,
            special_angle_max: u32::MAX,
            special_timer: 256,
        },
    );
    assert_eq!(at_threshold.radians.to_bits(), 0x3c8e_fa35);
    assert_eq!(at_threshold.special_timer, None);
    for exceptional in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0] {
        for index in 0..4 {
            let mut varied = rules;
            varied[index] = exceptional;
            angle_case(361, 25.0, false, varied, [0, 0], 0, [0, 0]);
        }
    }
    angle_case(361, 20.0, false, [0.6, 40.0, 20.0, 20.0], [0, 0], 0, [0, 0]);
}

#[test]
fn decay_thresholds_and_di_noops_retain_exact_source_branches() {
    for velocity in [
        [-0.0, 0.0],
        [0.0, -0.0],
        [0.001, 0.001],
        [1.0, 0.0],
        [0.0, 1.0],
        [-1.0, 0.0],
        [3.0, 4.0],
    ] {
        for decay in [0.0, 0.001, 1.0, 1.000001, 5.0, 5.000001] {
            let mut expected = [0.0; 2];
            // SAFETY: two readable and two writable floats, no shared state.
            unsafe { oracle_damage_decay(velocity.as_ptr(), decay, expected.as_mut_ptr()) };
            let actual = decay_air_knockback(velocity, decay);
            if velocity == [0.0; 2] || expected == [0.0; 2] {
                for (a, e) in actual.into_iter().zip(expected) {
                    exact(a, e);
                }
            } else {
                numerical(actual, expected, 5.0);
            }
        }
        for stick in [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [0.0, -1.0]] {
            let values = [velocity[0], velocity[1], stick[0], stick[1]];
            let mut expected = [0.0; 2];
            // SAFETY: four readable and two writable floats, thread-local rules.
            unsafe { oracle_damage_di(values.as_ptr(), 20.0, expected.as_mut_ptr()) };
            let actual = directional_influence(velocity, stick, 20.0);
            if stick == [0.0; 2] || velocity[0] * velocity[0] + velocity[1] * velocity[1] < 0.00001
            {
                for (a, e) in actual.into_iter().zip(expected) {
                    exact(a, e);
                }
            } else {
                numerical(actual, expected, 5.0);
            }
        }
    }
}

#[test]
fn merge_preserves_signed_zero_nan_and_boundary_window_semantics() {
    for special in [
        -0.0,
        0.0,
        f32::from_bits(1),
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        for index in 0..4 {
            let mut values = [4.0, -3.0, -2.0, -4.0];
            values[index] = special;
            for since_hit in [2, 3, 4] {
                merge_case(values, since_hit, 3);
            }
        }
    }
}

#[test]
fn decay_oracle_contains_the_unchanged_selected_upstream_block() {
    let wrapper = include_str!("oracle/damage_fighter.c");
    let block = wrapper
        .split("/* BEGIN VERBATIM AIR DECAY */\n")
        .nth(1)
        .unwrap()
        .split("\n/* END VERBATIM AIR DECAY */")
        .next()
        .unwrap();
    assert!(include_str!("oracle/original/damage_fighter.c").contains(block));
    assert!(block.contains("x204_knockbackFrameDecay"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn generated_displacement_and_armor_match(
        values in prop::array::uniform6(-20_f32..20_f32), timers in any::<[u8;2]>(),
        allowed in any::<bool>(), window in -1_i32..257_i32,
    ) {
        displacement_case(values,timers,allowed,window);
        let armor=[values[1],values[2]];
        // SAFETY: two readable float channels, thread-local common data.
        let expected=unsafe { oracle_damage_armor(values[0],armor.as_ptr(),values[3]) };
        exact(subtract_armor(values[0],armor,values[3]),expected);
        // SAFETY: scalar ABI, no global mutation.
        let expected=unsafe { oracle_damage_tilt_timer(timers[0],values[0],values[1],values[2]) };
        assert_eq!(tilt_timer(timers[0],values[0],values[1],values[2]),expected);
    }

    #[test]
    fn generated_angles_match(
        angle in prop_oneof![Just(361_i32), any::<i32>()], kb in -1000_f32..1000_f32,
        air in any::<bool>(), values in prop::array::uniform4(-100_f32..100_f32),
        bounds in any::<[u32;2]>(), timer in any::<i32>(), flags in any::<[u8;2]>()
    ) { angle_case(angle,kb,air,values,bounds,timer,flags); }

    #[test]
    fn generated_accumulation_matches(bits in any::<[u32;4]>(), since_hit in any::<i32>(), window in any::<i32>()) {
        merge_case(bits.map(f32::from_bits),since_hit,window);
    }

    #[test]
    fn generated_di_and_decay_match(
        velocity in prop::array::uniform2(-1000_f32..1000_f32),
        stick in prop::array::uniform2(-1_f32..1_f32), max_angle in 0_f32..90_f32,
        decay in 0_f32..1000_f32,
    ) {
        let values=[velocity[0],velocity[1],stick[0],stick[1]];
        let mut expected=[0.0;2];
        // SAFETY: array lengths match the adapters; common data is thread-local.
        unsafe { oracle_damage_di(values.as_ptr(),max_angle,expected.as_mut_ptr()); }
        let scale=velocity[0].abs().max(velocity[1].abs());
        numerical(directional_influence(velocity,stick,max_angle),expected,scale);
        // SAFETY: two readable and two writable floats, no shared state.
        unsafe { oracle_damage_decay(velocity.as_ptr(),decay,expected.as_mut_ptr()); }
        numerical(decay_air_knockback(velocity,decay),expected,scale);
    }
}
