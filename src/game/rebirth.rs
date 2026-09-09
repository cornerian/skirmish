//! Headless rebirth-platform travel, wait and release lifecycle.

use super::{Action, Controller, Error, Fighter, simulation};
use crate::fighter::rebirth::approach_velocity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub entry_positions: [[f32; 2]; 2],
    pub platform_positions: [[f32; 2]; 2],
    pub travel_frames: u32,
    pub wait_frames: u32,
    pub release_stick_threshold: f32,
}

pub(crate) fn validate(rules: &Rules, post_invincibility: u32) -> Result<(), Error> {
    if rules
        .entry_positions
        .into_iter()
        .flatten()
        .chain(rules.platform_positions.into_iter().flatten())
        .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
        || rules.travel_frames == 0
        || rules.travel_frames >= 1_000_000
        || rules.wait_frames == 0
        || rules.wait_frames >= 1_000_000
        || !rules.release_stick_threshold.is_finite()
        || !(0.0..=1.0).contains(&rules.release_stick_threshold)
        || rules.release_stick_threshold == 0.0
        || rules
            .travel_frames
            .checked_add(rules.wait_frames)
            .and_then(|frames| frames.checked_add(post_invincibility))
            .is_none()
    {
        return Err(Error::Data(
            "invalid explicit rebirth-platform rules".into(),
        ));
    }
    Ok(())
}

pub(crate) fn enter(fighter: &mut Fighter, rules: &Rules, player: usize, post_frames: u32) {
    fighter.position = rules.entry_positions[player];
    fighter.depth = 0.0;
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_velocity = 0.0;
    fighter.grounded = false;
    fighter.ground_line = None;
    fighter.contacts = [None; 4];
    fighter.skip_floor = None;
    fighter.facing = if rules.platform_positions[player][0] >= 0.0 {
        -1.0
    } else {
        1.0
    };
    fighter.locomotion.jumps_used = 0;
    fighter.invincibility = rules
        .travel_frames
        .saturating_add(rules.wait_frames)
        .saturating_add(post_frames);
    simulation::enter(fighter, Action::Rebirth);
}

pub(crate) fn update_animation(
    fighter: &mut Fighter,
    rules: Option<&Rules>,
    player: usize,
    post_frames: u32,
) {
    let Some(rules) = rules else { return };
    match fighter.action {
        Action::Rebirth if fighter.action_frame >= rules.travel_frames => {
            fighter.position = rules.platform_positions[player];
            fighter.velocity = [0.0; 2];
            simulation::enter(fighter, Action::RebirthWait);
        }
        Action::RebirthWait if fighter.action_frame >= rules.wait_frames => {
            release(fighter, post_frames);
        }
        _ => {}
    }
}

pub(crate) fn update_actions(
    fighter: &mut Fighter,
    rules: Option<&Rules>,
    input: Controller,
    post_frames: u32,
) -> bool {
    match fighter.action {
        Action::Rebirth => true,
        Action::RebirthWait => {
            if let Some(rules) = rules
                && (input.buttons != 0
                    || input.trigger != 0.0
                    || input
                        .stick
                        .into_iter()
                        .chain(input.cstick)
                        .any(|value| value.abs() >= rules.release_stick_threshold))
            {
                release(fighter, post_frames);
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn move_fighter(fighter: &mut Fighter, rules: &Rules, player: usize) {
    match fighter.action {
        Action::Rebirth => {
            let remaining = rules
                .travel_frames
                .saturating_sub(fighter.action_frame)
                .max(1);
            fighter.velocity = approach_velocity(
                fighter.position,
                rules.platform_positions[player],
                remaining,
            );
            for axis in 0..2 {
                fighter.position[axis] += fighter.velocity[axis];
            }
        }
        Action::RebirthWait => {
            fighter.position = rules.platform_positions[player];
            fighter.velocity = [0.0; 2];
        }
        _ => {}
    }
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::Rebirth | Action::RebirthWait)
}

pub(crate) fn invulnerable(action: Action) -> bool {
    owns_action(action)
}

fn release(fighter: &mut Fighter, post_frames: u32) {
    fighter.velocity = [0.0; 2];
    fighter.invincibility = post_frames;
    simulation::enter(fighter, Action::Fall);
}
