//! Ordinary Landing from `ftCo_Landing.c`. `Action::LandingFallSpecial` (air
//! dodge and other special landings, `escape_air.rs`) is the separate,
//! currently always-locked-out state (`allow_interrupt = false`) and is not
//! touched here; this module owns only the plain animation-length Landing
//! entered from `collision.rs`.
use crate::game::{Action, Fighter, data::FighterData};

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
    // The squat check is reached only after Landing_IASA's lag and
    // allow_interrupt RETURN_IFs. Keep this helper equivalent when called
    // directly by a dispatcher: a LandingFallSpecial or a stale airborne
    // landing record must never open SquatWait.
    fighter.grounded
        && fighter.landing_allow_interrupt
        && owns_action(fighter.action)
        && data
            .movement
            .normal_landing_lag
            .is_some_and(|lag| (fighter.action_frame as f32) < 1.0 + lag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Match, data::MatchData};

    fn fixture() -> (Match, FighterData) {
        let mut data: MatchData = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration fixture decodes");
        data.fighters[0].movement.landing_frames = 6;
        data.fighters[0].movement.normal_landing_lag = Some(3.0);
        let fighter_data = data.fighters[0].clone();
        (
            Match::new(data, 0).expect("integration match loads"),
            fighter_data,
        )
    }

    #[test]
    fn squat_window_requires_ordinary_interruptible_landing() {
        let (mut game, data) = fixture();
        let fighter = &mut game.state.fighters[0];
        fighter.action = Action::Landing;
        fighter.action_frame = 3;
        fighter.grounded = true;

        fighter.landing_allow_interrupt = false;
        assert!(!squat_window(fighter, &data));

        fighter.landing_allow_interrupt = true;
        assert!(squat_window(fighter, &data));

        // Source special landings use the same lag data but enter with
        // `allow_interrupt = false`; ftCo_Landing_IASA therefore cannot open
        // SquatWait for them.
        fighter.action = Action::LandingFallSpecial;
        assert!(!squat_window(fighter, &data));

        fighter.grounded = false;
        assert!(!squat_window(fighter, &data));
    }
}
