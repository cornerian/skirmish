//! A behavior-tested, incremental Rust translation of doldecomp/melee.
//!
//! This crate does not yet implement the complete game.

pub mod animation;
pub mod collision;
pub mod compat;
pub mod controller;
pub mod ctype;
pub mod fighter;
pub mod game;
pub mod id;
pub mod inventory;
pub mod menu;
pub mod presentation;
pub mod quaternion;
pub mod random;
#[cfg(feature = "renderer")]
pub mod renderer;
pub mod spline;
