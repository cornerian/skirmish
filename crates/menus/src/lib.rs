//! Native, headless menu input and navigation translated from `melee/mn/mnmain.c`.
//!
//! Branch navigation is implemented; requested scenes and panels are explicit
//! integration boundaries. Rendering, audio, and those leaf screens are not
//! implemented by this crate. See the crate README for function coverage.

pub mod controller;
pub mod input;
mod navigation;

pub use navigation::{
    Action, Destination, Entry, InvalidSelection, Menu, MenuState, Panel, Scene, Snapshot, Unlocks,
};
