//! Native, explicit data for the experimental two-player match slice.
//! Frame samples are physics poses, not rendering assets or HSD animation bytecode.
use melee_physics::{Attributes, bones, combat, ecb, stage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchData {
    pub schema: u32,
    /// Every current profile is experimental; this is not a compatibility toggle.
    pub profile: Profile,
    pub provenance: String,
    pub stage: Stage,
    pub fighters: [FighterData; 2],
    pub rules: Rules,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    IntegrationFixture,
    Experimental,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    /// Compact floor used when explicit geometry is absent.
    pub floor: Floor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<StageGeometry>,
    /// Left, right, bottom, top. Crossing any boundary kills in this slice.
    pub blast: [f32; 4],
    pub spawns: [[f32; 2]; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageGeometry {
    pub lines: Vec<stage::Line>,
    pub joints: Vec<stage::Joint>,
}

/// Environmental collision samples are physics data, independent of hurtboxes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollisionBox {
    Fixed {
        source: ecb::FixedSource,
    },
    Bones {
        indices: [usize; 6],
        parameters: ecb::JointParameters,
        flags: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Floor {
    pub left: f32,
    pub right: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub stocks: u8,
    pub countdown_frames: u32,
    pub time_limit_frames: u32,
    pub respawn_frames: u32,
    pub respawn_invincibility_frames: u32,
    pub friction_above_walk: f32,
    pub walk_accel_taper_gain: f32,
    pub fast_fall_threshold: f32,
    pub knockback_decay: f32,
    pub knockback_speed: f32,
    pub hitstun_scale: f32,
    pub knockback: KnockbackData,
    pub hitlag: HitlagData,
    pub damage: crate::damage::CombatRules,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockbackData {
    pub weight_scale: f32,
    pub weight_base: f32,
    pub maximum: f32,
    pub percent_scale: f32,
    pub damage_percent_scale: f32,
    pub fixed_damage: f32,
    pub growth_scale: f32,
    pub growth_base: f32,
}

impl KnockbackData {
    pub(crate) fn physics(&self) -> combat::KnockbackRules {
        combat::KnockbackRules {
            weight_scale: self.weight_scale,
            weight_base: self.weight_base,
            maximum: self.maximum,
            percent_scale: self.percent_scale,
            damage_percent_scale: self.damage_percent_scale,
            fixed_damage: self.fixed_damage,
            growth_scale: self.growth_scale,
            growth_base: self.growth_base,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitlagData {
    pub damage_scale: f32,
    pub base: f32,
    pub crouch_multiplier: f32,
}

impl HitlagData {
    pub(crate) fn physics(&self) -> combat::HitlagRules {
        combat::HitlagRules {
            damage_scale: self.damage_scale,
            base: self.base,
            crouch_multiplier: self.crouch_multiplier,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FighterData {
    pub name: String,
    pub movement: MovementData,
    pub weight: f32,
    pub collision_box: CollisionBox,
    pub bones: Vec<Bone>,
    pub hurtboxes: Vec<Capsule>,
    pub jab: Attack,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementData {
    pub ground_max_horizontal_velocity: f32,
    pub air_max_horizontal_velocity: f32,
    pub air_drift_stick_mul: f32,
    pub aerial_drift_base: f32,
    pub air_drift_max: f32,
    pub aerial_friction: f32,
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub fast_fall_velocity: f32,
    pub ground_friction: f32,
    pub walk_acceleration_mul: f32,
    pub walk_acceleration_base: f32,
    pub walk_max_velocity: f32,
    pub jump_startup_frames: u32,
    pub jump_vertical_velocity: f32,
    pub short_hop_vertical_velocity: f32,
    pub jump_horizontal_velocity: f32,
    pub jump_horizontal_max: f32,
    pub jump_momentum_multiplier: f32,
    pub landing_frames: u32,
}

impl MovementData {
    pub(crate) fn physics(&self) -> Attributes {
        Attributes {
            ground_max_horizontal_velocity: self.ground_max_horizontal_velocity,
            air_max_horizontal_velocity: self.air_max_horizontal_velocity,
            air_drift_stick_mul: self.air_drift_stick_mul,
            aerial_drift_base: self.aerial_drift_base,
            air_drift_max: self.air_drift_max,
            aerial_friction: self.aerial_friction,
            gravity: self.gravity,
            terminal_velocity: self.terminal_velocity,
            fast_fall_velocity: self.fast_fall_velocity,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bone {
    pub parent: Option<usize>,
    pub classical_scale: bool,
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Bone {
    pub(crate) fn physics(&self) -> bones::Bone {
        bones::Bone {
            parent: self.parent,
            classical_scale: self.classical_scale,
            local: bones::LocalTransform {
                translation: self.translation,
                rotation: self.rotation,
                scale: self.scale,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capsule {
    pub bone: usize,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
}

impl Capsule {
    pub(crate) fn physics(&self) -> bones::BoneCapsule {
        bones::BoneCapsule {
            bone: self.bone,
            start: self.start,
            end: self.end,
            radius: self.radius,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attack {
    /// Exactly one physics-pose sample per simulation frame, including recovery.
    /// No implicit interpolation or fallback for missing samples.
    pub frames: Vec<AttackFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackFrame {
    pub bones: Vec<Bone>,
    pub hitboxes: Vec<Hitbox>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hitbox {
    /// Hitboxes in the same group cannot hit the same victim twice per attack.
    pub group: u8,
    pub bone: usize,
    pub center: [f32; 3],
    pub radius: f32,
    pub damage: u32,
    /// Integral ordinary launch angles or 361, whose coefficients are explicit.
    pub angle_degrees: f32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
}
