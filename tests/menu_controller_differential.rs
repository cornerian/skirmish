//! Raw digital edge mapping and menu repeat compared with complete original C.
//! Analog conditioning and HSD's ordinary repeat are outside this adapter.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use menus::{
    controller::Controllers,
    input::{PadFrame, aggregate},
};
use proptest::prelude::*;
use std::sync::Mutex;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_menu_controllers_reset();
    fn oracle_menu_controllers_poll(buttons: *const u32, output: *mut PadFrame);
}

static ORACLE_LOCK: Mutex<()> = Mutex::new(());

fn compare_trace(trace: impl IntoIterator<Item = [u32; 4]>) {
    let _guard = ORACLE_LOCK.lock().unwrap();
    let mut controllers = Controllers::default();
    // SAFETY: no pointer arguments; original C globals are protected by the lock.
    unsafe { oracle_menu_controllers_reset() };
    for (frame, buttons) in trace.into_iter().enumerate() {
        let actual = controllers.poll(buttons);
        let mut expected = [PadFrame::default(); 5];
        // SAFETY: C reads four u32 values and writes five repr(C) PadFrame values.
        // The arrays are live, disjoint, and all C global access holds the lock.
        unsafe { oracle_menu_controllers_poll(buttons.as_ptr(), expected.as_mut_ptr()) };
        assert_eq!(
            actual,
            expected[..4],
            "frame {frame}, buttons {buttons:#x?}"
        );
        assert_eq!(aggregate(&actual), expected[4], "aggregate frame {frame}");
    }
}

#[test]
fn every_raw_bit_and_chord_repeats_releases_and_represses_like_c() {
    let trace = (0..32).flat_map(|bit| {
        let mask = 1_u32 << bit;
        let held = [mask, !mask, u32::MAX, 0];
        let released = [0, !mask, mask, 0];
        std::iter::repeat_n(held, 130)
            .chain(std::iter::repeat_n(released, 130))
            .chain(std::iter::repeat_n(held, 30))
    });
    compare_trace(trace);
}

#[test]
fn overlapping_stick_button_aliases_and_confirm_chords_match_c() {
    let right = 1 << 1;
    let stick_right = 1 << 19;
    let a = 1 << 8;
    let start = 1 << 12;
    let shoulders = (1 << 5) | (1 << 6);
    let trace = [
        [right, a, shoulders, 0],
        [right | stick_right, a | start, shoulders | start, 0],
        [stick_right, start, shoulders | start | a, 0],
        [0, 0, shoulders | a, 0],
        [right, a, start | a, 0],
        [right | stick_right, a | start, shoulders | start | a, 0],
    ]
    .into_iter()
    .flat_map(|buttons| std::iter::repeat_n(buttons, 130));
    compare_trace(trace);
}

#[test]
fn controller_adapter_declarations_and_masks_match_original_headers() {
    let adapter = include_str!("oracle/menu_controller.c");
    for (marker, original) in [
        ("PAD", include_str!("oracle/original/dolphin_pad.h")),
        (
            "STRUCT",
            include_str!("oracle/original/gm_controller.static.h"),
        ),
    ] {
        let body = adapter
            .split_once(&format!("/* BEGIN {marker} SNAPSHOT */\n"))
            .unwrap()
            .1
            .split_once(&format!("/* END {marker} SNAPSHOT */"))
            .unwrap()
            .0;
        assert!(
            original.contains(body),
            "{marker} differs from pinned header"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_four_port_holds_match_original_c(
        periods in prop::collection::vec((prop::array::uniform4(any::<u32>()), 1..=140_usize), 1..48),
    ) {
        compare_trace(periods.into_iter().flat_map(|(buttons, polls)| std::iter::repeat_n(buttons, polls)));
    }
}
