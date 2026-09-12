#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::compat::ctype;

unsafe extern "C" {
    fn oracle_ctype_flags(c: i32) -> u8;
    fn oracle_tolower(c: i32) -> i32;
    fn oracle_toupper(c: i32) -> i32;
    fn oracle_isalpha(c: i32) -> i32;
    fn oracle_isdigit(c: i32) -> i32;
    fn oracle_isspace(c: i32) -> i32;
    fn oracle_isupper(c: i32) -> i32;
    fn oracle_isxdigit(c: i32) -> i32;
}

fn compare(c: i32) {
    // SAFETY: C functions use only immutable tables and mask every index to a
    // byte. All i32 values are supported, including the special EOF case.
    unsafe {
        assert_eq!(ctype::flags(c), oracle_ctype_flags(c), "flags({c})");
        assert_eq!(ctype::tolower(c), oracle_tolower(c), "tolower({c})");
        assert_eq!(ctype::toupper(c), oracle_toupper(c), "toupper({c})");
        assert_eq!(ctype::isalpha(c), oracle_isalpha(c), "isalpha({c})");
        assert_eq!(ctype::isdigit(c), oracle_isdigit(c), "isdigit({c})");
        assert_eq!(ctype::isspace(c), oracle_isspace(c), "isspace({c})");
        assert_eq!(ctype::isupper(c), oracle_isupper(c), "isupper({c})");
        assert_eq!(ctype::isxdigit(c), oracle_isxdigit(c), "isxdigit({c})");
    }
}

#[test]
fn every_byte_and_signed_boundary_matches_upstream_tables() {
    for c in -256..=511 {
        compare(c);
    }
    for c in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        compare(c);
    }
}

proptest! {
    #[test]
    fn arbitrary_integer_inputs_match_upstream(c in any::<i32>()) {
        compare(c);
    }
}
