//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod bytecode;
pub mod collision;
pub mod controller;
pub mod ctype;
pub mod fighter;
pub mod game;
pub mod id;
pub mod inventory;
pub mod match_trace;
pub mod mbstring;
pub mod menu_cli;
pub mod quaternion;
pub mod random;
#[cfg(feature = "renderer")]
pub mod renderer;
pub mod replay_match;
pub mod replay_observation;
pub mod runner;
pub mod spline;
pub mod trace;
pub use menus;
pub use peppi_adapter as slippi;
pub use replay;
