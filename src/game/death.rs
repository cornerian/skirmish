//! Headless blast death actions, including delayed star and screen stock loss.

use super::{Action, Error, Fighter, simulation};
use serde::{Deserialize, Serialize};

pub use crate::fighter::death::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub normal_frames: u32,
    pub force_normal_top: [bool; 2],
    pub camera_disables_screen: bool,
    pub screen_chance_percent: i32,
    pub star: StarRules,
    pub screen: ScreenRules,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarRules {
    pub startup_frames: u32,
    pub ascent_frames: u32,
    pub finish_frames: u32,
    pub camera_top: f32,
    pub height_scale: f32,
    pub depth_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenRules {
    pub startup_frames: u32,
    pub approach_frames: u32,
    pub camera_hold_frames: u32,
    pub fall_frames: u32,
    pub finish_frames: u32,
    pub approach_start: [f32; 3],
    pub approach_end: [f32; 3],
    pub fall_vertical_velocity: f32,
    pub fall_depth_velocity: f32,
    pub gravity: f32,
    pub terminal_velocity: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub kind: Option<Kind>,
    pub phase: u8,
    pub timer: u32,
    /// Camera-relative model translation used by headless bone evaluation.
    pub camera_offset: [f32; 3],
    pub depth_velocity: f32,
    pub hidden: bool,
    pub stock_lost: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Update {
    None,
    LoseStock,
    Complete,
}

pub(crate) fn validate(rules: &Rules) -> Result<(), Error> {
    let frames = [
        rules.normal_frames,
        rules.star.startup_frames,
        rules.star.ascent_frames,
        rules.star.finish_frames,
        rules.screen.startup_frames,
        rules.screen.approach_frames,
        rules.screen.camera_hold_frames,
        rules.screen.fall_frames,
        rules.screen.finish_frames,
    ];
    let finite = [
        rules.star.camera_top,
        rules.star.height_scale,
        rules.star.depth_distance,
        rules.screen.fall_vertical_velocity,
        rules.screen.fall_depth_velocity,
        rules.screen.gravity,
        rules.screen.terminal_velocity,
    ]
    .into_iter()
    .chain(rules.screen.approach_start)
    .chain(rules.screen.approach_end)
    .all(|value| value.is_finite() && value.abs() <= 1_000_000.0);
    if frames
        .into_iter()
        .any(|frame| frame == 0 || frame >= 1_000_000)
        || !(0..=100).contains(&rules.screen_chance_percent)
        || !finite
        || rules.star.height_scale < 0.0
        || rules.screen.gravity < 0.0
        || rules.screen.terminal_velocity < 0.0
        || rules.screen.gravity > 0.0 && rules.screen.terminal_velocity == 0.0
    {
        return Err(Error::Data("invalid explicit blast-death rules".into()));
    }
    Ok(())
}

pub(crate) fn begin(fighter: &mut Fighter, kind: Kind, rules: &Rules) {
    fighter.grounded = false;
    fighter.ground_line = None;
    fighter.skip_floor = None;
    fighter.contacts = [None; 4];
    fighter.ground_velocity = 0.0;
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.hitlag = 0.0;
    fighter.hitstun = 0;
    fighter.di_pending = false;
    fighter.invincibility = 0;
    fighter.death = State {
        kind: Some(kind),
        timer: match kind {
            Kind::UpStar | Kind::UpStarIce => rules.star.startup_frames,
            Kind::UpScreen | Kind::UpScreenIce => rules.screen.startup_frames,
            _ => rules.normal_frames,
        },
        camera_offset: match kind {
            Kind::UpScreen | Kind::UpScreenIce => rules.screen.approach_start,
            _ => [0.0; 3],
        },
        ..State::default()
    };
    simulation::enter(fighter, action(kind));
}

pub(crate) fn update(fighter: &mut Fighter, rules: &Rules) -> Update {
    if !owns_action(fighter.action) {
        return Update::None;
    }
    fighter.death.timer -= 1;
    if fighter.death.timer != 0 {
        return Update::None;
    }
    match fighter.death.kind.expect("death actions require a kind") {
        Kind::UpStar | Kind::UpStarIce => update_star(fighter, rules),
        Kind::UpScreen | Kind::UpScreenIce => update_screen(fighter, rules),
        _ => Update::Complete,
    }
}

fn update_star(fighter: &mut Fighter, rules: &Rules) -> Update {
    match fighter.death.phase {
        0 => {
            fighter.velocity = [
                0.0,
                (rules.star.height_scale * rules.star.camera_top - fighter.position[1])
                    / rules.star.ascent_frames as f32,
            ];
            fighter.death.depth_velocity =
                rules.star.depth_distance / rules.star.ascent_frames as f32;
            fighter.death.timer = rules.star.ascent_frames;
            fighter.death.phase = 1;
            Update::None
        }
        1 => {
            fighter.velocity = [0.0; 2];
            fighter.death.camera_offset = [0.0; 3];
            fighter.death.depth_velocity = 0.0;
            fighter.death.timer = rules.star.finish_frames;
            fighter.death.phase = 2;
            fighter.death.hidden = true;
            fighter.death.stock_lost = true;
            Update::LoseStock
        }
        2 => Update::Complete,
        _ => unreachable!("validated death phase"),
    }
}

fn update_screen(fighter: &mut Fighter, rules: &Rules) -> Update {
    match fighter.death.phase {
        0 => {
            fighter.death.timer = rules.screen.approach_frames;
            fighter.death.phase = 1;
            Update::None
        }
        1 => {
            fighter.death.camera_offset = rules.screen.approach_end;
            fighter.death.timer = rules.screen.camera_hold_frames;
            fighter.death.phase = 2;
            simulation::enter(
                fighter,
                if fighter.death.kind == Some(Kind::UpScreenIce) {
                    Action::DeadUpFallHitCameraIce
                } else {
                    Action::DeadUpFallHitCamera
                },
            );
            Update::None
        }
        2 => {
            fighter.velocity[1] = rules.screen.fall_vertical_velocity;
            fighter.death.camera_offset[2] = rules.screen.fall_depth_velocity;
            fighter.death.timer = rules.screen.fall_frames;
            fighter.death.phase = 3;
            Update::None
        }
        3 => {
            fighter.velocity = [0.0; 2];
            fighter.death.timer = rules.screen.finish_frames;
            fighter.death.phase = 4;
            fighter.death.hidden = true;
            fighter.death.stock_lost = true;
            Update::LoseStock
        }
        4 => Update::Complete,
        _ => unreachable!("validated death phase"),
    }
}

pub(crate) fn move_fighter(fighter: &mut Fighter, rules: &Rules) {
    match fighter.death.kind {
        Some(Kind::UpStar | Kind::UpStarIce) if fighter.death.phase == 1 => {
            fighter.position[1] += fighter.velocity[1];
            fighter.depth += fighter.death.depth_velocity;
        }
        Some(Kind::UpScreen | Kind::UpScreenIce) if fighter.death.phase == 1 => {
            let total = rules.screen.approach_frames as f32;
            let progress = (total - fighter.death.timer as f32 + 1.0) / total;
            fighter.death.camera_offset = core::array::from_fn(|axis| {
                let start = rules.screen.approach_start[axis];
                start + (rules.screen.approach_end[axis] - start) * progress
            });
        }
        Some(Kind::UpScreen | Kind::UpScreenIce) if fighter.death.phase == 3 => {
            fighter.velocity[1] -= rules.screen.gravity;
            if fighter.velocity[1] < -rules.screen.terminal_velocity {
                fighter.velocity[1] = -rules.screen.terminal_velocity;
            }
            fighter.death.camera_offset[1] += fighter.velocity[1];
            fighter.death.camera_offset[2] += rules.screen.fall_depth_velocity;
        }
        _ => {}
    }
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::DeadDown
            | Action::DeadLeft
            | Action::DeadRight
            | Action::DeadUp
            | Action::DeadUpStar
            | Action::DeadUpStarIce
            | Action::DeadUpFall
            | Action::DeadUpFallHitCamera
            | Action::DeadUpFallHitCameraFlat
            | Action::DeadUpFallIce
            | Action::DeadUpFallHitCameraIce
    )
}

fn action(kind: Kind) -> Action {
    match kind {
        Kind::Left => Action::DeadLeft,
        Kind::Right => Action::DeadRight,
        Kind::Down => Action::DeadDown,
        Kind::Up => Action::DeadUp,
        Kind::UpStar => Action::DeadUpStar,
        Kind::UpStarIce => Action::DeadUpStarIce,
        Kind::UpScreen => Action::DeadUpFall,
        Kind::UpScreenIce => Action::DeadUpFallIce,
    }
}
