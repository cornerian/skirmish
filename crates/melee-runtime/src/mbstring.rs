//! Metrowerks' byte-truncating wide-string conversion (`MSL/mbstring.c`).

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("wide-string source ended before a low-byte NUL or the output limit")]
pub struct SourceExhausted;

/// Copies each 16-bit character's low byte, stopping after writing a zero byte
/// or filling `dest`. Returns the number of nonzero bytes written. This preserves
/// upstream's conversion, which is not UTF-16 to UTF-8 encoding.
///
/// A short source returns an error after copying its available prefix, replacing
/// the C implementation's out-of-bounds read with a checked failure.
pub fn wcstombs(dest: &mut [u8], source: &[u16]) -> Result<usize, SourceExhausted> {
    for (i, byte) in dest.iter_mut().enumerate() {
        *byte = *source.get(i).ok_or(SourceExhausted)? as u8;
        if *byte == 0 {
            return Ok(i);
        }
    }
    Ok(dest.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_byte_nul_stops_and_preserves_trailing_bytes() {
        let mut dest = [0xa5; 4];
        assert_eq!(wcstombs(&mut dest, &[0x141, 0x100, 0x142]), Ok(1));
        assert_eq!(dest, [b'A', 0, 0xa5, 0xa5]);
    }

    #[test]
    fn empty_output_and_exhausted_source_are_checked() {
        assert_eq!(wcstombs(&mut [], &[]), Ok(0));
        let mut dest = [0xa5; 3];
        assert_eq!(wcstombs(&mut dest, &[65]), Err(SourceExhausted));
        assert_eq!(dest, [65, 0xa5, 0xa5]);
    }
}
