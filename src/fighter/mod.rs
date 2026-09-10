//! Fighter movement and combat mechanics.

pub mod action_instance;
pub mod aerial;
pub mod clank;
pub mod combat;
pub mod combo;
mod compat;
pub mod damage;
pub mod death;
pub mod escape;
pub mod grab;
pub mod instance;
pub mod ledge;
pub mod locomotion;
pub mod movement;
pub mod nudge;
pub mod rebirth;
pub mod shield;
pub mod special;
pub mod stale;

pub use movement::{Attributes, Movement, decrement_toward_zero};
