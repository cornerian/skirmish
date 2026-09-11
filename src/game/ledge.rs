//! Static-stage ledge discovery and bone-attached ledge action scheduling.

use super::{
    Action, Controller, Error, Event, Fighter,
    data::{Attack, Bone, FighterData, MatchData, StageGeometry},
    simulation,
};
use crate::{
    collision::{
        bones::BoneCapsule,
        stage::{self, Stage},
    },
    fighter::ledge as input,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub catch_horizontal_range: f32,
    pub catch_vertical_below: f32,
    pub catch_vertical_above: f32,
    pub catch_down_threshold: f32,
    pub option_stick_threshold: f32,
    pub option_angle_radians: f32,
    pub wait_frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow: Option<SlowRules>,
    pub regrab_cooldown: u32,
    pub intangibility_frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlowRules {
    pub percent_threshold: f32,
    pub wait_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub attachment: Attachment,
    pub catch: Motion,
    pub wait: Frame,
    pub climb: Motion,
    pub jump: Jump,
    pub attack: AttackMotion,
    pub escape: Motion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow: Option<Options>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub climb: Motion,
    pub jump: Jump,
    pub attack: AttackMotion,
    pub escape: Motion,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub bone: usize,
    pub point: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub bones: Vec<Bone>,
    /// Bone-anchor displacement from the stage endpoint; +X points inward.
    pub anchor_offset: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub frames: Vec<Frame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jump {
    pub motion: Motion,
    pub release_frame: u32,
    /// +X points inward; Y points up.
    pub launch_velocity: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackMotion {
    pub attack: Attack,
    pub anchor_offsets: Vec<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

impl Side {
    fn inward(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub line: Option<usize>,
    pub side: Option<Side>,
    pub input_ready: bool,
    pub cooldown: u32,
    pub slow: bool,
}

/// The shared x488 branch used by all four ledge options and CliffWait's timer.
/// Writing it as the inverse source comparison retains its NaN behavior.
pub fn slow_variant(percent: f32, threshold: f32) -> bool {
    percent.partial_cmp(&threshold) != Some(core::cmp::Ordering::Less)
}

fn selected_slow(fighter: &Fighter, rules: &Rules) -> bool {
    rules
        .slow
        .is_some_and(|slow| slow_variant(fighter.percent, slow.percent_threshold))
}

fn wait_frames(fighter: &Fighter, rules: &Rules) -> u32 {
    if fighter.ledge.slow {
        rules
            .slow
            .map_or(rules.wait_frames, |slow| slow.wait_frames)
    } else {
        rules.wait_frames
    }
}

fn climb(parameters: &Parameters, slow: bool) -> &Motion {
    parameters
        .slow
        .as_ref()
        .filter(|_| slow)
        .map_or(&parameters.climb, |options| &options.climb)
}

fn jump(parameters: &Parameters, slow: bool) -> &Jump {
    parameters
        .slow
        .as_ref()
        .filter(|_| slow)
        .map_or(&parameters.jump, |options| &options.jump)
}

pub(crate) fn attack(parameters: &Parameters, slow: bool) -> &Attack {
    &attack_motion(parameters, slow).attack
}

fn attack_motion(parameters: &Parameters, slow: bool) -> &AttackMotion {
    parameters
        .slow
        .as_ref()
        .filter(|_| slow)
        .map_or(&parameters.attack, |options| &options.attack)
}

fn escape(parameters: &Parameters, slow: bool) -> &Motion {
    parameters
        .slow
        .as_ref()
        .filter(|_| slow)
        .map_or(&parameters.escape, |options| &options.escape)
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
) -> Result<(), Error> {
    if [
        rules.catch_horizontal_range,
        rules.catch_vertical_below,
        rules.catch_vertical_above,
    ]
    .into_iter()
    .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value))
        || ![rules.catch_down_threshold, rules.option_stick_threshold]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0 && value <= 1.0)
        || !rules.option_angle_radians.is_finite()
        || !(0.0..=core::f32::consts::FRAC_PI_2).contains(&rules.option_angle_radians)
        || rules.wait_frames == 0
        || rules.wait_frames >= 1_000_000
        || rules.slow.is_some_and(|slow| {
            !slow.percent_threshold.is_finite()
                || !(0.0..=1_000_000.0).contains(&slow.percent_threshold)
                || slow.wait_frames == 0
                || slow.wait_frames >= 1_000_000
        })
        || rules.slow.is_some() != parameters.slow.is_some()
        || rules.regrab_cooldown >= 1_000_000
        || rules.intangibility_frames >= 1_000_000
        || parameters.attachment.bone >= fighter.bones.len()
        || parameters
            .attachment
            .point
            .into_iter()
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data("invalid explicit ledge parameters".into()));
    }
    for motion in [&parameters.catch, &parameters.climb, &parameters.escape] {
        validate_motion(motion, fighter)?;
    }
    validate_jump(&parameters.jump, fighter)?;
    validate_frame(&parameters.wait, fighter)?;
    validate_attack(&parameters.attack, fighter)?;
    if let Some(slow) = &parameters.slow {
        validate_options(slow, fighter)?;
    }
    Ok(())
}

