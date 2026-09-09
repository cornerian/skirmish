//! Native original-C agreement for menu input and the scalar branch callbacks.
//! Leaf initializers are recorded as requests, before their unported behavior.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use menus::input::{InputState, PadFrame, confirming_port, translate};
use menus::{Destination, Menu, MenuState, Panel, Snapshot, Unlocks};
use proptest::prelude::*;
use std::sync::Mutex;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_menu_translate(
        cooldown: *mut u16,
        x2: *mut u16,
        x4: *mut i32,
        triggered: u64,
        repeated: u64,
    ) -> u32;
    fn oracle_menu_confirming_port(ports: *const u64) -> u8;
    fn oracle_menu_available(menu: u8, selection: u16, star: u8, sound: u8) -> u32;
    fn oracle_menu_available_before(menu: u8, selection: u16, star: u8, sound: u8) -> u32;
    fn oracle_menu_step(state: *mut u32, buttons: u32, star: u8, sound: u8, port: u8);
}

// The original functions use mutable globals, including the sampled controllers.
static ORACLE_LOCK: Mutex<()> = Mutex::new(());

fn compare_input(initial: InputState, frame: PadFrame) {
    let mut actual = initial;
    let mut expected = initial;
    let result = translate(&mut actual, frame);
    // SAFETY: all pointers address live scalar fields of the declared C widths.
    // The caller holds ORACLE_LOCK, and all scalar input combinations are defined.
    let expected_result = unsafe {
        oracle_menu_translate(
            &mut expected.cooldown,
            &mut expected.x2,
            &mut expected.x4,
            frame.triggered,
            frame.repeated,
        )
    };
    assert_eq!(
        result, expected_result,
        "initial {initial:?}, frame {frame:?}"
    );
    assert_eq!(actual, expected, "initial {initial:?}, frame {frame:?}");
}

fn compare_port(frames: [PadFrame; 4]) {
    let triggered = frames.map(|frame| frame.triggered);
    // SAFETY: triggered is a live four-element array and the caller holds the lock.
    let expected = unsafe { oracle_menu_confirming_port(triggered.as_ptr()) };
    assert_eq!(confirming_port(&frames), expected, "frames {frames:?}");
}

fn state_words(snapshot: Snapshot) -> [u32; 9] {
    let (kind, value) = match snapshot.pending {
        None => (0, 0),
        Some(Destination::Scene(scene)) => (1, u32::from(scene as u8)),
        Some(Destination::Panel(panel)) => (
            2,
            match panel {
                Panel::MultiMan => 1,
                Panel::VersusRecords => 2,
                Panel::BonusRecords => 3,
                Panel::MiscRecords => 4,
                Panel::Snapshots => 5,
                Panel::Archives => 6,
                Panel::SoundTest => 7,
                Panel::Special => 8,
                Panel::Rumble => 9,
                Panel::Sound => 10,
                Panel::Display => 11,
                Panel::Language => 12,
                Panel::EraseData => 13,
                Panel::Rules => 14,
                Panel::NameEntry => 15,
                Panel::EventMatch => 16,
            },
        ),
    };
    [
        u32::from(snapshot.menu as u8),
        u32::from(snapshot.previous_menu as u8),
        u32::from(snapshot.selection),
        snapshot.buttons,
        u32::from(snapshot.entering_menu),
        u32::from(snapshot.cooldown),
        u32::from(snapshot.controller_port),
        kind,
        value,
    ]
}

fn compare_step(actual: &mut MenuState, expected: &mut [u32; 9], buttons: u32, port: u8) {
    let initial = actual.snapshot();
    assert!(
        initial.pending.is_none(),
        "comparison ends at external delegation"
    );
    actual.step_for_port(buttons, port);
    // SAFETY: expected contains the nine scalar words documented by the adapter.
    // The initial state has a valid available selection in a supported branch,
    // port is 0..4, and the caller holds the C global-state lock.
    unsafe {
        oracle_menu_step(
            expected.as_mut_ptr(),
            buttons,
            u8::from(initial.unlocks.all_star),
            u8::from(initial.unlocks.sound_test),
            port,
        );
    }
    assert_eq!(
        state_words(actual.snapshot()),
        *expected,
        "initial {initial:?}, buttons {buttons:#x}, port {port}"
    );
}

