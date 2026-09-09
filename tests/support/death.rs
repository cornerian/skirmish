#![allow(dead_code)]

use skirmish::game::{
    data::MatchData,
    death::{Rules, ScreenRules, StarRules},
};

pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.death = Some(Rules {
        normal_frames: 3,
        force_normal_top: [false; 2],
        camera_disables_screen: false,
        screen_chance_percent: 0,
        star: StarRules {
            startup_frames: 2,
            ascent_frames: 3,
            finish_frames: 2,
            camera_top: 8.0,
            height_scale: 1.5,
            depth_distance: 6.0,
        },
        screen: ScreenRules {
            startup_frames: 2,
            approach_frames: 3,
            camera_hold_frames: 2,
            fall_frames: 3,
            finish_frames: 2,
            approach_start: [-3.0, 4.0, -6.0],
            approach_end: [0.0, 1.0, -2.0],
            fall_vertical_velocity: -0.5,
            fall_depth_velocity: 0.25,
            gravity: 0.5,
            terminal_velocity: 2.0,
        },
    });
    data
}
