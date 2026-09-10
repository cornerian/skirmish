//! Concrete Slippi replay integration for the native Skirmish match.
#![forbid(unsafe_code)]

pub mod match_validation;
pub mod observation;

pub use peppi_adapter as slippi;
pub use replay_validation::{
    BitDifference, Checkpoint, FrameStepper, Transition, ValidationError, ValidationReport,
    branch_from, compare_f32_bits, compare_f64_bits, validate, validate_fallible,
};
