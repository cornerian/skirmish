//! Fighter movement and combat mechanics.

pub mod action_instance;
pub mod aerial;
pub mod clank;
pub mod combat;
pub mod combo;
mod compat;
pub mod damage;
pub mod dash;
pub mod death;
pub mod edge;
pub mod escape;
pub mod escape_air;
pub mod fox_side_special;
pub mod grab;
pub mod idle;
pub mod instance;
pub mod jab;
pub mod ledge;
pub mod locomotion;
pub mod movement;
pub mod nudge;
pub mod rebirth;
pub mod shield;
pub mod smash;
pub mod special;
pub mod stale;
pub mod taunt;
pub mod tilt;

pub use movement::{Attributes, Movement, decrement_toward_zero};
