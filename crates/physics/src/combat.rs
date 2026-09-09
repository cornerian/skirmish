//! Selected native combat calculations with caller-provided game data.
//!
//! This covers capsule/sphere geometry, the base knockback formula, hitlag, and
//! the initial hitstun counter. It does not resolve full capsule pairs, hitbox
//! priorities, stale-move damage, armor, throws, directional influence, or move
//! scheduling. Source offsets identify coefficients whose asset values must be
//! supplied by the caller; this module invents no character or common constants.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
}

/// `lbColl_80005C44`: inclusive capsule/sphere intersection. `closest` receives
/// the point on the capsule axis unless the initial axis-aligned test rejects.
/// Degenerate axes use the original strict +/-0.00001 squared-length threshold.
pub fn capsule_sphere(
    capsule: &Capsule,
    center: [f32; 3],
    sphere_radius: f32,
    closest: &mut [f32; 3],
) -> bool {
    let radius = capsule.radius + sphere_radius;
    for (i, &coordinate) in center.iter().enumerate() {
        if capsule.start[i] > capsule.end[i] {
            if capsule.start[i] + radius < coordinate || capsule.end[i] - radius > coordinate {
                return false;
            }
        } else if capsule.start[i] - radius > coordinate || capsule.end[i] + radius < coordinate {
            return false;
        }
    }
    let delta = core::array::from_fn(|i| capsule.end[i] - capsule.start[i]);
    let offset = core::array::from_fn(|i| capsule.start[i] - center[i]);
    let length = dot(delta, delta);
    let projection = dot(delta, offset);
    let scale = if length < 0.00001 && length > -0.00001 {
        0.0
    } else {
        (-projection / length).clamp(0.0, 1.0)
    };
    *closest = core::array::from_fn(|i| delta[i] * scale + capsule.start[i]);
    let separation = core::array::from_fn(|i| closest[i] - center[i]);
    // The original negated comparison returns true for an unordered distance.
    (radius * radius).partial_cmp(&dot(separation, separation)) != Some(core::cmp::Ordering::Less)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnockbackRules {
    /// `ftCommonData::xF4`.
    pub weight_scale: f32,
    /// `ftCommonData::xF8`.
    pub weight_base: f32,
    /// `ftCommonData::x108`.
    pub maximum: f32,
    /// `ftCommonData::x110`.
    pub percent_scale: f32,
    /// `ftCommonData::x114`.
    pub damage_percent_scale: f32,
    /// `ftCommonData::x118`.
    pub fixed_damage: f32,
    /// `ftCommonData::x11C`.
    pub growth_scale: f32,
    /// `ftCommonData::x120`.
    pub growth_base: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnockbackHit {
    /// `HitCapsule::x24`.
    pub growth: u32,
    /// `HitCapsule::x28`: nonzero selects the fixed-knockback formula.
    pub fixed: u32,
    /// `HitCapsule::x2C`.
    pub base: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageState {
    /// `Fighter::dmg.x1830_percent`, truncated before the formula.
    pub percent: f32,
    /// `Fighter::dmg.x1838_percentTemp`.
    pub pending_damage: f32,
    /// When `x2225_b7` is set, the caller supplies `x6D8[0]` if `x2224_b2`
    /// is set, or `x6D4` otherwise. `None` selects ordinary percent truncation.
    pub count_override: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnockbackModifiers {
    pub stage: f32,
    pub attack: f32,
    pub defense: f32,
    pub weight: f32,
}

/// `ftColl_80079AB0`, with damage-count override selection represented explicitly.
/// `attack_damage` is its `unk_count` argument, not necessarily the final damage.
/// The source's nested order and inclusive maximum comparison are preserved.
pub fn knockback(
    rules: &KnockbackRules,
    hit: KnockbackHit,
    damage: DamageState,
    attack_damage: u32,
    modifiers: KnockbackModifiers,
) -> Result<f32, CombatError> {
    let w = modifiers.weight * rules.weight_scale;
    let inner = if hit.fixed != 0 {
        rules.fixed_damage * rules.percent_scale
            + rules.damage_percent_scale * (rules.fixed_damage * hit.fixed as f32)
    } else {
        let count = match damage.count_override {
            Some(count) => count,
            None => integer(damage.percent)?,
        };
        rules.percent_scale * (count as f32 + damage.pending_damage)
            + rules.damage_percent_scale
                * (attack_damage as f32 * (count as f32 + damage.pending_damage))
    };
    let result = modifiers.defense
        * (modifiers.attack
            * (modifiers.stage
                * ((0.01
                    * hit.growth as f32
                    * (rules.growth_scale
                        * ((rules.weight_base - (w * rules.weight_base) / (1.0 + w)) * inner)
                        + rules.growth_base))
                    + hit.base as f32)));
    Ok(if result >= rules.maximum {
        rules.maximum
    } else {
        result
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitlagRules {
    /// `ftCommonData::x198`.
    pub damage_scale: f32,
    /// `ftCommonData::x19C`.
    pub base: f32,
    /// `ftCommonData::x1A0`.
    pub crouch_multiplier: f32,
}

/// `ftCommon_CalcHitlag`. `crouching` means the source motion is Squat or
/// SquatWait; truncation occurs after each of the original three stages.
pub fn hitlag(
    damage: i32,
    crouching: bool,
    multiplier: f32,
    rules: &HitlagRules,
) -> Result<f32, CombatError> {
    let base = integer(damage as f32 * rules.damage_scale + rules.base)?;
    let result = integer(base as f32 * multiplier)? as f32;
    Ok(if crouching {
        integer(result * rules.crouch_multiplier)? as f32
    } else {
        result
    })
}

/// The hitstun-counter initialization in `ftCo_8008DCE0`: multiply by the
/// caller's `ftCommonData::x154`, truncate, and replace zero with one.
/// This is a selected excerpt, not a translation of that entire state transition.
/// The source then stores this value in a float timer. Callers simulating its
/// countdown must preserve that storage or bound durations to exact f32 integers.
pub fn initial_hitstun(knockback: f32, scale: f32) -> Result<i32, CombatError> {
    let frames = integer(knockback * scale)?;
    Ok(if frames == 0 { 1 } else { frames })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatError {
    UndefinedIntegerConversion,
}

impl core::fmt::Display for CombatError {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        out.write_str("float-to-i32 conversion is undefined for this combat input")
    }
}

impl core::error::Error for CombatError {}

fn integer(value: f32) -> Result<i32, CombatError> {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        Ok(value as i32)
    } else {
        Err(CombatError::UndefinedIntegerConversion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_includes_tangency_and_preserves_rejected_output() {
        let capsule = Capsule {
            start: [-1.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        let mut closest = [99.0; 3];
        assert!(capsule_sphere(&capsule, [0.0, 2.0, 0.0], 1.0, &mut closest));
        assert_eq!(closest, [0.0; 3]);
        assert!(!capsule_sphere(
            &capsule,
            [0.0, 2.0001, 0.0],
            1.0,
            &mut closest
        ));
        assert_eq!(closest, [0.0; 3]);
    }

    #[test]
    fn timing_retains_intermediate_truncation() {
        let rules = HitlagRules {
            damage_scale: 0.3,
            base: 1.0,
            crouch_multiplier: 0.5,
        };
        assert_eq!(hitlag(9, false, 1.5, &rules), Ok(4.0));
        assert_eq!(hitlag(9, true, 1.5, &rules), Ok(2.0));
        assert_eq!(initial_hitstun(0.0, 1.0), Ok(1));
        assert_eq!(initial_hitstun(3.9, 1.0), Ok(3));
        assert!(initial_hitstun(f32::INFINITY, 1.0).is_err());
    }
}
