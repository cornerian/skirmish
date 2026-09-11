//! Ordinary Landing from `ftCo_Landing.c`. `Action::LandingFallSpecial` (air
//! dodge and other special landings, `escape_air.rs`) is the separate,
//! currently always-locked-out state (`allow_interrupt = false`) and is not
//! touched here; this module owns only the plain animation-length Landing
//! entered from `collision.rs`.
use super::{Action, Fighter, data::FighterData};

pub(crate) fn owns_action(action: Action) -> bool {
    action == Action::Landing
}

/// `ftCo_Landing_IASA`'s first two `RETURN_IF`s (`ftCo_Landing.c:128-129`): a
/// lag floor gated by `co_attrs.normal_landing_lag` and
/// `mv.co.landing.allow_interrupt`. A missing `normal_landing_lag` keeps a
/// chainless Landing that never reaches the Wait chain.
pub(crate) fn interruptible(fighter: &Fighter, data: &FighterData) -> bool {
    fighter.grounded
        && owns_action(fighter.action)
        && fighter.landing_allow_interrupt
        && data
            .movement
            .normal_landing_lag
            .is_some_and(|lag| fighter.action_frame as f32 >= lag)
}

/// `ftCo_Landing_IASA:146-147`: the squat entry opens only while
/// `cur_anim_frame < frame_speed_mul + normal_landing_lag`. Ordinary
/// Landing's `anim_speed` is always `1.0` (`ftCo_Landing_Enter_Basic`), so
/// this is the single first interruptible frame only.
pub(crate) fn squat_window(fighter: &Fighter, data: &FighterData) -> bool {
    owns_action(fighter.action)
        && data
            .movement
            .normal_landing_lag
            .is_some_and(|lag| (fighter.action_frame as f32) < 1.0 + lag)
}
