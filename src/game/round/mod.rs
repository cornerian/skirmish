//! Match-owned lifecycle and presentation state.
//!
//! These modules are shared by the match scheduler and by multiple fighters;
//! fighter-local mechanics remain under [`crate::fighter`].

pub mod death;
pub mod effects;
pub mod entry;
pub mod landing;
pub mod rebirth;
pub mod stage_motion;
