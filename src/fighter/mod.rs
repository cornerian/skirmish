//! Fighter movement and combat mechanics.

pub mod aerial;
pub mod combat;
pub mod damage;
pub mod locomotion;
pub mod movement;
pub mod shield;
pub mod stale;

pub use movement::{Attributes, Movement, decrement_toward_zero};
