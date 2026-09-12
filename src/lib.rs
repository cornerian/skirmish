//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod collision;
pub mod compat;
pub mod controller;
pub mod ctype;
pub mod fighter;
pub mod game;
pub mod id;
pub mod inventory;
pub mod math;
pub mod quaternion;
pub mod random;
pub mod spline;
pub use menus;
