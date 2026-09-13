//! Fox/Falco neutral special (Blaster) checked against the pinned C
//! (`ftfoxspecialn.c`): the Start/Loop/End state machine's own Enter reset,
//! the turnaround-latch predicate (`ftFox_SpecialN_CheckLoopInput`), the
//! Start->Loop transition, Loop's own repeat-vs-end decision and same-frame
//! fire check, End's Wait/Fall/FallSpecial dispatch, and
//! `PrepareBlasterShot`/`FireBlasterShot` with the item spawn's own
//! angle/speed/kind captured. Each comparison pins the extracted callback's
//! exact behaviour with an inline Rust formula, matching
//! `fox_down_special_differential.rs`'s own style -- `tests/
//! game_neutral_special.rs` and `game_neutral_special_reflect.rs`
//! separately exercise the Rust engine's own mirror end to end.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_neutral_enter(
        ground: bool,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        self_vel_z_in: f32,
        out_msid: *mut i32,
        out_cmd_vars0: *mut i32,
        out_cmd_vars1: *mut i32,
        out_cmd_vars2: *mut i32,
        out_cmd_vars3: *mut i32,
        out_is_blaster_loop: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_self_vel_z: *mut f32,
    );
    fn oracle_neutral_check_loop_input(cmd0_in: i32, pressed_b: bool, is_loop_in: bool) -> bool;
    fn oracle_neutral_start_anim(ground: bool, frames_remaining: bool, out_msid: *mut i32) -> i32;
    fn oracle_neutral_loop_anim(
        ground: bool,
        frames_remaining: bool,
        is_blaster_loop_in: i32,
        cmd_vars2_in: i32,
        facing_dir_in: f32,
        angle_attr_in: f32,
        vel_attr_in: f32,
        kind_in: i32,
        out_msid: *mut i32,
        out_is_blaster_loop: *mut i32,
        out_fired: *mut i32,
        out_angle: *mut f64,
        out_speed: *mut f32,
        out_kind: *mut i32,
    );
    fn oracle_neutral_end_anim_ground(
        frames_remaining: bool,
        out_wait_calls: *mut i32,
        out_change_motion_state_calls: *mut i32,
    );
    fn oracle_neutral_end_anim_air(
        frames_remaining: bool,
        landing_lag_in: f32,
        out_fall_calls: *mut i32,
        out_fall_special_calls: *mut i32,
        out_fall_special_lag: *mut f32,
    );
    fn oracle_neutral_create_blaster_shot(
        cmd_vars2_in: i32,
        facing_dir_in: f32,
        angle_attr_in: f32,
        vel_attr_in: f32,
        kind_in: i32,
        out_fired: *mut i32,
        out_angle: *mut f64,
        out_speed: *mut f32,
        out_kind: *mut i32,
    );
    fn oracle_neutral_trace_new(
        ground: bool,
        angle_attr: f32,
        vel_attr: f32,
        kind: i32,
    ) -> *mut NeutralScriptTrace;
    fn oracle_neutral_trace_free(trace: *mut NeutralScriptTrace);
    fn oracle_neutral_trace_step(
        trace: *mut NeutralScriptTrace,
        pressed_b: bool,
        set_slot: i32,
        set_value: i32,
        frames_remaining: bool,
        out_phase: *mut i32,
        out_armed: *mut i32,
        out_fired: *mut i32,
    ) -> i32;
}

/// Opaque handle to the C oracle's own persistent per-trace `Fighter`
/// (`tests/oracle/neutral_special.c`'s `NeutralScriptTrace`).
#[repr(C)]
struct NeutralScriptTrace {
    _private: [u8; 0],
}

const MS_START_GROUND: i32 = 3000;
const MS_LOOP_GROUND: i32 = 3001;
const MS_END_GROUND: i32 = 3002;
const MS_START_AIR: i32 = 3003;
const MS_LOOP_AIR: i32 = 3004;
#[allow(dead_code)]
const MS_END_AIR: i32 = 3005;

