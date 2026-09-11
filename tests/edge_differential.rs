//! Floor-end clamp/teeter decision checked against the pinned
//! `mpColl_8004A45C_Floor` (mode 2) and `mpColl_8004A678_Floor` (mode 1)
//! callbacks (`tests/oracle/edge_floor.c`, aliasing the ordinary `mpcoll`
//! snapshot per `tests/oracle/adapters.json`).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::edge::{EdgeQuery, Mode, Side, clamped_position, resolve};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_edge_floor(
        mode: i32,
        cur_x: f32,
        cur_y: f32,
        cur_z: f32,
        ecb_bottom_x: f32,
        ecb_bottom_y: f32,
        ecb_left_x: f32,
        ecb_left_y: f32,
        ecb_right_x: f32,
        ecb_right_y: f32,
        edge_left_x: f32,
        edge_left_y: f32,
        edge_right_x: f32,
        edge_right_y: f32,
        facing_dir: i32,
        lstick_x: f32,
        wall_blocked: i32,
        out_pos: *mut f32,
        out_flags: *mut u32,
    ) -> i32;
}

const COLLIDE_LEFT_EDGE: u32 = 1 << 0;
const COLLIDE_RIGHT_EDGE: u32 = 1 << 1;
const COLLIDE_EDGE: u32 = 1 << 2;

#[allow(clippy::too_many_arguments)]
fn compare(
    mode: i32,
    cur: [f32; 3],
    ecb_bottom: [f32; 2],
    ecb_left: [f32; 2],
    ecb_right: [f32; 2],
    edge_left: [f32; 2],
    edge_right: [f32; 2],
    facing: f32,
    stick_x: f32,
    wall_blocked: bool,
) {
    let bottom_x = cur[0] + ecb_bottom[0];
    let query = EdgeQuery {
        bottom_x,
        left_x: edge_left[0],
        right_x: edge_right[0],
        facing,
        stick_x,
        stick_limit: 0.75,
    };
    let math_mode = if mode == 1 { Mode::Teeter } else { Mode::Clamp };
    let actual = resolve(math_mode, query, wall_blocked);

    let mut out_pos = [0.0f32; 2];
    let mut out_flags = 0u32;
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    let expected = unsafe {
        oracle_edge_floor(
            mode,
            cur[0],
            cur[1],
            cur[2],
            ecb_bottom[0],
            ecb_bottom[1],
            ecb_left[0],
            ecb_left[1],
            ecb_right[0],
            ecb_right[1],
            edge_left[0],
            edge_left[1],
            edge_right[0],
            edge_right[1],
            // Real facing_dir is a signed int (-1/0/1); the source only ever
            // compares it against exactly -1/1, so any nonzero sign suffices.
            if facing < 0.0 { -1 } else { 1 },
            stick_x,
            i32::from(wall_blocked),
            out_pos.as_mut_ptr(),
            &mut out_flags,
        )
    };
    assert_eq!(actual.is_some(), expected != 0, "on_edge mismatch");
    if let Some(resolution) = actual {
        let edge = if resolution.side == Side::Left {
            edge_left
        } else {
            edge_right
        };
        assert_eq!(out_pos, clamped_position(edge, ecb_bottom), "position");
        let expected_flag = match (mode, resolution.side) {
            (2, Side::Left) => COLLIDE_RIGHT_EDGE,
            (2, Side::Right) => COLLIDE_LEFT_EDGE,
            (1, _) => COLLIDE_EDGE,
            _ => unreachable!(),
        };
        assert_ne!(out_flags & expected_flag, 0, "flags");
    }
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_floor_end_inputs_match_the_pinned_callbacks(
        mode in prop_oneof![Just(1i32), Just(2i32)],
        cur_x in any::<u32>().prop_map(f32::from_bits),
        left_x in any::<u32>().prop_map(f32::from_bits),
        right_x in any::<u32>().prop_map(f32::from_bits),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        stick_x in any::<u32>().prop_map(f32::from_bits),
        wall_blocked in any::<bool>(),
    ) {
        // The pure decision only reads the X coordinate of every point; Y/Z
        // and the left/right ECB offsets are exercised by the exact
        // boundary cases below (their arithmetic is a straight subtraction,
        // already covered independently by `clamped_position`'s own unit
        // tests). NaN left/right endpoints would make every comparison
        // false in both languages identically; clamp them finite here so
        // `left_x <= right_x` is meaningful for a realistic floor.
        // ecb.bottom.x is always exactly 0.0: every ECB loader in
        // `src/collision/ecb.rs` (`load_fixed`, `load_joints`, `load_external`)
        // forces `bottom: [0.0, ...]`, matching the source's own convention
        // that the ECB bottom vertex sits directly below the pivot -- this is
        // the precondition `coll->cur_pos.x` (raw fighter position, not
        // ECB-adjusted) and Skirmish's `position[0] + ecb.bottom[0]` agree
        // under.
        let (left_x, right_x) = (finite(left_x).min(finite(right_x)), finite(left_x).max(finite(right_x)));
        compare(
            mode,
            [cur_x, 0.0, 0.0],
            [0.0, 0.0],
            [-1.5, 1.5],
            [1.5, 1.5],
            [left_x, 0.0],
            [right_x, 0.0],
            facing,
            stick_x,
            wall_blocked,
        );
    }
}

#[test]
fn exact_boundaries_and_wall_blocking() {
    for mode in [1, 2] {
        for wall_blocked in [false, true] {
            // Exactly at each end, both sides, both facings, both the
            // stick's exact 0.75 boundary and just inside it.
            for (cur_x, facing, stick_x) in [
                (0.0, 1.0, 0.0),
                (0.0, -1.0, 0.0),
                (10.0, 1.0, 0.0),
                (10.0, -1.0, 0.0),
                (10.0, 1.0, 0.75),
                (10.0, 1.0, 0.749),
                (0.0, -1.0, -0.75),
                (0.0, -1.0, -0.749),
                (-0.001, 1.0, 0.0),
                (10.001, 1.0, 0.0),
            ] {
                compare(
                    mode,
                    [cur_x, 0.0, 0.0],
                    [0.0, 0.0],
                    [-1.5, 1.5],
                    [1.5, 1.5],
                    [0.0, 0.0],
                    [10.0, 0.0],
                    facing,
                    stick_x,
                    wall_blocked,
                );
            }
        }
    }
}

#[test]
fn adapter_retains_both_complete_pinned_functions() {
    let mpcoll = include_str!("oracle/original/mpcoll.c");
    for header in [
        "bool mpColl_8004A45C_Floor(CollData* coll, int line_id)",
        "bool mpColl_8004A678_Floor(CollData* coll, int line_id)",
    ] {
        assert!(mpcoll.contains(header), "{header}");
    }
    let adapter = include_str!("oracle/edge_floor.c");
    assert!(adapter.contains("#include \"edge_floor_original.inc\""));
}
