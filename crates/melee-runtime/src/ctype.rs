//! Metrowerks ASCII classification and conversion, using Rust's byte methods.
//!
//! Classification returns the original integer mask, not a normalized boolean.
//! Inputs are reduced to their low byte, except case conversion preserves EOF.

pub const CONTROL: u8 = 0x01;
pub const MOTION: u8 = 0x02;
pub const SPACE: u8 = 0x04;
pub const PUNCTUATION: u8 = 0x08;
pub const DIGIT: u8 = 0x10;
pub const HEX_DIGIT: u8 = 0x20;
pub const LOWER_CASE: u8 = 0x40;
pub const UPPER_CASE: u8 = 0x80;

/// The byte's exact `__ctype_map` entry, including vertical-tab whitespace.
pub fn flags(c: i32) -> u8 {
    let c = c as u8;
    let category = match c {
        b'\t'..=b'\r' => MOTION,
        b' ' => SPACE,
        c if c.is_ascii_control() => CONTROL,
        c if c.is_ascii_punctuation() => PUNCTUATION,
        c if c.is_ascii_digit() => DIGIT,
        c if c.is_ascii_lowercase() => LOWER_CASE,
        c if c.is_ascii_uppercase() => UPPER_CASE,
        _ => 0,
    };
    category | if c.is_ascii_hexdigit() { HEX_DIGIT } else { 0 }
}

pub fn isalpha(c: i32) -> i32 {
    i32::from(flags(c) & (LOWER_CASE | UPPER_CASE))
}

pub fn isdigit(c: i32) -> i32 {
    i32::from(flags(c) & DIGIT)
}

pub fn isspace(c: i32) -> i32 {
    i32::from(flags(c) & (MOTION | SPACE))
}

pub fn isupper(c: i32) -> i32 {
    i32::from(flags(c) & UPPER_CASE)
}

pub fn isxdigit(c: i32) -> i32 {
    i32::from(flags(c) & HEX_DIGIT)
}

pub fn tolower(c: i32) -> i32 {
    if c == -1 {
        -1
    } else {
        i32::from((c as u8).to_ascii_lowercase())
    }
}

pub fn toupper(c: i32) -> i32 {
    if c == -1 {
        -1
    } else {
        i32::from((c as u8).to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_eof_and_low_byte_conversion() {
        assert_eq!(isalpha(i32::from(b'a')), 0x40);
        assert_eq!(isalpha(i32::from(b'A')), 0x80);
        assert_eq!(isspace(0x0b), 0x02);
        assert_eq!(isspace(0x20), 0x04);
        assert_eq!(flags(0x80), 0);
        assert_eq!(toupper(-1), -1);
        assert_eq!(tolower(0x141), i32::from(b'a'));
    }
}
