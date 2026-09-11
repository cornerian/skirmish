use skirmish::game::{data::MatchData, locomotion::RunAnimation};

/// Invented Run figatree length. `scaling` equals `tests/fixtures/game/
/// locomotion.json`'s `dash_max_velocity` (2.5) so a fighter at top ground
/// speed gets an animation rate of exactly 1.0 (matching
/// `ftCo_Run_Anim`'s own `ABS(vel) / run_animation_scaling`, `ftCo_Run.c:92`),
/// not the source ISO's own Run figatree/`run_animation_scaling` data.
pub const ANIMATION: RunAnimation = RunAnimation {
    length: 20.0,
    scaling: 2.5,
};

/// Install `movement.run_animation` on both fighters. Unlike Walk, no
/// `Rules` pairing is required.
pub fn profile(mut data: MatchData) -> MatchData {
    for fighter in &mut data.fighters {
        fighter.movement.run_animation = Some(ANIMATION);
    }
    data
}
