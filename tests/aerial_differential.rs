//! Pure aerial arithmetic versus exact pinned C bodies; no animation graphs or
//! native match scheduling are implied by these scalar comparisons.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::fighter::aerial::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_aerial_select(values: *const f32, rules: *const f32) -> i32;
    fn oracle_aerial_fresh(values: *const f32, rules: *const f32) -> i32;
    fn oracle_aerial_angle(stick: *const f32, cstick: i32) -> f32;
    fn oracle_aerial_lag(base: f32, age: u8, window: i32, divisor: f32, direction: i32) -> f32;
    fn oracle_aerial_rate(end: f32, lag: f32) -> f32;
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
fn selection_case(values: [f32; 7], rules: [f32; 3]) {
    // SAFETY: seven input floats and three coefficients; adapters initialize
    // all thread-local common data and expose only these bounded callbacks.
    let (direction, fresh) = unsafe {
        (
            oracle_aerial_select(values.as_ptr(), rules.as_ptr()),
            oracle_aerial_fresh(values.as_ptr(), rules.as_ptr()),
        )
    };
    let main = [values[0], values[1]];
    let current = [values[2], values[3]];
    let previous = [values[4], values[5]];
    let rules = SelectionRules {
        thresholds: [rules[0], rules[1]],
        vertical_angle: rules[2],
    };
    assert_eq!(
        i32::from(fresh_cstick(previous, current, rules.thresholds)),
        fresh
    );
    assert_eq!(
        select(main, current, previous, values[6], &rules) as i32,
        direction,
        "values={values:?} rules={rules:?}"
    );
}

#[test]
fn fresh_cstick_neutral_boundaries_reversal_and_axis_priority_match() {
    let almost = f32::from_bits(0.5_f32.to_bits() - 1);
    for main in [
        [0.0, 0.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 1.0],
    ] {
        for current in [
            [0.0, 0.0],
            [almost, 0.0],
            [0.5, 0.0],
            [-0.5, 0.0],
            [0.5, 0.5],
            [0.0, -0.5],
        ] {
            for previous in [[0.0, 0.0], [0.5, 0.0], [-0.5, 0.0], [0.5, 0.5]] {
                for facing in [-1.0, 1.0] {
                    selection_case(
                        [
                            main[0],
                            main[1],
                            current[0],
                            current[1],
                            previous[0],
                            previous[1],
                            facing,
                        ],
                        [0.5, 0.5, core::f32::consts::FRAC_PI_4],
                    );
                }
            }
        }
    }
    assert!(!fresh_cstick([1.0, 0.0], [-1.0, 0.0], [0.5; 2]));
    assert!(fresh_cstick([1.0, 0.0], [1.0, 0.5], [0.5; 2]));
    assert!(!fresh_cstick([0.5, 0.0], [1.0, 0.0], [0.5; 2]));
}

#[test]
fn folded_horizontal_angles_and_signed_zeros_match_source() {
    for stick in [
        [-1.0, 1.0],
        [1.0, 1.0],
        [-1.0, -1.0],
        [0.0, 0.0],
        [-0.0, 0.0],
        [0.0, -0.0],
        [-0.0, -0.0],
        [f32::NAN, 0.0],
        [f32::INFINITY, 1.0],
        [f32::NEG_INFINITY, -1.0],
    ] {
        for kind in [0, 1] {
            // SAFETY: two readable floats; both source angle helpers are called.
            let expected = unsafe { oracle_aerial_angle(stick.as_ptr(), kind) };
            exact(stick_angle(stick), expected);
        }
        selection_case(
            [stick[0], stick[1], 0.0, 0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.6],
        );
    }
    assert_eq!(stick_angle([-1.0, 1.0]), stick_angle([1.0, 1.0]));
    // The game's own `atan2f` (`lbtrigf.c`) is not the standard library
    // convention here: for `x == 0` (either sign) it returns `copysign(PI/2,
    // y)` unconditionally (its final branch copies only `y`'s sign onto a
    // bit-pattern `PI/2`), rather than distinguishing `x`'s sign to produce
    // `PI` for `atan2(+0, -0)`. Confirmed bit-exactly against the pinned
    // oracle above and by `crate::math`'s own full-domain differential
    // tests; see `docs/math.md`.
    assert_eq!(
        stick_angle([-0.0, 0.0]).to_bits(),
        core::f32::consts::FRAC_PI_2.to_bits()
    );
}

#[test]
fn l_cancel_window_is_strict_division_precedes_truncation_and_rate_uses_end_frame() {
    for base in [
        0.0, 0.5, 1.0, 2.0, 3.0, 9.0, 9.999999, 10.0, 10.000001, -1.0,
    ] {
        for age in [0, 2, 3, 254, 255] {
            for direction in 0..5 {
                // SAFETY: all divisions produce representable C int values;
                // direction is one of the five initialized landing lag branches.
                let expected = unsafe { oracle_aerial_lag(base, age, 3, 2.0, direction) };
                exact(landing_lag(base, age, 3, 2.0).unwrap(), expected);
            }
        }
    }
    assert_eq!(landing_lag(9.0, 2, 3, 2.0), Ok(4.0));
    assert_eq!(landing_lag(9.0, 3, 3, 2.0), Ok(9.0));
    assert_eq!(landing_lag(0.5, 2, 3, 2.0), Ok(1.0));
    for end in [-0.0, 0.0, 4.0, 8.5, 100.0, f32::INFINITY, f32::NAN] {
        for lag in [-0.0, 0.0, 1.0, 4.0, 9.0, f32::INFINITY, f32::NAN] {
            // SAFETY: floating division is defined for these values, including
            // infinity/NaN; transition side effects are excluded by the adapter.
            let expected = unsafe { oracle_aerial_rate(end, lag) };
            exact(landing_animation_rate(end, lag), expected);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]
    #[test]
    fn generated_selection_angles_and_landing_match(
        stick in prop::array::uniform6(-1_f32..1_f32), facing in prop_oneof![Just(-1_f32),Just(1_f32)],
        thresholds in prop::array::uniform2(0.1_f32..0.9_f32), angle in 0.1_f32..1.5_f32,
        lag in -1000_f32..1000_f32, age in any::<u8>(), window in -1_i32..257_i32,
        divisor in 0.01_f32..20_f32,end in -1000_f32..1000_f32,
    ) {
        selection_case([stick[0],stick[1],stick[2],stick[3],stick[4],stick[5],facing],[thresholds[0],thresholds[1],angle]);
        let input=[stick[0],stick[1]];
        // SAFETY: readable two-float vector and scalar kind selector.
        let expected=unsafe{oracle_aerial_angle(input.as_ptr(),0)};
        assert!((stick_angle(input)-expected).abs()<=2e-7);
        // SAFETY: bounded quotient is within the defined C integer domain.
        let expected=unsafe{oracle_aerial_lag(lag,age,window,divisor,2)};
        exact(landing_lag(lag,age,window,divisor).unwrap(),expected);
        // SAFETY: scalar ABI and thread-local capture.
        let expected=unsafe{oracle_aerial_rate(end,lag)};
        exact(landing_animation_rate(end,lag),expected);
    }
}
