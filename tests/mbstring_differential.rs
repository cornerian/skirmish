#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::mbstring::wcstombs;

unsafe extern "C" {
    fn oracle_wcstombs(dest: *mut u8, source: *const u16, max: u32) -> u32;
}

fn compare(source: &[u16], capacity: usize) {
    let mut actual = vec![0xa5; capacity];
    let mut expected = actual.clone();
    // SAFETY: source ends in a low-byte NUL, destination covers the full limit,
    // and the bounded capacity fits the target's 32-bit size_t.
    let expected_len =
        unsafe { oracle_wcstombs(expected.as_mut_ptr(), source.as_ptr(), capacity as u32) };
    assert_eq!(
        wcstombs(&mut actual, source).unwrap(),
        expected_len as usize
    );
    assert_eq!(actual, expected);
}

#[test]
fn every_wide_character_matches_including_truncated_nuls() {
    for value in 0..=u16::MAX {
        for capacity in [0, 1, 4] {
            compare(&[value, 0], capacity);
        }
    }
}

proptest! {
    #[test]
    fn strings_and_output_limits_match_original_c(
        mut source in prop::collection::vec(any::<u16>(), 0..256), capacity in 0usize..258,
    ) {
        source.push(0);
        compare(&source, capacity);
    }
}
