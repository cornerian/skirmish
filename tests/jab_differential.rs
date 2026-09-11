//! `ftCo_Attack1_CheckInput`, the first/second/third jab entries and IASA
//! follow-up checks, and the rapid jab's entry count, loop continuation and
//! loop IASA, checked against the pinned C bodies. This is a Rust mirror of
//! the exact per-frame protocol `oracle_jab_sequence` drives (see
//! `tests/oracle/attack1.c`), built only from the pure functions in
//! `skirmish::fighter::jab` plus the tiny local motion/flag bookkeeping
//! `docs/jabs.md` describes, so it checks both the arithmetic and the
//! dispatch order against the native module.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::jab::{Stage, buffer_follow_up, decay, loop_check, rapid_count, wait_press};

const WAIT: i32 = 14;
const FIRST: i32 = 44;
const SECOND: i32 = 45;
const THIRD: i32 = 46;
const START: i32 = 47;
const LOOP: i32 = 48;
const END: i32 = 49;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_jab_sequence(
        attrs: *const f32,
        frames: i32,
        events: *const u32,
        out_motion: *mut i32,
        out_window: *mut f32,
        out_flags: *mut i32,
        out_presses: *mut i32,
    ) -> i32;
}

/// Mirrors the host adapter's `Fighter` fields the pure functions touch:
/// `hitlag_mul` (window), `x2218_b1` (follow_up), `x2218_b2` (rapid_flag),
/// `unk_msid` (last), `mv.co.attack1.x0` (buffered), `mv.co.attack100.x0`
/// (loop_started), `mv.co.attack100.x4` (loop_input) and `x1A54` (presses).
#[derive(Clone, Copy, Debug, Default)]
struct Local {
    motion: i32,
    window: f32,
    follow_up: bool,
    rapid_flag: bool,
    last: Option<Stage>,
    buffered: bool,
    loop_started: bool,
    loop_input: bool,
    presses: i32,
}

impl Local {
    fn wait() -> Self {
        Local {
            motion: WAIT,
            ..Default::default()
        }
    }
}

/// `checkAttack11`.
fn enter_first(state: &mut Local, second_window: f32) {
    state.motion = FIRST;
    state.window = second_window;
    state.last = Some(Stage::First);
    state.buffered = false;
    state.follow_up = false;
    state.rapid_flag = false;
    state.presses = 0;
}

/// `doAttack12Normal`.
fn enter_second(state: &mut Local, third_window: f32) {
    state.motion = SECOND;
    state.window = third_window;
    state.last = Some(Stage::Second);
    state.buffered = false;
    state.follow_up = false;
}

/// `doAttack13`: only `x2218_b1` is cleared; `Fighter_ChangeMotionState`
/// (fighter.c:1143) still zeroes the window because 46 is outside 14..=17.
fn enter_third(state: &mut Local) {
    state.motion = THIRD;
    state.window = 0.0;
    state.follow_up = false;
}

/// `ftCo_800D6B00`: `Fighter_ChangeMotionState` zeroes the window because 47
/// is outside 14..=17, and nothing here restores it.
fn enter_start(state: &mut Local) {
    state.motion = START;
    state.window = 0.0;
    state.loop_started = false;
    state.loop_input = false;
}

/// `ft_8008A2BC`: the window is kept because 14 is inside 14..=17.
fn enter_wait(state: &mut Local) {
    state.motion = WAIT;
}

/// The Wait-chain jab check (`ftCo_Attack1_CheckInput`'s pressed branch, and
/// `ftCo_Attack13_IASA`'s forward to it): only runs on a fresh press; a press
/// that selects no stage still decays, matching the pinned fallthrough.
fn wait_dispatch(state: &mut Local, jab2: f32, jab3: f32, pressed: bool) {
    if !pressed {
        decay(&mut state.window);
        return;
    }
    match wait_press(state.window, state.follow_up, state.last) {
        Some(Stage::First) => enter_first(state, jab2),
        Some(Stage::Second) => enter_second(state, jab3),
        Some(Stage::Third) => enter_third(state),
        None => decay(&mut state.window),
    }
}

