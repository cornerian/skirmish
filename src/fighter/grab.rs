//! Throw-direction input predicates retained from `ftCo_Throw.c`.

/// `fn_800DA4C0`: a freshly pressed A button requests CatchAttack.
pub const fn pummel_pressed(pressed_buttons: u16) -> bool {
    pressed_buttons & 0x100 != 0
}

/// A main-stick horizontal threshold crossing has priority over vertical throws.
pub fn fresh_horizontal(current: f32, previous: f32, threshold: f32) -> bool {
    (previous < threshold && current >= threshold)
        || (previous > -threshold && current <= -threshold)
}

/// Up throw uses a positive threshold crossing.
pub fn fresh_up(current: f32, previous: f32, threshold: f32) -> bool {
    previous < threshold && current >= threshold
}

/// Down throw's common-data threshold is signed and normally negative.
pub fn fresh_down(current: f32, previous: f32, threshold: f32) -> bool {
    previous > threshold && current <= threshold
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowDirection {
    Forward,
    Backward,
    Up,
    Down,
}

/// `ftCo_800DD1E4` direction priority for main and C-stick crossings.
pub fn direction(
    current: [f32; 2],
    previous: [f32; 2],
    ccurrent: [f32; 2],
    cprevious: [f32; 2],
    facing: f32,
    thresholds: [f32; 3],
) -> Option<ThrowDirection> {
    let [horizontal, up, down] = thresholds;
    if fresh_horizontal(current[0], previous[0], horizontal) {
        Some(if current[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_horizontal(ccurrent[0], cprevious[0], horizontal) {
        Some(if ccurrent[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_up(current[1], previous[1], up) || fresh_up(ccurrent[1], cprevious[1], up) {
        Some(ThrowDirection::Up)
    } else if fresh_down(current[1], previous[1], down)
        || fresh_down(ccurrent[1], cprevious[1], down)
    {
        Some(ThrowDirection::Down)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_crossings_retain_source_strictness() {
        assert!(fresh_horizontal(0.7, 0.69, 0.7));
        assert!(!fresh_horizontal(0.7, 0.7, 0.7));
        assert!(fresh_horizontal(-0.7, -0.69, 0.7));
        assert!(fresh_up(0.6, 0.59, 0.6));
        assert!(!fresh_up(0.6, 0.6, 0.6));
        assert!(fresh_down(-0.6, -0.59, -0.6));
        assert!(!fresh_down(-0.6, -0.6, -0.6));
    }

    #[test]
    fn pummel_uses_only_the_fresh_a_bit() {
        assert!(!pummel_pressed(0));
        assert!(!pummel_pressed(0x200));
        assert!(pummel_pressed(0x100));
        assert!(pummel_pressed(0x110));
    }

    #[test]
    fn horizontal_main_and_cstick_precede_vertical_selection() {
        let thresholds = [0.7, 0.6, -0.6];
        assert_eq!(
            direction(
                [-0.7, 0.8],
                [0.0; 2],
                [0.8, 0.8],
                [0.0; 2],
                -1.0,
                thresholds,
            ),
            Some(ThrowDirection::Forward)
        );
        assert_eq!(
            direction([0.0, -0.7], [0.0; 2], [0.8, 0.8], [0.0; 2], 1.0, thresholds,),
            Some(ThrowDirection::Forward)
        );
    }
}
