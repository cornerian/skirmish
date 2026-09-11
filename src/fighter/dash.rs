//! Dash-phase input predicates and physics arithmetic from `ftCo_Dash.c`,
//! the facing-kept branch of `ftCo_AttackS4_8008C114`, `ftCo_800D8AE0` and
//! `ftCommon_ApplyFrictionGround`.

/// The three input phases `ftCo_Dash_IASA` dispatches per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Early,
    Middle,
    Late,
}

/// `ftCo_Dash_IASA`'s outer phase selection: `dash.x4 && frame <= x44` is
/// Early; otherwise `frame <= x4C` is Middle (this is also every frame of a
/// Turn-entered dash, since `dash.x4` is false there and the Early guard
/// never passes); anything past that is Late.
pub fn phase(from_input: bool, frame: f32, early: f32, middle: f32) -> Phase {
    if from_input && frame <= early {
        Phase::Early
    } else if frame <= middle {
        Phase::Middle
    } else {
        Phase::Late
    }
}

/// `ftCo_800D8AE0`'s tail: LR held with a nonzero buffer starts CatchDash;
/// otherwise the buffer counts down. Unlike `ftCo_800D8B9C`
/// (`dash_shield_grab`), no A press is required.
pub fn attack_dash_grab(lr_held: bool, buffer: &mut f32) -> bool {
    if lr_held && *buffer != 0.0 {
        return true;
    }
    if *buffer != 0.0 {
        *buffer -= 1.0;
    }
    false
}

/// `checkFacingDir` of `ftCo_AttackS4_8008C114`: a fresh A press with the
/// facing-relative stick at or beyond the dash-smash threshold. Unlike the
/// ordinary Wait-chain check, there is no stick-age window.
pub fn dash_forward_smash(a_pressed: bool, stick_x: f32, facing: f32, threshold: f32) -> bool {
    a_pressed && stick_x * facing >= threshold
}

/// `fp->gr_vel += -(fp->gr_vel * x54) * friction_mul`: the shared block at
/// the end of `ftCo_Dash_IASA`, reached by every phase branch that falls out
/// of its `if`/`else` instead of returning (see `docs/dash-attack.md`).
/// `ft_GetGroundFrictionMultiplier` (the stage/metal multiplier) is fixed at
/// 1.0 in this profile.
pub fn transition_friction(gr_vel: f32, x54: f32, friction_mul: f32) -> f32 {
    gr_vel + -(gr_vel * x54) * friction_mul
}

/// `ftCommon_ApplyFrictionGround`: one frame of deceleration toward zero by
/// `amount`, without overshooting past zero. Returns the new ground velocity
/// (the source instead stores the acceleration delta that yields it).
pub fn apply_friction(gr_vel: f32, amount: f32) -> f32 {
    let mut friction = amount;
    if friction.abs() > gr_vel.abs() {
        friction = -gr_vel;
    } else if gr_vel > 0.0 {
        friction = -friction;
    }
    gr_vel + friction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_needs_input_entry_and_the_early_limit() {
        assert_eq!(phase(true, 3.0, 3.0, 12.0), Phase::Early);
        assert_eq!(phase(true, 4.0, 3.0, 12.0), Phase::Middle);
        assert_eq!(phase(false, 0.0, 3.0, 12.0), Phase::Middle);
        assert_eq!(phase(true, 12.0, 3.0, 12.0), Phase::Middle);
        assert_eq!(phase(true, 13.0, 3.0, 12.0), Phase::Late);
        assert_eq!(phase(false, 13.0, 3.0, 12.0), Phase::Late);
    }

    #[test]
    fn attack_dash_grab_counts_down_only_while_armed() {
        let mut buffer = 2.0;
        assert!(!attack_dash_grab(false, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(attack_dash_grab(true, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(!attack_dash_grab(false, &mut buffer));
        assert_eq!(buffer, 0.0);
        assert!(!attack_dash_grab(true, &mut buffer));
        assert_eq!(buffer, 0.0);
    }

    #[test]
    fn dash_forward_smash_has_no_age_window() {
        assert!(dash_forward_smash(true, 0.8, 1.0, 0.8));
        assert!(dash_forward_smash(true, -0.8, -1.0, 0.8));
        assert!(!dash_forward_smash(true, 0.79, 1.0, 0.8));
        assert!(!dash_forward_smash(false, 1.0, 1.0, 0.8));
    }

    #[test]
    fn transition_friction_scales_by_the_fraction_and_multiplier() {
        assert_eq!(transition_friction(10.0, 0.25, 1.0), 7.5);
        assert_eq!(transition_friction(-10.0, 0.25, 1.0), -7.5);
        assert_eq!(transition_friction(10.0, 0.25, 0.0), 10.0);
    }

    #[test]
    fn apply_friction_clamps_at_zero_without_overshoot() {
        assert_eq!(apply_friction(5.0, 2.0), 3.0);
        assert_eq!(apply_friction(-5.0, 2.0), -3.0);
        assert_eq!(apply_friction(1.0, 5.0), 0.0);
        assert_eq!(apply_friction(-1.0, 5.0), 0.0);
        assert_eq!(apply_friction(0.0, 0.0), 0.0);
    }
}
