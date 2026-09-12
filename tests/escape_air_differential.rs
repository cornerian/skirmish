//! Air-dodge input, launch, decay and FallSpecial platform landing checked
//! against the complete pinned C bodies.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::escape_air::{decay, launch_velocity, platform_landing};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_air_dodge_request(
        pressed: u32,
        timer: i32,
        motion: *mut i32,
        timer_out: *mut i32,
    ) -> i32;
    fn oracle_air_dodge_launch(
        stick: *const f32,
        deadzone: *const f32,
        force: f32,
        velocity: *mut f32,
    );
    fn oracle_air_dodge_decay(velocity: *const f32, decay: f32, skip: i32, out: *mut f32) -> i32;
    fn oracle_fall_special_platform_landing(
        line_id: i32,
        flags: u32,
        stick_y: f32,
        threshold: f32,
    ) -> i32;
}

fn original_launch(stick: [f32; 2], deadzone: [f32; 2], force: f32) -> ([f32; 2], i32) {
    let mut out = [0.0_f32; 3];
    // SAFETY: the adapter reads two pairs of floats and writes three floats.
    unsafe { oracle_air_dodge_launch(stick.as_ptr(), deadzone.as_ptr(), force, out.as_mut_ptr()) };
    ([out[0], out[1]], out[2] as i32)
}

