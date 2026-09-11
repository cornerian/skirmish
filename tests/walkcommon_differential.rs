//! Walk-kind selection, animation-rate selection and the retype start-frame
//! remap (`ftwalkcommon.c`, `tests/oracle/walkcommon.c`,
//! `tests/oracle/original/walkcommon.c`) versus the pure
//! `skirmish::fighter::locomotion` helpers. `walk_kind`/`walk_animation_rate`
//! are pure chained float comparisons/arithmetic and compared over the full
//! binary32 domain, including NaN; `walk_retype_frame`'s C float-to-`s32`
//! truncation is undefined for non-finite/out-of-range inputs (Rust's `as
//! i32` saturates instead), so its differential is scoped to the finite,
//! positive frame/length domain this codebase ever calls it with (validated
//! figatree lengths and a live `cur_anim_frame`).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::locomotion::{WalkKind, walk_animation_rate, walk_kind, walk_retype_frame};

#[repr(C)]
struct WalkRetypeResult {
    changed: i32,
    new_kind: i32,
    start_frame: i32,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_walk_type(gr_vel: f32, accel_mul: f32, middle: f32, fast: f32, walk_max: f32) -> i32;
    fn oracle_walk_rate(
        gr_vel: f32,
        facing: f32,
        kind: i32,
        rates: *const f32,
        x0: f32,
        friction_mul: f32,
    ) -> f32;
    fn oracle_walk_retype(
        cur_kind: i32,
        cur_frame: f32,
        cur_len: f32,
        lengths: *const f32,
        gr_vel: f32,
        accel_mul: f32,
        middle_threshold: f32,
        fast_threshold: f32,
        walk_max: f32,
    ) -> WalkRetypeResult;
}

fn exact(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} != NaN");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn kind_of(n: i32) -> WalkKind {
    match n {
        0 => WalkKind::Slow,
        1 => WalkKind::Middle,
        _ => WalkKind::Fast,
    }
}

fn compare_type(gr_vel: f32, accel_mul: f32, middle: f32, fast: f32, walk_max: f32) {
    let expected = walk_kind(gr_vel, accel_mul, middle, fast, walk_max);
    // SAFETY: five scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe { oracle_walk_type(gr_vel, accel_mul, middle, fast, walk_max) };
    assert_eq!(
        kind_of(actual),
        expected,
        "gr_vel={gr_vel} accel_mul={accel_mul}"
    );
}

fn compare_rate(gr_vel: f32, facing: f32, kind: i32, rates: [f32; 3], x0: f32, friction_mul: f32) {
    let expected = walk_animation_rate(gr_vel, facing, kind_of(kind), rates, x0, friction_mul);
    // SAFETY: scalar inputs and a 3-element array borrowed for the call only.
    let actual =
        unsafe { oracle_walk_rate(gr_vel, facing, kind, rates.as_ptr(), x0, friction_mul) };
    exact(actual, expected);
}

// Fixed thresholds/velocities that deterministically select each target
// kind, so every (cur_kind, new_kind) pair -- including a same-kind no-op
// -- is reachable independent of the frame/length values under test.
const RETYPE_MIDDLE: f32 = 0.4;
const RETYPE_FAST: f32 = 0.8;
const RETYPE_WALK_MAX: f32 = 10.0;
const RETYPE_VELOCITY: [f32; 3] = [0.0, 4.0, 8.0];