/// One simulated frame in `oracle_jab_sequence`'s exact order: (a) apply the
/// event bits, (b) the motion's anim callback, (c) the motion's input
/// callback.
fn step(state: &mut Local, attrs: [f32; 3], event: u32) {
    let pressed = event & 1 != 0;
    let released = event & 2 != 0;
    state.follow_up = event & 4 != 0;
    if event & 8 != 0 {
        state.rapid_flag = event & 16 != 0;
    }
    let allow_interrupt = event & 32 != 0;
    let anim_end = event & 64 != 0;
    let loop_check_bit = event & 128 != 0;
    let frame_zero = event & 256 != 0;
    let [jab2, jab3, rapid_window] = attrs;

    // (b) anim callback.
    if state.motion == LOOP {
        if frame_zero {
            state.loop_started = true;
        }
        if loop_check_bit && loop_check(state.loop_started, &mut state.loop_input) {
            state.motion = END;
        }
    } else if anim_end {
        match state.motion {
            FIRST | SECOND | THIRD | END => enter_wait(state),
            START => state.motion = LOOP,
            _ => {}
        }
    }

    // (c) input callback.
    match state.motion {
        WAIT => wait_dispatch(state, jab2, jab3, pressed),
        FIRST | SECOND | THIRD => {
            if rapid_count(
                &mut state.presses,
                pressed,
                released,
                rapid_window as i32,
                state.rapid_flag,
            ) {
                enter_start(state);
            } else {
                match state.motion {
                    FIRST => {
                        if buffer_follow_up(
                            &mut state.window,
                            pressed,
                            &mut state.buffered,
                            state.follow_up,
                        ) {
                            enter_second(state, jab3);
                        }
                    }
                    SECOND => {
                        if buffer_follow_up(
                            &mut state.window,
                            pressed,
                            &mut state.buffered,
                            state.follow_up,
                        ) {
                            enter_third(state);
                        }
                    }
                    THIRD if allow_interrupt => wait_dispatch(state, jab2, jab3, pressed),
                    _ => {}
                }
            }
        }
        LOOP if pressed || released => state.loop_input = true,
        _ => {}
    }
}