#[test]
fn copied_menu_timer_and_input_enum_match_the_pinned_header() {
    let adapter = include_str!("oracle/menus.c");
    let original = include_str!("oracle/original/mn_inlines.h");
    for (start, end) in [
        ("static inline void Menu_DecrementAnimTimer(void)", "\n}"),
        ("typedef enum _MenuInput", "} MenuInput;"),
    ] {
        let excerpt = &adapter[adapter.find(start).unwrap()..];
        let excerpt = &excerpt[..excerpt.find(end).unwrap() + end.len()];
        assert!(
            original.contains(excerpt),
            "adapter excerpt changed: {start}"
        );
    }
}

#[test]
fn copied_pad_masks_match_the_pinned_dolphin_header() {
    let adapter = include_str!("oracle/menus.c");
    let original = include_str!("oracle/original/dolphin_pad.h");
    for line in adapter
        .lines()
        .filter(|line| line.starts_with("#define PAD_") && !line.contains("STACK"))
    {
        let definition = line.split_whitespace().collect::<Vec<_>>();
        let original_line = original
            .lines()
            .find(|candidate| candidate.split_whitespace().nth(1) == Some(definition[1]))
            .unwrap();
        let tokens = original_line
            .split("//")
            .next()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        assert_eq!(definition, tokens);
    }
}

#[test]
fn all_menu_unlock_predicates_and_visible_counts_match_c() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for menu in Menu::ALL {
        for all_star in [false, true] {
            for sound_test in [false, true] {
                let unlocks = Unlocks {
                    all_star,
                    sound_test,
                };
                for selection in (0..=32).chain([255, 256, 32767, u16::MAX]) {
                    // SAFETY: scalar indices are defined for both predicates;
                    // even out-of-range indices intentionally match C's behavior.
                    let available = unsafe {
                        oracle_menu_available(
                            menu as u8,
                            selection,
                            all_star.into(),
                            sound_test.into(),
                        )
                    };
                    // SAFETY: same lock and scalar domain; the upper bound is u16.
                    let before = unsafe {
                        oracle_menu_available_before(
                            menu as u8,
                            selection,
                            all_star.into(),
                            sound_test.into(),
                        )
                    };
                    assert_eq!(menu.is_available(selection, unlocks), available != 0);
                    assert_eq!(u32::from(menu.available_before(selection, unlocks)), before);
                }
            }
        }
    }
}

#[test]
fn every_branch_selection_and_translated_input_combination_match_c() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for menu in Menu::ALL {
        for all_star in [false, true] {
            for sound_test in [false, true] {
                let unlocks = Unlocks {
                    all_star,
                    sound_test,
                };
                for selection in 0..menu.selection_count() {
                    let Ok(initial) = MenuState::at(menu, selection, unlocks) else {
                        continue;
                    };
                    for buttons in 0..4096 {
                        let mut actual = initial.clone();
                        let mut expected = state_words(initial.snapshot());
                        // Exercise all confirming ports across otherwise identical
                        // masks (irrelevant L/R bits do not affect branch dispatch).
                        let port = ((buttons >> 6) & 3) as u8;
                        compare_step(&mut actual, &mut expected, buttons, port);
                    }
                }
            }
        }
    }
}

#[test]
fn entry_and_nested_transition_cooldowns_match_original_callbacks() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    let mut actual = MenuState::new(Unlocks::default());
    let mut expected = state_words(actual.snapshot());
    // Initial20, Main -> OnePlayer, child5, OnePlayer -> Regular, child5.
    // Held translated Confirm is discarded during cooldown and acts next poll.
    for _ in 0..32 {
        compare_step(&mut actual, &mut expected, 0x10, 3);
    }
    assert_eq!(actual.snapshot().menu, Menu::RegularMatch);
    assert!(actual.snapshot().pending.is_none());
    compare_step(&mut actual, &mut expected, 0x10, 2);
    assert!(actual.snapshot().pending.is_some());
}

