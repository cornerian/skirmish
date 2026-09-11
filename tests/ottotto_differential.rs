//! Ottotto/OttottoWait dispatch checked against the pinned `ftCo_Ottotto.c`
//! (`tests/oracle/ottotto.c`, `tests/oracle/original/ottotto.c`): the
//! complete `ftCo_Ottotto_IASA` call order and the shared
//! `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll` fall/exit decision.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::edge::exit_distance_exceeded;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_ottotto_entry_gate(edge_set: i32, sandbag: i32, out_motion: *mut i32) -> i32;
    fn oracle_ottotto_iasa(
        scripted: u32,
        out_calls: *mut u8,
        out_count: *mut u8,
        out_fired: *mut u8,
    );
    fn oracle_ottotto_coll(
        is_wait_variant: i32,
        ground_ok: i32,
        facing_dir: f32,
        cur_x: f32,
        edge_x: f32,
        x478: f32,
        x47c: f32,
    ) -> i32;
    fn oracle_ottotto_enter(ottotto_motion: *mut i32, ottotto_wait_motion: *mut i32);
}

/// `CALL_*` codes from `tests/oracle/ottotto.c`, in `ftCo_Ottotto_IASA`'s
/// exact pinned order.
const ORDER: [u8; 19] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];

/// A `RETURN_IF` chain: visit each check in order, stopping at the first
/// one whose scripted bit is set.
fn expected_trace(answers: u32) -> (Vec<u8>, u8) {
    let mut calls = Vec::new();
    for &code in &ORDER {
        calls.push(code);
        if answers & (1 << code) != 0 {
            return (calls, code);
        }
    }
    (calls, 0)
}

fn compare_iasa(answers: u32) {
    let (expected_calls, expected_fired) = expected_trace(answers);
    let mut out_calls = [0u8; 16];
    let mut out_count = 0u8;
    let mut out_fired = 0u8;
    // SAFETY: the adapter owns all C state; every out-pointer is a live buffer.
    unsafe {
        oracle_ottotto_iasa(
            answers,
            out_calls.as_mut_ptr(),
            &mut out_count,
            &mut out_fired,
        );
    }
    assert_eq!(&out_calls[..out_count as usize], &expected_calls[..]);
    assert_eq!(out_fired, expected_fired);
}

/// `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`: ground lost falls; otherwise
/// the shared exit-distance check enters Wait; otherwise it stays.
fn expected_coll(ground_ok: bool, cur_x: f32, edge_x: f32, x478: f32, x47c: f32) -> i32 {
    if !ground_ok {
        1
    } else if exit_distance_exceeded(cur_x, edge_x, x478, x47c) {
        2
    } else {
        0
    }
}

fn compare_coll(
    is_wait_variant: bool,
    ground_ok: bool,
    facing: f32,
    cur_x: f32,
    edge_x: f32,
    x478: f32,
    x47c: f32,
) {
    let expected = expected_coll(ground_ok, cur_x, edge_x, x478, x47c);
    // SAFETY: the adapter owns all C state; no pointers are passed.
    let actual = unsafe {
        oracle_ottotto_coll(
            i32::from(is_wait_variant),
            i32::from(ground_ok),
            facing,
            cur_x,
            edge_x,
            x478,
            x47c,
        )
    };
    assert_eq!(actual, expected);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_iasa_answers_match_the_pinned_dispatch_order(answers in any::<u32>()) {
        compare_iasa(answers);
    }

    #[test]
    fn arbitrary_coll_inputs_match_the_pinned_fall_exit_decision(
        is_wait_variant in any::<bool>(),
        ground_ok in any::<bool>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        cur_x in any::<u32>().prop_map(f32::from_bits),
        edge_x in any::<u32>().prop_map(f32::from_bits),
        x478 in any::<u32>().prop_map(f32::from_bits),
        x47c in any::<u32>().prop_map(f32::from_bits),
    ) {
        compare_coll(is_wait_variant, ground_ok, facing, cur_x, edge_x, x478, x47c);
    }
}

#[test]
fn entry_gate_exact_boundaries() {
    // ftCo_8009A3C8: Collide_Edge set and no Sandbag exemption enters
    // Ottotto (motion 245); either condition failing does not. The Sandbag
    // exemption (`x2228_b2`) itself is not modeled (no such character
    // exists in this profile).
    let mut motion = 0;
    // SAFETY: the adapter owns all C state; the out-pointer is a live integer.
    unsafe {
        assert_eq!(oracle_ottotto_entry_gate(1, 0, &mut motion), 1);
        assert_eq!(motion, 245);
        motion = 0;
        assert_eq!(oracle_ottotto_entry_gate(0, 0, &mut motion), 0);
        assert_eq!(motion, 0);
        motion = 0;
        assert_eq!(oracle_ottotto_entry_gate(1, 1, &mut motion), 0);
        assert_eq!(motion, 0);
    }
}

#[test]
fn coll_exact_boundaries() {
    for is_wait_variant in [false, true] {
        compare_coll(is_wait_variant, false, 1.0, 0.0, 0.0, 5.0, 1.0);
        compare_coll(is_wait_variant, true, 1.0, 6.0, 0.0, 5.0, 1.0);
        compare_coll(is_wait_variant, true, 1.0, 6.0001, 0.0, 5.0, 1.0);
        compare_coll(is_wait_variant, true, -1.0, -6.0001, 0.0, 5.0, 1.0);
        compare_coll(is_wait_variant, true, f32::NAN, f32::NAN, 0.0, 5.0, 1.0);
    }
}

#[test]
fn entry_motion_ids_match_slippi_states() {
    let (mut ottotto, mut ottotto_wait) = (0, 0);
    // SAFETY: the adapter owns all C state; both out-pointers are live.
    unsafe { oracle_ottotto_enter(&mut ottotto, &mut ottotto_wait) };
    assert_eq!(ottotto, 245);
    assert_eq!(ottotto_wait, 246);
}

#[test]
fn adapter_retains_every_pinned_function() {
    let source = include_str!("oracle/original/ottotto.c");
    for header in [
        "bool ftCo_8009A3C8(Fighter_GObj* gobj)",
        "void ftCo_8009A410(Fighter_GObj* gobj)",
        "void ftCo_Ottotto_IASA(Fighter_GObj* gobj)",
        "void ftCo_Ottotto_Coll(Fighter_GObj* gobj)",
        "void ftCo_8009A6B8(Fighter_GObj* gobj)",
        "void ftCo_OttottoWait_Coll(Fighter_GObj* gobj)",
    ] {
        assert!(source.contains(header), "{header}");
    }
    assert!(include_str!("oracle/ottotto.c").contains("#include \"ottotto_original.inc\""));
}
