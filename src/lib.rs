//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod inventory;
pub mod match_trace;
pub mod replay_match;
pub mod replay_observation;
pub mod runner;
pub mod trace;
pub use arena as game;
pub use input;
pub use peppi_adapter as slippi;
pub use physics;
pub use replay;
pub use runtime::{bytecode, ctype, id, mbstring, quaternion, random, spline};
