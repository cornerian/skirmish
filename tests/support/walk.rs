use skirmish::game::{
    data::MatchData,
    locomotion::{WalkAnimation, WalkRules},
};

/// Invented walk-kind thresholds
/// (`walk_middle_animation_stick_threshold`/`walk_fast_stick_threshold`):
/// fractions of `walk_max_velocity` (1.5 in the integration fixture) chosen
/// so a full-stick walk ramp from a stand reaches all three kinds within a
/// few frames.
pub const RULES: WalkRules = WalkRules {
    middle_threshold: 0.3,
    fast_threshold: 0.7,
};

/// Invented figatree lengths and animation-rate divisors (not the source
/// ISO's WalkSlow/Middle/Fast data). Distinct lengths make each kind's
/// wrap point and the retype remap arithmetic independently observable.
pub const ANIMATION: WalkAnimation = WalkAnimation {
    lengths: [10.0, 12.0, 14.0],
    rates: [1.5, 1.0, 0.75],
};

/// Install `rules.walk` and `movement.walk_animation` on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.walk = Some(RULES);
    for fighter in &mut data.fighters {
        fighter.movement.walk_animation = Some(ANIMATION);
    }
    data
}
