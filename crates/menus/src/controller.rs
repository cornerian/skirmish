//! Digital controller history and the accelerated menu repeat from
//! `gm/gm_1A36.c`. Inputs have already passed HSD's analog-to-digital conversion.

use crate::input::PadFrame;

const INITIAL_DELAY: u16 = 20;
const MEDIUM_AFTER: u16 = 40;
const FAST_AFTER: u16 = 100;

// The low 32 bits retain the HSD digital inputs. Game-menu aliases occupy the
// high bits, as defined in Dolphin's pad.h in the pinned upstream source.
const ALIASES: [(u32, u64); 6] = [
    ((1 << 8) | (1 << 12), 1 << 32), // A or Start: confirm
    (1 << 9, 1 << 33),               // B: cancel
    ((1 << 3) | (1 << 16), 1 << 36), // D-pad or stick up
    ((1 << 2) | (1 << 17), 1 << 37), // D-pad or stick down
    ((1 << 0) | (1 << 18), 1 << 38), // D-pad or stick left
    ((1 << 1) | (1 << 19), 1 << 39), // D-pad or stick right
];
const CHORDS: [(u32, u64); 2] = [
    ((1 << 6) | (1 << 5) | (1 << 12), 1 << 34),
    ((1 << 6) | (1 << 5) | (1 << 8) | (1 << 12), 1 << 35),
];

/// Per-port digital history for four native controllers.
///
/// Supply HSD-compatible digital button words, including stick direction bits
/// when available. An absent or disconnected controller is represented by zero.
/// Analog thresholds, device discovery and button assignment belong to callers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Controllers {
    ports: [PortHistory; 4],
}

