//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod inventory;
pub mod runner;
pub mod trace;
pub use melee_runtime::{bytecode, ctype, mbstring, random, spline};
