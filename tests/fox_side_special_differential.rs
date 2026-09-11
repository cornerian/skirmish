//! Fox/Falco side special checked against the complete pinned C: the common
//! grounded/aerial dispatch (`ftCo_SpecialS.c`, `ftCo_SpecialAir.c`) and
//! every Start/Dash/End phase callback (`ftfoxspecials.c`). Compares entry
//! selection, speed arithmetic, gravity delays, frictions, TransN
//! velocities and the conversion/exit decisions bit-exactly. The `doEnter`
//! ground-friction-multiplier gap documented in `docs/fox-side-special.md`
//! is exercised directly (`entry_multiplier_gap_is_the_documented_one`)
//! rather than silently excluded from the suite.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::{
    Movement,
    characters::fox::{entry_ground_velocity, has_input, should_turn},
};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_special_s_check_input(
        has_fox_entry: bool,
        pressed_buttons: u32,
        stick_x: f32,
        turn_threshold: f32,
        facing: f32,
        x688: u8,
        retention: f32,
        friction_multiplier: f32,
        gr_vel_in: f32,
        out_entry_calls: *mut i32,
        out_update_facing: *mut i32,
        out_gr_vel: *mut f32,
        out_facing: *mut f32,
    ) -> i32;
    fn oracle_special_s_has_input(pressed_buttons: u32, stick_x: f32, threshold: f32) -> i32;
    fn oracle_special_air_check_input(
        has_hi: bool,
        has_lw: bool,
        has_s: bool,
        has_n: bool,
        pressed_buttons: u32,
        stick_x: f32,
        stick_y: f32,
        vertical_threshold: f32,
        side_threshold: f32,
        turn_threshold: f32,
        neutral_threshold: f32,
        facing: f32,
        x676_x: f32,
        x2228_b7: bool,
        out_facing: *mut f32,
        out_update_facing: *mut i32,
    ) -> i32;

    fn oracle_fox_start_enter(
        ground: bool,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        max_jumps: i32,
        x24: f32,
        x28: f32,
        out_msid: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_jumps_used: *mut i32,
        out_gravity_delay: *mut f32,
    );
    fn oracle_fox_start_phys(
        ground: bool,
        gravity_delay_in: f32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        ground_friction: f32,
        walk_max_vel: f32,
        x2c: f32,
        x30: f32,
        terminal_velocity: f32,
        above_walk_friction_mul: f32,
        out_gravity_delay: *mut f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_fox_dash_phys(
        ground: bool,
        has_trans_n: bool,
        trans_n_z: f32,
        trans_n_y: f32,
        facing: f32,
        ground_friction: f32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_fox_dash_iasa(ground_owns_dispatch: bool, air_actually: bool, pressed_b: bool)
    -> i32;
    fn oracle_fox_end_enter(
        ground: bool,
        facing: f32,
        x34: f32,
        x3c: f32,
        x44: f32,
        out_msid: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_gravity_delay: *mut f32,
    );
    fn oracle_fox_end_phys(
        ground: bool,
        gravity_delay_in: f32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        ground_friction: f32,
        x38: f32,
        x40: f32,
        x48: f32,
        terminal_velocity: f32,
        out_gravity_delay: *mut f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_fox_start_coll(
        ground: bool,
        coll_result: bool,
        ledge_check: bool,
        ledge_common: bool,
        out_msid: *mut i32,
    ) -> i32;
    fn oracle_fox_dash_coll(
        ground: bool,
        coll_result: bool,
        ledge_check: bool,
        ledge_common: bool,
        out_msid: *mut i32,
    ) -> i32;
    fn oracle_fox_end_coll(
        ground: bool,
        coll_result: bool,
        ledge_check: bool,
        ledge_common: bool,
        out_fall_calls: *mut i32,
        out_landing_calls: *mut i32,
        out_landing_allow: *mut bool,
        out_landing_lag: *mut f32,
    ) -> i32;
}

const MS_START_GROUND: i32 = 1000;
const MS_DASH_GROUND: i32 = 1001;
const MS_END_GROUND: i32 = 1002;
const MS_START_AIR: i32 = 1003;
const MS_DASH_AIR: i32 = 1004;
const MS_END_AIR: i32 = 1005;

/// NaN payload propagation through arithmetic is unspecified, so two NaN
/// results are equivalent for parity purposes even when their bit patterns
/// differ; a non-NaN result must still match exactly.
fn same_bits(label: &str, actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

fn mirror_friction_ground(gr_vel: f32, friction: f32) -> f32 {
    let mut m = Movement {
        ground_velocity: gr_vel,
        ..Movement::default()
    };
    m.friction_ground(friction);
    m.ground_velocity + m.ground_acceleration
}

fn mirror_friction_air(self_vel_x: f32, friction: f32) -> f32 {
    let mut m = Movement {
        self_velocity: [self_vel_x, 0.0, 0.0],
        ..Movement::default()
    };
    m.friction_air(friction);
    m.self_velocity[0] + m.animation_velocity[0]
}

fn mirror_fall(self_vel_y: f32, gravity: f32, terminal: f32) -> f32 {
    let mut m = Movement {
        self_velocity: [0.0, self_vel_y, 0.0],
        ..Movement::default()
    };
    m.fall(gravity, terminal);
    m.self_velocity[1]
}

// ---- Entry selection (`ftCo_SpecialS_HasInput`/`CheckInput`, `doEnter`). ----

fn compare_special_s_has_input(pressed: bool, stick_x: f32, threshold: f32) {
    let expected =
        unsafe { oracle_special_s_has_input(u32::from(pressed) << 13, stick_x, threshold) };
    assert_eq!(has_input(pressed, stick_x, threshold), expected != 0);
}

/// `doEnter`'s multiplier fixed at 1.0, matching what the Rust side
/// actually implements (the per-floor-material lookup is unmodeled; see
/// `docs/fox-side-special.md`).
fn compare_special_s_check_input(
    has_fox_entry: bool,
    fresh_b: bool,
    stick_x: f32,
    turn_threshold: f32,
    facing: f32,
    x688: u8,
    retention: f32,
) {
    let (mut entry_calls, mut update_facing, mut out_gr_vel, mut out_facing) = (0, 0, 0.0, 0.0);
    let gr_vel_in = 7.0f32;
    let fired = unsafe {
        oracle_special_s_check_input(
            has_fox_entry,
            u32::from(fresh_b) << 13,
            stick_x,
            turn_threshold,
            facing,
            x688,
            retention,
            1.0,
            gr_vel_in,
            &mut entry_calls,
            &mut update_facing,
            &mut out_gr_vel,
            &mut out_facing,
        )
    };
    let expected_fired = has_fox_entry && x688 == 0;
    assert_eq!(fired != 0, expected_fired);
    if !expected_fired {
        return;
    }
    assert_eq!(entry_calls, 1);
    let turned = should_turn(stick_x, facing, turn_threshold);
    assert_eq!(update_facing != 0, turned);
    let expected_facing = if turned { -facing } else { facing };
    same_bits("facing", out_facing, expected_facing);
    let expected_gr_vel = entry_ground_velocity(gr_vel_in, retention);
    same_bits("gr_vel", out_gr_vel, expected_gr_vel);
}

#[allow(clippy::too_many_arguments)]
fn mirror_special_air(
    has_hi: bool,
    has_lw: bool,
    has_s: bool,
    has_n: bool,
    fresh_b: bool,
    stick_x: f32,
    stick_y: f32,
    vertical_threshold: f32,
    side_threshold: f32,
    turn_threshold: f32,
    neutral_threshold: f32,
    facing: f32,
    x676_x: f32,
    x2228_b7: bool,
) -> (i32, f32) {
    if !fresh_b {
        return (0, facing);
    }
    if stick_y >= vertical_threshold {
        return (if has_hi { 1 } else { 0 }, facing);
    }
    if stick_y <= -vertical_threshold {
        return (if has_lw { 2 } else { 0 }, facing);
    }
    if stick_x.abs() >= side_threshold {
        if !has_s {
            return (0, facing);
        }
        let out_facing = if should_turn(stick_x, facing, turn_threshold) {
            -facing
        } else {
            facing
        };
        return (3, out_facing);
    }
    if !has_n {
        return (0, facing);
    }
    let flip = x676_x < neutral_threshold
        && ((facing == -1.0 && x2228_b7) || (facing == 1.0 && !x2228_b7));
    (4, if flip { -facing } else { facing })
}

#[allow(clippy::too_many_arguments)]
fn compare_special_air(
    has_hi: bool,
    has_lw: bool,
    has_s: bool,
    has_n: bool,
    fresh_b: bool,
    stick_x: f32,
    stick_y: f32,
    vertical_threshold: f32,
    side_threshold: f32,
    turn_threshold: f32,
    neutral_threshold: f32,
    facing: f32,
    x676_x: f32,
    x2228_b7: bool,
) {
    let (mut out_facing, mut update_facing) = (0.0, 0);
    let table = unsafe {
        oracle_special_air_check_input(
            has_hi,
            has_lw,
            has_s,
            has_n,
            u32::from(fresh_b) << 13,
            stick_x,
            stick_y,
            vertical_threshold,
            side_threshold,
            turn_threshold,
            neutral_threshold,
            facing,
            x676_x,
            x2228_b7,
            &mut out_facing,
            &mut update_facing,
        )
    };
    let (expected_table, expected_facing) = mirror_special_air(
        has_hi,
        has_lw,
        has_s,
        has_n,
        fresh_b,
        stick_x,
        stick_y,
        vertical_threshold,
        side_threshold,
        turn_threshold,
        neutral_threshold,
        facing,
        x676_x,
        x2228_b7,
    );
    assert_eq!(table, expected_table);
    same_bits("facing", out_facing, expected_facing);
}

// ---- Start phase: Enter, Phys. ----

fn compare_start_enter(ground: bool, gr_vel_in: f32, self_vel_x_in: f32, x24: f32, x28: f32) {
    let max_jumps = 5;
    let (mut msid, mut gr_vel, mut svx, mut svy, mut jumps, mut delay) = (0, 0.0, 0.0, 0.0, 0, 0.0);
    unsafe {
        oracle_fox_start_enter(
            ground,
            gr_vel_in,
            self_vel_x_in,
            max_jumps,
            x24,
            x28,
            &mut msid,
            &mut gr_vel,
            &mut svx,
            &mut svy,
            &mut jumps,
            &mut delay,
        );
    }
    same_bits("gravity_delay", delay, x24);
    if ground {
        assert_eq!(msid, MS_START_GROUND);
        same_bits("gr_vel", gr_vel, gr_vel_in / x28);
    } else {
        assert_eq!(msid, MS_START_AIR);
        same_bits("self_vel_x", svx, self_vel_x_in / x28);
        same_bits("self_vel_y", svy, 0.0);
        assert_eq!(jumps, max_jumps);
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_start_phys(
    ground: bool,
    gravity_delay_in: f32,
    gr_vel_in: f32,
    self_vel_x_in: f32,
    self_vel_y_in: f32,
    ground_friction: f32,
    walk_max_vel: f32,
    x2c: f32,
    x30: f32,
    terminal_velocity: f32,
    above_walk_mul: f32,
) {
    let (mut delay, mut gr_vel, mut svx, mut svy) = (0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_fox_start_phys(
            ground,
            gravity_delay_in,
            gr_vel_in,
            self_vel_x_in,
            self_vel_y_in,
            ground_friction,
            walk_max_vel,
            x2c,
            x30,
            terminal_velocity,
            above_walk_mul,
            &mut delay,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    let expected_delay = if gravity_delay_in != 0.0 {
        gravity_delay_in - 1.0
    } else {
        gravity_delay_in
    };
    same_bits("gravity_delay", delay, expected_delay);
    if ground {
        let mut friction = ground_friction;
        if gr_vel_in.abs() > walk_max_vel {
            friction *= above_walk_mul;
        }
        same_bits(
            "gr_vel",
            gr_vel,
            mirror_friction_ground(gr_vel_in, friction),
        );
    } else {
        let expected_svy = if gravity_delay_in != 0.0 {
            self_vel_y_in
        } else {
            mirror_fall(self_vel_y_in, x30, terminal_velocity)
        };
        same_bits("self_vel_y", svy, expected_svy);
        same_bits("self_vel_x", svx, mirror_friction_air(self_vel_x_in, x2c));
    }
}

// ---- Dash phase: TransN Phys, IASA shortening. ----

#[allow(clippy::too_many_arguments)]
fn compare_dash_phys(
    ground: bool,
    has_trans_n: bool,
    trans_n_z: f32,
    trans_n_y: f32,
    facing: f32,
    ground_friction: f32,
    gr_vel_in: f32,
    self_vel_x_in: f32,
    self_vel_y_in: f32,
) {
    let (mut gr_vel, mut svx, mut svy) = (0.0, 0.0, 0.0);
    unsafe {
        oracle_fox_dash_phys(
            ground,
            has_trans_n,
            trans_n_z,
            trans_n_y,
            facing,
            ground_friction,
            gr_vel_in,
            self_vel_x_in,
            self_vel_y_in,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    if ground {
        let expected = if has_trans_n {
            trans_n_z * facing
        } else {
            mirror_friction_ground(gr_vel_in, ground_friction)
        };
        same_bits("gr_vel", gr_vel, expected);
    } else {
        same_bits("self_vel_x", svx, trans_n_z * facing);
        same_bits("self_vel_y", svy, trans_n_y);
    }
}

fn compare_dash_iasa(ground_owns: bool, air_actually: bool, fresh_b: bool) {
    let msid = unsafe { oracle_fox_dash_iasa(ground_owns, air_actually, fresh_b) };
    let expected = if !fresh_b {
        0
    } else if air_actually {
        MS_END_AIR
    } else {
        MS_END_GROUND
    };
    assert_eq!(msid, expected);
}

// ---- End phase: Enter, Phys. ----

fn compare_end_enter(ground: bool, facing: f32, x34: f32, x3c: f32, x44: f32) {
    let (mut msid, mut gr_vel, mut svx, mut svy, mut delay) = (0, 0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_fox_end_enter(
            ground,
            facing,
            x34,
            x3c,
            x44,
            &mut msid,
            &mut gr_vel,
            &mut svx,
            &mut svy,
            &mut delay,
        );
    }
    same_bits("gravity_delay", delay, x44);
    if ground {
        assert_eq!(msid, MS_END_GROUND);
        same_bits("gr_vel", gr_vel, x34 * facing);
    } else {
        assert_eq!(msid, MS_END_AIR);
        same_bits("self_vel_x", svx, x3c * facing);
        same_bits("self_vel_y", svy, 0.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_end_phys(
    ground: bool,
    gravity_delay_in: f32,
    gr_vel_in: f32,
    self_vel_x_in: f32,
    self_vel_y_in: f32,
    ground_friction: f32,
    x38: f32,
    x40: f32,
    x48: f32,
    terminal_velocity: f32,
) {
    let (mut delay, mut gr_vel, mut svx, mut svy) = (0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_fox_end_phys(
            ground,
            gravity_delay_in,
            gr_vel_in,
            self_vel_x_in,
            self_vel_y_in,
            ground_friction,
            x38,
            x40,
            x48,
            terminal_velocity,
            &mut delay,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    let expected_delay = if gravity_delay_in != 0.0 {
        gravity_delay_in - 1.0
    } else {
        gravity_delay_in
    };
    same_bits("gravity_delay", delay, expected_delay);
    if ground {
        same_bits("gr_vel", gr_vel, mirror_friction_ground(gr_vel_in, x38));
    } else {
        let expected_svy = if gravity_delay_in != 0.0 {
            self_vel_y_in
        } else {
            mirror_fall(self_vel_y_in, x48, terminal_velocity)
        };
        same_bits("self_vel_y", svy, expected_svy);
        same_bits("self_vel_x", svx, mirror_friction_air(self_vel_x_in, x40));
    }
}

// ---- Conversion/exit decisions (Coll). ----

fn compare_start_coll(ground: bool, coll_result: bool, ledge_check: bool, ledge_common: bool) {
    let mut msid = 0;
    let code =
        unsafe { oracle_fox_start_coll(ground, coll_result, ledge_check, ledge_common, &mut msid) };
    if ground {
        assert_eq!(code, if coll_result { 0 } else { 1 });
        if !coll_result {
            assert_eq!(msid, MS_START_AIR);
        }
    } else {
        let expected = if ledge_check {
            2
        } else if ledge_common {
            3
        } else {
            0
        };
        assert_eq!(code, expected);
        if ledge_check {
            assert_eq!(msid, MS_START_GROUND);
        }
    }
}

fn compare_dash_coll(ground: bool, coll_result: bool, ledge_check: bool, ledge_common: bool) {
    let mut msid = 0;
    let code =
        unsafe { oracle_fox_dash_coll(ground, coll_result, ledge_check, ledge_common, &mut msid) };
    if ground {
        assert_eq!(code, if coll_result { 0 } else { 1 });
        if !coll_result {
            assert_eq!(msid, MS_DASH_AIR);
        }
    } else {
        let expected = if ledge_check {
            2
        } else if ledge_common {
            3
        } else {
            0
        };
        assert_eq!(code, expected);
        if ledge_check {
            assert_eq!(msid, MS_DASH_GROUND);
        }
    }
}

fn compare_end_coll(ground: bool, coll_result: bool, ledge_check: bool, ledge_common: bool) {
    let (mut fall_calls, mut landing_calls, mut landing_allow, mut landing_lag) =
        (0, 0, false, 0.0);
    let code = unsafe {
        oracle_fox_end_coll(
            ground,
            coll_result,
            ledge_check,
            ledge_common,
            &mut fall_calls,
            &mut landing_calls,
            &mut landing_allow,
            &mut landing_lag,
        )
    };
    if ground {
        assert_eq!(code, if coll_result { 0 } else { 1 });
        assert_eq!(fall_calls, if coll_result { 0 } else { 1 });
    } else {
        let expected = if ledge_check {
            2
        } else if ledge_common {
            3
        } else {
            0
        };
        assert_eq!(code, expected);
        assert_eq!(landing_calls, if ledge_check { 1 } else { 0 });
        if ledge_check {
            assert!(!landing_allow);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_special_s_has_input(
        pressed in any::<bool>(),
        stick in any::<u32>(),
        threshold in any::<u32>(),
    ) {
        compare_special_s_has_input(pressed, f32::from_bits(stick), f32::from_bits(threshold));
    }

    #[test]
    fn arbitrary_special_s_check_input(
        has_entry in any::<bool>(),
        fresh_b in any::<bool>(),
        stick in any::<u32>(),
        turn_threshold in any::<u32>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32), any::<u32>().prop_map(f32::from_bits)],
        x688 in any::<u8>(),
        retention in any::<u32>(),
    ) {
        compare_special_s_check_input(
            has_entry, fresh_b, f32::from_bits(stick), f32::from_bits(turn_threshold),
            facing, x688, f32::from_bits(retention),
        );
    }

    #[test]
    fn arbitrary_special_air_check_input(
        has_hi in any::<bool>(), has_lw in any::<bool>(), has_s in any::<bool>(), has_n in any::<bool>(),
        fresh_b in any::<bool>(),
        stick_x in any::<u32>(), stick_y in any::<u32>(),
        vertical_threshold in any::<u32>(), side_threshold in any::<u32>(),
        turn_threshold in any::<u32>(), neutral_threshold in any::<u32>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        x676_x in any::<u32>(), x2228_b7 in any::<bool>(),
    ) {
        compare_special_air(
            has_hi, has_lw, has_s, has_n, fresh_b,
            f32::from_bits(stick_x), f32::from_bits(stick_y),
            f32::from_bits(vertical_threshold), f32::from_bits(side_threshold),
            f32::from_bits(turn_threshold), f32::from_bits(neutral_threshold),
            facing, f32::from_bits(x676_x), x2228_b7,
        );
    }

    #[test]
    fn arbitrary_start_enter(
        ground in any::<bool>(),
        gr_vel in any::<u32>(), self_vel_x in any::<u32>(),
        x24 in any::<u32>(), x28 in any::<u32>(),
    ) {
        compare_start_enter(ground, f32::from_bits(gr_vel), f32::from_bits(self_vel_x), f32::from_bits(x24), f32::from_bits(x28));
    }

    #[test]
    fn arbitrary_start_phys(
        ground in any::<bool>(),
        delay in any::<u32>(), gr_vel in any::<u32>(), svx in any::<u32>(), svy in any::<u32>(),
        friction in any::<u32>(), walk_max in any::<u32>(), x2c in any::<u32>(), x30 in any::<u32>(),
        terminal in any::<u32>(), above_walk in any::<u32>(),
    ) {
        compare_start_phys(
            ground, f32::from_bits(delay), f32::from_bits(gr_vel), f32::from_bits(svx), f32::from_bits(svy),
            f32::from_bits(friction), f32::from_bits(walk_max), f32::from_bits(x2c), f32::from_bits(x30),
            f32::from_bits(terminal), f32::from_bits(above_walk),
        );
    }

    #[test]
    fn arbitrary_dash_phys(
        ground in any::<bool>(), has_trans_n in any::<bool>(),
        trans_z in any::<u32>(), trans_y in any::<u32>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        friction in any::<u32>(), gr_vel in any::<u32>(), svx in any::<u32>(), svy in any::<u32>(),
    ) {
        compare_dash_phys(
            ground, has_trans_n, f32::from_bits(trans_z), f32::from_bits(trans_y), facing,
            f32::from_bits(friction), f32::from_bits(gr_vel), f32::from_bits(svx), f32::from_bits(svy),
        );
    }

    #[test]
    fn arbitrary_dash_iasa(ground_owns in any::<bool>(), air_actually in any::<bool>(), fresh_b in any::<bool>()) {
        compare_dash_iasa(ground_owns, air_actually, fresh_b);
    }

    #[test]
    fn arbitrary_end_enter(
        ground in any::<bool>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        x34 in any::<u32>(), x3c in any::<u32>(), x44 in any::<u32>(),
    ) {
        compare_end_enter(ground, facing, f32::from_bits(x34), f32::from_bits(x3c), f32::from_bits(x44));
    }

    #[test]
    fn arbitrary_end_phys(
        ground in any::<bool>(),
        delay in any::<u32>(), gr_vel in any::<u32>(), svx in any::<u32>(), svy in any::<u32>(),
        friction in any::<u32>(), x38 in any::<u32>(), x40 in any::<u32>(), x48 in any::<u32>(),
        terminal in any::<u32>(),
    ) {
        compare_end_phys(
            ground, f32::from_bits(delay), f32::from_bits(gr_vel), f32::from_bits(svx), f32::from_bits(svy),
            f32::from_bits(friction), f32::from_bits(x38), f32::from_bits(x40), f32::from_bits(x48),
            f32::from_bits(terminal),
        );
    }

    #[test]
    fn arbitrary_coll_decisions(
        ground in any::<bool>(), coll in any::<bool>(), ledge_check in any::<bool>(), ledge_common in any::<bool>(),
    ) {
        compare_start_coll(ground, coll, ledge_check, ledge_common);
        compare_dash_coll(ground, coll, ledge_check, ledge_common);
        compare_end_coll(ground, coll, ledge_check, ledge_common);
    }
}

#[test]
fn special_air_check_input_regression_from_the_intermittent_c_oracle_race() {
    // Recorded `arbitrary_special_air_check_input` shrink that failed
    // intermittently under the default (multi-threaded) test runner:
    // `oracle_special_air_check_input` (`tests/oracle/special_air.c`) wrote
    // its per-call thresholds into a plain `static` `FtCommonData`/
    // `ftCommonData_` instead of a thread-local one (the same class of race
    // `3b56a66` fixed for the shared `ftCommonData` struct elsewhere), so a
    // concurrently-running property test on a separate libtest thread could
    // stomp these thresholds mid-call. Pinned here so a regression in this
    // adapter (or a real check-input arithmetic regression) is caught even
    // by a single-threaded run.
    compare_special_air(
        true,
        true,
        false,
        true,
        true,
        f32::from_bits(1577405011),
        f32::from_bits(1127699417),
        f32::from_bits(1793199457),
        f32::from_bits(2485434146),
        f32::from_bits(1274847487),
        f32::from_bits(3007116098),
        1.0,
        f32::from_bits(1662328526),
        true,
    );
}

#[test]
fn boundaries() {
    for &pressed in &[true, false] {
        for &stick in &[0.0, 0.2875, -0.2875, 1.0, -1.0, f32::NAN, f32::INFINITY] {
            compare_special_s_has_input(pressed, stick, 0.2875);
        }
    }
    for &(has_entry, fresh_b, stick, turn, facing, age, retention) in &[
        (true, true, 0.6, 0.2, 1.0, 0u8, 0.5f32),
        (true, true, -0.6, 0.2, 1.0, 0, 0.5),
        (true, true, -0.2, 0.2, 1.0, 0, 0.5),
        (true, true, 0.6, 0.2, 1.0, 1, 0.5),
        (false, true, 0.6, 0.2, 1.0, 0, 0.5),
        (true, true, f32::NAN, 0.2, 1.0, 0, 0.5),
        (true, true, 0.6, 0.2, f32::NAN, 0, 0.5),
    ] {
        compare_special_s_check_input(has_entry, fresh_b, stick, turn, facing, age, retention);
    }
    for &(hi, lw, s, n, sx, sy) in &[
        (true, true, true, true, 0.0f32, 0.9f32),
        (true, true, true, true, 0.0, -0.9),
        (true, true, true, true, 0.6, 0.0),
        (false, true, true, true, 0.0, 0.9),
        (true, false, true, true, 0.0, -0.9),
        (true, true, false, true, 0.6, 0.0),
        (true, true, true, false, 0.0, 0.0),
        (true, true, true, true, f32::NAN, f32::NAN),
    ] {
        compare_special_air(
            hi, lw, s, n, true, sx, sy, 0.6, 0.2875, 0.2, 0.7, 1.0, 0.0, false,
        );
    }
    for &ground in &[true, false] {
        compare_start_enter(ground, 0.0, 0.0, 2.0, 2.0);
        compare_start_enter(ground, f32::NAN, f32::INFINITY, 2.0, 0.0);
        compare_start_phys(ground, 2.0, 0.0, 0.0, 0.0, 0.2, 1.0, 0.02, 0.01, 3.0, 1.0);
        compare_start_phys(ground, 0.0, 5.0, 5.0, -5.0, 0.2, 1.0, 0.02, 0.01, 3.0, 1.0);
        compare_dash_phys(ground, true, 2.0, -0.2, 1.0, 0.2, 0.0, 0.0, 0.0);
        compare_dash_phys(ground, false, 2.0, -0.2, 1.0, 0.2, 5.0, 5.0, 0.0);
        compare_end_enter(ground, 1.0, 1.5, 1.2, 1.0);
        compare_end_enter(ground, -1.0, 1.5, 1.2, 0.0);
        compare_end_phys(ground, 1.0, 5.0, 5.0, -5.0, 0.05, 0.05, 0.03, 0.02, 3.0);
        compare_end_phys(ground, 0.0, 5.0, 5.0, -5.0, 0.05, 0.05, 0.03, 0.02, 3.0);
    }
    for &(ground_owns, air, fresh) in &[
        (true, false, true),
        (true, false, false),
        (false, true, true),
        (false, true, false),
    ] {
        compare_dash_iasa(ground_owns, air, fresh);
    }
    for &ground in &[true, false] {
        for &coll in &[true, false] {
            for &ledge_check in &[true, false] {
                for &ledge_common in &[true, false] {
                    compare_start_coll(ground, coll, ledge_check, ledge_common);
                    compare_dash_coll(ground, coll, ledge_check, ledge_common);
                    compare_end_coll(ground, coll, ledge_check, ledge_common);
                }
            }
        }
    }
}

/// `doEnter` scales its blend by `ft_GetGroundFrictionMultiplier(fp)`
/// (`ft_081B.c:1235-1240`), a per-floor-material lookup this port does not
/// model (`fighter::characters::fox::entry_ground_velocity` always uses
/// 1.0). Demonstrates the gap directly: on ordinary terrain (multiplier
/// 1.0) the two agree; away from 1.0 they deliberately, documentedly
/// diverge.
#[test]
fn entry_multiplier_gap_is_the_documented_one() {
    let (mut entry_calls, mut update_facing, mut out_gr_vel, mut out_facing) = (0, 0, 0.0, 0.0);
    let gr_vel_in = 10.0f32;
    let retention = 0.25f32;
    unsafe {
        oracle_special_s_check_input(
            true,
            1 << 13,
            0.0,
            0.0,
            1.0,
            0,
            retention,
            1.0,
            gr_vel_in,
            &mut entry_calls,
            &mut update_facing,
            &mut out_gr_vel,
            &mut out_facing,
        );
    }
    same_bits(
        "ordinary terrain",
        out_gr_vel,
        entry_ground_velocity(gr_vel_in, retention),
    );
    unsafe {
        oracle_special_s_check_input(
            true,
            1 << 13,
            0.0,
            0.0,
            1.0,
            0,
            retention,
            0.5, // a slippery-floor multiplier this port cannot express
            gr_vel_in,
            &mut entry_calls,
            &mut update_facing,
            &mut out_gr_vel,
            &mut out_facing,
        );
    }
    assert_ne!(
        out_gr_vel.to_bits(),
        entry_ground_velocity(gr_vel_in, retention).to_bits(),
        "the multiplier gap must remain observable, not silently patched over"
    );
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let fox = include_str!("oracle/original/fox_specials.c");
    let special_s = include_str!("oracle/original/special_s.c");
    let special_air = include_str!("oracle/original/special_air.c");
    for header in [
        "void ftFx_SpecialSStart_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialAirSStart_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialS_Phys(HSD_GObj* gobj)",
        "void ftFx_SpecialAirS_Phys(HSD_GObj* gobj)",
        "void ftFx_SpecialSEnd_Coll(HSD_GObj* gobj)",
        "void ftFx_SpecialAirSEnd_Coll(HSD_GObj* gobj)",
    ] {
        assert!(fox.contains(header), "{header}");
    }
    assert!(special_s.contains("bool ftCo_SpecialS_CheckInput(Fighter_GObj* gobj)"));
    assert!(special_air.contains("bool ftCo_SpecialAir_CheckInput(Fighter_GObj* gobj)"));
    let adapter = include_str!("oracle/fox_specials.c");
    assert!(adapter.contains("#include \"fox_specials_original.inc\""));
}