impl Controllers {
    /// Advance each port once and return the trigger and accelerated repeat
    /// words consumed by the menu input routines.
    ///
    /// A changed button word immediately emits only its newly pressed bits and
    /// resets acceleration, even when the change consists solely of a release.
    /// The original routine loads timers with 20, 8, 4 and 2, then decrements
    /// before returning; this means 21, 9, 5 and 3 polls between repeat events.
    pub fn poll(&mut self, buttons: [u32; 4]) -> [PadFrame; 4] {
        std::array::from_fn(|port| self.ports[port].poll(buttons[port]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PortHistory {
    previous: u32,
    repeat_timer: u16,
    unchanged_frames: u16,
}

impl Default for PortHistory {
    fn default() -> Self {
        Self {
            previous: 0,
            repeat_timer: INITIAL_DELAY,
            unchanged_frames: 0,
        }
    }
}

impl PortHistory {
    fn poll(&mut self, buttons: u32) -> PadFrame {
        // HSD_PadRenewCopyStatus derives these edges before gm adds aliases.
        // Mapping held inputs first would suppress a fresh A press while Start
        // is already held, or a D-pad press while the same stick direction is held.
        let newly_pressed = buttons & !self.previous;
        let changed = buttons != self.previous;
        self.previous = buttons;
        let triggered = map_buttons(newly_pressed, buttons);

        let repeated = if changed {
            self.repeat_timer = INITIAL_DELAY;
            self.unchanged_frames = 0;
            triggered
        } else {
            self.unchanged_frames = (self.unchanged_frames + 1).min(FAST_AFTER);
            if self.repeat_timer != 0 {
                self.repeat_timer -= 1;
                0
            } else {
                self.repeat_timer = if self.unchanged_frames >= FAST_AFTER {
                    2
                } else if self.unchanged_frames >= MEDIUM_AFTER {
                    4
                } else {
                    8
                };
                map_buttons(buttons, buttons)
            }
        };

        PadFrame {
            triggered,
            repeated,
        }
    }
}

/// `gm_801A3714` maps any-of aliases independently on held and edge words.
/// `gm_801A3820` adds a chord edge only if the whole chord is currently held.
fn map_buttons(bits: u32, held: u32) -> u64 {
    let mut mapped = u64::from(bits);
    for (source, alias) in ALIASES {
        if bits & source != 0 {
            mapped |= alias;
        }
    }
    for (source, alias) in CHORDS {
        if held & source == source && bits & source != 0 {
            mapped |= alias;
        }
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 1 << 8;
    const B: u32 = 1 << 9;
    const START: u32 = 1 << 12;
    const RIGHT: u32 = 1 << 1;
    const STICK_RIGHT: u32 = 1 << 19;
    const CONFIRM: u64 = 1 << 32;
    const CANCEL: u64 = 1 << 33;
    const ANY_RIGHT: u64 = 1 << 39;

    fn poll_first(controllers: &mut Controllers, buttons: u32) -> PadFrame {
        controllers.poll([buttons, 0, 0, 0])[0]
    }

    #[test]
    fn repeat_schedule_retains_original_timer_boundaries() {
        let mut controllers = Controllers::default();
        let mut repeat_frames = Vec::new();
        for frame in 0..=115 {
            let input = poll_first(&mut controllers, RIGHT);
            assert_eq!(input.triggered, if frame == 0 { 2 | ANY_RIGHT } else { 0 });
            if input.repeated != 0 {
                assert_eq!(input.repeated, 2 | ANY_RIGHT);
                repeat_frames.push(frame);
            }
        }
        assert_eq!(
            repeat_frames,
            [
                0, 21, 30, 39, 48, 53, 58, 63, 68, 73, 78, 83, 88, 93, 98, 103, 106, 109, 112, 115,
            ]
        );
    }

    #[test]
    fn releasing_one_button_resets_repeat_of_the_remaining_button() {
        let mut controllers = Controllers::default();
        for _ in 0..110 {
            poll_first(&mut controllers, RIGHT | A);
        }
        let release = poll_first(&mut controllers, RIGHT);
        assert_eq!(release.triggered, 0);
        assert_eq!(release.repeated, 0);
        for _ in 0..20 {
            assert_eq!(poll_first(&mut controllers, RIGHT).repeated, 0);
        }
        assert_eq!(poll_first(&mut controllers, RIGHT).repeated, 2 | ANY_RIGHT);
        for _ in 0..8 {
            assert_eq!(poll_first(&mut controllers, RIGHT).repeated, 0);
        }
        assert_eq!(poll_first(&mut controllers, RIGHT).repeated, 2 | ANY_RIGHT);
    }

    #[test]
    fn aliases_preserve_edges_from_independent_physical_inputs() {
        let mut controllers = Controllers::default();
        let first = poll_first(&mut controllers, START | STICK_RIGHT);
        assert_eq!(
            first.triggered,
            u64::from(START | STICK_RIGHT) | CONFIRM | ANY_RIGHT
        );
        let second = poll_first(&mut controllers, START | STICK_RIGHT | A | RIGHT);
        assert_eq!(second.triggered, u64::from(A | RIGHT) | CONFIRM | ANY_RIGHT);
        assert_eq!(second.repeated, second.triggered);
        let release = poll_first(&mut controllers, START | STICK_RIGHT);
        assert_eq!(release.triggered, 0);
        assert_eq!(release.repeated, 0);
    }

    #[test]
    fn simultaneous_changes_repeat_only_new_presses_and_ports_are_independent() {
        let mut controllers = Controllers::default();
        controllers.poll([RIGHT, A, 0, 0]);
        for _ in 0..20 {
            controllers.poll([RIGHT, A, 0, 0]);
        }
        let frames = controllers.poll([RIGHT | B, A, START, 0]);
        assert_eq!(frames[0].triggered, u64::from(B) | CANCEL);
        assert_eq!(frames[0].repeated, frames[0].triggered);
        assert_eq!(frames[1].triggered, 0);
        assert_eq!(frames[1].repeated, u64::from(A) | CONFIRM);
        assert_eq!(frames[2].triggered, u64::from(START) | CONFIRM);
        assert_eq!(frames[2].repeated, frames[2].triggered);
        assert_eq!(frames[3].triggered, 0);
        assert_eq!(frames[3].repeated, 0);
    }

    #[test]
    fn release_and_repress_starts_with_an_immediate_repeat() {
        let mut controllers = Controllers::default();
        poll_first(&mut controllers, A);
        for _ in 0..120 {
            let idle = poll_first(&mut controllers, 0);
            assert_eq!(idle.triggered, 0);
            assert_eq!(idle.repeated, 0);
        }
        let pressed = poll_first(&mut controllers, A);
        assert_eq!(pressed.triggered, u64::from(A) | CONFIRM);
        assert_eq!(pressed.repeated, pressed.triggered);
        assert_eq!(poll_first(&mut controllers, A).repeated, 0);
    }

    #[test]
    fn chord_aliases_require_the_full_current_chord_and_a_member_edge() {
        let mut controllers = Controllers::default();
        let shoulders = (1 << 6) | (1 << 5);
        assert_eq!(
            poll_first(&mut controllers, shoulders).triggered,
            u64::from(shoulders)
        );
        let start = poll_first(&mut controllers, shoulders | START);
        assert_eq!(start.triggered, u64::from(START) | CONFIRM | (1 << 34));
        let a = poll_first(&mut controllers, shoulders | START | A);
        assert_eq!(a.triggered, u64::from(A) | CONFIRM | (1 << 35));
        let b = poll_first(&mut controllers, shoulders | START | A | B);
        assert_eq!(b.triggered, u64::from(B) | CANCEL);
        let release = poll_first(&mut controllers, START | A | B);
        assert_eq!(release.triggered, 0);
        assert_eq!(release.repeated, 0);
    }
}