fn same_bits(label: &str, actual: f32, expected: f32) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{label}: {actual} (0x{:08x}) != {expected} (0x{:08x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

/// `ftFox_SpecialN_PrepareBlasterShot`'s own launch-angle formula, inline:
/// `fp->facing_dir == 1.0F ? x10 : M_PI - x10`.
fn expected_launch_angle(facing_dir: f32, angle_attr: f64) -> f64 {
    if facing_dir == 1.0 {
        angle_attr
    } else {
        core::f64::consts::PI - angle_attr
    }
}

fn compare_enter(ground: bool, gr_vel_in: f32, svx: f32, svy: f32, svz: f32) {
    let (
        mut msid,
        mut c0,
        mut c1,
        mut c2,
        mut c3,
        mut is_loop,
        mut gr_vel,
        mut out_svx,
        mut out_svy,
        mut out_svz,
    ) = (0, 0, 0, 0, 0, 0, 0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_neutral_enter(
            ground,
            gr_vel_in,
            svx,
            svy,
            svz,
            &mut msid,
            &mut c0,
            &mut c1,
            &mut c2,
            &mut c3,
            &mut is_loop,
            &mut gr_vel,
            &mut out_svx,
            &mut out_svy,
            &mut out_svz,
        );
    }
    assert_eq!(
        msid,
        if ground {
            MS_START_GROUND
        } else {
            MS_START_AIR
        }
    );
    assert_eq!((c0, c1, c2, c3), (0, 0, 0, 0));
    assert_eq!(is_loop, 0);
    if ground {
        // `ftFx_SpecialN_Enter`'s own explicit zero, unconditionally, after
        // `ftCommon_8007D7FC` (which this adapter captures rather than
        // reimplements -- see its own header note).
        same_bits("gr_vel", gr_vel, 0.0);
        same_bits("self_vel_x", out_svx, 0.0);
        same_bits("self_vel_y", out_svy, 0.0);
        same_bits("self_vel_z", out_svz, 0.0);
    } else {
        // `ftFx_SpecialAirN_Enter` touches neither `gr_vel` nor `self_vel`.
        same_bits("gr_vel", gr_vel, gr_vel_in);
        same_bits("self_vel_x", out_svx, svx);
        same_bits("self_vel_y", out_svy, svy);
        same_bits("self_vel_z", out_svz, svz);
    }
    let _ = (svx, svy, svz);
}

fn compare_check_loop_input(cmd0: i32, pressed_b: bool, is_loop_in: bool) {
    let expected = is_loop_in || (cmd0 != 0 && pressed_b);
    let actual = unsafe { oracle_neutral_check_loop_input(cmd0, pressed_b, is_loop_in) };
    assert_eq!(actual, expected);
}

fn compare_start_anim(ground: bool, frames_remaining: bool) {
    let mut msid = 0;
    let transitions = unsafe { oracle_neutral_start_anim(ground, frames_remaining, &mut msid) };
    if frames_remaining {
        assert_eq!(transitions, 0);
    } else {
        assert_eq!(transitions, 1);
        assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_loop_anim(
    ground: bool,
    frames_remaining: bool,
    is_blaster_loop_in: bool,
    cmd_vars2_in: i32,
    facing_dir_in: f32,
    angle_attr: f32,
    vel_attr: f32,
    kind: i32,
) {
    let (mut msid, mut is_loop_out, mut fired, mut angle, mut speed, mut out_kind) =
        (0, 0, 0, 0.0, 0.0, 0);
    unsafe {
        oracle_neutral_loop_anim(
            ground,
            frames_remaining,
            is_blaster_loop_in as i32,
            cmd_vars2_in,
            facing_dir_in,
            angle_attr,
            vel_attr,
            kind,
            &mut msid,
            &mut is_loop_out,
            &mut fired,
            &mut angle,
            &mut speed,
            &mut out_kind,
        );
    }
    if frames_remaining {
        assert_eq!(msid, 0, "no transition mid-clip");
        assert_eq!(
            is_loop_out, is_blaster_loop_in as i32,
            "isBlasterLoop untouched mid-clip"
        );
    } else if is_blaster_loop_in {
        assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
        assert_eq!(
            is_loop_out, 0,
            "a repeat cycle resets isBlasterLoop for the next one"
        );
    } else {
        assert_eq!(msid, if ground { MS_END_GROUND } else { MS_END_AIR });
    }
    // The same-frame fire check (`cmd_vars[2] != 0`) is independent of the
    // loop/end transition decision above -- both branches re-derive it.
    if cmd_vars2_in != 0 {
        assert_eq!(fired, 1);
        same_bits(
            "angle",
            angle as f32,
            expected_launch_angle(facing_dir_in, angle_attr as f64) as f32,
        );
        same_bits("speed", speed, vel_attr);
        assert_eq!(out_kind, kind);
    } else {
        assert_eq!(fired, 0);
    }
}

fn compare_end_anim_ground(frames_remaining: bool) {
    let (mut wait_calls, mut change_calls) = (0, 0);
    unsafe {
        oracle_neutral_end_anim_ground(frames_remaining, &mut wait_calls, &mut change_calls);
    }
    if frames_remaining {
        assert_eq!(wait_calls, 0);
    } else {
        // Direct `Wait` re-entry, no landing lag, no further motion-state
        // change of its own.
        assert_eq!(wait_calls, 1);
        assert_eq!(change_calls, 0);
    }
}

fn compare_end_anim_air(frames_remaining: bool, landing_lag: f32) {
    let (mut fall_calls, mut fall_special_calls, mut fall_special_lag) = (0, 0, 0.0);
    unsafe {
        oracle_neutral_end_anim_air(
            frames_remaining,
            landing_lag,
            &mut fall_calls,
            &mut fall_special_calls,
            &mut fall_special_lag,
        );
    }
    if frames_remaining {
        assert_eq!((fall_calls, fall_special_calls), (0, 0));
        return;
    }
    if landing_lag == 0.0 {
        assert_eq!((fall_calls, fall_special_calls), (1, 0));
    } else {
        assert_eq!((fall_calls, fall_special_calls), (0, 1));
        same_bits("fall_special_lag", fall_special_lag, landing_lag);
    }
}

fn compare_create_blaster_shot(
    cmd_vars2_in: i32,
    facing_dir_in: f32,
    angle_attr: f32,
    vel_attr: f32,
    kind: i32,
) {
    let (mut fired, mut angle, mut speed, mut out_kind) = (0, 0.0, 0.0, 0);
    unsafe {
        oracle_neutral_create_blaster_shot(
            cmd_vars2_in,
            facing_dir_in,
            angle_attr,
            vel_attr,
            kind,
            &mut fired,
            &mut angle,
            &mut speed,
            &mut out_kind,
        );
    }
    if cmd_vars2_in != 0 {
        assert_eq!(fired, 1);
        same_bits(
            "angle",
            angle as f32,
            expected_launch_angle(facing_dir_in, angle_attr as f64) as f32,
        );
        same_bits("speed", speed, vel_attr);
        assert_eq!(out_kind, kind);
    } else {
        assert_eq!(fired, 0);
    }
}

/// The test's own restatement of `neutral::apply_script_frame`
/// (`src/game/characters/fox/neutral.rs`) -- see that function's own
/// citation of `ftaction.c:456-475`. Kept independent of the production
/// function (this suite's own established style: hand-derive the expected
/// formula, then check the real extracted decomp agrees), and returns the
/// single `(slot, value)` edge applied this frame, if any, so the caller can
/// feed the identical event into `oracle_neutral_trace_step` -- this port's
/// synthetic tables below never set two slots on the same frame, matching
/// every real Fox/Falco script this batch inspected.
fn script_edge(table: &[[Option<u32>; 4]], frame: usize) -> Option<(usize, u32)> {
    let row = table.get(frame)?;
    let previous = frame.checked_sub(1).and_then(|f| table.get(f));
    for (slot, value) in row.iter().enumerate() {
        if let Some(value) = value {
            let is_new_assignment = match previous {
                Some(p) => p[slot] != Some(*value),
                None => true,
            };
            if is_new_assignment {
                return Some((slot, *value));
            }
        }
    }
    None
}

/// Builds a synthetic forward-filled script table (matching the exporter's
/// own shape): `len` frames, slot `slot` becomes `Some(value)` from frame
/// `set_frame` onward, every other slot/frame `None`.
fn script_table(len: usize, slot: usize, set_frame: usize, value: u32) -> Vec<[Option<u32>; 4]> {
    let mut table = vec![[None; 4]; len];
    for row in &mut table[set_frame..] {
        row[slot] = Some(value);
    }
    table
}

/// Drives the real decomp's Start/Loop IASA+Anim across `total_ticks`
/// simulated frames, applying `start_table`/`loop_table`'s own script edges
/// (`script_edge`) each tick exactly as `neutral::apply_script_frame` would,
/// and a fresh-B press on every tick listed in `press_ticks`. Returns the
/// per-tick `(phase, armed, fired)` the oracle reports, `phase` 0/1/2
/// matching `NeutralScriptTrace`'s own Start/Loop/End enum.
fn run_script_trace(
    ground: bool,
    start_table: &[[Option<u32>; 4]],
    loop_table: &[[Option<u32>; 4]],
    press_ticks: &[usize],
    total_ticks: usize,
) -> Vec<(i32, i32, i32)> {
    let trace = unsafe { oracle_neutral_trace_new(ground, 0.0, 7.0, 54) };
    let mut phase = 0usize;
    let mut frame_in_phase = 0usize;
    let mut out = Vec::with_capacity(total_ticks);
    for tick in 0..total_ticks {
        let (table, len) = if phase == 0 {
            (start_table, start_table.len())
        } else {
            (loop_table, loop_table.len())
        };
        let edge = script_edge(table, frame_in_phase);
        let pressed = press_ticks.contains(&tick);
        let frames_remaining = frame_in_phase + 1 < len;
        let (mut out_phase, mut out_armed, mut out_fired) = (0, 0, 0);
        unsafe {
            oracle_neutral_trace_step(
                trace,
                pressed,
                edge.map_or(-1, |(slot, _)| slot as i32),
                edge.map_or(0, |(_, value)| value as i32),
                frames_remaining,
                &mut out_phase,
                &mut out_armed,
                &mut out_fired,
            );
        }
        out.push((out_phase, out_armed, out_fired));
        if frame_in_phase + 1 >= len {
            phase = out_phase as usize;
            frame_in_phase = 0;
        } else {
            frame_in_phase += 1;
        }
    }
    unsafe { oracle_neutral_trace_free(trace) };
    out
}

/// The real Fox ground timings this batch's own exported `fighters/fox.json`
/// showed (`specials.neutral.script`): Start is 7 frames with `cmd_vars[0]`
/// set at frame 4; Loop is 10 frames with `cmd_vars[2]` set at frame 5.
/// Drives two full Loop passes (a fresh press during Start arms the first;
/// arming is *not* held state -- `ftFox_SpecialN_CheckLoopInput`'s own
/// `pressed_buttons` is a single-frame edge, so the second pass, reset by
/// `FinishLoopTransition`, needs its own fresh press) and checks every
/// tick's `(phase, armed, fired)` against the exact source citations:
/// `ftFox_SpecialN_CheckLoopInput` (`cmd_vars[0] != 0 && B pressed`) and
/// `CreateBlasterShot`/Loop's own inline check (`cmd_vars[2] != 0`, cleared
/// the same frame it fires).
#[test]
fn script_trace_matches_the_real_fox_timings() {
    let start_table = script_table(7, 0, 4, 1);
    let loop_table = script_table(10, 2, 5, 1);
    // Absolute ticks: Start is ticks 0..6, Loop pass 1 is ticks 7..16, Loop
    // pass 2 is ticks 17..26. A press at tick 4 arms pass 1 (during Start's
    // own tail); a second press at tick 20 (pass 2's own frame_in_phase 3)
    // re-arms pass 2 after the repeat transition's reset.
    let press_ticks = [4, 20];
    let trace = run_script_trace(true, &start_table, &loop_table, &press_ticks, 27);

    for (tick, entry) in trace[0..4].iter().enumerate() {
        assert_eq!(entry.1, 0, "armed before cmd_vars[0] sets, tick {tick}");
    }
    for (tick, entry) in trace[4..16].iter().enumerate() {
        let tick = tick + 4;
        assert_eq!(
            entry.1, 1,
            "armed once cmd_vars[0] sets and B presses, tick {tick}"
        );
    }
    // The Loop1->Loop2 transition (tick 16, Loop1's own last frame) resets
    // `isBlasterLoop` for the new pass (`FinishLoopTransition`), observed
    // here as this port's own oracle reads it *after* that same-frame reset.
    for (tick, entry) in trace[16..20].iter().enumerate() {
        let tick = tick + 16;
        assert_eq!(
            entry.1, 0,
            "unarmed after the reset, before pass 2's own press, tick {tick}"
        );
    }
    for (tick, entry) in trace[20..26].iter().enumerate() {
        let tick = tick + 20;
        assert_eq!(
            entry.1, 1,
            "re-armed by pass 2's own fresh press, tick {tick}"
        );
    }
    // Tick 26 is pass 2's own last frame: armed (from tick 20's press) means
    // it repeats into a third pass, whose own `FinishLoopTransition` resets
    // `isBlasterLoop` the same frame -- the same reset already seen at
    // tick 16, one pass earlier.
    assert_eq!(
        trace[26].1, 0,
        "isBlasterLoop resets on the second repeat transition"
    );

    for (tick, (_, _, fired)) in trace.iter().enumerate() {
        let expected = tick == 12 || tick == 22;
        assert_eq!(
            *fired != 0,
            expected,
            "fire only on each pass's own frame-5, tick {tick}"
        );
    }

    for (tick, entry) in trace[0..6].iter().enumerate() {
        assert_eq!(entry.0, 0, "still Start, tick {tick}");
    }
    // Tick 6 is Start's own last frame (`frame_in_phase == 6`, `len == 7`):
    // its IASA/Anim still run against the Start table, but the reported
    // phase already reflects this same tick's Start->Loop transition.
    for (tick, entry) in trace[6..27].iter().enumerate() {
        let tick = tick + 6;
        assert_eq!(entry.0, 1, "Loop, tick {tick}");
    }
}

// A press strictly before `cmd_vars[0]` sets never arms, no matter how
// close to the set frame; a press on or after it always does (matching
// `ftFox_SpecialN_CheckLoopInput`'s plain `!= 0` gate, no debounce/edge
// subtlety of its own beyond the fresh-press check already covered by
// `check_loop_input_matches_arbitrary_inputs`). Fuzzes the set frame and
// press frame independently across a single Start phase (Loop never
// entered -- `frames_remaining` stays `true` throughout, so no transition
// muddies the read).
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn script_trace_arms_only_at_or_after_the_scripted_frame(
        set_frame in 0usize..12,
        press_frame in 0usize..12,
        start_len in 13usize..20,
    ) {
        let start_table = script_table(start_len, 0, set_frame, 1);
        let loop_table = script_table(1, 2, 0, 1);
        let trace = run_script_trace(true, &start_table, &loop_table, &[press_frame], start_len - 1);
        let expected_armed = press_frame >= set_frame;
        prop_assert_eq!(
            trace.last().unwrap().1 != 0,
            expected_armed,
            "set_frame={} press_frame={}",
            set_frame,
            press_frame
        );
    }
}

#[test]
fn adapter_statements_are_verbatim_in_the_pinned_sources() {
    let adapter = include_str!("oracle/neutral_special.c");
    let source = include_str!("oracle/original/ftfox_inlines.h");
    let block = adapter
        .split("/* BEGIN VERBATIM CHECK LOOP INPUT */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM CHECK LOOP INPUT */")
        .next()
        .unwrap();
    assert!(source.contains(block));
}

#[test]
fn known_values_match() {
    compare_enter(true, 3.0, 1.0, 2.0, 0.0);
    compare_enter(false, 3.0, 1.0, 2.0, 0.0);
    compare_start_anim(true, true);
    compare_start_anim(true, false);
    compare_start_anim(false, false);
    compare_loop_anim(true, false, true, 1, 1.0, 0.0, 7.0, 54);
    compare_loop_anim(true, false, false, 0, -1.0, 0.0, 7.0, 54);
    compare_end_anim_ground(false);
    compare_end_anim_air(false, 0.0);
    compare_end_anim_air(false, 12.0);
    compare_create_blaster_shot(1, 1.0, 0.0, 7.0, 54);
    compare_create_blaster_shot(0, 1.0, 0.0, 7.0, 54);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn enter_matches_arbitrary_velocities(
        ground in any::<bool>(),
        gr_vel in -50.0f32..50.0,
        svx in -50.0f32..50.0,
        svy in -50.0f32..50.0,
        svz in -50.0f32..50.0,
    ) {
        compare_enter(ground, gr_vel, svx, svy, svz);
    }

    #[test]
    fn check_loop_input_matches_arbitrary_inputs(
        cmd0 in any::<i32>(),
        pressed_b in any::<bool>(),
        is_loop in any::<bool>(),
    ) {
        compare_check_loop_input(cmd0, pressed_b, is_loop);
    }

    #[test]
    fn start_anim_matches(ground in any::<bool>(), frames_remaining in any::<bool>()) {
        compare_start_anim(ground, frames_remaining);
    }

    #[test]
    #[allow(clippy::too_many_arguments)]
    fn loop_anim_matches(
        ground in any::<bool>(),
        frames_remaining in any::<bool>(),
        is_blaster_loop in any::<bool>(),
        fire in any::<bool>(),
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        angle_attr in -10.0f32..10.0,
        vel_attr in 0.0f32..20.0,
        kind in any::<i32>(),
    ) {
        compare_loop_anim(
            ground,
            frames_remaining,
            is_blaster_loop,
            if fire { 1 } else { 0 },
            facing_dir,
            angle_attr,
            vel_attr,
            kind,
        );
    }

    #[test]
    fn end_anim_ground_matches(frames_remaining in any::<bool>()) {
        compare_end_anim_ground(frames_remaining);
    }

    #[test]
    fn end_anim_air_matches(frames_remaining in any::<bool>(), landing_lag in 0.0f32..40.0) {
        compare_end_anim_air(frames_remaining, landing_lag);
    }

    #[test]
    fn create_blaster_shot_matches(
        fire in any::<bool>(),
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        angle_attr in -10.0f32..10.0,
        vel_attr in 0.0f32..20.0,
        kind in any::<i32>(),
    ) {
        compare_create_blaster_shot(if fire { 1 } else { 0 }, facing_dir, angle_attr, vel_attr, kind);
    }
}
