//! Throw-direction input predicates retained from `ftCo_Throw.c`.

use serde::{Deserialize, Serialize};

const MASH_BUTTONS: u32 = 0x8000_0F00;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MashState {
    pub axes: [i8; 2],
    pub shake_enabled: bool,
    pub shaking: bool,
    pub shake_frame: u8,
    pub shake_frames: u8,
}

/// Complete `ftCommon_GrabMash` timer, axis-latch, and shake-state mutation.
pub fn mash(
    timer: &mut f32,
    state: &mut MashState,
    pressed_buttons: u32,
    stick: [f32; 2],
    penalty: f32,
    threshold: f32,
) -> bool {
    let mut result = false;
    if pressed_buttons & MASH_BUTTONS != 0 {
        *timer -= penalty;
        result = true;
    }
    let previous = state.axes;
    for (value, axis) in stick.into_iter().zip(&mut state.axes) {
        if value < -threshold {
            *axis = -1;
        }
        if value > threshold {
            *axis = 1;
        }
    }
    if state.axes != previous {
        *timer -= penalty;
        result = true;
    }
    if result && state.shake_enabled {
        state.shaking = true;
        state.shake_frame = state.shake_frame.wrapping_add(1);
        if state.shake_frame >= state.shake_frames {
            state.shake_frame = 0;
        }
    } else {
        state.shaking = false;
    }
    result
}

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
    fn grab_mash_preserves_strict_latches_double_penalty_and_shake_wrap() {
        let mut timer = 10.0;
        let mut state = MashState {
            shake_enabled: true,
            shake_frame: 1,
            shake_frames: 3,
            ..Default::default()
        };
        assert!(!mash(&mut timer, &mut state, 0, [0.5, -0.5], 2.0, 0.5));
        assert_eq!(timer, 10.0);
        assert!(mash(&mut timer, &mut state, 0x100, [0.6, 0.0], 2.0, 0.5));
        assert_eq!(timer, 6.0);
        assert_eq!(state.axes, [1, 0]);
        assert_eq!(state.shake_frame, 2);
        assert!(!mash(&mut timer, &mut state, 0, [0.0; 2], 2.0, 0.5));
        assert_eq!(state.axes, [1, 0]);
        assert!(!state.shaking);
        assert!(mash(&mut timer, &mut state, 1 << 31, [-0.6, 0.6], 2.0, 0.5));
        assert_eq!(timer, 2.0);
        assert_eq!(state.axes, [-1, 1]);
        assert_eq!(state.shake_frame, 0);
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
