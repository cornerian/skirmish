//! Explicit damage rules and the experimental scheduler's damage integration.
//! Native helpers preserve selected source arithmetic; action ordering, facing
//! selection, and floor response remain the documented match-slice policy.
use crate::{
    Action, Error, Event, Fighter, State,
    data::{Hitbox, MatchData},
};
use physics::{combat, damage};
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
) -> Result<(), Error> {
    let victim = 1 - attacker;
    let rules = &data.rules;
    let target = &state.fighters[victim];
    let knockback = combat::knockback(
        &rules.knockback.physics(),
        combat::KnockbackHit {
            growth: hit.growth,
            fixed: hit.fixed,
            base: hit.base,
        },
        combat::DamageState {
            percent: target.percent,
            pending_damage: hit.damage as f32,
            count_override: None,
        },
        hit.damage,
        combat::KnockbackModifiers {
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
            weight: data.fighters[victim].weight,
        },
    )
    .map_err(physics)?;
    let hitlag =
        combat::hitlag(hit.damage as i32, false, 1.0, &rules.hitlag.physics()).map_err(physics)?;
    let hitstun = combat::initial_hitstun(knockback, rules.hitstun_scale).map_err(physics)?;
    if !knockback.is_finite() || !hitlag.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 || hitlag < 0.0 || hitstun < 1 {
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
    let merged = damage::merge_knockback(
        target.knockback,
        incoming,
        target.damage_elapsed,
        rules.damage.knockback_replace_window,
    );
    if !angle.radians.is_finite() || merged.into_iter().any(|value| !value.is_finite()) {
        return Err(Error::NonFinite);
    }
    state.fighters[attacker].hitlag = state.fighters[attacker].hitlag.max(hitlag);
    let target = &mut state.fighters[victim];
    target.percent = (target.percent + hit.damage as f32).min(999.0);
    target.hitlag = target.hitlag.max(hitlag);
    target.hitstun = hitstun as u32;
    target.velocity = [0.0; 2];
    target.ground_velocity = 0.0;
    target.knockback = merged;
    target.grounded = false;
    target.ground_line = None;
    target.fast_fall = false;
    target.action = Action::Damage;
    target.action_frame = 0;
    target.hit_groups = 0;
    target.hitboxes = [crate::hitboxes::Track::default(); 4];
    target.damage_elapsed = 0;
    target.damage_angle_flag = 0;
    if let Some(timer) = angle.special_timer {
        target.damage_angle_flag = 1;
        target.damage_angle_timer = timer;
    }
    // A zero-hitlag hit has no expiry callback in the examined upstream path.
    target.di_pending = target.hitlag > 0.0;
    state.events.push(Event::Hit {
        attacker,
        victim,
        damage: hit.damage as f32,
        knockback,
    });
    Ok(())
}

/// Called once when positive hitlag reaches zero, before normal motion resumes.
/// ASDI, SDI and LR/vcancel are separate behaviors and are not inferred here.
pub(crate) fn exit_hitlag(
    fighter: &mut Fighter,
    stick: [f32; 2],
    rules: &CombatRules,
) -> Result<(), Error> {
    if fighter.di_pending {
        let influenced =
            damage::directional_influence(fighter.knockback, stick, rules.di_max_degrees);
        if influenced.into_iter().any(|value| !value.is_finite()) {
            return Err(Error::NonFinite);
        }
        fighter.knockback = influenced;
        fighter.di_pending = false;
    }
    Ok(())
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
