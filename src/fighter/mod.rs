//! Mechanics and state belonging to one fighter, including aerial and shield
//! behavior. Match, inter-fighter interactions, and deeper game logic live in
//! [`crate::game`].
#![forbid(unsafe_code)]

pub mod aerial;
pub mod combat;
pub mod damage;
pub mod dash;
pub mod edge;
pub mod escape;
pub mod escape_air;
pub mod helpers;
pub mod idle;
pub mod jab;
pub mod ledge;
pub mod locomotion;
pub mod movement;
pub mod shield;
pub mod smash;
pub mod special;
pub mod specials;
pub mod state;
pub mod taunt;
pub mod tilt;

pub use movement::{Attributes, Movement, decrement_toward_zero};