#[test]
fn every_relevant_mask_combination_and_each_irrelevant_bit_match_c() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    // Independent source-bit ordering from Dolphin pad.h, including semantic
    // high bits: raw A/Start do not automatically produce Confirm here.
    let trigger_bits = [8, 12, 32, 33, 6, 5, 10, 11];
    let repeat_bits = [36, 37, 38, 39];
    for combination in 0..4096_u32 {
        let mut frame = PadFrame::default();
        for (index, bit) in trigger_bits.into_iter().enumerate() {
            if combination & (1 << index) != 0 {
                frame.triggered |= 1 << bit;
            }
        }
        for (index, bit) in repeat_bits.into_iter().enumerate() {
            if combination & (1 << (index + 8)) != 0 {
                frame.repeated |= 1 << bit;
            }
        }
        compare_input(InputState::default(), frame);
    }
    for bit in 0..64 {
        compare_input(
            InputState::default(),
            PadFrame {
                triggered: 1 << bit,
                repeated: 0,
            },
        );
        compare_input(
            InputState::default(),
            PadFrame {
                triggered: 0,
                repeated: 1 << bit,
            },
        );
    }
}

#[test]
fn every_cooldown_value_and_auxiliary_field_reset_match_c() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for cooldown in 0..=u16::MAX {
        compare_input(
            InputState {
                cooldown,
                x2: u16::MAX - cooldown,
                x4: i32::MIN + i32::from(cooldown),
            },
            PadFrame {
                triggered: u64::MAX,
                repeated: u64::MAX,
            },
        );
    }
}

#[test]
fn all_simultaneous_confirming_port_combinations_match_c() {
    let _guard = ORACLE_LOCK.lock().unwrap();
    for combination in 0..16 {
        compare_port(std::array::from_fn(|port| PadFrame {
            triggered: if combination & (1 << port) != 0 {
                1 << 32
            } else {
                0
            },
            // Repetition must not count as a fresh confirm.
            repeated: 1 << 32,
        }));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn arbitrary_input_masks_and_state_match_c(
        triggered in any::<u64>(), repeated in any::<u64>(),
        cooldown in any::<u16>(), x2 in any::<u16>(), x4 in any::<i32>(),
    ) {
        let _guard = ORACLE_LOCK.lock().unwrap();
        let frame = PadFrame { triggered, repeated };
        compare_input(InputState { cooldown, x2, x4 }, frame);
        compare_input(InputState { cooldown: 0, x2, x4 }, frame);
    }

    #[test]
    fn arbitrary_port_frames_match_c(
        triggered in prop::array::uniform4(any::<u64>()),
        repeated in prop::array::uniform4(any::<u64>()),
    ) {
        let _guard = ORACLE_LOCK.lock().unwrap();
        compare_port(std::array::from_fn(|port| PadFrame {
            triggered: triggered[port], repeated: repeated[port],
        }));
    }

    #[test]
    fn generated_navigation_traces_match_c_until_delegation(
        menu_index in 0..Menu::ALL.len(), all_star in any::<bool>(), sound_test in any::<bool>(),
        inputs in prop::collection::vec((
            prop_oneof![8 => 0_u32..16, 1 => Just(0x10_u32), 1 => Just(0x20_u32)],
            0_u8..4,
        ), 1..96),
    ) {
        let _guard = ORACLE_LOCK.lock().unwrap();
        let mut actual = MenuState::at(Menu::ALL[menu_index], 0, Unlocks { all_star, sound_test }).unwrap();
        let mut expected = state_words(actual.snapshot());
        for (buttons, port) in inputs {
            compare_step(&mut actual, &mut expected, buttons, port);
            if actual.snapshot().pending.is_some() { break; }
        }
    }
}
