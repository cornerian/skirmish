//! Source-compatible wrappers around library math operations.

/// C's conditional minimum keeps the left operand on ties or unordered inputs.
/// Delegate ordinary values to std; preserve signed zero and raw NaN payloads.
pub fn min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() || a == b {
        a
    } else {
        a.min(b)
    }
}

/// C's conditional maximum has the same left-operand rule.
pub fn max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() || a == b {
        a
    } else {
        a.max(b)
    }
}

/// HSD's atan2 opcode has a special vertical-axis convention, including (0, 0).
/// The ordinary case uses libm and the source's double-precision degree scale.
pub fn atan2_degrees(y: f32, x: f32) -> f32 {
    if x == 0.0 {
        if y >= 0.0 { 90.0 } else { -90.0 }
    } else {
        (57.29577951308232 * f64::from(crate::math::atan2f(y, x))) as f32
    }
}
