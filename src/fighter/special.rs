//! Exact neutral-special input region retained from `ftCo_800D67C4`.

const BUTTON_B: u16 = 0x0200;

/// A fresh B press with both main-stick axes strictly inside their thresholds.
pub fn neutral_input(pressed_buttons: u16, stick: [f32; 2], thresholds: [f32; 2]) -> bool {
    pressed_buttons & BUTTON_B != 0
        && absolute(stick[0]) < thresholds[0]
        && absolute(stick[1]) < thresholds[1]
}

fn absolute(value: f32) -> f32 {
    if value < 0.0 { -value } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_region_is_strict_on_both_axes() {
        assert!(neutral_input(BUTTON_B, [0.49, -0.49], [0.5; 2]));
        assert!(!neutral_input(BUTTON_B, [0.5, 0.0], [0.5; 2]));
        assert!(!neutral_input(0, [0.0; 2], [0.5; 2]));
    }
}
