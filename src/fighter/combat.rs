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
///
/// Three `a * b + c`-shaped subexpressions here are each a single Gekko
/// `fmadds` on the real hardware (`tools/ppc_fma_audit.py ftColl_80079AB0`;
/// `docs/math.md`), not a separately-rounded multiply then add, so each is
/// written with `f32::mul_add` to reproduce that one rounding: the two
/// (mutually exclusive) branch inner-terms below, and the shared growth/
/// scale terms after them.
pub fn knockback(
    rules: &KnockbackRules,
    hit: KnockbackHit,
    damage: DamageState,
    attack_damage: u32,
    modifiers: KnockbackModifiers,
) -> Result<f32, CombatError> {
    let w = modifiers.weight * rules.weight_scale;
    let inner = if hit.fixed != 0 {
        // `x118 * x110 + x114 * (x118 * x28)` -- the fixed-damage branch's
        // own `fmadds`.
        rules.damage_percent_scale.mul_add(
            rules.fixed_damage * hit.fixed as f32,
            rules.fixed_damage * rules.percent_scale,
        )
    } else {
        let count = match damage.count_override {
            Some(count) => count,
            None => integer(damage.percent)?,
        };
        let count_and_pending = count as f32 + damage.pending_damage;
        // `x110 * (count + pending) + x114 * (unk_count * (count + pending))`
        // -- the percent branch's own `fmadds`.
        rules.damage_percent_scale.mul_add(
            attack_damage as f32 * count_and_pending,
            rules.percent_scale * count_and_pending,
        )
    };
    let decay_factor = rules.weight_base - (w * rules.weight_base) / (1.0 + w);
    // `x11C * (decay_factor * inner) + x120`.
    let growth_term = rules
        .growth_scale
        .mul_add(decay_factor * inner, rules.growth_base);
    // `0.01 * growth * growth_term + x2C`.
    let scaled = (0.01 * hit.growth as f32).mul_add(growth_term, hit.base as f32);
    let result = modifiers.defense * (modifiers.attack * (modifiers.stage * scaled));
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
/// `dmg * x198 + x19C` is a single Gekko `fmadds`
/// (`tools/ppc_fma_audit.py ftCommon_CalcHitlag`; `docs/math.md`), so it is
/// computed with `f32::mul_add` rather than a separately-rounded multiply
/// and add.
pub fn hitlag(
    damage: i32,
    crouching: bool,
    multiplier: f32,
    rules: &HitlagRules,
) -> Result<f32, CombatError> {
    let base = integer((damage as f32).mul_add(rules.damage_scale, rules.base))?;
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

    /// `ftCommon_CalcHitlag`'s `dmg * x198 + x19C` is a single Gekko
    /// `fmadds` (`tools/ppc_fma_audit.py ftCommon_CalcHitlag`;
    /// `docs/math.md`), one rounding rather than two. At these inputs the
    /// two ways of computing it land on opposite sides of an integer
    /// boundary before the subsequent truncation, so a regression to plain
    /// `dmg as f32 * rules.damage_scale + rules.base` is caught by the
    /// *outcome*, not just an internal bit pattern: naive multiply-then-add
    /// gives `1772.9998779296875` (truncating to 1772), while the fused
    /// `f32::mul_add` -- matching the real hardware -- gives exactly
    /// `1773.0`. Values hand-verified against `libm`'s `fmaf` outside Rust.
    #[test]
    fn hitlag_matches_the_hardware_fused_rounding_not_naive_two_rounding() {
        let rules = HitlagRules {
            damage_scale: f32::from_bits(0x400f_d0da), // 2.247122287750244
            base: f32::from_bits(0xc00e_81a8),         // -2.226663589477539
            crouch_multiplier: 1.0,
        };
        assert_eq!(hitlag(790, false, 1.0, &rules), Ok(1773.0));
        assert_ne!(
            790_f32 * rules.damage_scale + rules.base,
            790_f32.mul_add(rules.damage_scale, rules.base),
            "the test inputs should straddle a rounding boundary; if this \
             assertion fails the inputs above no longer demonstrate anything"
        );
    }

    /// A recorded divergence from an early run of
    /// `tests/combat_differential.rs`'s `knockback_matches_c` (before that
    /// test was given the documented small-relative-tolerance comparison
    /// for the pinned `KNOCKBACK` macro's own two-products-summed `inner`
    /// term -- see that test's own comment and `docs/math.md`):
    /// `knockback`'s three `f32::mul_add` call sites reproduce
    /// `-1.1778402e17` (bits `0xdbd1_39f9`), hand-verified against `libm`'s
    /// `fmaf` outside Rust, bit-for-bit -- pinned here independent of the C
    /// oracle's own limitations for this function.
    #[test]
    fn knockback_matches_a_hardware_fused_value_pinned_from_the_retail_dol() {
        let rules = KnockbackRules {
            weight_scale: f32::from_bits(0x410a_51b5),
            weight_base: f32::from_bits(0xc0f4_771e),
            maximum: f32::from_bits(0xcc25_3ed9),
            percent_scale: f32::from_bits(0xc0aa_72d6),
            damage_percent_scale: f32::from_bits(0xc0f4_7858),
            fixed_damage: f32::from_bits(0x3df9_f25d),
            growth_scale: f32::from_bits(0x3fc6_2f3e),
            growth_base: f32::from_bits(0xc11e_1169),
        };
        let hit = KnockbackHit {
            growth: 154_012_587,
            fixed: 16_863_776,
            base: 4_234_953_701,
        };
        let damage = DamageState {
            percent: f32::from_bits(0xc301_4c57),
            pending_damage: f32::from_bits(0x446a_3863),
            count_override: Some(1_207_400_802),
        };
        let modifiers = KnockbackModifiers {
            stage: f32::from_bits(0x4116_28de),
            attack: f32::from_bits(0xc107_a386),
            defense: f32::from_bits(0xc11e_fa34),
            weight: f32::from_bits(0xbead_149b),
        };
        let result = knockback(&rules, hit, damage, 3_491_357_132, modifiers).unwrap();
        assert_eq!(result.to_bits(), 0xdbd1_39f9);
    }
}
