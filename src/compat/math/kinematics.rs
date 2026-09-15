//! Shared fighter arithmetic preserved from the melee source.
//!
//! These functions contain scalar/vector calculations only. Callers retain
//! ownership of input policy, move phases, state transitions, and resource
//! dispatch. The arithmetic is kept in source order where f32 rounding and
//! the source's truthy/relational behavior are observable.

/// `ftCo_SpecialS_HasInput` (`ftCo_SpecialS.c:15-23`): a fresh B press with
/// the stick past the side threshold on either sign.
pub fn has_input(fresh_b_press: bool, stick_x: f32, side_threshold: f32) -> bool {
    fresh_b_press && stick_x.abs() >= side_threshold
}

/// The opposing-stick turn predicate from `ftCo_SpecialS_CheckInput` and
/// `ftCo_SpecialAir_CheckInput` (`ftCo_SpecialS.c:25-39`).
pub fn should_turn(stick_x: f32, facing: f32, turn_threshold: f32) -> bool {
    stick_x * facing < -turn_threshold
}

/// `ftCo_800C97A8` (`ftCo_Turn.c:28-36`): the inclusive ordinary
/// standing-turn predicate reused by mid-move turn checks.
pub fn should_turn_mid_move(stick_x: f32, facing: f32, turn_threshold: f32) -> bool {
    stick_x * facing <= turn_threshold
}

/// `doEnter` (`ftCo_SpecialS.c:41-49`): blend ground velocity toward zero by
/// the ground-speed-retention fraction. The source's floor-friction lookup is
/// represented by its ordinary-terrain multiplier of `1.0` in this profile.
pub fn entry_ground_velocity(ground_velocity: f32, retention: f32) -> f32 {
    ground_velocity + -(ground_velocity * (1.0 - retention))
}

/// `sqrtf_accurate` (`MSL/math_ppc.h`): four double-precision Newton
/// iterations with the source's fused multiply-add rounding.
fn sqrt_accurate(x: f32) -> f32 {
    if x > 0.0 {
        let x64 = f64::from(x);
        let mut guess = 1.0 / x64.sqrt();
        for _ in 0..4 {
            let refined = x64.mul_add(-(guess * guess), 3.0);
            guess = (0.5 * guess) * refined;
        }
        (x64 * guess) as f32
    } else {
        x
    }
}

/// `lbVector_AngleXY`: clamped angle between two XY vectors (Z ignored).
/// The dot product retains the source's single `fmadds`, and the non-zero
/// product check deliberately propagates NaN products through `acosf`.
pub fn angle_xy(a: [f32; 3], b: [f32; 2]) -> f32 {
    let len_a = sqrt_accurate(a[0] * a[0] + a[1] * a[1]);
    let len_b = sqrt_accurate(b[0] * b[0] + b[1] * b[1]);
    let product = len_a * len_b;
    if product != 0.0 {
        let dot = a[1].mul_add(b[1], a[0] * b[0]);
        let cosine = (dot / product).clamp(-1.0, 1.0);
        crate::compat::math::trig::acosf(cosine)
    } else {
        0.0
    }
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
        assert!(should_turn_mid_move(-0.5, 1.0, -0.3));
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

    #[test]
    fn angle_xy_zero_vectors_return_zero() {
        assert_eq!(angle_xy([0.0, 0.0, 0.0], [0.0, 0.0]), 0.0);
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [0.0, 0.0]), 0.0);
        assert_eq!(angle_xy([0.0, 0.0, 0.0], [1.0, 0.0]), 0.0);
    }

    #[test]
    fn angle_xy_parallel_and_perpendicular() {
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [1.0, 0.0]), 0.0);
        assert_eq!(
            angle_xy([1.0, 0.0, 0.0], [0.0, 1.0]),
            std::f32::consts::FRAC_PI_2
        );
        assert_eq!(
            angle_xy([0.0, 1.0, 0.0], [1.0, 0.0]),
            std::f32::consts::FRAC_PI_2
        );
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [-1.0, 0.0]), std::f32::consts::PI);
    }

    #[test]
    fn angle_xy_nan_product_propagates_nan_not_zero() {
        let huge = 2.0e19_f32;
        let angle = angle_xy([0.0, 0.0, 0.0], [huge, 0.0]);
        assert!((huge * huge).is_infinite());
        assert!(angle.is_nan(), "expected NaN, got {angle}");
    }

    #[test]
    fn angle_xy_is_nan_safe_for_nan_inputs() {
        assert!(angle_xy([f32::NAN, 0.0, 0.0], [1.0, 0.0]).is_nan());
        assert!(angle_xy([1.0, 0.0, 0.0], [f32::NAN, 0.0]).is_nan());
    }
}
