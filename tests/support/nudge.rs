//! Explicit synthetic push attributes and common parameters, not character data.
#![allow(dead_code)]
use skirmish::fighter::nudge::{Body, Neighbors, Rules};

pub fn rules() -> Rules {
    Rules {
        horizontal_step: 0.25,
        depth_step: 0.125,
        depth_limit: 0.5,
        follower_depth_step: 0.375,
        follower_depth_limit: 0.75,
    }
}
pub fn body(player_id: u8, x: f32) -> Body {
    Body {
        position: [x, 0.0, 0.0],
        deferred_position: [0.0; 3],
        facing: 1.0,
        center_offset: 0.0,
        half_width: 1.0,
        player_id,
        floor: Some(0),
        follower_of: None,
        inactive: false,
        holds_victim: false,
        nudge_disabled: false,
        hitlag: false,
        overlap_disabled: false,
    }
}
pub fn floor() -> [Neighbors; 1] {
    [Neighbors::default()]
}
