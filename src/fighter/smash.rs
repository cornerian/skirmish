//! Smash-attack input predicates from `ftCo_AttackS4.c`, `ftCo_AttackHi4.c`
//! and `ftCo_AttackLw4.c`, the fresh C-stick predicates from `ft_0DF1.c`,
//! and the smash charge arithmetic from `ft_0DF0.c`.

/// `Fighter::smash_attrs.state`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeState {
    #[default]
    None,
    PreCharge,
    Charging,
    Release,
}

/// `checkLStick` of `ftCo_AttackS4_CheckInput`: a fresh A press with the
/// stick at or beyond the dash-smash magnitude inside its window.
pub fn forward_smash_input(
    a_pressed: bool,
    stick_x: f32,
    tilt_x_age: u8,
    stick_threshold: f32,
    window: u8,
) -> bool {
    a_pressed && stick_x.abs() >= stick_threshold && tilt_x_age < window
}

/// Complete `ftCo_800DF1C8`: the C-stick crossed the dash-smash magnitude on
/// either side this frame.
pub fn fresh_cstick_smash_x(previous_x: f32, current_x: f32, stick_threshold: f32) -> bool {
    previous_x.abs() < stick_threshold && current_x.abs() >= stick_threshold
}

/// The facing the smash adopts: `x >= 0 ? +1 : -1`, so zero and negative
/// zero face right and NaN faces left.
pub fn stick_sign(x: f32) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// `checkLStick` of `ftCo_AttackHi4_CheckInput`: inclusive stick Y with the
/// byte age strictly below the float window `xD0`. `checkLStickNoD0`
/// (KneeBend) drops the window.
pub fn up_smash_input(
    a_pressed: bool,
    stick_y: f32,
    tilt_y_age: u8,
    threshold: f32,
    window: f32,
) -> bool {
    a_pressed && stick_y >= threshold && f32::from(tilt_y_age) < window
}

/// `checkLStick` of `ftCo_AttackLw4_CheckInput` with the float window `xD8`.
pub fn down_smash_input(
    a_pressed: bool,
    stick_y: f32,
    tilt_y_age: u8,
    threshold: f32,
    window: f32,
) -> bool {
    a_pressed && stick_y <= threshold && f32::from(tilt_y_age) < window
}

/// `ftCo_800DEEB8`: released charges scale damage by the charged fraction of
/// the extra multiplier; every other state leaves it untouched.
pub fn charge_damage(
    damage: f32,
    state: ChargeState,
    frames: f32,
    hold_frames: f32,
    multiplier: f32,
) -> f32 {
    if state != ChargeState::Release {
        return damage;
    }
    damage * ((multiplier - 1.0) * (frames / hold_frames) + 1.0)
}

/// `ftCo_800DF0D0`: the pre-charge frame commits to charging only while the
/// logical A is held, and releasing it ends a charge. Returns the new state.
pub fn charge_input(state: ChargeState, a_held: bool) -> ChargeState {
    match state {
        ChargeState::PreCharge => {
            if a_held {
                ChargeState::Charging
            } else {
                ChargeState::None
            }
        }
        ChargeState::Charging if !a_held => ChargeState::Release,
        other => other,
    }
}

/// `ftCo_800DEF38`'s charging branch: count a frame and release at the hold
/// limit, clamping the count to it. Returns the new state.
pub fn charge_tick(state: ChargeState, frames: &mut f32, hold_frames: f32) -> ChargeState {
    if state != ChargeState::Charging {
        return state;
    }
    *frames += 1.0;
    if *frames >= hold_frames {
        if *frames > hold_frames {
            *frames = hold_frames;
        }
        return ChargeState::Release;
    }
    ChargeState::Charging
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_input_needs_fresh_magnitude_inside_the_window() {
        assert!(forward_smash_input(true, -0.8, 3, 0.8, 4));
        assert!(!forward_smash_input(true, 0.79, 0, 0.8, 4));
        assert!(!forward_smash_input(true, 1.0, 4, 0.8, 4));
        assert!(!forward_smash_input(false, 1.0, 0, 0.8, 4));
    }

    #[test]
    fn cstick_crossing_uses_absolute_values_on_both_frames() {
        assert!(fresh_cstick_smash_x(0.0, 0.8, 0.8));
        assert!(fresh_cstick_smash_x(0.5, -1.0, 0.8));
        assert!(!fresh_cstick_smash_x(-0.8, 0.8, 0.8));
        assert!(!fresh_cstick_smash_x(0.0, 0.79, 0.8));
    }

    #[test]
    fn sign_treats_zero_as_forward() {
        assert_eq!(stick_sign(0.0), 1.0);
        assert_eq!(stick_sign(-0.0), 1.0);
        assert_eq!(stick_sign(-0.1), -1.0);
        assert_eq!(stick_sign(f32::NAN), -1.0);
    }

    #[test]
    fn vertical_inputs_are_inclusive_with_strict_windows() {
        assert!(up_smash_input(true, 0.7, 3, 0.7, 4.0));
        assert!(!up_smash_input(true, 0.7, 4, 0.7, 4.0));
        assert!(up_smash_input(true, 0.7, 4, 0.7, 4.5));
        assert!(down_smash_input(true, -0.7, 0, -0.7, 4.0));
        assert!(!down_smash_input(true, -0.69, 0, -0.7, 4.0));
    }

    #[test]
    fn charge_state_machine_matches_the_source_transitions() {
        assert_eq!(
            charge_input(ChargeState::PreCharge, true),
            ChargeState::Charging
        );
        assert_eq!(
            charge_input(ChargeState::PreCharge, false),
            ChargeState::None
        );
        assert_eq!(
            charge_input(ChargeState::Charging, true),
            ChargeState::Charging
        );
        assert_eq!(
            charge_input(ChargeState::Charging, false),
            ChargeState::Release
        );
        assert_eq!(
            charge_input(ChargeState::Release, true),
            ChargeState::Release
        );
        let mut frames = 58.0;
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut frames, 60.0),
            ChargeState::Charging
        );
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut frames, 60.0),
            ChargeState::Release
        );
        assert_eq!(frames, 60.0);
        let mut over = 60.5;
        assert_eq!(
            charge_tick(ChargeState::Charging, &mut over, 60.0),
            ChargeState::Release
        );
        assert_eq!(over, 60.0);
        assert_eq!(
            charge_tick(ChargeState::Release, &mut over, 60.0),
            ChargeState::Release
        );
    }

    #[test]
    fn charge_damage_scales_only_released_charges() {
        assert_eq!(
            charge_damage(10.0, ChargeState::Charging, 30.0, 60.0, 1.4),
            10.0
        );
        assert_eq!(
            charge_damage(10.0, ChargeState::Release, 30.0, 60.0, 1.4),
            12.0
        );
        assert_eq!(
            charge_damage(10.0, ChargeState::Release, 0.0, 60.0, 1.4),
            10.0
        );
    }
}
