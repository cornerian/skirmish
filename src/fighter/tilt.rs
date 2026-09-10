//! Grounded tilt input predicates from `ftCo_AttackS3.c`, `ftCo_AttackHi3.c`
//! and `ftCo_AttackLw3.c`. The stick angle is `ftCo_GetLStickAngle`
//! (`atan2f(y, |x|)`), supplied by callers; item branches are not modeled.

/// Forward-tilt angle variants selected by `decideAngle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardVariant {
    High,
    HighSlight,
    Straight,
    LowSlight,
    Low,
}

/// `ftCo_AttackS3_CheckInput` after its item branches: a fresh A press with
/// the facing-relative stick at or beyond `x98` and the folded stick angle
/// strictly inside `x20_radians`.
pub fn forward_tilt(
    a_pressed: bool,
    stick_x: f32,
    facing: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_x * facing >= stick_threshold && angle.abs() < angle_limit
}

/// Complete `decideAngle` selection. `thresholds` are `x9C`, `xA0`, `xA4`
/// and `xA8` in radians; `available` says whether the High, HighSlight,
/// LowSlight and Low animations exist (the source tests their figatree
/// entries, which the decomp labels with the motion ids of neighboring
/// states).
pub fn forward_variant(angle: f32, thresholds: [f32; 4], available: [bool; 4]) -> ForwardVariant {
    let [high, high_slight, low_slight, low] = thresholds;
    let [has_high, has_high_slight, has_low_slight, has_low] = available;
    if angle > high && has_high {
        ForwardVariant::High
    } else if angle > high_slight && has_high_slight {
        ForwardVariant::HighSlight
    } else if angle < low && has_low {
        ForwardVariant::Low
    } else if angle < low_slight && has_low_slight {
        ForwardVariant::LowSlight
    } else {
        ForwardVariant::Straight
    }
}

/// `ftCo_AttackHi3_CheckInput` after its item branch.
pub fn up_tilt(
    a_pressed: bool,
    stick_y: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_y >= stick_threshold && angle > angle_limit
}

/// `ftCo_AttackLw3_CheckInput` and its in-action `checkItemThrowInput`
/// after their item branches.
pub fn down_tilt(
    a_pressed: bool,
    stick_y: f32,
    angle: f32,
    stick_threshold: f32,
    angle_limit: f32,
) -> bool {
    a_pressed && stick_y <= stick_threshold && angle < -angle_limit
}

/// `checkPadA`: a fresh A press re-enters the down tilt once the script has
/// raised its repeat flag, and otherwise arms the `attacklw3.x0` buffer.
/// Returns whether the tilt restarts now.
pub fn down_tilt_repeat(a_pressed: bool, repeat_ready: bool, buffer: &mut bool) -> bool {
    if !a_pressed {
        return false;
    }
    if repeat_ready {
        return true;
    }
    *buffer = true;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_tilt_uses_inclusive_stick_and_strict_angle_bounds() {
        assert!(forward_tilt(true, 0.5, 1.0, 0.49, 0.5, 0.5));
        assert!(forward_tilt(true, -0.5, -1.0, -0.49, 0.5, 0.5));
        assert!(!forward_tilt(true, 0.5, 1.0, 0.5, 0.5, 0.5));
        assert!(!forward_tilt(true, 0.49, 1.0, 0.0, 0.5, 0.5));
        assert!(!forward_tilt(false, 1.0, 1.0, 0.0, 0.5, 0.5));
        assert!(!forward_tilt(true, f32::NAN, 1.0, 0.0, 0.5, 0.5));
    }

    #[test]
    fn forward_variant_prefers_high_then_slight_then_low_and_honors_availability() {
        let thresholds = [0.4, 0.2, -0.2, -0.4];
        let all = [true; 4];
        assert_eq!(forward_variant(0.5, thresholds, all), ForwardVariant::High);
        assert_eq!(
            forward_variant(0.3, thresholds, all),
            ForwardVariant::HighSlight
        );
        assert_eq!(
            forward_variant(0.0, thresholds, all),
            ForwardVariant::Straight
        );
        assert_eq!(
            forward_variant(-0.3, thresholds, all),
            ForwardVariant::LowSlight
        );
        assert_eq!(forward_variant(-0.5, thresholds, all), ForwardVariant::Low);
        assert_eq!(
            forward_variant(0.5, thresholds, [false, true, true, true]),
            ForwardVariant::HighSlight
        );
        assert_eq!(
            forward_variant(-0.5, thresholds, [true, true, false, false]),
            ForwardVariant::Straight
        );
        assert_eq!(
            forward_variant(-0.5, thresholds, [true, true, true, false]),
            ForwardVariant::LowSlight
        );
        assert_eq!(
            forward_variant(f32::NAN, thresholds, all),
            ForwardVariant::Straight
        );
    }

    #[test]
    fn up_and_down_tilts_use_inclusive_stick_and_strict_angle_bounds() {
        assert!(up_tilt(true, 0.5, 0.51, 0.5, 0.5));
        assert!(!up_tilt(true, 0.5, 0.5, 0.5, 0.5));
        assert!(!up_tilt(true, 0.49, 1.0, 0.5, 0.5));
        assert!(down_tilt(true, -0.5, -0.51, -0.5, 0.5));
        assert!(!down_tilt(true, -0.5, -0.5, -0.5, 0.5));
        assert!(!down_tilt(false, -1.0, -1.0, -0.5, 0.5));
    }

    #[test]
    fn down_tilt_repeat_buffers_early_presses() {
        let mut buffer = false;
        assert!(!down_tilt_repeat(false, false, &mut buffer));
        assert!(!buffer);
        assert!(!down_tilt_repeat(true, false, &mut buffer));
        assert!(buffer);
        assert!(down_tilt_repeat(true, true, &mut buffer));
    }
}
