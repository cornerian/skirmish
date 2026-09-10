//! Ordinary wall-jump interrupt and the shared PassiveWallJump motion.
//!
//! Contact queries and moving-stage velocity live in the game collision layer;
//! this module retains the source callback's scalar comparisons and state writes.
use super::{
    Action, Controller, Fighter,
    data::{Bone, FighterData},
    simulation,
};
use serde::{Deserialize, Serialize};

const INPUT_DISABLED: u8 = 254;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Shared processed-stick boundary used to age horizontal tilts.
    pub tilt_deadzone: f32,
    /// Common x768; the armed contact timer is compared to this as a float.
    pub input_window: f32,
    /// Common x76C; inclusive stick displacement away from the wall.
    pub stick_threshold: f32,
    /// Common x770; strict horizontal-tilt age window.
    pub tilt_window: f32,
    /// Common x774; frozen PassiveWallJump callbacks before launch.
    pub startup_frames: u32,
    /// Common passive_wall_vel_y_base for repeated-jump vertical decay.
    pub vertical_velocity_base: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    pub can_walljump: bool,
    pub minimum_approach_speed: f32,
    pub horizontal_velocity: f32,
    pub vertical_velocity: f32,
    /// Complete fighter-specific PassiveWallJump physics poses.
    pub frames: Vec<Vec<Bone>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// -1 for a right-facing wall at the fighter's left, +1 at the right.
    pub wall_side: f32,
    /// Current line-point X displacement supplied by `mpGetSpeed`, or zero on
    /// the source query's failure path.
    pub wall_velocity_x: Option<f32>,
    pub position_delta_x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    /// The pre-increment wall-jump count passed as the vertical exponent.
    pub vertical_exponent: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub input_timer: u8,
    pub wall_side: f32,
    pub used: u8,
    pub active: bool,
    pub startup_timer: u32,
    pub vertical_exponent: u8,
}

impl Default for State {
    fn default() -> Self {
        Self {
            input_timer: INPUT_DISABLED,
            wall_side: 0.0,
            used: 0,
            active: false,
            startup_timer: 0,
            vertical_exponent: 0,
        }
    }
}

/// Complete state/comparison kernel of `ftWallJump_8008169C` after map queries.
pub fn interrupt(
    state: &mut State,
    can_walljump: bool,
    contact: Option<Contact>,
    minimum_approach_speed: f32,
    input: Controller,
    tilt_x_age: u8,
    rules: &Rules,
) -> Option<Trigger> {
    if !can_walljump {
        return None;
    }
    let Some(contact) = contact else {
        state.input_timer = INPUT_DISABLED;
        return None;
    };

    if state.input_timer < INPUT_DISABLED && contact.wall_side == state.wall_side {
        state.input_timer += 1;
    } else {
        let wall_relative_velocity =
            (contact.position_delta_x - contact.wall_velocity_x.unwrap_or(0.0)).abs();
        if wall_relative_velocity > minimum_approach_speed {
            state.wall_side = contact.wall_side;
            state.input_timer = 0;
        }
    }

    if (state.input_timer as f32) < rules.input_window
        && ((state.wall_side == -1.0 && input.stick[0] >= rules.stick_threshold)
            || (state.wall_side == 1.0 && input.stick[0] <= -rules.stick_threshold))
        && (tilt_x_age as f32) < rules.tilt_window
    {
        let trigger = Trigger {
            vertical_exponent: state.used,
        };
        state.input_timer = INPUT_DISABLED;
        state.used = state.used.saturating_add(1);
        Some(trigger)
    } else {
        None
    }
}

pub(crate) fn enter(fighter: &mut Fighter, rules: &Rules, trigger: Trigger) {
    simulation::enter(fighter, Action::PassiveWallJump);
    fighter.facing = -fighter.wall_jump.wall_side;
    fighter.velocity = [0.0; 2];
    fighter.ground_velocity = 0.0;
    fighter.grounded = false;
    fighter.ground_line = None;
    fighter.fast_fall = false;
    fighter.locomotion.tilt_x_age = INPUT_DISABLED;
    fighter.locomotion.tilt_y_age = INPUT_DISABLED;
    fighter.wall_jump.active = true;
    fighter.wall_jump.startup_timer = rules.startup_frames;
    fighter.wall_jump.vertical_exponent = trigger.vertical_exponent;
}

