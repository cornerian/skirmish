//! Grounded shield-evasion input predicates from `ftCo_Escape.c` and the
//! C-stick helpers in `ft_0DF1.c`. Callers own the motion transition, the
//! per-character Samus/Yoshi entry branches and the unread `x324` flag copy.

/// Roll direction relative to the fighter's facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollDirection {
    Forward,
    Backward,
}

/// `inlineA1`: a fresh main-stick horizontal excursion at or beyond `x31C`
/// whose shared tilt age is inside the `x320` window.
pub fn main_stick_roll(stick_x: f32, tilt_x_age: u8, threshold: f32, window: u8) -> bool {
    stick_x.abs() >= threshold && tilt_x_age < window
}

/// Complete `ftCo_800DF8B0`: held horizontal C-stick at or beyond `x31C`,
/// without any freshness requirement.
pub fn cstick_roll(cstick_x: f32, threshold: f32) -> bool {
    cstick_x.abs() >= threshold
}

/// `stick_x * facing_dir >= 0` selects EscapeF; the exact source comparison
/// keeps zero products (including signed zero and `-0.0` facing) forward.
pub fn roll_direction(stick_x: f32, facing: f32) -> RollDirection {
    if stick_x * facing >= 0.0 {
        RollDirection::Forward
    } else {
        RollDirection::Backward
    }
}

/// Complete `ftCo_8009917C` selection: the fresh main stick has priority, the
/// held C-stick is the fallback, and the chosen axis value picks the direction.
pub fn roll_request(
    stick_x: f32,
    tilt_x_age: u8,
    cstick_x: f32,
    facing: f32,
    threshold: f32,
    window: u8,
) -> Option<RollDirection> {
    let source = if main_stick_roll(stick_x, tilt_x_age, threshold, window) {
        stick_x
    } else if cstick_roll(cstick_x, threshold) {
        cstick_x
    } else {
        return None;
    };
    Some(roll_direction(source, facing))
}

/// `inlineB0`: a fresh downward main-stick excursion at or below `x314`
/// (a negative native value) inside the `x318` window.
pub fn main_stick_spot_dodge(stick_y: f32, tilt_y_age: u8, threshold: f32, window: u8) -> bool {
    stick_y <= threshold && tilt_y_age < window
}

/// Complete `ftCo_800DF8E8`: held downward C-stick at or below `x314`.
pub fn cstick_spot_dodge(cstick_y: f32, threshold: f32) -> bool {
    cstick_y <= threshold
}

/// Complete `ftCo_8009980C` selection without its transition side effect.
pub fn spot_dodge_request(
    stick_y: f32,
    tilt_y_age: u8,
    cstick_y: f32,
    threshold: f32,
    window: u8,
) -> bool {
    main_stick_spot_dodge(stick_y, tilt_y_age, threshold, window)
        || cstick_spot_dodge(cstick_y, threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_stick_roll_uses_inclusive_magnitude_and_strict_age() {
        assert!(main_stick_roll(0.7, 3, 0.7, 4));
        assert!(main_stick_roll(-0.7, 0, 0.7, 4));
        assert!(!main_stick_roll(0.699, 0, 0.7, 4));
        assert!(!main_stick_roll(1.0, 4, 0.7, 4));
        assert!(!main_stick_roll(f32::NAN, 0, 0.7, 4));
    }

    #[test]
    fn cstick_roll_ignores_freshness() {
        assert!(cstick_roll(0.7, 0.7));
        assert!(cstick_roll(-1.0, 0.7));
        assert!(!cstick_roll(0.69, 0.7));
        assert!(!cstick_roll(f32::NAN, 0.7));
    }

    #[test]
    fn direction_follows_the_signed_product_with_zero_forward() {
        assert_eq!(roll_direction(0.8, 1.0), RollDirection::Forward);
        assert_eq!(roll_direction(0.8, -1.0), RollDirection::Backward);
        assert_eq!(roll_direction(-0.8, -1.0), RollDirection::Forward);
        assert_eq!(roll_direction(0.0, -1.0), RollDirection::Forward);
        assert_eq!(roll_direction(-0.0, 1.0), RollDirection::Forward);
        assert_eq!(roll_direction(f32::NAN, 1.0), RollDirection::Backward);
    }

    #[test]
    fn roll_request_prefers_the_fresh_main_stick_over_the_held_cstick() {
        assert_eq!(
            roll_request(-1.0, 0, 1.0, 1.0, 0.7, 4),
            Some(RollDirection::Backward)
        );
        assert_eq!(
            roll_request(-1.0, 4, 1.0, 1.0, 0.7, 4),
            Some(RollDirection::Forward)
        );
        assert_eq!(
            roll_request(-1.0, 4, -1.0, 1.0, 0.7, 4),
            Some(RollDirection::Backward)
        );
        assert_eq!(roll_request(0.5, 0, 0.5, 1.0, 0.7, 4), None);
    }

    #[test]
    fn spot_dodge_accepts_fresh_main_stick_or_held_cstick() {
        assert!(main_stick_spot_dodge(-0.7, 3, -0.7, 4));
        assert!(!main_stick_spot_dodge(-0.69, 0, -0.7, 4));
        assert!(!main_stick_spot_dodge(-1.0, 4, -0.7, 4));
        assert!(cstick_spot_dodge(-0.7, -0.7));
        assert!(!cstick_spot_dodge(-0.69, -0.7));
        assert!(spot_dodge_request(-1.0, 0, 0.0, -0.7, 4));
        assert!(spot_dodge_request(-1.0, 200, -0.7, -0.7, 4));
        assert!(!spot_dodge_request(-1.0, 200, 0.0, -0.7, 4));
        assert!(!spot_dodge_request(f32::NAN, 0, f32::NAN, -0.7, 4));
    }
}
