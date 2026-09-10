//! Native, explicit data for the experimental two-player match slice.
//! Frame samples are physics poses, not rendering assets or HSD animation bytecode.
use crate::{
    collision::{bones, ecb, stage},
    fighter::{Attributes, combat},
};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<super::stage_motion::Rules>,
    /// Left, right, bottom, top. Top eligibility is configured in `Rules`.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staling: Option<crate::fighter::stale::Rules>,
    pub stocks: u8,
    pub countdown_frames: u32,
    pub time_limit_frames: u32,
    pub respawn_frames: u32,
    pub respawn_invincibility_frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebirth: Option<super::rebirth::Rules>,
    pub friction_above_walk: f32,
    pub walk_accel_taper_gain: f32,
    pub fast_fall_threshold: f32,
    pub knockback_decay: f32,
    pub knockback_speed: f32,
    pub hitstun_scale: f32,
    /// Common-data x4F0: airborne top KOs require upward knockback strictly
    /// above this value. None retains the original synthetic fixture's rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_ko_min_knockback: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death: Option<super::death::Rules>,
    pub knockback: KnockbackData,
    pub hitlag: HitlagData,
    pub damage: super::damage::CombatRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<super::shield::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clank: Option<super::clank::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge: Option<super::nudge::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grab: Option<super::grab::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<super::ledge::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_jump: Option<super::wall_jump::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<super::escape::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_air: Option<super::escape_air::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilt: Option<super::tilt::Rules>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebound: Option<super::clank::Animation>,
    pub name: String,
    pub movement: MovementData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locomotion: Option<super::locomotion::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<super::damage::Armor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_tech: Option<super::damage::FloorTechAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knockdown: Option<super::damage::KnockdownAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_response: Option<super::damage::SurfaceResponseAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_tech: Option<super::damage::SurfaceTechAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_jump: Option<super::wall_jump::Attributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_poses: Option<super::damage::DamagePoseAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<super::shield::Attributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge: Option<super::nudge::Attributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grab: Option<super::grab::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<super::ledge::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special: Option<super::special::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<super::escape::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_air: Option<super::escape_air::Parameters>,
    pub weight: f32,
    pub collision_box: CollisionBox,
    pub bones: Vec<Bone>,
    pub hurtboxes: Vec<Hurtbox>,
    pub jab: Attack,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aerials: Option<super::aerial::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilts: Option<super::tilt::Parameters>,
}

impl FighterData {
    pub(crate) fn attack(
        &self,
        action: super::Action,
        prone: Option<super::damage::ProneOrientation>,
        slow_ledge: bool,
    ) -> Option<&Attack> {
        if action == super::Action::Jab {
            return Some(&self.jab);
        }
        if super::tilt::owns_action(action) {
            return super::tilt::attack(self.tilts.as_ref()?, action);
        }
        if action == super::Action::CliffAttack {
            return Some(super::ledge::attack(self.ledge.as_ref()?, slow_ledge));
        }
        if action == super::Action::DownAttack {
            return Some(&self.knockdown.as_ref()?.variant(prone)?.attack);
        }
        if let Some(attack) = self
            .special
            .as_ref()
            .and_then(|parameters| super::special::attack(action, parameters))
        {
            return Some(attack);
        }
        let index = super::aerial::attack_index(action)?;
        Some(&self.aerials.as_ref()?.moves[index].attack)
    }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HurtboxState {
    #[default]
    Enabled,
    Disabled,
    Intangible,
}

impl HurtboxState {
    /// `lbColl_80007ECC` accepts only `HurtCapsule_Enabled` for ordinary hits
    /// and grabboxes.
    pub const fn accepts_contact(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Fighter-wide `Fighter::x1988` collision state written by subaction body
/// scripts. Slippi serializes these exact values (0, 1, 2) before the timed
/// game-induced counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyState {
    #[default]
    Normal,
    Invincible,
    Intangible,
}

impl BodyState {
    /// `ftcoll.c` skips every hurt capsule while the fighter is intangible or
    /// invincible; the invincible branch registers no damage in this profile.
    pub const fn accepts_contact(self) -> bool {
        matches!(self, Self::Normal)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hurtbox {
    pub bone: usize,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub state: HurtboxState,
    #[serde(default = "default_true")]
    pub grabbable: bool,
}

impl Hurtbox {
    pub(crate) fn physics(&self) -> bones::BoneCapsule {
        bones::BoneCapsule {
            bone: self.bone,
            start: self.start,
            end: self.end,
            radius: self.radius,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attack {
    /// Native move-table identity. Sentinel1 is exempt from stale-move damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// Exactly one physics-pose sample per simulation frame, including recovery.
    /// No implicit interpolation or fallback for missing samples.
    pub frames: Vec<AttackFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackFrame {
    pub bones: Vec<Bone>,
    pub hitboxes: Vec<Hitbox>,
    /// Complete hurtbox eligibility sample for this frame. An empty sample
    /// preserves legacy resources by inheriting each hurtbox's base state.
    #[serde(default)]
    pub hurtbox_states: Vec<HurtboxState>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitElement {
    #[default]
    Normal,
    Inert,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hitbox {
    /// Ordinary collision-script clank/rebound bits. False retains older fixtures.
    #[serde(default)]
    pub clank: bool,
    #[serde(default)]
    pub rebound: bool,
    /// Collision element branch. Unsupported elements are rejected by serde.
    #[serde(default)]
    pub element: HitElement,
    /// Same-group hitboxes share victim history while active. The legacy profile
    /// without clank data retains its simpler per-attack group mask.
    pub group: u8,
    pub bone: usize,
    pub center: [f32; 3],
    pub radius: f32,
    pub damage: u32,
    /// Additional integer shield damage (HitCapsule::x34), before clamping.
    #[serde(default)]
    pub shield_damage: i32,
    /// Integral ordinary launch angles, 361 with explicit coefficients, or the
    /// contact-relative 362 angle used by fighter hitboxes.
    pub angle_degrees: f32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
}

#[cfg(test)]
mod tests {
    use super::HurtboxState;

    #[test]
    fn ordinary_contact_accepts_only_the_source_enabled_state() {
        assert!(HurtboxState::Enabled.accepts_contact());
        assert!(!HurtboxState::Disabled.accepts_contact());
        assert!(!HurtboxState::Intangible.accepts_contact());
    }
}
