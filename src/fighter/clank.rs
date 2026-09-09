//! Ordinary grounded hitbox clashes from `ftColl_8007699C` and Rebound.
//!
//! Callers provide post-staling damage, confirm swept overlap and preserve the
//! original pair/slot order. This is the ordinary non-Slash, non-Catch, non-Inert
//! fighter-hit branch; grabs, items, bypass flags and the scheduler are external.
//! Slash-vs-Slash consumes shared RNG for sound and is deliberately unsupported.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Common data x3CC; the comparison is strict after truncating both damages.
    pub damage_gap: i32,
    /// Common data x3D0 and x3D4.
    pub duration_scale: f32,
    pub duration_base: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Victim {
    /// Zero is an empty slot, otherwise a stable native fighter/object identity.
    pub id: u32,
    /// Existing timer is preserved on duplicate type-3 registration.
    pub remaining: u32,
}

/// Original victims_1 table and x44 replacement cursor, specialized to type 3.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Victims {
    entries: [Victim; 12],
    next: u8,
}
impl Victims {
    pub fn from_parts(entries: [Victim; 12], next: u8) -> Option<Self> {
        (next < 12).then_some(Self { entries, next })
    }
    pub fn entries(&self) -> &[Victim; 12] {
        &self.entries
    }
    pub fn next(&self) -> u8 {
        self.next
    }
    pub fn contains(&self, id: u32) -> bool {
        id != 0 && self.entries.iter().any(|entry| entry.id == id)
    }
    /// `lbColl_80008688(type=3)`: fill the first hole before using the ring.
    /// A duplicate preserves both its old timer and the replacement cursor.
    pub fn record(&mut self, id: u32) -> Result<bool, Error> {
        if id == 0 {
            return Err(Error::InvalidIdentity);
        }
        if self.contains(id) {
            return Ok(false);
        }
        let index = match self.entries.iter().position(|entry| entry.id == 0) {
            Some(index) => index,
            None => {
                let index = usize::from(self.next);
                self.next = (self.next + 1) % 12;
                index
            }
        };
        self.entries[index] = Victim { id, remaining: 0 };
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Hit {
    pub enabled: bool,
    pub group: u32,
    /// Cached HitCapsule::damage, including staling. Not raw attack damage.
    pub damage: f32,
    pub clank: bool,
    pub rebound: bool,
    pub hits_grounded: bool,
    pub victims: Victims,
}

/// Fighter's pending dmg.int_value, x191C and dmg.facing_dir. Start a collision
/// pass with the caller's existing accumulators; only a strictly greater damage
/// overwrites the response. A non-rebounding hit leaves old duration/direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Response {
    pub damage: i32,
    pub rebound_duration: f32,
    pub towards: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fighter {
    pub id: u32,
    pub grounded: bool,
    pub x: f32,
    pub hits: [Hit; 4],
    pub response: Response,
}

/// Eligibility within the declared ordinary branch of ftColl_80078C70.
/// Capture restrictions, disabled fighter hits and bypass flags must be excluded
/// by the caller. Hitlag alone is not an exclusion in the original collision proc.
pub fn eligible(fighters: &[Fighter; 2], slots: [usize; 2]) -> bool {
    fighters[0].id != 0
        && fighters[1].id != 0
        && fighters[0].id != fighters[1].id
        && fighters.iter().enumerate().all(|(side, f)| {
            f.grounded
                && f.hits.get(slots[side]).is_some_and(|hit| {
                    hit.enabled
                        && hit.clank
                        && hit.hits_grounded
                        && !hit.victims.contains(fighters[1 - side].id)
                })
        })
}

/// The complete ordinary gameplay mutation of ftColl_8007699C, after overlap.
/// `second_candidates` is the caller's ftColl_804D6560 bitmap. First the second
/// side is suppressed, then the first; the return value means the FIRST side was
/// suppressed. Its caller breaks that slot scan only on true. Same-group active
/// slots register the opponent even if those other slots cannot themselves clank.
/// No stale-move queue is updated. Errors preserve both fighters and the bitmap.
/// Like the original callee, this function does not repeat `eligible` checks.
pub fn resolve_pair(
    fighters: &mut [Fighter; 2],
    slots: [usize; 2],
    second_candidates: &mut [bool; 4],
    rules: &Rules,
) -> Result<bool, Error> {
    if fighters.iter().any(|f| f.id == 0) || fighters[0].id == fighters[1].id {
        return Err(Error::InvalidIdentity);
    }
    if slots.into_iter().any(|slot| slot >= 4) {
        return Err(Error::InvalidSlot);
    }
    if rules.damage_gap < 0
        || ![rules.duration_scale, rules.duration_base]
            .into_iter()
            .all(f32::is_finite)
        || fighters.iter().any(|f| {
            f.response.damage < 0
                || ![f.x, f.response.rebound_duration, f.response.towards]
                    .into_iter()
                    .all(f32::is_finite)
        })
    {
        return Err(Error::InvalidParameters);
    }
    let damage = [
        fighters[0].hits[slots[0]].damage,
        fighters[1].hits[slots[1]].damage,
    ];
    let integers = [integer_damage(damage[0])?, integer_damage(damage[1])?];
    let mut next = fighters.clone();
    let mut candidates = *second_candidates;
    for side in [1, 0] {
        if integers[side] - rules.damage_gap < integers[1 - side] {
            let hit = next[side].hits[slots[side]];
            let opponent = next[1 - side].id;
            for (slot, same_group) in next[side].hits.iter_mut().enumerate() {
                if same_group.enabled && same_group.group == hit.group {
                    let inserted = same_group.victims.record(opponent)?;
                    if side == 1 && inserted {
                        candidates[slot] = false;
                    }
                }
            }
            // getEnvDmg's nonzero fractional minimum affects hitlag/rebound,
            // but does not affect the preceding priority comparison.
            let amount = if damage[side] != 0.0 && integers[side] == 0 {
                1
            } else {
                integers[side]
            };
            if amount > next[side].response.damage {
                next[side].response.damage = amount;
                if hit.rebound && next[side].grounded {
                    let duration = amount as f32 * rules.duration_scale + rules.duration_base;
                    if !duration.is_finite() {
                        return Err(Error::InvalidParameters);
                    }
                    next[side].response.rebound_duration = duration;
                    next[side].response.towards = if next[side].x < next[1 - side].x {
                        1.0
                    } else {
                        -1.0
                    };
                }
            }
        }
    }
    *fighters = next;
    *second_candidates = candidates;
    Ok(integers[0] - rules.damage_gap < integers[1])
}

fn integer_damage(damage: f32) -> Result<i32, Error> {
    if (0.0..2_147_483_648.0).contains(&damage) {
        Ok(damage as i32)
    } else {
        Err(Error::InvalidDamage)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReboundRules {
    pub animation_length: f32,
    /// Common data x3D8 and x3DC.
    pub push_scale: f32,
    pub push_base: f32,
    /// Result of ft_GetGroundFrictionMultiplier, including its caller policy.
    pub surface_friction_multiplier: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rebound {
    pub animation_rate: f32,
    /// mv.co.rebound.x0: original raw impulse, also the first-physics marker.
    pub impulse: f32,
    /// xE8_ground_accel_2, integrated into gr_vel after projecting self velocity.
    pub ground_acceleration: f32,
}

/// Arithmetic of ftCo_80099D9C and ftCommon_800804A0. The caller has selected
/// rebound after damage/shield precedence and supplied a positive duration.
pub fn rebound(duration: f32, towards: f32, rules: &ReboundRules) -> Result<Rebound, Error> {
    if duration <= 0.0
        || ![
            duration,
            towards,
            rules.animation_length,
            rules.push_scale,
            rules.push_base,
            rules.surface_friction_multiplier,
        ]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(Error::InvalidParameters);
    }
    let impulse = -towards * (duration * rules.push_scale + rules.push_base);
    let ground_acceleration = if rules.surface_friction_multiplier < 1.0 {
        impulse * rules.surface_friction_multiplier
    } else {
        impulse
    };
    let animation_rate = (rules.animation_length + 0.1) / duration;
    if ![impulse, ground_acceleration, animation_rate]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(Error::InvalidParameters);
    }
    Ok(Rebound {
        animation_rate,
        impulse,
        ground_acceleration,
    })
}

/// ftCo_Rebound_Phys: clear a nonzero raw-impulse marker and skip friction once.
/// A zero impulse (including -0) takes the friction branch immediately.
pub fn apply_rebound_friction(impulse: &mut f32) -> bool {
    if *impulse != 0.0 {
        *impulse = 0.0;
        false
    } else {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("clank requires distinct nonzero native fighter identities")]
    InvalidIdentity,
    #[error("clank hitbox slot is outside 0..4")]
    InvalidSlot,
    #[error("cached clank damage is outside the nonnegative C int conversion range")]
    InvalidDamage,
    #[error("invalid or nonfinite clank/rebound parameters")]
    InvalidParameters,
}
