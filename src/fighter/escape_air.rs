//! Air-dodge arithmetic from `ftCo_EscapeAir.c` and the FallSpecial landing
//! predicate from `ftCo_FallSpecial.c`. Callers own the motion transitions,
//! the item-throw restore copy, the `x334` item timer and tether interrupts.

/// `inlineA0` of `ftCo_80099A9C`: `escapeair_force` along the main-stick angle,
/// or a zero vector while both axes sit strictly inside `escapeair_deadzone`.
pub fn launch_velocity(stick: [f32; 2], deadzone: [f32; 2], force: f32) -> [f32; 2] {
    if stick[0].abs() < deadzone[0] && stick[1].abs() < deadzone[1] {
        return [0.0, 0.0];
    }
    let angle = libm::atan2f(stick[1], stick[0]);
    [force * libm::cosf(angle), force * libm::sinf(angle)]
}

/// `ftCo_EscapeAir_Phys` while the script has not raised its skip-decay flag:
/// both self-velocity axes scale by `escapeair_decay` and no gravity applies.
pub fn decay(velocity: [f32; 2], factor: f32) -> [f32; 2] {
    [velocity[0] * factor, velocity[1] * factor]
}

/// Complete `ftCo_80096CC8`: FallSpecial lands on a one-way platform only
/// while the main stick is strictly above `x25C`; solid floors always land.
pub fn platform_landing(line_present: bool, platform: bool, stick_y: f32, threshold: f32) -> bool {
    line_present && (!platform || stick_y > threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_uses_strict_absolute_comparisons_on_both_axes() {
        assert_eq!(launch_velocity([0.2, -0.2], [0.3, 0.3], 2.0), [0.0, 0.0]);
        assert_eq!(launch_velocity([-0.0, 0.0], [0.3, 0.3], 2.0), [0.0, 0.0]);
        let launched = launch_velocity([0.3, 0.0], [0.3, 0.3], 2.0);
        assert_eq!(launched, [2.0, 0.0]);
        let up = launch_velocity([0.0, 1.0], [0.3, 0.3], 2.0);
        assert!(up[0].abs() < 1e-6 && (up[1] - 2.0).abs() < 1e-6);
        assert!(launch_velocity([f32::NAN, 0.0], [0.3, 0.3], 2.0)[0].is_nan());
    }

    #[test]
    fn decay_scales_both_axes_without_gravity() {
        assert_eq!(decay([2.0, -1.0], 0.5), [1.0, -0.5]);
        assert_eq!(decay([0.0, 0.0], 0.9), [0.0, 0.0]);
    }

    #[test]
    fn platform_landing_requires_the_stick_strictly_above_the_threshold() {
        assert!(platform_landing(true, false, -1.0, -0.5));
        assert!(platform_landing(true, true, -0.49, -0.5));
        assert!(!platform_landing(true, true, -0.5, -0.5));
        assert!(!platform_landing(false, false, 1.0, -0.5));
    }
}
