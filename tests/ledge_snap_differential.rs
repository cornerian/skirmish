//! Differential coverage for `fighter::ledge::snap_catch_left`/`snap_catch_right`
//! against the pinned `mpColl_80044164`/`mpColl_800443C4` (`mp/mpcoll.c`), via
//! `tests/oracle/ledge_snap.c`. See that adapter's header comment for why the
//! floor-database box search and the `mpCheckMultiple` obstruction checks are
//! stubbed rather than ported: `game::ledge::scan` already knows it is
//! testing one single, isolated, disconnected ledge endpoint, so the box
//! search this differential forces (a floor found exactly at the tested
//! endpoint, with no obstruction) is the only scenario this codebase's ledge
//! model can produce.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::ledge::{box_left, box_right, snap_catch_left, snap_catch_right};

unsafe extern "C" {
    fn oracle_ledge_snap(
        side: i32,
        cur_x: f32,
        cur_y: f32,
        prev_x: f32,
        prev_y: f32,
        ecb_right_x: f32,
        ecb_left_x: f32,
        ecb_bottom_x: f32,
        ecb_bottom_y: f32,
        ecb_top_x: f32,
        ecb_top_y: f32,
        snap_x: f32,
        snap_y: f32,
        snap_height: f32,
        ledge_id: i32,
        contact_x: f32,
        contact_y: f32,
        edge_x: f32,
        edge_y: f32,
        box_out: *mut f32,
    ) -> u64;
}

