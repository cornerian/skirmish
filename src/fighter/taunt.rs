//! Taunt (AppealSR/AppealSL) entry predicates from `ftCo_AppealS.c`. Callers
//! own the motion transition and the per-character side effects
//! (`ftCo_800DEA28`'s Young Link/Dr. Mario/Ganondorf branches, `ftCo_
//! 800DEBD0`'s debug-only Peach/Zelda hooks and Kirby re-init).

/// Which taunt motion `ftCo_800DEAE8` selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
}

/// `ftCo_800DE9B8`: a fresh D-pad-up press.
pub fn pressed(pressed_buttons: u16, dpad_up: u16) -> bool {
    pressed_buttons & dpad_up != 0
}

/// `ftCo_800DEAE8`'s selection: AppealSL when facing left and the SL
/// figatree exists (`ftData_80085FD4(fp, ms->anim_id)->x8 != 0`, modeled
/// here as `left_available`), else AppealSR. The exact `== -1.0` comparison
/// matches the source (not `< 0.0`, so e.g. `-0.5` still selects AppealSR).
pub fn select_side(facing: f32, left_available: bool) -> Side {
    if facing == -1.0 && left_available {
        Side::Left
    } else {
        Side::Right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_checks_the_exact_bit() {
        assert!(pressed(0x8, 0x8));
        assert!(!pressed(0x4, 0x8));
        assert!(!pressed(0, 0x8));
    }

    #[test]
    fn side_requires_exact_negative_one_facing_and_availability() {
        assert_eq!(select_side(-1.0, true), Side::Left);
        assert_eq!(select_side(-1.0, false), Side::Right);
        assert_eq!(select_side(1.0, true), Side::Right);
        assert_eq!(select_side(-0.5, true), Side::Right);
        assert_eq!(select_side(f32::NAN, true), Side::Right);
    }
}
