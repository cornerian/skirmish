//! All default stick byte pairs and trigger bytes are compared with original C.
//! Randomized tests additionally cover custom calibration and full PADStatus data.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use melee_input::{ClampRegion, DEFAULT_REGION, PadStatus, StickRegion, clamp};
use proptest::prelude::*;
use std::sync::Mutex;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_input_stick(stick: *mut i8, maximum: i8, corner: i8, deadzone: i8);
    fn oracle_input_trigger(trigger: u8, minimum: u8, maximum: u8) -> u8;
    fn oracle_input_clamp(status: *mut PadStatus, region: *const ClampRegion);
}

// Original ClampRegion is mutable global C data. Only trigger and full-status
// calls touch it; lock those calls across all tests in this integration binary.
static ORACLE_LOCK: Mutex<()> = Mutex::new(());

fn compare_stick(stick: [i8; 2], region: StickRegion) {
    if let Ok(actual) = region.clamp(stick) {
        let mut expected = stick;
        // SAFETY: expected has two live bytes. Rust evaluation has established
        // that this calibration does not encounter C integer division by zero.
        unsafe {
            oracle_input_stick(
                expected.as_mut_ptr(),
                region.maximum,
                region.corner,
                region.deadzone,
            );
        }
        assert_eq!(actual, expected, "stick {stick:?}, region {region:?}");
    }
}

fn compare_batch(initial: [PadStatus; 4], region: ClampRegion) {
    let mut actual = initial;
    if region.apply(&mut actual).is_ok() {
        let _guard = ORACLE_LOCK.lock().unwrap();
        let mut expected = initial;
        // SAFETY: repr(C) structures reproduce every scalar field and its layout,
        // arrays are live, and the previous evaluation ruled out division by zero.
        unsafe { oracle_input_clamp(expected.as_mut_ptr(), &region) };
        assert_eq!(actual, expected, "initial {initial:?}, region {region:?}");
    } else {
        assert_eq!(
            actual, initial,
            "undefined custom C calibration must not partially mutate Rust state"
        );
    }
}

fn status() -> impl Strategy<Value = PadStatus> {
    (
        any::<u16>(),
        prop::array::uniform2(any::<i8>()),
        prop::array::uniform2(any::<i8>()),
        prop::array::uniform2(any::<u8>()),
        prop::array::uniform2(any::<u8>()),
        prop_oneof![3 => Just(0_i8), 1 => any::<i8>()],
    )
        .prop_map(
            |(buttons, stick, substick, triggers, analog, error)| PadStatus {
                buttons,
                stick,
                substick,
                triggers,
                analog,
                error,
            },
        )
}

fn region(bytes: [u8; 8]) -> ClampRegion {
    ClampRegion {
        trigger_minimum: bytes[0],
        trigger_maximum: bytes[1],
        stick: StickRegion {
            deadzone: bytes[2] as i8,
            maximum: bytes[3] as i8,
            corner: bytes[4] as i8,
        },
        substick: StickRegion {
            deadzone: bytes[5] as i8,
            maximum: bytes[6] as i8,
            corner: bytes[7] as i8,
        },
    }
}

#[test]
fn both_default_sticks_match_c_for_every_signed_byte_pair() {
    for x in i8::MIN..=i8::MAX {
        for y in i8::MIN..=i8::MAX {
            for region in [DEFAULT_REGION.stick, DEFAULT_REGION.substick] {
                assert!(region.clamp([x, y]).is_ok());
                compare_stick([x, y], region);
            }
        }
    }
}

#[test]
fn default_trigger_matches_c_for_every_byte() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for trigger in u8::MIN..=u8::MAX {
        // SAFETY: all arguments are scalar bytes and the original operation is
        // defined for all of them. Shared C calibration is locked.
        let expected = unsafe { oracle_input_trigger(trigger, 30, 180) };
        assert_eq!(DEFAULT_REGION.clamp_trigger(trigger), expected);
    }
}

#[test]
fn all_trigger_calibrations_match_c_at_every_boundary() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for minimum in u8::MIN..=u8::MAX {
        for maximum in u8::MIN..=u8::MAX {
            let region = ClampRegion {
                trigger_minimum: minimum,
                trigger_maximum: maximum,
                ..DEFAULT_REGION
            };
            for trigger in [
                0,
                minimum.saturating_sub(1),
                minimum,
                minimum.saturating_add(1),
                maximum.saturating_sub(1),
                maximum,
                maximum.saturating_add(1),
                255,
            ] {
                // SAFETY: scalar input has no undefined cases; C global is locked.
                let expected = unsafe { oracle_input_trigger(trigger, minimum, maximum) };
                assert_eq!(region.clamp_trigger(trigger), expected);
            }
        }
    }
}

#[test]
fn sample_error_codes_and_unconditioned_fields_match_c() {
    assert_eq!(std::mem::size_of::<PadStatus>(), 12);
    assert_eq!(std::mem::size_of::<ClampRegion>(), 8);
    for error in i8::MIN..=i8::MAX {
        let statuses = [PadStatus {
            buttons: 0xABCD,
            stick: [-128, 127],
            substick: [127, -128],
            triggers: [255, 128],
            analog: [17, 252],
            error,
        }; 4];
        compare_batch(statuses, DEFAULT_REGION);
        let mut actual = statuses;
        clamp(&mut actual);
        if error != 0 {
            assert_eq!(actual, statuses);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn arbitrary_stick_calibration_matches_c_where_defined(
        values in prop::array::uniform5(any::<i8>()),
    ) {
        let [x, y, maximum, corner, deadzone] = values;
        compare_stick([x, y], StickRegion { maximum, corner, deadzone });
    }

    #[test]
    fn default_four_controller_batches_match_c(initial in prop::array::uniform4(status())) {
        compare_batch(initial, DEFAULT_REGION);
    }

    #[test]
    fn arbitrary_calibration_batches_match_c_where_defined(
        initial in prop::array::uniform4(status()),
        calibration in prop::array::uniform8(any::<u8>()),
    ) {
        compare_batch(initial, region(calibration));
    }

    #[test]
    fn repeated_conditioning_preserves_original_non_idempotence(
        initial in prop::array::uniform4(status()), count in 1..=8_usize,
    ) {
        let _guard = ORACLE_LOCK.lock().unwrap();
        let mut actual = initial;
        let mut expected = initial;
        for _ in 0..count {
            clamp(&mut actual);
            // SAFETY: C receives four valid repr(C) samples and known valid default
            // calibration. The original mutable global is protected by the lock.
            unsafe { oracle_input_clamp(expected.as_mut_ptr(), &DEFAULT_REGION) };
            prop_assert_eq!(actual, expected);
        }
    }
}
