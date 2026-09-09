//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod inventory;
pub mod match_trace;
pub mod runner;
pub mod trace;
pub use melee_input as input;
pub use melee_physics as physics;
pub use melee_runtime::{bytecode, ctype, id, mbstring, quaternion, random, spline};
pub use skirmish_match as game;
pub use skirmish_replay as replay;
