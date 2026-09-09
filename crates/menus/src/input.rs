//! Menu input decoding from `mn_80229624` and confirming-port arbitration
//! from `mn_802295AC`. Frames contain the game controller map's semantic bits,
//! including its already computed trigger and repeat masks.

use serde::{Deserialize, Serialize};

pub const UP: u32 = 1;
pub const DOWN: u32 = 1 << 1;
pub const LEFT: u32 = 1 << 2;
pub const RIGHT: u32 = 1 << 3;
pub const CONFIRM: u32 = 1 << 4;
pub const BACK: u32 = 1 << 5;
pub const L_TRIGGER: u32 = 1 << 6;
pub const R_TRIGGER: u32 = 1 << 7;
pub const START_BUTTON: u32 = 1 << 8;
pub const A_BUTTON: u32 = 1 << 9;
pub const X_BUTTON: u32 = 1 << 10;
pub const Y_BUTTON: u32 = 1 << 11;

/// Dolphin/HSD digital button bits and the game controller map's aliases.
pub mod pad {
    pub const LEFT: u64 = 1;
    pub const RIGHT: u64 = 1 << 1;
    pub const DOWN: u64 = 1 << 2;
    pub const UP: u64 = 1 << 3;
    pub const R: u64 = 1 << 5;
    pub const L: u64 = 1 << 6;
    pub const A: u64 = 1 << 8;
    pub const B: u64 = 1 << 9;
    pub const X: u64 = 1 << 10;
    pub const Y: u64 = 1 << 11;
    pub const START: u64 = 1 << 12;
    pub const STICK_UP: u64 = 1 << 16;
    pub const STICK_DOWN: u64 = 1 << 17;
    pub const STICK_LEFT: u64 = 1 << 18;
    pub const STICK_RIGHT: u64 = 1 << 19;
    pub const CONFIRM: u64 = 1 << 32;
    pub const CANCEL: u64 = 1 << 33;
    pub const ANY_UP: u64 = 1 << 36;
    pub const ANY_DOWN: u64 = 1 << 37;
    pub const ANY_LEFT: u64 = 1 << 38;
    pub const ANY_RIGHT: u64 = 1 << 39;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct InputState {
    pub cooldown: u16,
    pub x2: u16,
    pub x4: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[repr(C)]
#[serde(deny_unknown_fields)]
pub struct PadFrame {
    pub triggered: u64,
    pub repeated: u64,
}

/// One input poll, including the original animation cooldown side effects.
/// Call once per menu tick; this is not a timer measured in wall-clock time.
pub fn translate(state: &mut InputState, frame: PadFrame) -> u32 {
    if state.cooldown != 0 {
        state.cooldown -= 1;
        state.x2 = 0;
        state.x4 = 0;
        return 0;
    }
    decode(frame)
}

/// Decode masks without advancing a timer, for a `MenuState` which owns its
/// own cooldown. Raw A/B bits alone do not synthesize semantic Confirm/Back.
pub fn decode(frame: PadFrame) -> u32 {
    let mut result = 0;
    for (source, target) in [
        (pad::A, A_BUTTON),
        (pad::START, START_BUTTON),
        (pad::CONFIRM, CONFIRM),
        (pad::CANCEL, BACK),
        (pad::L, L_TRIGGER),
        (pad::R, R_TRIGGER),
        (pad::X, X_BUTTON),
        (pad::Y, Y_BUTTON),
    ] {
        if frame.triggered & source != 0 {
            result |= target;
        }
    }
    for (source, target) in [
        (pad::ANY_UP, UP),
        (pad::ANY_DOWN, DOWN),
        (pad::ANY_LEFT, LEFT),
        (pad::ANY_RIGHT, RIGHT),
    ] {
        if frame.repeated & source != 0 {
            result |= target;
        }
    }
    result
}

/// The lowest numbered port triggering Confirm wins; no confirmation returns
/// port zero, exactly as upstream (it does not indicate controller presence).
pub fn confirming_port(frames: &[PadFrame; 4]) -> u8 {
    frames
        .iter()
        .position(|frame| frame.triggered & pad::CONFIRM != 0)
        .unwrap_or(0) as u8
}

/// The controller map's aggregate fifth slot, consumed by branch menus.
pub fn aggregate(frames: &[PadFrame; 4]) -> PadFrame {
    frames
        .iter()
        .fold(PadFrame::default(), |all, frame| PadFrame {
            triggered: all.triggered | frame.triggered,
            repeated: all.repeated | frame.repeated,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_and_repeats_are_distinct() {
        assert_eq!(
            decode(PadFrame {
                triggered: pad::A | pad::CONFIRM | pad::ANY_UP,
                repeated: pad::ANY_DOWN | pad::CANCEL,
            }),
            A_BUTTON | CONFIRM | DOWN
        );
        assert_eq!(
            decode(PadFrame {
                triggered: pad::B,
                repeated: 0
            }),
            0
        );
    }

    #[test]
    fn cooldown_consumes_polls_and_clears_auxiliary_fields() {
        let frame = PadFrame {
            triggered: pad::CONFIRM,
            repeated: pad::ANY_UP,
        };
        let mut state = InputState {
            cooldown: 2,
            x2: 8,
            x4: -4,
        };
        assert_eq!(translate(&mut state, frame), 0);
        assert_eq!(
            state,
            InputState {
                cooldown: 1,
                ..InputState::default()
            }
        );
        assert_eq!(translate(&mut state, frame), 0);
        assert_eq!(translate(&mut state, frame), CONFIRM | UP);
    }

    #[test]
    fn aggregate_keeps_all_ports_but_confirmation_chooses_the_first() {
        let mut frames = [PadFrame::default(); 4];
        frames[1].triggered = pad::CONFIRM;
        frames[2].triggered = pad::CONFIRM | pad::START;
        frames[3].repeated = pad::ANY_DOWN;
        assert_eq!(confirming_port(&frames), 1);
        assert_eq!(decode(aggregate(&frames)), CONFIRM | START_BUTTON | DOWN);
        assert_eq!(confirming_port(&[PadFrame::default(); 4]), 0);
    }
}
