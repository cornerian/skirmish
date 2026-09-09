//! Ordinary aerial selection and landing arithmetic from ftCo_AttackAir.c,
//! ft_0DF1.c, ftcommon.c and ftCo_LandingAir.c. The caller supplies calibrated
//! sticks, common-data thresholds and the correct animation's end frame.
use super::combat::CombatError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionRules {
    /// ftCommonData::xDC/xE0; also the C-stick excursion thresholds.
    pub thresholds: [f32; 2],
    /// ftCommonData::x20_radians, with strict up/down angle comparisons.
    pub vertical_angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Neutral,
    Forward,
    Back,
    Up,
    Down,
}

/// ftCo_800DF478. Either axis must newly cross its absolute threshold. A direct
/// sign reversal while still outside the threshold is not a fresh excursion.
pub fn fresh_cstick(previous: [f32; 2], current: [f32; 2], thresholds: [f32; 2]) -> bool {
    (absolute(previous[0]) < thresholds[0] && absolute(current[0]) >= thresholds[0])
        || (absolute(previous[1]) < thresholds[1] && absolute(current[1]) >= thresholds[1])
}

/// ftCo_GetLStickAngle / ftCo_GetCStickAngle use atan2(y, ABS(x)), independent
/// of facing. Runtime/platform.h's comparison-based ABS retains negative zero.
/// libm replaces the target transcendental routine; numerical parity is tested.
pub fn stick_angle([x, y]: [f32; 2]) -> f32 {
    libm::atan2f(y, absolute(x))
}

/// ftCo_AttackAir_GetMsidFromCStick. Only a fresh C-stick overrides the main
/// stick; neutral is an AND of strict axis thresholds, then vertical selection
/// precedes the facing-relative horizontal test (which includes equality).
pub fn select(
    main: [f32; 2],
    cstick: [f32; 2],
    previous_cstick: [f32; 2],
    facing: f32,
    rules: &SelectionRules,
) -> Direction {
    let [x, y] = if fresh_cstick(previous_cstick, cstick, rules.thresholds) {
        cstick
    } else {
        main
    };
    let angle = stick_angle([x, y]);
    if absolute(x) < rules.thresholds[0] && absolute(y) < rules.thresholds[1] {
        Direction::Neutral
    } else if angle > rules.vertical_angle {
        Direction::Up
    } else if angle < -rules.vertical_angle {
        Direction::Down
    } else if x * facing >= 0.0 {
        Direction::Forward
    } else {
        Direction::Back
    }
}

/// The lag branch in ftCo_LandingAir_EnterWithLag, with landing-lag script flag
/// enabled and a valid ordinary aerial selected. Auto-cancel/basic landing is
/// a separate caller decision. Invalid C float-to-int inputs return an error.
pub fn landing_lag(
    base: f32,
    shield_age: u8,
    window: i32,
    divisor: f32,
) -> Result<f32, CombatError> {
    if i32::from(shield_age) < window {
        let divided = base / divisor;
        if !(-2_147_483_648.0..2_147_483_648.0).contains(&divided) {
            return Err(CombatError::UndefinedIntegerConversion);
        }
        let frames = divided as i32;
        Ok(if frames == 0 { 1.0 } else { frames as f32 })
    } else {
        Ok(base)
    }
}

/// ftCo_LandingAir_EnterWithMsidLag's ftAnim_SetAnimRate argument. End frame
/// comes from ftAnim_8006F484's selected animation, not a count of sampled poses.
pub fn landing_animation_rate(end_frame: f32, lag: f32) -> f32 {
    (end_frame + 0.1) / lag
}

fn absolute(value: f32) -> f32 {
    if value < 0.0 { -value } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_lag_conversions_are_errors_only_on_the_cancel_path() {
        for (lag, divisor) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (1.0, 0.0),
            (0.0, 0.0),
            (2_147_483_648.0, 1.0),
        ] {
            assert!(landing_lag(lag, 0, 3, divisor).is_err());
            let unchanged = landing_lag(lag, 3, 3, divisor).unwrap();
            if lag.is_nan() {
                assert!(unchanged.is_nan());
            } else {
                assert_eq!(unchanged.to_bits(), lag.to_bits());
            }
        }
        assert_eq!(
            landing_lag(-2_147_483_648.0, 0, 3, 1.0),
            Ok(-2_147_483_648.0)
        );
    }
}
