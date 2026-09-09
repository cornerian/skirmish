//! Exact stick-region decision retained from `ftCo_8009AAFC`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickOption {
    Climb,
    Drop,
}

/// Select a main/C-stick ledge option after the caller applies the magnitude gate.
pub fn stick_option(
    main_stick: bool,
    input_ready: bool,
    stick_x: f32,
    angle: f32,
    facing: f32,
    angle_threshold: f32,
) -> Option<StickOption> {
    if angle > angle_threshold || (angle > -angle_threshold && stick_x * facing >= 0.0) {
        (main_stick && input_ready).then_some(StickOption::Climb)
    } else {
        input_ready.then_some(StickOption::Drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_regions_preserve_strict_angle_and_main_stick_climb_gate() {
        let threshold = 0.7;
        assert_eq!(
            stick_option(true, true, 1.0, 0.0, 1.0, threshold),
            Some(StickOption::Climb)
        );
        assert_eq!(
            stick_option(true, true, 0.0, -1.0, 1.0, threshold),
            Some(StickOption::Drop)
        );
        assert_eq!(stick_option(false, true, 1.0, 0.0, 1.0, threshold), None);
        assert_eq!(
            stick_option(true, false, 1.0, threshold, 1.0, threshold),
            None
        );
    }
}
