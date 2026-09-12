//! Source-compatible math: the runtime's own trigonometry, quaternion and
//! spline interpolation, PRNG, and `fminf`-style operator quirks.

pub mod operators;
pub mod quaternion;
pub mod random;
pub mod spline;
pub mod trig;
