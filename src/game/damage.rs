//! Explicit damage rules and the experimental scheduler's damage integration.
//! Native helpers preserve selected source arithmetic; action ordering and
//! facing selection remain the documented match-slice policy.
use super::{
    Action, Error, Event, Fighter, State,
    data::{Bone, FighterData, Hitbox, MatchData},
};
use crate::fighter::{combat, damage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatRules {
    pub angle_361_airborne_radians: f32,
    pub angle_361_grounded_max_degrees: f32,
    pub angle_361_low_knockback: f32,
    pub angle_361_high_knockback: f32,
    pub di_max_degrees: f32,
    pub special_angle_min: u32,
    pub special_angle_max: u32,
    pub special_angle_timer: i32,
    pub knockback_replace_window: i32,
    /// None retains the explicitly incomplete legacy match profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displacement: Option<HitlagDisplacementRules>,
    /// Explicit damage-floor state profile. None preserves the legacy slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_response: Option<FloorResponseRules>,
    /// Optional ordinary wall/ceiling damage reflection profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_response: Option<SurfaceResponseRules>,
    /// Optional wall/ceiling tech timing and input profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_tech: Option<SurfaceTechRules>,
    /// Optional ordinary Damage motion thresholds (common x158/x15C/x160).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_motion: Option<DamageMotionRules>,
    /// Optional grounded launch projection and friction profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_launch: Option<GroundLaunchRules>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageMotionRules {
    pub thresholds: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundLaunchRules {
    pub fly_bounce_angle_radians: f32,
    pub fly_bounce_vertical_multiplier: f32,
    /// Common x200, multiplied by each fighter's ground friction.
    pub ground_knockback_friction_multiplier: f32,
}

impl GroundLaunchRules {
    fn physics(&self) -> damage::GroundLaunchRules {
        damage::GroundLaunchRules {
            fly_bounce_angle_radians: self.fly_bounce_angle_radians,
            fly_bounce_vertical_multiplier: self.fly_bounce_vertical_multiplier,
        }
    }
}

/// Complete physics-pose samples for the source's 15 ordinary Damage motions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamagePoseAttributes {
    /// One source low/middle/high selector per fighter hurtbox.
    pub hurtbox_heights: Vec<damage::HurtHeight>,
    /// Levels 1..3, then hurt height low/middle/high.
    pub ground: [[Vec<Vec<Bone>>; 3]; 3],
    /// Air levels 1..3; hurt height is ignored by the source table.
    pub air: [Vec<Vec<Bone>>; 3],
    /// Fly hurt height low/middle/high.
    pub fly: [Vec<Vec<Bone>>; 3],
}

impl DamagePoseAttributes {
    fn motion(&self, motion: damage::DamageMotion) -> &Vec<Vec<Bone>> {
        match motion {
            damage::DamageMotion::Ground { level, height } => {
                &self.ground[level as usize][height.index()]
            }
            damage::DamageMotion::Air { level } => &self.air[level as usize],
            damage::DamageMotion::Fly { height } => &self.fly[height.index()],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorResponseRules {
    pub tumble_knockback_threshold: f32,
    pub tech_window: f32,
    pub tech_repeat_lockout: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tech_roll: Option<FloorTechRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knockdown_options: Option<KnockdownRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_invincibility: Option<RecoveryInvincibilityRules>,
    /// Optional low-damage reaction while the fighter is prone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_damage: Option<DownDamageRules>,
    pub passive_frames: u32,
    pub down_bound_frames: u32,
    pub down_wait_frames: u32,
    pub down_stand_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownDamageRules {
    /// Strict upper bound for the current hit's pending damage.
    pub pending_damage_threshold: i32,
    /// Number of supplied DownDamage animation/physics samples.
    pub frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockdownRules {
    pub horizontal_stick_threshold: f32,
    pub stand_stick_threshold: f32,
    pub vertical_angle_radians: f32,
    pub attack_cstick_threshold: f32,
    pub bound_attack_window: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryInvincibilityRules {
    pub passive_frames: u32,
    pub tech_roll_frames: u32,
    pub missed_roll_frames: u32,
    pub stand_frames: u32,
    pub attack_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockdownAttributes {
    pub passive_poses: Vec<Vec<Bone>>,
    pub orientation: ProneOrientationRules,
    pub face_up: ProneRecoveryAttributes,
    pub face_down: ProneRecoveryAttributes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProneOrientation {
    FaceUp,
    FaceDown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProneOrientationRules {
    pub hip_bone: usize,
    pub use_z_axis: bool,
    pub invert: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProneRecoveryAttributes {
    pub bound_poses: Vec<Vec<Bone>>,
    pub wait_poses: Vec<Vec<Bone>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_poses: Option<Vec<Vec<Bone>>>,
    pub forward: FloorTechMotion,
    pub backward: FloorTechMotion,
    pub stand_poses: Vec<Vec<Bone>>,
    pub attack: super::data::Attack,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechRules {
    pub stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechAttributes {
    pub forward: FloorTechMotion,
    pub backward: FloorTechMotion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechMotion {
    /// One headless physics/pose sample per action frame.
    pub frames: Vec<FloorTechFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechFrame {
    pub bones: Vec<Bone>,
    /// Source TransN delta along local forward for this frame.
    pub root_translation: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceResponseRules {
    pub knockback_threshold: f32,
    pub velocity_multiplier: f32,
    pub lockout_frames: u8,
    pub wall_frames: u32,
    pub ceiling_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceTechRules {
    pub wall_freeze_frames: u32,
    pub wall_frames: u32,
    pub wall_jump_frames: u32,
    pub ceiling_frames: u32,
    pub ceiling_horizontal_frame: u32,
    pub jump_stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceTechAttributes {
    pub passive_wall_velocity: f32,
    pub wall_jump_horizontal_velocity: f32,
    pub wall_jump_vertical_velocity: f32,
    pub passive_ceiling_velocity: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SurfaceTechState {
    pub timer: u32,
    pub ceiling_velocity_applied: bool,
}

/// Native common-data coefficients. Main-stick SDI/ASDI only; the current
/// controller contract has no C-stick channel or LR/vcancel response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitlagDisplacementRules {
    pub axis_thresholds: [f32; 2],
    pub minimum_stick_magnitude: f32,
    pub sdi_window: u8,
    pub sdi_distance: f32,
    pub asdi_distance: f32,
}

/// Ordinary persistent armor channels; dynamic metal/state modifiers remain
/// separate. Minimum knockback is required explicitly, never guessed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Armor {
    pub armor0: f32,
    pub armor1: f32,
    pub minimum_knockback: f32,
}

pub(crate) fn validate_armor(armor: &Armor) -> Result<(), Error> {
    if [armor.armor0, armor.armor1, armor.minimum_knockback]
        .into_iter()
        .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value))
    {
        return Err(Error::Data("invalid explicit armor parameters".into()));
    }
    Ok(())
}

pub(crate) fn validate_surface_tech_attributes(
    attributes: &SurfaceTechAttributes,
) -> Result<(), Error> {
    if [
        attributes.passive_wall_velocity,
        attributes.wall_jump_horizontal_velocity,
        attributes.wall_jump_vertical_velocity,
        attributes.passive_ceiling_velocity,
    ]
    .into_iter()
    .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
    {
        Ok(())
    } else {
        Err(Error::Data(
            "invalid fighter damage-surface tech attributes".into(),
        ))
    }
}

pub(crate) fn validate_damage_pose_attributes(
    attributes: &DamagePoseAttributes,
    fighter: &FighterData,
) -> Result<(), Error> {
    if attributes.hurtbox_heights.len() != fighter.hurtboxes.len() {
        return Err(Error::Data(
            "damage poses require one height per hurtbox".into(),
        ));
    }
    for motion in attributes
        .ground
        .iter()
        .flatten()
        .chain(&attributes.air)
        .chain(&attributes.fly)
    {
        if motion.is_empty() || motion.len() > 4096 {
            return Err(Error::Data(
                "damage motions require 1..4096 physics samples".into(),
            ));
        }
        for pose in motion {
            super::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_floor_tech_attributes(
    attributes: &FloorTechAttributes,
    fighter: &FighterData,
    invincibility_frames: u32,
) -> Result<(), Error> {
    for motion in [&attributes.forward, &attributes.backward] {
        validate_ground_motion(motion, fighter)?;
        if invincibility_frames > motion.frames.len() as u32 {
            return Err(Error::Data(
                "floor-tech invincibility exceeds the supplied motion".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_knockdown_attributes(
    attributes: &KnockdownAttributes,
    fighter: &FighterData,
    profile: &FloorResponseRules,
) -> Result<(), Error> {
    if attributes.orientation.hip_bone >= fighter.bones.len() {
        return Err(Error::Data(
            "prone orientation requires a valid hip bone".into(),
        ));
    }
    validate_poses(&attributes.passive_poses, profile.passive_frames, fighter)?;
    for variant in [&attributes.face_up, &attributes.face_down] {
        validate_ground_motion(&variant.forward, fighter)?;
        validate_ground_motion(&variant.backward, fighter)?;
        for (poses, frames) in [
            (&variant.bound_poses, profile.down_bound_frames),
            (&variant.wait_poses, profile.down_wait_frames),
            (&variant.stand_poses, profile.down_stand_frames),
        ] {
            validate_poses(poses, frames, fighter)?;
        }
        match (&profile.down_damage, &variant.damage_poses) {
            (Some(rules), Some(poses)) => validate_poses(poses, rules.frames, fighter)?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "down-damage rules require poses for both orientations".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "down-damage poses require explicit common rules".into(),
                ));
            }
            (None, None) => {}
        }
        if let Some(invincibility) = &profile.recovery_invincibility
            && (invincibility.missed_roll_frames > variant.forward.frames.len() as u32
                || invincibility.missed_roll_frames > variant.backward.frames.len() as u32
                || invincibility.attack_frames > variant.attack.frames.len() as u32)
        {
            return Err(Error::Data(
                "knockdown invincibility exceeds the supplied action".into(),
            ));
        }
    }
    Ok(())
}

fn validate_poses(poses: &[Vec<Bone>], frames: u32, fighter: &FighterData) -> Result<(), Error> {
    if poses.len() != frames as usize {
        return Err(Error::Data(
            "floor-recovery poses must match the configured duration".into(),
        ));
    }
    for pose in poses {
        super::validation::validate_animation_pose(pose, fighter)?;
    }
    Ok(())
}

fn validate_ground_motion(motion: &FloorTechMotion, fighter: &FighterData) -> Result<(), Error> {
    if motion.frames.is_empty() || motion.frames.len() > 4096 {
        return Err(Error::Data(
            "ground recovery requires 1..4096 physics samples".into(),
        ));
    }
    for frame in &motion.frames {
        if !frame.root_translation.is_finite() || frame.root_translation.abs() > 1_000_000.0 {
            return Err(Error::Data(
                "invalid ground-recovery root translation".into(),
            ));
        }
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    Ok(())
}

impl FloorTechAttributes {
    pub(crate) fn motion(&self, action: Action) -> Option<&FloorTechMotion> {
        match action {
            Action::PassiveStandF => Some(&self.forward),
            Action::PassiveStandB => Some(&self.backward),
            _ => None,
        }
    }
}

impl KnockdownAttributes {
    pub(crate) fn variant(
        &self,
        orientation: Option<ProneOrientation>,
    ) -> Option<&ProneRecoveryAttributes> {
        match orientation? {
            ProneOrientation::FaceUp => Some(&self.face_up),
            ProneOrientation::FaceDown => Some(&self.face_down),
        }
    }

    pub(crate) fn motion(
        &self,
        action: Action,
        orientation: Option<ProneOrientation>,
    ) -> Option<&FloorTechMotion> {
        let variant = self.variant(orientation)?;
        match action {
            Action::DownForward => Some(&variant.forward),
            Action::DownBack => Some(&variant.backward),
            _ => None,
        }
    }
}

impl CombatRules {
    fn angle_rules(&self) -> damage::LaunchAngleRules {
        damage::LaunchAngleRules {
            airborne_radians: self.angle_361_airborne_radians,
            grounded_max_degrees: self.angle_361_grounded_max_degrees,
            grounded_low_knockback: self.angle_361_low_knockback,
            grounded_high_knockback: self.angle_361_high_knockback,
            special_angle_min: self.special_angle_min,
            special_angle_max: self.special_angle_max,
            special_timer: self.special_angle_timer,
        }
    }
}

pub(crate) fn validate_rules(rules: &CombatRules) -> Result<(), Error> {
    if ![
        rules.angle_361_airborne_radians,
        rules.angle_361_grounded_max_degrees,
        rules.angle_361_low_knockback,
        rules.angle_361_high_knockback,
        rules.di_max_degrees,
    ]
    .into_iter()
    .all(|value| value.is_finite())
        || !(0.0..=core::f32::consts::TAU).contains(&rules.angle_361_airborne_radians)
        || !(0.0..=360.0).contains(&rules.angle_361_grounded_max_degrees)
        || rules.angle_361_low_knockback < 0.0
        || rules.angle_361_low_knockback >= rules.angle_361_high_knockback
        || rules.angle_361_high_knockback > 1_000_000.0
        || !(0.0..=180.0).contains(&rules.di_max_degrees)
        || rules.special_angle_min > rules.special_angle_max
        || !(0..=255).contains(&rules.special_angle_timer)
        || !(0..1_000_000).contains(&rules.knockback_replace_window)
    {
        return Err(Error::Data("invalid explicit damage rules".into()));
    }
    if let Some(profile) = &rules.displacement
        && (profile
            .axis_thresholds
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0 || value > 1.0)
            || !profile.minimum_stick_magnitude.is_finite()
            || !(0.0..=2.0).contains(&profile.minimum_stick_magnitude)
            || profile.sdi_window == 0
            || profile.sdi_window == 255
            || [profile.sdi_distance, profile.asdi_distance]
                .into_iter()
                .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value)))
    {
        return Err(Error::Data(
            "invalid explicit hitlag displacement rules".into(),
        ));
    }
    if let Some(profile) = &rules.damage_motion
        && (!profile
            .thresholds
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
            || !(profile.thresholds[0] < profile.thresholds[1]
                && profile.thresholds[1] < profile.thresholds[2]))
    {
        return Err(Error::Data(
            "invalid explicit damage-motion thresholds".into(),
        ));
    }
    if let Some(profile) = &rules.ground_launch
        && (!(0.0..=core::f32::consts::FRAC_PI_2).contains(&profile.fly_bounce_angle_radians)
            || ![
                profile.fly_bounce_vertical_multiplier,
                profile.ground_knockback_friction_multiplier,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value)))
    {
        return Err(Error::Data("invalid explicit grounded-launch rules".into()));
    }
    if rules.ground_launch.is_some() && rules.damage_motion.is_none() {
        return Err(Error::Data(
            "grounded launch requires damage-motion thresholds".into(),
        ));
    }
    if let Some(profile) = &rules.floor_response
        && (![profile.tumble_knockback_threshold, profile.tech_window]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
            || profile.tech_window > 255.0
            || !(0..=255).contains(&profile.tech_repeat_lockout)
            || [
                profile.passive_frames,
                profile.down_bound_frames,
                profile.down_wait_frames,
                profile.down_stand_frames,
            ]
            .into_iter()
            .any(|frames| frames == 0 || frames >= 1_000_000))
    {
        return Err(Error::Data(
            "invalid explicit damage-floor response rules".into(),
        ));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.tech_roll.as_ref())
        && (!profile.stick_threshold.is_finite()
            || profile.stick_threshold <= 0.0
            || profile.stick_threshold > 1.0)
    {
        return Err(Error::Data("invalid explicit floor-tech roll rules".into()));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.knockdown_options.as_ref())
        && (![
            profile.horizontal_stick_threshold,
            profile.stand_stick_threshold,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0 && value <= 1.0)
            || !profile.attack_cstick_threshold.is_finite()
            || !(0.0..=1.0).contains(&profile.attack_cstick_threshold)
            || !profile.vertical_angle_radians.is_finite()
            || !(0.0..=core::f32::consts::FRAC_PI_2).contains(&profile.vertical_angle_radians)
            || !profile.bound_attack_window.is_finite()
            || !(0.0..=255.0).contains(&profile.bound_attack_window))
    {
        return Err(Error::Data(
            "invalid explicit knockdown-option rules".into(),
        ));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.down_damage.as_ref())
        && (!(0..=1_000_000).contains(&profile.pending_damage_threshold)
            || profile.frames == 0
            || profile.frames >= 1_000_000)
    {
        return Err(Error::Data("invalid explicit down-damage rules".into()));
    }
    if let Some(invincibility) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.recovery_invincibility.as_ref())
    {
        let floor = rules.floor_response.as_ref().unwrap();
        if [
            invincibility.passive_frames,
            invincibility.tech_roll_frames,
            invincibility.missed_roll_frames,
            invincibility.stand_frames,
            invincibility.attack_frames,
        ]
        .into_iter()
        .any(|frames| frames >= 1_000_000)
            || invincibility.passive_frames > floor.passive_frames
            || invincibility.stand_frames > floor.down_stand_frames
            || floor.tech_roll.is_none() && invincibility.tech_roll_frames != 0
            || floor.knockdown_options.is_none()
                && (invincibility.missed_roll_frames != 0
                    || invincibility.stand_frames != 0
                    || invincibility.attack_frames != 0)
        {
            return Err(Error::Data(
                "invalid explicit floor-recovery invincibility".into(),
            ));
        }
    }
    if let Some(profile) = &rules.surface_response
        && ((!profile.knockback_threshold.is_finite()
            || !(0.0..=1_000_000.0).contains(&profile.knockback_threshold))
            || !profile.velocity_multiplier.is_finite()
            || !(0.0..=1.0).contains(&profile.velocity_multiplier)
            || profile.wall_frames == 0
            || profile.ceiling_frames == 0
            || profile.wall_frames >= 1_000_000
            || profile.ceiling_frames >= 1_000_000)
    {
        return Err(Error::Data(
            "invalid explicit damage-surface response rules".into(),
        ));
    }
    if rules.surface_response.is_some() && rules.floor_response.is_none() {
        return Err(Error::Data(
            "damage-surface response requires explicit tumble rules".into(),
        ));
    }
    if let Some(profile) = &rules.surface_tech
        && (!profile.jump_stick_threshold.is_finite()
            || !(0.0..=1.0).contains(&profile.jump_stick_threshold)
            || profile.jump_stick_threshold == 0.0
            || profile.wall_freeze_frames == 0
            || profile.wall_freeze_frames >= profile.wall_frames
            || profile.wall_freeze_frames >= profile.wall_jump_frames
            || profile.ceiling_horizontal_frame >= profile.ceiling_frames
            || [
                profile.wall_frames,
                profile.wall_jump_frames,
                profile.ceiling_frames,
            ]
            .into_iter()
            .any(|frames| frames == 0 || frames >= 1_000_000))
    {
        return Err(Error::Data(
            "invalid explicit damage-surface tech rules".into(),
        ));
    }
    if rules.surface_tech.is_some() && rules.floor_response.is_none() {
        return Err(Error::Data(
            "damage-surface tech requires explicit tumble rules".into(),
        ));
    }
    Ok(())
}

/// The original damage transition assigns/merges launch velocity before hitlag,
/// installs a post-hitlag callback, then resets the elapsed-hit counter. The
/// caller resolves simultaneous contacts before invoking this helper.
pub(crate) fn apply_hit(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
    staled: super::staling::Hit,
    hurt_height: damage::HurtHeight,
) -> Result<(), Error> {
    let victim = 1 - attacker;
    let rules = &data.rules;
    let target = &state.fighters[victim];
    let was_grounded = target.grounded;
    let down_damage_face_up = rules
        .damage
        .floor_response
        .as_ref()
        .and_then(|floor| floor.down_damage.as_ref())
        .and_then(|profile| {
            damage::down_damage_face_up(
                matches!(
                    target.action,
                    Action::DownBound | Action::DownWait | Action::DownDamage
                ),
                target.action == Action::DownWait && target.prone == Some(ProneOrientation::FaceUp),
                false,
                staled.damage,
                profile.pending_damage_threshold,
            )
        });
    let knockback = combat::knockback(
        &rules.knockback.physics(),
        combat::KnockbackHit {
            growth: hit.growth,
            fixed: hit.fixed,
            base: hit.base,
        },
        combat::DamageState {
            percent: target.percent,
            pending_damage: staled.damage,
            count_override: None,
        },
        staled.base_damage,
        combat::KnockbackModifiers {
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
            weight: data.fighters[victim].weight,
        },
    )
    .map_err(physics)?;
    if !knockback.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 {
        return Err(Error::Physics(
            "combat rules produced negative knockback".into(),
        ));
    }
    let knockback = data.fighters[victim]
        .armor
        .as_ref()
        .map_or(knockback, |armor| {
            damage::subtract_armor(
                knockback,
                [armor.armor0, armor.armor1],
                armor.minimum_knockback,
            )
        });
    let damage_motion = rules.damage.damage_motion.as_ref().map(|profile| {
        damage::damage_motion(
            knockback,
            rules.hitstun_scale,
            profile.thresholds,
            !target.grounded,
            hurt_height,
        )
    });
    let attacker_hitlag = combat::hitlag(staled.damage as i32, false, 1.0, &rules.hitlag.physics())
        .map_err(physics)?;
    let hitlag = combat::hitlag(
        staled.damage as i32,
        matches!(target.action, Action::Squat | Action::SquatWait),
        1.0,
        &rules.hitlag.physics(),
    )
    .map_err(physics)?;
    let hitstun = combat::initial_hitstun(knockback, rules.hitstun_scale).map_err(physics)?;
    if !knockback.is_finite() || !hitlag.is_finite() || !attacker_hitlag.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 || hitlag < 0.0 || attacker_hitlag < 0.0 || hitstun < 1 {
        return Err(Error::Physics(
            "combat rules produced a negative magnitude or invalid timer".into(),
        ));
    }
    let angle = damage::launch_angle(
        hit.angle_degrees as i32,
        knockback,
        !target.grounded,
        &rules.damage.angle_rules(),
    );
    let speed = knockback * rules.knockback_speed;
    let facing = state.fighters[attacker].facing;
    // The slice chooses direction from the attacker's facing. Other upstream
    // facing selection, grounded launch projection and ice bounce are unported.
    let incoming = [
        speed * libm::cosf(angle.radians) * facing,
        speed * libm::sinf(angle.radians),
    ];
    let ground_launch = (was_grounded && rules.damage.ground_launch.is_some()).then(|| {
        damage::ground_launch(
            incoming,
            [target.floor_normal[0], target.floor_normal[1]],
            down_damage_face_up.is_some()
                || matches!(damage_motion, Some(damage::DamageMotion::Fly { .. })),
            &rules.damage.ground_launch.as_ref().unwrap().physics(),
        )
    });
    let incoming = ground_launch.map_or(incoming, |launch| launch.knockback);
    let merged = damage::merge_knockback(
        target.knockback,
        incoming,
        target.damage_elapsed,
        rules.damage.knockback_replace_window,
    );
    if !angle.radians.is_finite() || merged.into_iter().any(|value| !value.is_finite()) {
        return Err(Error::NonFinite);
    }
    state.fighters[attacker].hitlag = state.fighters[attacker].hitlag.max(attacker_hitlag);
    let target = &mut state.fighters[victim];
    target.percent = (target.percent + staled.damage).min(999.0);
    target.hitlag = target.hitlag.max(hitlag);
    target.hitstun = hitstun as u32;
    target.velocity = [0.0; 2];
    target.ground_velocity = 0.0;
    target.knockback = merged;
    target.ground_knockback = ground_launch.map_or(0.0, |launch| launch.ground_knockback);
    if was_grounded && ground_launch.is_none_or(|launch| launch.airborne) {
        // ftCommon_8007D5D4 consumes the ground jump when damage leaves ground.
        target.locomotion.jumps_used = 1;
    }
    if ground_launch.is_none_or(|launch| launch.airborne) {
        target.grounded = false;
        target.ground_line = None;
    }
    target.fast_fall = false;
    super::ledge::release_on_damage(target, rules.ledge.as_ref());
    super::simulation::enter(
        target,
        if down_damage_face_up.is_some() {
            Action::DownDamage
        } else {
            Action::Damage
        },
    );
    target.damage_motion = if down_damage_face_up.is_none() {
        damage_motion
    } else {
        None
    };
    if let Some(face_up) = down_damage_face_up {
        target.prone = Some(if face_up {
            ProneOrientation::FaceUp
        } else {
            ProneOrientation::FaceDown
        });
        // The source stores hitstun in the same union slot used by DownWait.
        target.down_timer = hitstun as u32;
    }
    target.damage_elapsed = 0;
    target.locomotion.tilt_x_age = 254;
    target.locomotion.tilt_y_age = 254;
    target.damage_angle_flag = 0;
    target.last_damage_surface = None;
    target.reflect_lockout = 0;
    if let Some(timer) = angle.special_timer {
        target.damage_angle_flag = 1;
        target.damage_angle_timer = timer;
    }
    // A zero-hitlag hit has no expiry callback in the examined upstream path.
    target.di_pending = target.hitlag > 0.0;
    target.tumbling = rules
        .damage
        .floor_response
        .as_ref()
        .is_some_and(|profile| knockback >= profile.tumble_knockback_threshold);
    state.events.push(Event::Hit {
        attacker,
        victim,
        damage: staled.damage,
        knockback,
    });
    Ok(())
}

/// Priority-1 portions of the ordinary tumble, knockdown and neutral-tech
/// graph. Durations are explicit resources because animation data is not yet
/// available for every fighter.
pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &super::data::FighterData,
    rules: &CombatRules,
    input: super::Controller,
) {
    fighter.reflect_lockout = fighter.reflect_lockout.saturating_sub(1);
    if fighter.action == Action::DownDamage
        && let Some(profile) = rules
            .floor_response
            .as_ref()
            .and_then(|floor| floor.down_damage.as_ref())
    {
        if fighter.action_frame < profile.frames {
            fighter.down_timer = fighter.down_timer.saturating_sub(1);
        }
        if fighter.action_frame >= profile.frames {
            if fighter.grounded {
                let action = if fighter.down_timer == 0 {
                    Action::DownStand
                } else {
                    Action::DownWait
                };
                enter_recovery(fighter, action, rules.floor_response.as_ref().unwrap());
            } else {
                super::simulation::enter(fighter, Action::Fall);
            }
            return;
        }
    }
    if let Some(motion) = ground_motion(fighter, data)
        && fighter.action_frame as usize >= motion.frames.len()
    {
        super::simulation::enter(fighter, Action::Wait);
        return;
    }
    if fighter.action == Action::DownAttack
        && data.knockdown.as_ref().is_some_and(|attributes| {
            attributes
                .variant(fighter.prone)
                .is_some_and(|variant| fighter.action_frame as usize >= variant.attack.frames.len())
        })
    {
        super::simulation::enter(fighter, Action::Wait);
        return;
    }
    if fighter.action == Action::DownBound
        && let Some(floor) = &rules.floor_response
        && fighter.action_frame >= floor.down_bound_frames
    {
        let action = floor
            .knockdown_options
            .as_ref()
            .and_then(|rules| {
                damage::down_bound_option(
                    knockdown_input(fighter, input),
                    [
                        fighter.locomotion.attack_a_age,
                        fighter.locomotion.attack_b_age,
                    ],
                    &knockdown_rules(rules),
                )
            })
            .map(knockdown_action)
            .unwrap_or(Action::DownWait);
        if action == Action::DownWait {
            fighter.down_timer = floor.down_wait_frames;
        }
        enter_recovery(fighter, action, floor);
        return;
    }
    if let (Some(profile), Some(attributes)) = (&rules.surface_tech, &data.surface_tech) {
        let duration = match fighter.action {
            Action::PassiveWall | Action::PassiveWallJump => {
                if fighter.surface_tech.timer != 0 {
                    fighter.surface_tech.timer -= 1;
                    if fighter.surface_tech.timer == 0 {
                        if fighter.action == Action::PassiveWall {
                            fighter.velocity[0] = fighter.facing * attributes.passive_wall_velocity;
                        } else {
                            fighter.velocity = [
                                fighter.facing * attributes.wall_jump_horizontal_velocity,
                                attributes.wall_jump_vertical_velocity,
                            ];
                        }
                    }
                }
                Some(if fighter.action == Action::PassiveWall {
                    profile.wall_frames
                } else {
                    profile.wall_jump_frames
                })
            }
            Action::PassiveCeiling => {
                if !fighter.surface_tech.ceiling_velocity_applied
                    && fighter.action_frame >= profile.ceiling_horizontal_frame
                {
                    fighter.velocity[0] = input.stick[0] * attributes.passive_ceiling_velocity;
                    fighter.surface_tech.ceiling_velocity_applied = true;
                }
                Some(profile.ceiling_frames)
            }
            _ => None,
        };
        if duration.is_some_and(|duration| fighter.action_frame >= duration) {
            super::simulation::enter(fighter, Action::Fall);
            return;
        }
    }
    let surface_next = rules
        .surface_response
        .as_ref()
        .and_then(|response| match fighter.action {
            Action::FlyReflectWall if fighter.action_frame >= response.wall_frames => {
                Some(Action::DamageFall)
            }
            Action::FlyReflectCeiling if fighter.action_frame >= response.ceiling_frames => {
                Some(Action::DamageFall)
            }
            _ => None,
        });
    if let Some(action) = surface_next {
        super::simulation::enter(fighter, action);
        return;
    }
    let motion_ended = fighter.damage_motion.is_none_or(|motion| {
        data.damage_poses
            .as_ref()
            .is_none_or(|poses| fighter.action_frame as usize >= poses.motion(motion).len())
    });
    let Some(profile) = &rules.floor_response else {
        if fighter.action == Action::Damage && fighter.hitstun == 0 && motion_ended {
            super::simulation::enter(
                fighter,
                if fighter.grounded {
                    Action::Wait
                } else {
                    Action::Fall
                },
            );
        }
        return;
    };
    let next = match fighter.action {
        Action::Damage if fighter.hitstun == 0 && motion_ended => Some(if fighter.grounded {
            Action::Wait
        } else if fighter.tumbling {
            Action::DamageFall
        } else {
            Action::Fall
        }),
        Action::Passive if fighter.action_frame >= profile.passive_frames => Some(Action::Wait),
        Action::DownWait => {
            fighter.down_timer = fighter.down_timer.saturating_sub(1);
            (fighter.down_timer == 0).then_some(Action::DownStand)
        }
        Action::DownStand if fighter.action_frame >= profile.down_stand_frames => {
            Some(Action::Wait)
        }
        _ => None,
    };
    if let Some(action) = next {
        enter_recovery(fighter, action, profile);
    }
}

/// Selected ordinary Damage physics pose. If hitstun outlasts its animation,
/// the last supplied pose remains sampled until the action can exit.
pub(crate) fn damage_pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    let motion = fighter.damage_motion?;
    let frames = data.damage_poses.as_ref()?.motion(motion);
    frames
        .get(fighter.action_frame as usize)
        .or_else(|| frames.last())
}

/// DownWait's source priority: get-up attack, roll, then stand. This owns the
/// action even when no option is selected so unrelated state machines cannot
/// consume input while the fighter remains knocked down.
pub(crate) fn update_actions(
    fighter: &mut Fighter,
    rules: &CombatRules,
    input: super::Controller,
) -> bool {
    if fighter.action != Action::DownWait {
        return false;
    }
    // DownBound's animation callback owns its final input decision. DownWait's
    // IASA begins on the following frame if that decision selected no option.
    if fighter.action_frame == 0 {
        return true;
    }
    let Some(floor) = &rules.floor_response else {
        return true;
    };
    let Some(rules) = &floor.knockdown_options else {
        return true;
    };
    let action = damage::knockdown_option(knockdown_input(fighter, input), &knockdown_rules(rules))
        .map(knockdown_action);
    if let Some(action) = action {
        enter_recovery(fighter, action, floor);
    }
    true
}

pub(crate) fn can_reflect(
    fighter: &Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
) -> bool {
    use crate::collision::stage::Surface;
    let Some(profile) = &rules.surface_response else {
        return false;
    };
    if !fighter.tumbling
        || fighter.last_damage_surface == Some(surface)
        || !matches!(
            fighter.action,
            Action::Damage
                | Action::DamageFall
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        )
    {
        return false;
    }
    if matches!(surface, Surface::LeftWall | Surface::RightWall)
        && fighter.action == Action::FlyReflectWall
        && fighter.reflect_lockout != 0
    {
        return false;
    }
    match surface {
        Surface::LeftWall => fighter.knockback[0] > profile.knockback_threshold,
        Surface::RightWall => fighter.knockback[0] < -profile.knockback_threshold,
        Surface::Ceiling => fighter.knockback[1] > profile.knockback_threshold,
        Surface::Floor => false,
    }
}

pub(crate) fn can_surface_tech(
    fighter: &Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
) -> bool {
    use crate::collision::stage::Surface;
    let (Some(_), Some(floor)) = (&rules.surface_tech, &rules.floor_response) else {
        return false;
    };
    fighter.tumbling
        && !matches!(surface, Surface::Floor)
        && matches!(
            fighter.action,
            Action::Damage
                | Action::DamageFall
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        )
        && !(matches!(surface, Surface::LeftWall | Surface::RightWall)
            && fighter.action == Action::FlyReflectWall
            && fighter.reflect_lockout != 0)
        && damage::can_tech(
            false,
            fighter.locomotion.tech_press_age,
            fighter.locomotion.previous_tech_press_age,
            floor.tech_window,
            floor.tech_repeat_lockout,
        )
}

pub(crate) fn surface_tech(
    fighter: &mut Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
    input: super::Controller,
) -> bool {
    use crate::collision::stage::Surface;
    let profile = rules.surface_tech.as_ref().unwrap();
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.ground_velocity = 0.0;
    fighter.grounded = false;
    fighter.ground_line = None;
    let jump = matches!(surface, Surface::LeftWall | Surface::RightWall)
        && damage::wall_tech_jumps(
            fighter.locomotion.jump_press_age,
            input.stick[1],
            rules.floor_response.as_ref().unwrap().tech_window,
            profile.jump_stick_threshold,
        );
    if matches!(surface, Surface::LeftWall | Surface::RightWall) {
        fighter.facing = if surface == Surface::LeftWall {
            -1.0
        } else {
            1.0
        };
        fighter.locomotion.tilt_x_age = 254;
        fighter.locomotion.tilt_y_age = 254;
    }
    super::simulation::enter(
        fighter,
        match (surface, jump) {
            (Surface::Ceiling, _) => Action::PassiveCeiling,
            (_, true) => Action::PassiveWallJump,
            _ => Action::PassiveWall,
        },
    );
    fighter.surface_tech = SurfaceTechState {
        timer: if surface == Surface::Ceiling {
            0
        } else {
            profile.wall_freeze_frames
        },
        ceiling_velocity_applied: false,
    };
    jump
}

pub(crate) fn reflect(
    fighter: &mut Fighter,
    surface: crate::collision::stage::Surface,
    normal: [f32; 3],
    rules: &CombatRules,
) {
    let profile = rules.surface_response.as_ref().unwrap();
    let reflected = damage::reflect_velocity(
        fighter.velocity,
        fighter.knockback,
        [normal[0], normal[1]],
        profile.velocity_multiplier,
    );
    fighter.velocity = [0.0; 2];
    fighter.knockback = reflected.knockback;
    fighter.ground_knockback = 0.0;
    fighter.ground_velocity = 0.0;
    fighter.facing = reflected.facing;
    fighter.grounded = false;
    fighter.ground_line = None;
    fighter.last_damage_surface = Some(surface);
    fighter.reflect_lockout = profile.lockout_frames;
    super::simulation::enter(
        fighter,
        if matches!(surface, crate::collision::stage::Surface::Ceiling) {
            Action::FlyReflectCeiling
        } else {
            Action::FlyReflectWall
        },
    );
}

/// Damage-floor callback shared by Damage and DamageFall. False leaves a
/// non-tumbling or profile-free damage action under its existing policy.
pub(crate) fn land(
    fighter: &mut Fighter,
    data: &FighterData,
    pose: &crate::collision::bones::Pose,
    rules: &CombatRules,
    input: super::Controller,
) -> Result<bool, Error> {
    if fighter.action == Action::DownDamage {
        // DownDamage's airborne collision callback converts to ground without
        // entering the ordinary tech/missed-tech landing graph.
        fighter.tumbling = false;
        return Ok(true);
    }
    let Some(profile) = &rules.floor_response else {
        return Ok(false);
    };
    if !fighter.tumbling {
        return Ok(false);
    }
    let action = if damage::can_tech(
        false,
        fighter.locomotion.tech_press_age,
        fighter.locomotion.previous_tech_press_age,
        profile.tech_window,
        profile.tech_repeat_lockout,
    ) {
        profile
            .tech_roll
            .as_ref()
            .and_then(|roll| {
                damage::tech_roll_direction(input.stick[0], fighter.facing, roll.stick_threshold)
            })
            .map_or(Action::Passive, |direction| match direction {
                damage::TechRoll::Forward => Action::PassiveStandF,
                damage::TechRoll::Backward => Action::PassiveStandB,
            })
    } else {
        Action::DownBound
    };
    enter_recovery(fighter, action, profile);
    if action == Action::DownBound {
        if let Some(attributes) = &data.knockdown {
            let matrix = pose
                .world_matrix(attributes.orientation.hip_bone)
                .map_err(physics)?;
            fighter.prone = Some(
                if damage::prone_face_up(
                    matrix,
                    attributes.orientation.use_z_axis,
                    attributes.orientation.invert,
                ) {
                    ProneOrientation::FaceUp
                } else {
                    ProneOrientation::FaceDown
                },
            );
        }
        fighter.locomotion.attack_a_age = 255;
        fighter.locomotion.attack_b_age = 255;
    }
    Ok(true)
}

pub(crate) fn ground_recovery_pose<'a>(
    fighter: &Fighter,
    data: &'a FighterData,
) -> Option<&'a [Bone]> {
    if let Some(attributes) = &data.knockdown {
        if fighter.action == Action::Passive {
            return attributes
                .passive_poses
                .get(fighter.action_frame as usize)
                .map(Vec::as_slice);
        }
        let variant = attributes.variant(fighter.prone)?;
        let poses = match fighter.action {
            Action::DownBound => Some(&variant.bound_poses),
            Action::DownWait => Some(&variant.wait_poses),
            Action::DownDamage => variant.damage_poses.as_ref(),
            Action::DownStand => Some(&variant.stand_poses),
            _ => None,
        };
        if let Some(poses) = poses {
            return poses.get(fighter.action_frame as usize).map(Vec::as_slice);
        }
    }
    ground_motion(fighter, data)?
        .frames
        .get(fighter.action_frame as usize)
        .map(|frame| frame.bones.as_slice())
}

pub(crate) fn ground_recovery_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    let frame = ground_motion(fighter, data)?
        .frames
        .get(fighter.action_frame as usize)?;
    Some(frame.root_translation * fighter.facing)
}

fn ground_motion<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a FloorTechMotion> {
    data.floor_tech
        .as_ref()
        .and_then(|attributes| attributes.motion(fighter.action))
        .or_else(|| {
            data.knockdown
                .as_ref()
                .and_then(|attributes| attributes.motion(fighter.action, fighter.prone))
        })
}

fn knockdown_input(fighter: &Fighter, input: super::Controller) -> damage::KnockdownInput {
    let pressed = input.buttons & !fighter.previous_input.buttons;
    damage::KnockdownInput {
        main: input.stick,
        cstick: input.cstick,
        previous_cstick: fighter.previous_input.cstick,
        facing: fighter.facing,
        attack_pressed: pressed & (super::BUTTON_A | super::BUTTON_B) != 0,
        shoulder_pressed: pressed & (super::BUTTON_L | super::BUTTON_R) != 0,
    }
}

fn knockdown_rules(rules: &KnockdownRules) -> damage::KnockdownRules {
    damage::KnockdownRules {
        horizontal_stick_threshold: rules.horizontal_stick_threshold,
        stand_stick_threshold: rules.stand_stick_threshold,
        vertical_angle_radians: rules.vertical_angle_radians,
        attack_cstick_threshold: rules.attack_cstick_threshold,
        bound_attack_window: rules.bound_attack_window,
    }
}

fn knockdown_action(option: damage::KnockdownOption) -> Action {
    match option {
        damage::KnockdownOption::Attack => Action::DownAttack,
        damage::KnockdownOption::Forward => Action::DownForward,
        damage::KnockdownOption::Backward => Action::DownBack,
        damage::KnockdownOption::Stand => Action::DownStand,
    }
}

fn enter_recovery(fighter: &mut Fighter, action: Action, floor: &FloorResponseRules) {
    super::simulation::enter(fighter, action);
    let frames = floor
        .recovery_invincibility
        .as_ref()
        .map_or(0, |rules| match action {
            Action::Passive => rules.passive_frames,
            Action::PassiveStandF | Action::PassiveStandB => rules.tech_roll_frames,
            Action::DownForward | Action::DownBack => rules.missed_roll_frames,
            Action::DownStand => rules.stand_frames,
            Action::DownAttack => rules.attack_frames,
            _ => 0,
        });
    fighter.invincibility = fighter.invincibility.max(frames);
}

/// Called on frozen damage frames after the timer decrement, while it remains
/// positive. The last tick runs only the exit callback. Collision response runs
/// afterward using the displaced position and frozen collision pose. Input is
/// already sampled into the same x670/x671 ages used by ordinary locomotion.
pub(crate) fn during_hitlag(
    fighter: &mut Fighter,
    stick: [f32; 2],
    rules: &CombatRules,
) -> Result<(), Error> {
    if let Some(profile) = &rules.displacement
        && fighter.di_pending
    {
        let mut timers = [fighter.locomotion.tilt_x_age, fighter.locomotion.tilt_y_age];
        damage::smash_displacement(
            &mut fighter.position,
            stick,
            &mut timers,
            true,
            profile.minimum_stick_magnitude,
            i32::from(profile.sdi_window),
            profile.sdi_distance,
        );
        [fighter.locomotion.tilt_x_age, fighter.locomotion.tilt_y_age] = timers;
        finite_position(fighter)?;
    }
    Ok(())
}

/// Called once when positive hitlag reaches zero, before normal motion resumes.
/// Main-stick ASDI precedes DI, including when the stick has been held throughout
/// hitlag. Attacker hitlag does not install this damage callback.
pub(crate) fn exit_hitlag(
    fighter: &mut Fighter,
    input: super::Controller,
    rules: &CombatRules,
) -> Result<(), Error> {
    if fighter.di_pending {
        if let Some(profile) = &rules.displacement {
            // ftCo_Damage_OnExitHitlag gives a held C-stick priority for ASDI.
            // Main-stick DI below remains independent of that choice.
            let [x, y] = input.cstick;
            let stick = if x * x + y * y
                >= profile.minimum_stick_magnitude * profile.minimum_stick_magnitude
            {
                input.cstick
            } else {
                input.stick
            };
            damage::automatic_displacement(
                &mut fighter.position,
                stick,
                profile.minimum_stick_magnitude,
                profile.asdi_distance,
            );
            finite_position(fighter)?;
        }
        let influenced =
            damage::directional_influence(fighter.knockback, input.stick, rules.di_max_degrees);
        if influenced.into_iter().any(|value| !value.is_finite()) {
            return Err(Error::NonFinite);
        }
        fighter.knockback = influenced;
        fighter.di_pending = false;
    }
    Ok(())
}

fn finite_position(fighter: &Fighter) -> Result<(), Error> {
    if fighter.position.into_iter().any(|value| !value.is_finite()) {
        Err(Error::NonFinite)
    } else {
        Ok(())
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
