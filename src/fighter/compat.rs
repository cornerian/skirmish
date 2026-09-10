//! Small host wrappers for source-language arithmetic quirks shared by ports.

/// Runtime/platform.h's comparison-based `ABS` macro. Unlike `f32::abs`, this
/// preserves negative zero and the sign bit of an unordered NaN.
#[inline]
pub(crate) fn comparison_abs(value: f32) -> f32 {
    if value < 0.0 { -value } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_abs_preserves_unordered_and_negative_zero_bits() {
        assert_eq!(comparison_abs(-0.0).to_bits(), (-0.0_f32).to_bits());
        let nan = f32::from_bits(0xffc0_1234);
        assert_eq!(comparison_abs(nan).to_bits(), nan.to_bits());
        assert_eq!(comparison_abs(-2.0), 2.0);
    }
}