fn compare_retype(cur_kind: i32, new_kind: i32, cur_frame: f32, cur_len: f32, lengths: [f32; 3]) {
    // SAFETY: scalar inputs and a 3-element array borrowed for the call only.
    let result = unsafe {
        oracle_walk_retype(
            cur_kind,
            cur_frame,
            cur_len,
            lengths.as_ptr(),
            RETYPE_VELOCITY[new_kind as usize],
            1.0,
            RETYPE_MIDDLE,
            RETYPE_FAST,
            RETYPE_WALK_MAX,
        )
    };
    assert_eq!(result.new_kind, new_kind);
    assert_eq!(result.changed != 0, new_kind != cur_kind);
    if result.changed == 0 {
        return;
    }
    let expected = walk_retype_frame(cur_frame, cur_len, lengths[new_kind as usize]);
    assert_eq!(
        result.start_frame, expected,
        "cur_kind={cur_kind} new_kind={new_kind} cur_frame={cur_frame} cur_len={cur_len} lengths={lengths:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_walk_type(
        gr_vel in any::<u32>().prop_map(f32::from_bits),
        accel_mul in any::<u32>().prop_map(f32::from_bits),
        middle in any::<u32>().prop_map(f32::from_bits),
        fast in any::<u32>().prop_map(f32::from_bits),
        walk_max in any::<u32>().prop_map(f32::from_bits),
    ) {
        compare_type(gr_vel, accel_mul, middle, fast, walk_max);
    }

    #[test]
    fn arbitrary_walk_rate(
        gr_vel in any::<u32>().prop_map(f32::from_bits),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        kind in 0..3i32,
        r0 in any::<u32>().prop_map(f32::from_bits),
        r1 in any::<u32>().prop_map(f32::from_bits),
        r2 in any::<u32>().prop_map(f32::from_bits),
        x0 in any::<u32>().prop_map(f32::from_bits),
        friction_mul in any::<u32>().prop_map(f32::from_bits),
    ) {
        compare_rate(gr_vel, facing, kind, [r0, r1, r2], x0, friction_mul);
    }

    #[test]
    fn arbitrary_walk_retype(
        cur_kind in 0..3i32,
        new_kind in 0..3i32,
        cur_frame in 0.0f32..2000.0,
        cur_len in 1.0f32..2000.0,
        l0 in 1.0f32..2000.0,
        l1 in 1.0f32..2000.0,
        l2 in 1.0f32..2000.0,
    ) {
        compare_retype(cur_kind, new_kind, cur_frame, cur_len, [l0, l1, l2]);
    }
}

#[test]
fn exact_boundaries() {
    // gr_vel exactly at each threshold, and negative velocity.
    for accel_mul in [1.0, 2.0] {
        compare_type(4.0 * accel_mul, accel_mul, 0.4, 0.8, 10.0);
        compare_type(3.999_f32 * accel_mul, accel_mul, 0.4, 0.8, 10.0);
        compare_type(8.0 * accel_mul, accel_mul, 0.4, 0.8, 10.0);
        compare_type(7.999_f32 * accel_mul, accel_mul, 0.4, 0.8, 10.0);
        compare_type(-8.0 * accel_mul, accel_mul, 0.4, 0.8, 10.0);
    }
    compare_type(f32::NAN, 1.0, 0.4, 0.8, 10.0);
    compare_type(f32::INFINITY, 1.0, f32::NEG_INFINITY, 0.8, 10.0);

    for kind in 0..3 {
        // v * facing exactly zero, and exactly negative, are both rate 0.
        compare_rate(0.0, 1.0, kind, [4.0, 8.0, 12.0], 2.0, 1.0);
        compare_rate(-4.0, 1.0, kind, [4.0, 8.0, 12.0], 2.0, 1.0);
        compare_rate(4.0, -1.0, kind, [4.0, 8.0, 12.0], 2.0, 1.0);
        compare_rate(f32::NAN, 1.0, kind, [4.0, 8.0, 12.0], 2.0, 1.0);
        compare_rate(4.0, 1.0, kind, [4.0, 8.0, 12.0], f32::NAN, 0.5);
    }

    for cur_kind in 0..3 {
        for new_kind in 0..3 {
            for lengths in [[10.0, 12.0, 14.0], [14.0, 10.0, 12.0]] {
                let length = lengths[cur_kind as usize];
                // Frame exactly at the length (wraps to 0 before remapping).
                compare_retype(cur_kind, new_kind, length, length, lengths);
                compare_retype(cur_kind, new_kind, 0.0, length, lengths);
                compare_retype(cur_kind, new_kind, length - 1.0, length, lengths);
            }
        }
    }
}

#[test]
fn adapter_retains_the_complete_pinned_functions() {
    let source = include_str!("oracle/original/walkcommon.c");
    for signature in [
        "FtWalkType ftWalkCommon_GetWalkType(HSD_GObj* gobj)",
        "bool ftWalkCommon_800DFC70(HSD_GObj* gobj)",
        "void ftWalkCommon_800DFCA4(",
        "void ftWalkCommon_800DFDDC(HSD_GObj* gobj)",
        "void ftWalkCommon_800DFEC8(",
    ] {
        assert!(source.contains(signature), "missing {signature}");
    }
    assert!(include_str!("oracle/walkcommon.c").contains("#include \"walkcommon_original.inc\""));
}