/// NaN payload propagation through arithmetic is unspecified, so two NaN
/// results are equivalent for parity purposes even when their bit patterns
/// differ; a non-NaN result must still match exactly.
fn same_bits(label: &str, actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

fn compare_decay(velocity: [f32; 2], factor: f32, skip: bool) {
    let mut out = [0.0_f32; 2];
    // SAFETY: the adapter reads two floats and writes two floats.
    let fell = unsafe {
        oracle_air_dodge_decay(velocity.as_ptr(), factor, i32::from(skip), out.as_mut_ptr())
    };
    if skip {
        assert_eq!(fell, 1);
        // Unmultiplied passthrough: an exact copy, safe to compare bit-exact
        // even for a NaN velocity.
        assert_eq!(out.map(f32::to_bits), velocity.map(f32::to_bits));
    } else {
        assert_eq!(fell, 0);
        let expected = decay(velocity, factor);
        same_bits("x", out[0], expected[0]);
        same_bits("y", out[1], expected[1]);
    }
}

fn compare_platform(line_id: i32, flags: u32, stick_y: f32, threshold: f32) {
    // SAFETY: scalar arguments only.
    let expected =
        unsafe { oracle_fall_special_platform_landing(line_id, flags, stick_y, threshold) };
    assert_eq!(
        platform_landing(line_id != -1, flags & 0x100 != 0, stick_y, threshold),
        expected != 0
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn generated_launches_match_within_a_bounded_absolute_difference(
        stick in prop::array::uniform2(-1.0_f32..=1.0),
        deadzone in prop::array::uniform2(0.0_f32..=1.0),
        force in 0.0_f32..=10.0,
    ) {
        // `crate::math::atan2f`/`cosf`/`sinf` (`launch_velocity`) and this
        // adapter's own renamed pinned bodies (`tests/oracle/escape_air.c`'s
        // header comment) are the same algorithm now, closing most of the
        // gap from the old `libm`-vs-oracle tolerance this test used to
        // need -- but not all of it: `crate::math::sinf`/`cosf` are shipped
        // fused (`docs/math.md`'s fused-multiply-add finding), which this
        // adapter's pinned, `-ffp-contract=off` body is not, so a bounded
        // absolute difference (scaled by `force`) is expected and correct,
        // not a bug. A raw ULP bound is the wrong tool here for the same
        // reason `tests/math_differential.rs`'s `close_abs` is: right where
        // `cosf`/`sinf` cross zero, `force * cosf(angle)`'s ULP distance from
        // the oracle explodes even though the absolute difference stays
        // small (confirmed directly at `stick == [0.0, 0.3]`, an exact
        // `atan2f` quadrant boundary).
        let actual = launch_velocity(stick, deadzone, force);
        let (expected, motion) = original_launch(stick, deadzone, force);
        prop_assert_eq!(motion, 236);
        for axis in 0..2 {
            let diff = (actual[axis] - expected[axis]).abs();
            let bound = 2e-5 * force.max(1.0);
            prop_assert!(
                diff <= bound,
                "axis {axis}: {:?} != {:?} ({diff} > {bound})",
                actual[axis],
                expected[axis]
            );
        }
    }

    #[test]
    fn arbitrary_decays_match(velocity in prop::array::uniform2(any::<u32>()), factor in any::<u32>(), skip in any::<bool>()) {
        compare_decay(velocity.map(f32::from_bits), f32::from_bits(factor), skip);
    }

    #[test]
    fn arbitrary_platform_landings_match(line_id in any::<i32>(), flags in any::<u32>(), stick in any::<u32>(), threshold in any::<u32>()) {
        compare_platform(line_id, flags, f32::from_bits(stick), f32::from_bits(threshold));
    }

    #[test]
    fn arbitrary_requests_match(pressed in any::<u32>(), timer in any::<i32>()) {
        let (mut motion, mut timer_out) = (0, 0);
        // SAFETY: both out-pointers are live integers.
        let result = unsafe { oracle_air_dodge_request(pressed, timer, &mut motion, &mut timer_out) };
        let expected = pressed & 0x60 != 0;
        prop_assert_eq!(result != 0, expected);
        prop_assert_eq!(motion, if expected { 236 } else { 0 });
        prop_assert_eq!(timer_out, if expected { timer } else { 0 });
    }
}

#[test]
fn deadzone_zero_and_axis_boundaries_match() {
    let source = include_str!("oracle/original/escape_air.c");
    let fall = include_str!("oracle/original/fall_special.c");
    let adapter = include_str!("oracle/escape_air.c");
    assert!(source.contains("bool ftCo_80099A58(Fighter_GObj* gobj)"));
    assert!(source.contains("static inline void inlineA0(Fighter* fp)"));
    assert!(source.contains("void ftCo_80099A9C(Fighter_GObj* gobj, int timer)"));
    assert!(source.contains("void ftCo_EscapeAir_Phys(Fighter_GObj* gobj)"));
    assert!(fall.contains("bool ftCo_80096CC8(Fighter_GObj* gobj, int line_id)"));
    assert!(adapter.contains("#include \"escape_air_angle_original.inc\""));
    assert!(adapter.contains("#include \"escape_air_original.inc\""));
    for (stick, deadzone, force) in [
        ([0.2, -0.2], [0.3, 0.3], 6.0),
        ([-0.0, 0.0], [0.3, 0.3], 6.0),
        ([0.3, 0.0], [0.3, 0.3], 6.0),
        ([0.0, 0.3], [0.3, 0.3], 6.0),
        ([1.0, 0.0], [0.3, 0.3], 6.0),
        ([-1.0, 0.0], [0.3, 0.3], 6.0),
        ([f32::NAN, 0.0], [0.3, 0.3], 6.0),
        ([0.5, 0.5], [0.0, 0.0], 0.0),
        ([0.5, 0.5], [0.3, 0.3], f32::INFINITY),
    ] {
        let actual = launch_velocity(stick, deadzone, force);
        let (expected, _) = original_launch(stick, deadzone, force);
        for axis in 0..2 {
            let (a, e) = (actual[axis], expected[axis]);
            if e.is_nan() {
                assert!(
                    a.is_nan(),
                    "{stick:?} {deadzone:?} {force} axis {axis}: {a:?} != NaN"
                );
            } else if !a.is_finite() || !e.is_finite() {
                assert_eq!(
                    a.to_bits(),
                    e.to_bits(),
                    "{stick:?} {deadzone:?} {force} axis {axis}: {a:?} != {e:?}"
                );
            } else {
                // See `generated_launches_match_within_a_few_ulp`'s comment:
                // `[0.0, 0.3]` is an exact `atan2f` quadrant boundary, where
                // a bounded absolute `cosf`/`sinf` difference is expected.
                let diff = (a - e).abs();
                let bound = 2e-5 * force.max(1.0);
                assert!(
                    diff <= bound,
                    "{stick:?} {deadzone:?} {force} axis {axis}: {a:?} != {e:?} ({diff} > {bound})"
                );
            }
        }
    }
    compare_decay([2.0, -1.0], 0.5, false);
    compare_decay([f32::NAN, f32::INFINITY], -0.0, false);
    compare_decay([2.0, -1.0], 0.5, true);
    compare_platform(3, 0x100, -0.5, -0.5);
    compare_platform(3, 0x100, -0.49, -0.5);
    compare_platform(3, 0, -1.0, -0.5);
    compare_platform(-1, 0, 1.0, -0.5);
}
