//! Compatibility re-export of the game's own trigonometry.
//!
//! The implementation is shared with `skirmish-script-runtime` so native
//! gameplay and embedded fighter scripts cannot drift mathematically.

pub use skirmish_compat_math::trig::*;
