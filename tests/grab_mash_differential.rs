//! Complete pinned `ftCommon_GrabMash` versus the safe Rust state transition.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::grab::{MashState, mash};

unsafe extern "C" {
    fn oracle_grab_mash(
        timer: *mut f32,
        axes: *mut i8,
        shake: *mut u8,
        pressed: u32,
        stick_x: f32,
        stick_y: f32,
        penalty: f32,
        threshold: f32,
    ) -> i32;
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_timer_inputs_and_latched_state_match_original(
        timer_bits in any::<u32>(), axes in any::<[i8; 2]>(),
        enabled in any::<bool>(), shaking in any::<bool>(),
        frame in any::<u8>(), frames in any::<u8>(), pressed in any::<u32>(),
        stick_bits in any::<[u32; 2]>(), penalty_bits in any::<u32>(),
        threshold_bits in any::<u32>(),
    ) {
        let mut expected_timer = f32::from_bits(timer_bits);
        let mut expected_axes = axes;
        let mut expected_shake = [u8::from(enabled), u8::from(shaking), frame, frames];
        let stick = stick_bits.map(f32::from_bits);
        let penalty = f32::from_bits(penalty_bits);
        let threshold = f32::from_bits(threshold_bits);
        // SAFETY: every pointer addresses initialized writable scalar storage;
        // the adapter resets all original globals read by the selected body.
        let expected = unsafe {
            oracle_grab_mash(
                &mut expected_timer,
                expected_axes.as_mut_ptr(),
                expected_shake.as_mut_ptr(),
                pressed,
                stick[0],
                stick[1],
                penalty,
                threshold,
            ) != 0
        };
        let mut actual_timer = f32::from_bits(timer_bits);
        let mut actual_state = MashState {
            axes,
            shake_enabled: enabled,
            shaking,
            shake_frame: frame,
            shake_frames: frames,
        };
        let actual = mash(
            &mut actual_timer,
            &mut actual_state,
            pressed,
            stick,
            penalty,
            threshold,
        );
        prop_assert_eq!(actual, expected);
        // NaN payload propagation through the timer's `-= penalty` arithmetic
        // is unspecified, so two NaN results are equivalent for parity
        // purposes even when their bit patterns differ.
        if expected_timer.is_nan() {
            prop_assert!(actual_timer.is_nan());
        } else {
            prop_assert_eq!(actual_timer.to_bits(), expected_timer.to_bits());
        }
        prop_assert_eq!(actual_state.axes, expected_axes);
        prop_assert_eq!(u8::from(actual_state.shake_enabled), expected_shake[0]);
        prop_assert_eq!(u8::from(actual_state.shaking), expected_shake[1]);
        prop_assert_eq!(actual_state.shake_frame, expected_shake[2]);
        prop_assert_eq!(actual_state.shake_frames, expected_shake[3]);
    }
}

#[test]
fn adapter_selects_the_complete_pinned_function() {
    let original = include_str!("oracle/original/ftcommon.c");
    assert!(original.contains("bool ftCommon_GrabMash(Fighter* fp, float arg1)\n{"));
    assert!(include_str!("oracle/grab_mash.c").contains("#include \"grab_mash_original.inc\""));
}