/// Runs before ordinary damage animation because both mechanics share the
/// source PassiveWallJump motion ID.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData, rules: Option<&Rules>) {
    if fighter.action != Action::PassiveWallJump || !fighter.wall_jump.active {
        return;
    }
    let (Some(rules), Some(attributes)) = (rules, data.wall_jump.as_ref()) else {
        return;
    };
    if fighter.wall_jump.startup_timer != 0 {
        fighter.wall_jump.startup_timer -= 1;
        if fighter.wall_jump.startup_timer == 0 {
            fighter.velocity = launch_velocity(
                fighter.facing,
                attributes.horizontal_velocity,
                attributes.vertical_velocity,
                rules.vertical_velocity_base,
                fighter.wall_jump.used,
                fighter.wall_jump.vertical_exponent,
            );
        }
    }
    if fighter.action_frame as usize >= attributes.frames.len() {
        simulation::enter(fighter, Action::Fall);
    }
}

/// PassiveWallJump launch branch from `ftCo_PassiveWall_Anim`.
pub fn launch_velocity(
    facing: f32,
    horizontal: f32,
    vertical: f32,
    vertical_base: f32,
    used: u8,
    exponent: u8,
) -> [f32; 2] {
    [
        facing * horizontal,
        if used == 0 {
            vertical
        } else {
            vertical * vertical_base.powf(exponent.into())
        },
    ]
}

pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    (fighter.action == Action::PassiveWallJump && fighter.wall_jump.active)
        .then(|| {
            data.wall_jump
                .as_ref()?
                .frames
                .get(fighter.action_frame as usize)
        })
        .flatten()
}

pub(crate) fn landed(fighter: &mut Fighter) {
    fighter.wall_jump.used = 0;
    fighter.wall_jump.input_timer = INPUT_DISABLED;
    fighter.wall_jump.wall_side = 0.0;
    fighter.wall_jump.active = false;
    fighter.wall_jump.startup_timer = 0;
    fighter.wall_jump.vertical_exponent = 0;
}

pub(crate) fn validate_rules(rules: &Rules) -> bool {
    [
        rules.tilt_deadzone,
        rules.input_window,
        rules.stick_threshold,
        rules.tilt_window,
        rules.vertical_velocity_base,
    ]
    .into_iter()
    .all(f32::is_finite)
        && (0.0..=1.0).contains(&rules.tilt_deadzone)
        && rules.tilt_deadzone > 0.0
        && (0.0..=255.0).contains(&rules.input_window)
        && rules.input_window > 0.0
        && (0.0..=1.0).contains(&rules.stick_threshold)
        && rules.stick_threshold > 0.0
        && (0.0..=255.0).contains(&rules.tilt_window)
        && rules.tilt_window > 0.0
        && (0.0..=1.0).contains(&rules.vertical_velocity_base)
}

pub(crate) fn validate_attributes(attributes: &Attributes) -> bool {
    [
        attributes.minimum_approach_speed,
        attributes.horizontal_velocity,
        attributes.vertical_velocity,
    ]
    .into_iter()
    .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
        && attributes.horizontal_velocity > 0.0
        && attributes.vertical_velocity > 0.0
        && !attributes.frames.is_empty()
        && attributes.frames.len() < 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules {
            tilt_deadzone: 0.3,
            input_window: 3.0,
            stick_threshold: 0.7,
            tilt_window: 2.0,
            startup_frames: 2,
            vertical_velocity_base: 0.8,
        }
    }

    #[test]
    fn source_boundaries_are_strict_except_for_stick_displacement() {
        let contact = Some(Contact {
            wall_side: -1.0,
            wall_velocity_x: Some(0.0),
            position_delta_x: -1.01,
        });
        let mut state = State::default();
        assert!(
            interrupt(
                &mut state,
                true,
                contact,
                1.0,
                Controller::default(),
                254,
                &rules(),
            )
            .is_none()
        );
        assert_eq!(state.input_timer, 0);
        assert!(
            interrupt(
                &mut state,
                true,
                contact,
                1.0,
                Controller {
                    stick: [0.7, 0.0],
                    ..Controller::default()
                },
                1,
                &rules(),
            )
            .is_some()
        );

        state = State {
            input_timer: 2,
            wall_side: -1.0,
            ..State::default()
        };
        assert!(
            interrupt(
                &mut state,
                true,
                contact,
                1.0,
                Controller {
                    stick: [1.0, 0.0],
                    ..Controller::default()
                },
                2,
                &rules(),
            )
            .is_none()
        );
        assert_eq!(state.input_timer, 3);
    }

    #[test]
    fn missing_contact_disables_only_an_enabled_fighter_timer() {
        let mut disabled = State {
            input_timer: 4,
            ..State::default()
        };
        interrupt(
            &mut disabled,
            false,
            None,
            0.0,
            Controller::default(),
            0,
            &rules(),
        );
        assert_eq!(disabled.input_timer, 4);

        interrupt(
            &mut disabled,
            true,
            None,
            0.0,
            Controller::default(),
            0,
            &rules(),
        );
        assert_eq!(disabled.input_timer, INPUT_DISABLED);
    }
}