fn validate_options(options: &Options, fighter: &FighterData) -> Result<(), Error> {
    for motion in [&options.climb, &options.escape] {
        validate_motion(motion, fighter)?;
    }
    validate_jump(&options.jump, fighter)?;
    validate_attack(&options.attack, fighter)
}

fn validate_jump(jump: &Jump, fighter: &FighterData) -> Result<(), Error> {
    validate_motion(&jump.motion, fighter)?;
    if jump.release_frame == 0
        || jump.release_frame as usize >= jump.motion.frames.len()
        || jump
            .launch_velocity
            .into_iter()
            .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value))
    {
        return Err(Error::Data("invalid ledge jump".into()));
    }
    Ok(())
}

fn validate_attack(attack: &AttackMotion, fighter: &FighterData) -> Result<(), Error> {
    if attack.anchor_offsets.len() != attack.attack.frames.len()
        || attack.attack.frames.is_empty()
        || attack.attack.frames.len() > 4096
        || attack
            .anchor_offsets
            .iter()
            .flatten()
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data("invalid ledge attack motion".into()));
    }
    for frame in &attack.attack.frames {
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    Ok(())
}

pub(crate) fn valid_state(fighter: &Fighter) -> bool {
    match (fighter.ledge.line, fighter.ledge.side) {
        (Some(_), Some(_)) => attached(fighter),
        (None, None) => !matches!(
            fighter.action,
            Action::CliffCatch
                | Action::CliffWait
                | Action::CliffClimb
                | Action::CliffAttack
                | Action::CliffEscape
        ),
        _ => false,
    }
}

fn validate_motion(motion: &Motion, fighter: &FighterData) -> Result<(), Error> {
    if motion.frames.is_empty() || motion.frames.len() > 4096 {
        return Err(Error::Data(
            "ledge motion requires 1..4096 complete physics frames".into(),
        ));
    }
    for frame in &motion.frames {
        validate_frame(frame, fighter)?;
    }
    Ok(())
}

fn validate_frame(frame: &Frame, fighter: &FighterData) -> Result<(), Error> {
    if frame
        .anchor_offset
        .into_iter()
        .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data("invalid ledge root motion".into()));
    }
    super::validation::validate_animation_pose(&frame.bones, fighter)?;
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::CliffCatch
            | Action::CliffWait
            | Action::CliffClimb
            | Action::CliffJump
            | Action::CliffAttack
            | Action::CliffEscape
    )
}

pub(crate) fn attached(fighter: &Fighter) -> bool {
    owns_action(fighter.action) && fighter.ledge.line.is_some()
}

pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &FighterData,
    geometry: &StageGeometry,
    rules: Option<&Rules>,
) -> Result<(), Error> {
    fighter.ledge.cooldown = fighter.ledge.cooldown.saturating_sub(1);
    let Some(parameters) = &data.ledge else {
        return Ok(());
    };
    let Some(rules) = rules else { return Ok(()) };
    match fighter.action {
        Action::CliffCatch if fighter.action_frame as usize >= parameters.catch.frames.len() => {
            simulation::enter(fighter, Action::CliffWait);
            fighter.ledge.input_ready = false;
            fighter.ledge.slow = selected_slow(fighter, rules);
        }
        Action::CliffWait if fighter.action_frame >= wait_frames(fighter, rules) => {
            drop(fighter, rules)
        }
        Action::CliffClimb
            if fighter.action_frame as usize
                >= climb(parameters, fighter.ledge.slow).frames.len() =>
        {
            finish_on_floor(fighter, geometry)?;
        }
        Action::CliffAttack
            if fighter.action_frame as usize
                >= attack(parameters, fighter.ledge.slow).frames.len() =>
        {
            finish_on_floor(fighter, geometry)?;
        }
        Action::CliffEscape
            if fighter.action_frame as usize
                >= escape(parameters, fighter.ledge.slow).frames.len() =>
        {
            finish_on_floor(fighter, geometry)?;
        }
        Action::CliffJump => {
            let jump = jump(parameters, fighter.ledge.slow);
            if fighter.action_frame == jump.release_frame && fighter.ledge.line.is_some() {
                fighter.ledge.line = None;
                fighter.ledge.side = None;
                fighter.ledge.cooldown = rules.regrab_cooldown;
                fighter.velocity = [
                    fighter.facing * jump.launch_velocity[0],
                    jump.launch_velocity[1],
                ];
                fighter.locomotion.jumps_used = 1;
            }
            if fighter.action_frame as usize >= jump.motion.frames.len() {
                simulation::enter(fighter, Action::Fall);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    controller: Controller,
) -> bool {
    if !owns_action(fighter.action) {
        return false;
    }
    if fighter.action != Action::CliffWait {
        return true;
    }
    let (Some(_), Some(rules)) = (&data.ledge, rules) else {
        return true;
    };
    let pressed = controller.buttons & !fighter.previous_input.buttons;
    let action = if pressed & super::BUTTON_A != 0 {
        Some(Action::CliffAttack)
    } else if pressed & (super::BUTTON_L | super::BUTTON_R) != 0 {
        Some(Action::CliffEscape)
    } else if pressed & (super::BUTTON_X | super::BUTTON_Y) != 0 {
        Some(Action::CliffJump)
    } else {
        let main_active = controller
            .stick
            .into_iter()
            .any(|value| value.abs() >= rules.option_stick_threshold);
        let c_active = controller
            .cstick
            .into_iter()
            .any(|value| value.abs() >= rules.option_stick_threshold);
        if !main_active && !c_active {
            fighter.ledge.input_ready = true;
            None
        } else {
            let (main, stick) = if main_active {
                (true, controller.stick)
            } else {
                (false, controller.cstick)
            };
            let angle = libm::atan2f(stick[1], stick[0].abs());
            input::stick_option(
                main,
                fighter.ledge.input_ready,
                stick[0],
                angle,
                fighter.facing,
                rules.option_angle_radians,
            )
            .map(|option| match option {
                input::StickOption::Climb => Action::CliffClimb,
                input::StickOption::Drop => Action::Fall,
            })
        }
    };
    if let Some(Action::Fall) = action {
        drop(fighter, rules);
    } else if let Some(action) = action {
        simulation::enter(fighter, action);
    }
    true
}

pub(crate) fn skip_jump_physics(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.action == Action::CliffJump
        && fighter.ledge.line.is_none()
        && data.ledge.as_ref().is_some_and(|parameters| {
            fighter.action_frame == jump(parameters, fighter.ledge.slow).release_frame
        })
}

pub(crate) fn release_on_damage(fighter: &mut Fighter, rules: Option<&Rules>) {
    if fighter.ledge.line.is_some() {
        fighter.ledge.line = None;
        fighter.ledge.side = None;
        fighter.ledge.cooldown = rules.map_or(0, |rules| rules.regrab_cooldown);
    }
}

pub(crate) fn scan(
    data: &MatchData,
    state: &mut super::State,
    stage: &Stage<'_>,
    geometry: &StageGeometry,
    controllers: [Controller; 2],
) -> Result<(), Error> {
    let Some(rules) = &data.rules.ledge else {
        return Ok(());
    };
    for (player, controller) in controllers.into_iter().enumerate() {
        let fighter = &state.fighters[player];
        let Some(parameters) = &data.fighters[player].ledge else {
            continue;
        };
        if fighter.ledge.cooldown > 0
            || fighter.ledge.line.is_some()
            || fighter.grounded
            || fighter.velocity[1] + fighter.knockback[1] > 0.0
            || controller.stick[1] <= -rules.catch_down_threshold
            || !catchable(fighter.action)
        {
            continue;
        }
        let pose = simulation::pose(fighter, &data.fighters[player])?;
        let anchor =
            BoneCapsule::sphere(parameters.attachment.bone, parameters.attachment.point, 0.0)
                .transform(&pose, 1.0)
                .map_err(physics)?
                .start;
        let mut selected = None;
        for (line_id, line) in geometry.lines.iter().enumerate() {
            if line.flags & (stage::FLOOR | stage::ENABLED) != (stage::FLOOR | stage::ENABLED)
                || line.flags & stage::HIDDEN != 0
                || u32::from(line.material_flags) & stage::LEDGE == 0
            {
                continue;
            }
            for side in [Side::Left, Side::Right] {
                let connected = stage
                    .neighbor(line_id, side == Side::Right)
                    .map_err(physics)?
                    .is_some();
                if connected
                    || state.fighters.iter().any(|other| {
                        other.ledge.line == Some(line_id) && other.ledge.side == Some(side)
                    })
                {
                    continue;
                }
                let point = endpoint(line, side);
                let dx = anchor[0] - point[0];
                let dy = anchor[1] - point[1];
                if dx.abs() <= rules.catch_horizontal_range
                    && (-rules.catch_vertical_below..=rules.catch_vertical_above).contains(&dy)
                {
                    let distance = dx * dx + dy * dy;
                    if selected.is_none_or(|(_, _, old)| distance < old) {
                        selected = Some((line_id, side, distance));
                    }
                }
            }
        }
        let Some((line, side, _)) = selected else {
            continue;
        };
        let fighter = &mut state.fighters[player];
        fighter.facing = side.inward();
        fighter.velocity = [0.0; 2];
        fighter.knockback = [0.0; 2];
        fighter.ground_knockback = 0.0;
        fighter.ground_velocity = 0.0;
        fighter.grounded = false;
        fighter.ground_line = None;
        fighter.ledge.line = Some(line);
        fighter.ledge.side = Some(side);
        fighter.ledge.input_ready = false;
        fighter.ledge.slow = selected_slow(fighter, rules);
        fighter.intangibility = fighter.intangibility.max(rules.intangibility_frames);
        simulation::enter(fighter, Action::CliffCatch);
        attach(fighter, &data.fighters[player], geometry)?;
        state.events.push(Event::LedgeCaught { player, line, side });
    }
    Ok(())
}

pub(crate) fn attach(
    fighter: &mut Fighter,
    data: &FighterData,
    geometry: &StageGeometry,
) -> Result<(), Error> {
    let parameters = data
        .ledge
        .as_ref()
        .ok_or_else(|| Error::Data("ledge state requires fighter resources".into()))?;
    let line_id = fighter
        .ledge
        .line
        .ok_or_else(|| Error::Physics("attached ledge action has no line".into()))?;
    let side = fighter
        .ledge
        .side
        .ok_or_else(|| Error::Physics("attached ledge action has no side".into()))?;
    let line = geometry
        .lines
        .get(line_id)
        .ok_or_else(|| Error::Physics("ledge line is outside stage geometry".into()))?;
    let offset = anchor_offset(fighter, parameters)?;
    let point = endpoint(line, side);
    let target = [
        point[0] + side.inward() * offset[0],
        point[1] + offset[1],
        offset[2],
    ];
    let mut local = fighter.clone();
    local.position = [0.0; 2];
    local.depth = 0.0;
    let pose = simulation::pose(&local, data)?;
    let anchor = BoneCapsule::sphere(parameters.attachment.bone, parameters.attachment.point, 0.0)
        .transform(&pose, 1.0)
        .map_err(physics)?
        .start;
    fighter.position = [target[0] - anchor[0], target[1] - anchor[1]];
    fighter.depth = target[2] - anchor[2];
    Ok(())
}

pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let parameters = data.ledge.as_ref()?;
    match fighter.action {
        Action::CliffCatch => parameters
            .catch
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CliffWait => Some(&parameters.wait.bones),
        Action::CliffClimb => climb(parameters, fighter.ledge.slow)
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CliffJump => jump(parameters, fighter.ledge.slow)
            .motion
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CliffAttack => attack_motion(parameters, fighter.ledge.slow)
            .attack
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CliffEscape => escape(parameters, fighter.ledge.slow)
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        _ => None,
    }
}

fn anchor_offset(fighter: &Fighter, parameters: &Parameters) -> Result<[f32; 3], Error> {
    let frame = fighter.action_frame as usize;
    match fighter.action {
        Action::CliffCatch => parameters
            .catch
            .frames
            .get(frame)
            .map(|frame| frame.anchor_offset),
        Action::CliffWait => Some(parameters.wait.anchor_offset),
        Action::CliffClimb => climb(parameters, fighter.ledge.slow)
            .frames
            .get(frame)
            .map(|frame| frame.anchor_offset),
        Action::CliffJump => jump(parameters, fighter.ledge.slow)
            .motion
            .frames
            .get(frame)
            .map(|frame| frame.anchor_offset),
        Action::CliffAttack => attack_motion(parameters, fighter.ledge.slow)
            .anchor_offsets
            .get(frame)
            .copied(),
        Action::CliffEscape => escape(parameters, fighter.ledge.slow)
            .frames
            .get(frame)
            .map(|frame| frame.anchor_offset),
        _ => None,
    }
    .ok_or_else(|| Error::Physics("ledge pose frame is outside supplied animation".into()))
}

fn finish_on_floor(fighter: &mut Fighter, geometry: &StageGeometry) -> Result<(), Error> {
    let line = fighter
        .ledge
        .line
        .ok_or_else(|| Error::Physics("ledge option lost its supporting line".into()))?;
    let surface = geometry
        .lines
        .get(line)
        .ok_or_else(|| Error::Physics("ledge line is outside stage geometry".into()))?;
    let delta = [
        surface.end[0] - surface.start[0],
        surface.end[1] - surface.start[1],
    ];
    let length = libm::sqrtf(delta[0] * delta[0] + delta[1] * delta[1]);
    if !length.is_finite() || length <= 0.0 {
        return Err(Error::Physics("invalid ledge floor normal".into()));
    }
    fighter.floor_normal = [-delta[1] / length, delta[0] / length, 0.0];
    fighter.grounded = true;
    fighter.ground_line = Some(line);
    fighter.contacts[0] = Some(line);
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.ground_velocity = 0.0;
    fighter.ledge.line = None;
    fighter.ledge.side = None;
    super::locomotion::landed(fighter);
    simulation::enter(fighter, Action::Wait);
    Ok(())
}

fn drop(fighter: &mut Fighter, rules: &Rules) {
    fighter.ledge.line = None;
    fighter.ledge.side = None;
    fighter.ledge.cooldown = rules.regrab_cooldown;
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.grounded = false;
    fighter.ground_line = None;
    simulation::enter(fighter, Action::Fall);
}

fn catchable(action: Action) -> bool {
    matches!(
        action,
        Action::Fall
            | Action::DamageFall
            | Action::Jump
            | Action::JumpAerial
            | Action::AttackAirN
            | Action::AttackAirF
            | Action::AttackAirB
            | Action::AttackAirHi
            | Action::AttackAirLw
            // ftFx_SpecialAirSStart_Coll / ftFx_SpecialAirS_Coll /
            // ftFx_SpecialAirSEnd_Coll each check `ft_CheckGroundAndLedge`
            // then `ftCliffCommon_80081298`, exactly like the aerial attacks.
            | Action::SpecialAirSStart
            | Action::SpecialAirS
            | Action::SpecialAirSEnd
    )
}

fn endpoint(line: &stage::Line, side: Side) -> [f32; 2] {
    match side {
        Side::Left => line.start,
        Side::Right => line.end,
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::slow_variant;

    #[test]
    fn slow_variant_is_the_inverse_of_the_source_strict_quick_branch() {
        assert!(!slow_variant(99.999, 100.0));
        assert!(slow_variant(100.0, 100.0));
        assert!(slow_variant(100.001, 100.0));
        assert!(slow_variant(f32::NAN, 100.0));
    }
}
