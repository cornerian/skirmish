//! Fighter movement and combat mechanics.

pub mod combat;
pub mod damage;
pub mod locomotion;
pub mod movement;

pub use movement::{Attributes, Movement, decrement_toward_zero};