fn bits_eq(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[allow(clippy::too_many_arguments)]
fn call(
    side: i32,
    cur: [f32; 2],
    prev: [f32; 2],
    ecb_right_x: f32,
    ecb_left_x: f32,
    ecb_bottom: [f32; 2],
    snap: [f32; 3],
    ledge_id: i32,
    contact: [f32; 2],
    edge: [f32; 2],
) -> (bool, [f32; 4]) {
    let mut boxed = [0.0f32; 4];
    let packed = unsafe {
        oracle_ledge_snap(
            side,
            cur[0],
            cur[1],
            prev[0],
            prev[1],
            ecb_right_x,
            ecb_left_x,
            ecb_bottom[0],
            ecb_bottom[1],
            0.0,
            0.0,
            snap[0],
            snap[1],
            snap[2],
            ledge_id,
            contact[0],
            contact[1],
            edge[0],
            edge[1],
            boxed.as_mut_ptr(),
        )
    };
    (packed as u32 != 0, boxed)
}

proptest! {
    /// Pure box construction, independent of the stubbed floor search: safe
    /// over the full `f32` domain because both sides run the identical
    /// arithmetic in the identical grouping (no artificial substitution).
    #[test]
    fn snap_box_left_matches_the_pinned_bounds(
        cur_x in any::<f32>(), cur_y in any::<f32>(),
        prev_x in any::<f32>(), prev_y in any::<f32>(),
        ecb_right_x in any::<f32>(),
        snap_x in any::<f32>(), snap_y in any::<f32>(), snap_height in any::<f32>(),
    ) {
        let (_, original) = call(
            0,
            [cur_x, cur_y],
            [prev_x, prev_y],
            ecb_right_x,
            0.0,
            [0.0, 0.0],
            [snap_x, snap_y, snap_height],
            -1,
            [0.0, 0.0],
            [0.0, 0.0],
        );
        let rust = box_left([cur_x, cur_y], [prev_x, prev_y], ecb_right_x, snap_x, snap_y, snap_height);
        for i in 0..4 {
            prop_assert!(bits_eq(original[i], rust[i]), "index {i}: {original:?} vs {rust:?}");
        }
    }

    #[test]
    fn snap_box_right_matches_the_pinned_bounds(
        cur_x in any::<f32>(), cur_y in any::<f32>(),
        prev_x in any::<f32>(), prev_y in any::<f32>(),
        ecb_left_x in any::<f32>(),
        snap_x in any::<f32>(), snap_y in any::<f32>(), snap_height in any::<f32>(),
    ) {
        let (_, original) = call(
            1,
            [cur_x, cur_y],
            [prev_x, prev_y],
            0.0,
            ecb_left_x,
            [0.0, 0.0],
            [snap_x, snap_y, snap_height],
            -1,
            [0.0, 0.0],
            [0.0, 0.0],
        );
        let rust = box_right([cur_x, cur_y], [prev_x, prev_y], ecb_left_x, snap_x, snap_y, snap_height);
        for i in 0..4 {
            prop_assert!(bits_eq(original[i], rust[i]), "index {i}: {original:?} vs {rust:?}");
        }
    }

    /// The post-box predicate (edge/ECB-bottom threshold checks), under the
    /// forced "floor found exactly at the tested endpoint, no obstruction"
    /// scenario `game::ledge::scan`'s model always produces. `contact` is
    /// pinned equal to `edge`, so the source's own `contact.x - edge.x <
    /// 5.0F` tolerance term is exercised at its always-true zero-distance
    /// case; a bounded, finite domain (matching this profile's own resource
    /// validation range) avoids the unrelated `inf - inf = NaN` artifact
    /// that pinning contact to edge would otherwise introduce right at
    /// infinity.
    #[test]
    fn snap_catch_left_matches_the_pinned_function_when_the_tip_is_found(
        cur_x in -1.0e6f32..=1.0e6,
        cur_y in -1.0e6f32..=1.0e6,
        ecb_bottom_x in -1.0e6f32..=1.0e6,
        ecb_bottom_y in -1.0e6f32..=1.0e6,
        edge_x in -1.0e6f32..=1.0e6,
        edge_y in -1.0e6f32..=1.0e6,
    ) {
        // A very-left previous position and a very-wide ecb_right_x/
        // snap_height force this candidate's own box to always contain the
        // pinned edge, regardless of cur_x/cur_y within the bounded domain
        // above -- isolating the two direct threshold checks below (already
        // exercised, since ecb_bottom/edge are free) from the box arithmetic
        // its own differential above already covers independently.
        let previous = [-1.0e7, cur_y];
        let (original, _) = call(
            0,
            [cur_x, cur_y],
            previous,
            0.0,
            0.0,
            [ecb_bottom_x, ecb_bottom_y],
            [0.0, 0.0, 0.0],
            0,
            [edge_x, edge_y],
            [edge_x, edge_y],
        );
        let rust = snap_catch_left(
            [cur_x, cur_y],
            previous,
            1.0e7,
            [ecb_bottom_x, ecb_bottom_y],
            0.0,
            0.0,
            1.0e7,
            [edge_x, edge_y],
        );
        prop_assert_eq!(rust, original);
    }

    #[test]
    fn snap_catch_right_matches_the_pinned_function_when_the_tip_is_found(
        cur_x in -1.0e6f32..=1.0e6,
        cur_y in -1.0e6f32..=1.0e6,
        ecb_bottom_x in -1.0e6f32..=1.0e6,
        ecb_bottom_y in -1.0e6f32..=1.0e6,
        edge_x in -1.0e6f32..=1.0e6,
        edge_y in -1.0e6f32..=1.0e6,
    ) {
        let previous = [1.0e7, cur_y];
        let (original, _) = call(
            1,
            [cur_x, cur_y],
            previous,
            0.0,
            -1.0e7,
            [ecb_bottom_x, ecb_bottom_y],
            [0.0, 0.0, 0.0],
            0,
            [edge_x, edge_y],
            [edge_x, edge_y],
        );
        let rust = snap_catch_right(
            [cur_x, cur_y],
            previous,
            -1.0e7,
            [ecb_bottom_x, ecb_bottom_y],
            0.0,
            0.0,
            1.0e7,
            [edge_x, edge_y],
        );
        prop_assert_eq!(rust, original);
    }
}

#[test]
fn no_ledge_found_never_catches() {
    let (original, _) = call(
        0,
        [0.0, 0.0],
        [0.0, 0.0],
        1.0e6,
        0.0,
        [-6.0, -30.0],
        [10.0, 4.0, 8.0],
        -1,
        [0.0, 0.0],
        [0.0, 0.0],
    );
    assert!(!original);
}

#[test]
fn adapter_uses_the_complete_pinned_definitions() {
    let original = include_str!("oracle/original/mpcoll.c");
    assert!(original.contains("bool mpColl_80044164(CollData* cd, int* p_ledge_id)\n{"));
    assert!(original.contains("bool mpColl_800443C4(CollData* cd, int* p_ledge_id)\n{"));
    let adapter = include_str!("oracle/ledge_snap.c");
    assert!(adapter.contains("#include \"ledge_snap_original.inc\""));
}
