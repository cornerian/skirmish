//! Fox/Falco side-special (Illusion/Phantasm) scalar arithmetic from
//! `ftCo_SpecialS.c` and the entry portion of `ftfoxspecials.c`. Callers own
//! state, motion transitions and resource dispatch.

/// `ftCo_SpecialS_HasInput` (`ftCo_SpecialS.c:15-23`): a fresh B press with
/// the stick past the side threshold on either sign.
pub fn has_input(fresh_b_press: bool, stick_x: f32, side_threshold: f32) -> bool {
    fresh_b_press && stick_x.abs() >= side_threshold
}

/// The turn branch shared by `ftCo_SpecialS_CheckInput` (`ftCo_SpecialS.c:
/// 25-39`) and the side branch of `ftCo_SpecialAir_CheckInput`
/// (`ftCo_SpecialAir.c:32-42`): turn when the stick opposes the current
/// facing beyond the turn threshold.
pub fn should_turn(stick_x: f32, facing: f32, turn_threshold: f32) -> bool {
    stick_x * facing < -turn_threshold
}

/// `ftCo_800C97A8` (`ftCo_Turn.c:28-36`): the ordinary standing-turn
/// predicate, reused verbatim by Fox/Falco's Reflector Loop/Turn IASA
/// (`ftFx_SpecialLwTurn_Check`) mid-move. `turn_threshold` is the same
/// (already negative) common-data value as ordinary standing Turn's own
/// `locomotion::Parameters::turn_threshold`, not side special's own `x220`
/// entry-turn threshold above.
pub fn should_turn_mid_move(stick_x: f32, facing: f32, turn_threshold: f32) -> bool {
    stick_x * facing <= turn_threshold
}

/// `doEnter` (`ftCo_SpecialS.c:41-49`): blend ground velocity toward zero by
/// the ground-speed-retention fraction. The source additionally scales this
/// by `ft_GetGroundFrictionMultiplier(fp)` (`ft_081B.c:1235-1240`), the
/// current floor material's friction multiplier (1.0 on ordinary terrain,
/// used for Ice Climbers' fixed 1.0 and other surfaces' `mpColl_8004CA6C`
/// lookup); this profile has no modeled per-surface friction-material table,
/// so it always applies the ordinary-terrain value 1.0. Report and keep this
/// simplification rather than silently claim full parity on textured floors.
pub fn entry_ground_velocity(ground_velocity: f32, retention: f32) -> f32 {
    ground_velocity + -(ground_velocity * (1.0 - retention))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_input_requires_both_the_press_and_the_stick_threshold() {
        assert!(has_input(true, 0.3, 0.2875));
        assert!(!has_input(false, 0.3, 0.2875));
        assert!(!has_input(true, 0.1, 0.2875));
        assert!(has_input(true, -0.3, 0.2875));
    }

    #[test]
    fn should_turn_uses_the_strict_source_comparison() {
        assert!(should_turn(-1.0, 1.0, 0.5));
        assert!(!should_turn(-0.5, 1.0, 0.5));
        assert!(!should_turn(1.0, 1.0, 0.5));
    }

    #[test]
    fn should_turn_mid_move_uses_the_inclusive_source_comparison() {
        // facing 1.0, turn_threshold -0.3 (already negative, matching
        // locomotion::Parameters::turn_threshold's own validated range).
        assert!(should_turn_mid_move(-0.5, 1.0, -0.3));
        // Exactly at the boundary still turns (source uses `<=`, unlike
        // should_turn's strict `<`).
        assert!(should_turn_mid_move(-0.3, 1.0, -0.3));
        assert!(!should_turn_mid_move(-0.2, 1.0, -0.3));
        assert!(!should_turn_mid_move(0.5, 1.0, -0.3));
    }

    #[test]
    fn entry_ground_velocity_blends_toward_zero() {
        assert_eq!(entry_ground_velocity(10.0, 0.0), 0.0);
        assert_eq!(entry_ground_velocity(10.0, 1.0), 10.0);
        assert_eq!(entry_ground_velocity(10.0, 0.5), 5.0);
    }
}
