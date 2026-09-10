#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::grab::capture_alignment;

unsafe extern "C" {
    fn oracle_capture_alignment(
        position: *const f32,
        holder_anchor: *const f32,
        victim_anchor: *const f32,
        threshold: f32,
        scale_y: f32,
        output: *mut f32,
    ) -> i32;
}

proptest! {
    #[test]
    fn captured_alignment_matches_original(
        position in any::<[f32; 3]>(),
        holder_anchor in any::<[f32; 3]>(),
        victim_anchor in any::<[f32; 3]>(),
        threshold in any::<f32>(),
        scale_y in any::<f32>(),
    ) {
        let mut expected = [0.0; 3];
        let lifted = unsafe {
            oracle_capture_alignment(
                position.as_ptr(),
                holder_anchor.as_ptr(),
                victim_anchor.as_ptr(),
                threshold,
                scale_y,
                expected.as_mut_ptr(),
            ) != 0
        };
        let actual = capture_alignment(
            position,
            holder_anchor,
            victim_anchor,
            threshold,
            scale_y,
        );
        prop_assert_eq!(actual.1, lifted);
        for (actual, expected) in actual.0.into_iter().zip(expected) {
            prop_assert!(
                actual.to_bits() == expected.to_bits() || actual.is_nan() && expected.is_nan(),
                "actual={actual:?} expected={expected:?}",
            );
        }
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/capture_pulled.c");
    assert!(original.contains("bool fn_800DAD18(Fighter_GObj* gobj)\n{"));
    let adapter = include_str!("oracle/capture_alignment.c");
    assert!(adapter.contains("#include \"capture_alignment_original.inc\""));
}