fn compare_sequence(attrs: [f32; 3], events: &[u32]) {
    let frames = events.len();
    let mut out_motion = vec![0i32; frames];
    let mut out_window = vec![0f32; frames];
    let mut out_flags = vec![0i32; frames];
    let mut out_presses = vec![0i32; frames];
    // SAFETY: the adapter writes exactly `frames` entries into each output
    // array, which are all sized to `frames` here.
    unsafe {
        oracle_jab_sequence(
            attrs.as_ptr(),
            frames as i32,
            events.as_ptr(),
            out_motion.as_mut_ptr(),
            out_window.as_mut_ptr(),
            out_flags.as_mut_ptr(),
            out_presses.as_mut_ptr(),
        );
    }
    let mut state = Local::wait();
    for (i, &event) in events.iter().enumerate() {
        step(&mut state, attrs, event);
        assert_eq!(state.motion, out_motion[i], "frame {i} motion {events:?}");
        assert_eq!(
            state.window.to_bits(),
            out_window[i].to_bits(),
            "frame {i} window {events:?}"
        );
        let expected_flags = i32::from(state.follow_up)
            | (i32::from(state.rapid_flag) << 1)
            | (i32::from(state.buffered) << 2)
            | (i32::from(state.loop_started) << 3)
            | (i32::from(state.loop_input) << 4);
        assert_eq!(expected_flags, out_flags[i], "frame {i} flags {events:?}");
        assert_eq!(
            state.presses, out_presses[i],
            "frame {i} presses {events:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_jab_sequences_match(
        jab2 in 0.0_f32..8.0,
        jab3 in 0.0_f32..8.0,
        rapid in 1_i32..5,
        events in prop::collection::vec(any::<u32>(), 1..=24),
    ) {
        compare_sequence([jab2, jab3, rapid as f32], &events);
    }
}

#[test]
fn boundary_sequences_match() {
    // A full first-second-third combo: a fresh press starts the first jab,
    // the follow-up flag (raised from a reachable pose) fires the moment a
    // fresh press lands, and the third jab plays out uninterrupted.
    const P: u32 = 1; // pressed
    const R: u32 = 2; // released
    const F: u32 = 4; // follow_up_ready
    const AI: u32 = 32; // allow_interrupt
    const END_ANIM: u32 = 64; // animation end
    compare_sequence(
        [6.0, 6.0, 3.0],
        &[
            P,     // enter first
            0,     // held
            R | F, // release, follow-up flag raised
            P | F, // fresh press -> second
            0,     // held
            R | F, // release, follow-up flag raised
            P | F, // fresh press -> third
            0,
            0,
            R | AI, // allow_interrupt raised, no press: decays
            0,
            0,
            0,
            END_ANIM | AI, // third jab ends -> Wait
        ],
    );

    // A follow-up press from Wait inside the window, then an expired window
    // falling back to a fresh first jab.
    compare_sequence(
        [3.0, 3.0, 3.0],
        &[
            P,            // enter first
            END_ANIM | F, // first jab ends into Wait with follow_up raised
            0,            // Wait decays: window 3 -> 2
            0,            // window 2 -> 1
            P | F,        // follow-up press inside the window -> second
        ],
    );
    compare_sequence(
        [1.0, 1.0, 3.0],
        &[
            P,            // enter first
            END_ANIM | F, // -> Wait, window 1
            0,            // window 1 -> 0
            0,            // stays at 0
            P,            // window expired: fresh first jab
        ],
    );

    // A rapid entry: two fresh presses and a release reach the window (3).
    compare_sequence(
        [6.0, 6.0, 3.0],
        &[
            P, // enter first; uncounted
            R, // count 1
            P, // count 2
            R, // count 3 -> Attack100Start
        ],
    );

    // The rapid loop continues while input is latched, then ends without it.
    const LOOP_CHECK: u32 = 128;
    const FRAME_ZERO: u32 = 256;
    compare_sequence(
        [6.0, 6.0, 3.0],
        &[
            P, R, P, R,          // -> Attack100Start
            END_ANIM,   // -> Attack100Loop
            FRAME_ZERO, // loop frame 0
            P,          // latch input for the check
            0, LOOP_CHECK, // continues (input was latched)
            FRAME_ZERO, // second cycle's frame 0
            0, 0, LOOP_CHECK, // ends: no input was latched
        ],
    );
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let attack1 = include_str!("oracle/original/attack1.c");
    let attack100 = include_str!("oracle/original/attack100.c");
    assert!(attack1.contains("bool ftCo_Attack1_CheckInput(Fighter_GObj* gobj)"));
    assert!(attack1.contains("static void checkAttack11(Fighter_GObj* gobj)"));
    assert!(attack1.contains("bool checkAttack12(Fighter_GObj* gobj)"));
    assert!(attack1.contains("bool checkAttack13(Fighter_GObj* gobj)"));
    assert!(attack1.contains("void ftCo_Attack11_IASA(Fighter_GObj* gobj)"));
    assert!(attack1.contains("void ftCo_Attack12_IASA(Fighter_GObj* gobj)"));
    assert!(attack1.contains("void ftCo_Attack13_IASA(Fighter_GObj* gobj)"));
    assert!(attack100.contains("bool ftCo_Attack_800D6A50(Fighter_GObj* gobj)"));
    assert!(attack100.contains("void ftCo_800D6B00(Fighter_GObj* gobj, enum_t msid)"));
    assert!(attack100.contains("void ftCo_Attack100Loop_Anim(Fighter_GObj* gobj)"));
    assert!(attack100.contains("void ftCo_Attack100Loop_IASA(Fighter_GObj* gobj)"));
}
