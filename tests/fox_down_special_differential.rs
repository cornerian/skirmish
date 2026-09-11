//! Fox/Falco down special (Reflector) checked against the pinned C
//! (`ftfoxspeciallw.c`): the Start/Loop/Turn/Hit/End state machine's
//! release-lag countdown, Turn's flip-once step, the Loop IASA short-
//! circuit order (turn-check, then jump-cancel), the `ftFx_SpecialLwHit_
//! Check` End-vs-Loop decision, the platform drop's own reflect-hit side
//! effect, every phase's ground/air conversion, and the shared air Phys
//! arithmetic. Each comparison pins the extracted callback's exact
//! behaviour with an inline Rust formula (the same style
//! `fox_side_special_differential.rs` already established), not by
//! constructing a full `Match` -- `tests/game_fox_down_special.rs` and the
//! self-recorded replay regressions in `crates/cli/tests/replay_match.rs`
//! separately exercise the Rust engine's own mirror end to end.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_down_enter(
        ground: bool,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        x98: f32,
        x9c: f32,
        xa4: i32,
        xa8: f32,
        out_msid: *mut i32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_release_lag: *mut i32,
        out_is_release: *mut i32,
        out_gravity_delay: *mut i32,
    );
    fn oracle_down_phys(
        phase: i32,
        ground: bool,
        gravity_delay_in: i32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        fall_accel: f32,
        terminal_velocity: f32,
        ground_friction: f32,
        walk_max_vel: f32,
        aerial_friction: f32,
        air_drift_max: f32,
        over_drift_step: f32,
        above_walk_friction_mul: f32,
        out_gravity_delay: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_air_drift_recovery(
        self_vel_x: f32,
        aerial_friction: f32,
        air_drift_max: f32,
        x1fc: f32,
        out_anim_vel_x: *mut f32,
    ) -> bool;
    fn oracle_down_anim(
        phase: i32,
        ground: bool,
        held_b: bool,
        release_lag_in: i32,
        is_release_in: i32,
        turn_frames_in: i32,
        cmd0_in: i32,
        x9c: f32,
        frames_remaining: bool,
        out_msid: *mut i32,
        out_release_lag: *mut i32,
        out_is_release: *mut i32,
        out_turn_frames: *mut i32,
        out_cmd0: *mut i32,
        out_reflecting: *mut bool,
        out_reflect_hit_calls: *mut i32,
    );
    fn oracle_down_end_anim(
        ground: bool,
        frames_remaining: bool,
        out_wait_calls: *mut i32,
        out_fall_calls: *mut i32,
    );
    fn oracle_down_loop_iasa(
        ground: bool,
        turn_result: bool,
        jump_result: bool,
        out_msid: *mut i32,
        out_turn_calls: *mut i32,
        out_jump_calls: *mut i32,
        out_pass_calls: *mut i32,
        out_reflecting: *mut bool,
    ) -> i32;
    fn oracle_down_hit_check(
        ground: bool,
        release_lag_in: i32,
        is_release_in: i32,
        out_msid: *mut i32,
        out_reflecting: *mut bool,
        out_reflect_hit_calls: *mut i32,
    ) -> i32;
    fn oracle_down_turn_check(
        ground: bool,
        x9c: f32,
        facing_in: f32,
        out_msid: *mut i32,
        out_reflecting: *mut bool,
        out_facing: *mut f32,
        out_turn_frames: *mut i32,
        out_cmd0: *mut i32,
    );
    fn oracle_down_pass(
        phase: i32,
        cur_anim_frame: f32,
        pass_velocity_y: f32,
        out_msid: *mut i32,
        out_self_vel_y: *mut f32,
        out_reflecting: *mut bool,
        out_reflect_hit_calls: *mut i32,
    );
    fn oracle_down_conversion(
        phase: i32,
        ground_side: bool,
        coll_result: bool,
        out_msid: *mut i32,
        out_reflecting: *mut bool,
        out_reflect_hit_calls: *mut i32,
    ) -> i32;
}

// Matches `ftfoxspeciallw.c`'s own arbitrary-but-distinct motion id enum.
// Not every one of the ten is asserted against by name below (Hit is
// reached only through `oracle_down_hit_check`'s own boolean result, not a
// captured msid); kept complete for readability against the C enum.
const MS_START_GROUND: i32 = 2000;
const MS_LOOP_GROUND: i32 = 2001;
#[allow(dead_code)]
const MS_HIT_GROUND: i32 = 2002;
const MS_END_GROUND: i32 = 2003;
const MS_TURN_GROUND: i32 = 2004;
const MS_START_AIR: i32 = 2005;
const MS_LOOP_AIR: i32 = 2006;
#[allow(dead_code)]
const MS_HIT_AIR: i32 = 2007;
const MS_END_AIR: i32 = 2008;
const MS_TURN_AIR: i32 = 2009;

fn same_bits(label: &str, actual: f32, expected: f32) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{label}: {actual} (0x{:08x}) != {expected} (0x{:08x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

fn compare_enter(ground: bool, svx_in: f32, svy_in: f32, x98: f32, x9c: f32, xa4: i32, xa8: f32) {
    let (mut msid, mut svx, mut svy, mut lag, mut release, mut delay) = (0, 0.0, 0.0, 0, 0, 0);
    unsafe {
        oracle_down_enter(
            ground,
            svx_in,
            svy_in,
            x98,
            x9c,
            xa4,
            xa8,
            &mut msid,
            &mut svx,
            &mut svy,
            &mut lag,
            &mut release,
            &mut delay,
        );
    }
    assert_eq!(delay, xa4);
    assert_eq!(release, 0);
    if ground {
        assert_eq!(msid, MS_START_GROUND);
        same_bits("self_vel_x", svx, svx_in);
        same_bits("self_vel_y", svy, svy_in);
    } else {
        assert_eq!(msid, MS_START_AIR);
        same_bits("self_vel_x", svx, svx_in / xa8);
        same_bits("self_vel_y", svy, 0.0);
    }
    // release_lag is only meaningfully compared when finite/representable;
    // the source stores it as an `s32`, so a fractional `x98` truncates.
    assert_eq!(lag, x98 as i32);
}

#[allow(clippy::too_many_arguments)]
fn compare_phys(
    phase: i32,
    ground: bool,
    delay_in: i32,
    gr_vel_in: f32,
    svx_in: f32,
    svy_in: f32,
    fall_accel: f32,
    terminal_velocity: f32,
    ground_friction: f32,
    walk_max: f32,
    aerial_friction: f32,
    air_drift_max: f32,
    over_drift_step: f32,
    above_walk_mul: f32,
) {
    let (mut delay, mut gr_vel, mut svx, mut svy) = (0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_down_phys(
            phase,
            ground,
            delay_in,
            gr_vel_in,
            svx_in,
            svy_in,
            fall_accel,
            terminal_velocity,
            ground_friction,
            walk_max,
            aerial_friction,
            air_drift_max,
            over_drift_step,
            above_walk_mul,
            &mut delay,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    if ground {
        // None of the five ground Phys callbacks touch gravityDelay --
        // confirmed by reading every one of them; unlike the side special,
        // this move's grounded phases never tick it.
        assert_eq!(delay, delay_in);
        let friction = if gr_vel_in.abs() > walk_max {
            ground_friction * above_walk_mul
        } else {
            ground_friction
        };
        let mut f = friction;
        if f.abs() > gr_vel_in.abs() {
            f = -gr_vel_in;
        } else if gr_vel_in > 0.0 {
            f = -f;
        }
        same_bits("gr_vel", gr_vel, gr_vel_in + f);
    } else {
        // ftFox_SpecialLw_InlinePhys/every other air Phys: the gravity-delay
        // countdown and fall are conditional, but the `ftCommon_8007CF58`
        // call after them is unconditional either way, and itself branches
        // on whether self_vel.x is already past air_drift_max.
        if delay_in != 0 {
            assert_eq!(delay, delay_in - 1);
            same_bits("self_vel_y", svy, svy_in);
        } else {
            assert_eq!(delay, 0);
            let mut fallen = svy_in - fall_accel;
            if fallen < -terminal_velocity {
                fallen = -terminal_velocity;
            }
            same_bits("self_vel_y", svy, fallen);
        }
        same_bits(
            "self_vel_x",
            svx,
            expected_drift_or_friction(svx_in, aerial_friction, air_drift_max, over_drift_step),
        );
    }
}

/// `ftCommon_8007CF58`'s own arithmetic, inline: decelerate toward
/// `air_drift_max` using the common over-drift step once already past it,
/// else apply ordinary aerial friction toward zero.
fn expected_drift_or_friction(
    vel: f32,
    aerial_friction: f32,
    air_drift_max: f32,
    over_drift_step: f32,
) -> f32 {
    if vel.abs() > air_drift_max {
        let mut accel = over_drift_step;
        if accel.abs() >= vel.abs() {
            accel = -vel;
        } else if vel > 0.0 {
            accel = -over_drift_step;
        }
        vel + accel
    } else {
        let mut f = aerial_friction;
        if f.abs() >= vel.abs() {
            f = -vel;
        } else if vel > 0.0 {
            f = -f;
        }
        vel + f
    }
}

fn compare_air_drift_recovery(
    self_vel_x: f32,
    aerial_friction: f32,
    air_drift_max: f32,
    x1fc: f32,
) {
    let mut anim_vel_x = 0.0;
    let over = unsafe {
        oracle_air_drift_recovery(
            self_vel_x,
            aerial_friction,
            air_drift_max,
            x1fc,
            &mut anim_vel_x,
        )
    };
    assert_eq!(over, self_vel_x.abs() > air_drift_max);
    let expected = if over {
        let mut accel = x1fc;
        if accel.abs() >= self_vel_x.abs() {
            accel = -self_vel_x;
        } else if self_vel_x > 0.0 {
            accel = -x1fc;
        }
        accel
    } else {
        let mut f = aerial_friction;
        if f.abs() >= self_vel_x.abs() {
            f = -self_vel_x;
        } else if self_vel_x > 0.0 {
            f = -f;
        }
        f
    };
    same_bits("anim_vel_x", anim_vel_x, expected);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_enter(
        ground in any::<bool>(),
        svx in any::<u32>(), svy in any::<u32>(),
        x98 in 0i32..=200, x9c in 1.0f32..30.0, xa4 in 0i32..=60,
        xa8 in prop_oneof![Just(1.0f32), Just(2.0f32), Just(3.0f32)],
    ) {
        compare_enter(ground, f32::from_bits(svx), f32::from_bits(svy), x98 as f32, x9c, xa4, xa8);
    }

    #[test]
    fn arbitrary_ground_phys(
        phase in 0i32..5,
        gr_vel in any::<u32>(),
        friction in 0.0f32..2.0, walk_max in 0.0f32..5.0, above_walk in 0.5f32..2.0,
    ) {
        compare_phys(phase, true, 0, f32::from_bits(gr_vel), 0.0, 0.0, 0.0, 0.0, friction, walk_max, 0.0, 0.0, 0.0, above_walk);
    }

    #[test]
    fn arbitrary_air_phys(
        phase in 0i32..5,
        delay in 0i32..=3,
        // A bounded, not-fully-arbitrary svx (rather than any::<u32>()'s
        // full bit pattern) so the over-drift-maximum branch is actually
        // exercised at proptest's normal case count, not just theoretically
        // reachable; `arbitrary_air_phys_full_range` below still covers the
        // complete f32 domain including svx.abs() > air_drift_max there.
        svx in -6.0f32..6.0, svy in any::<u32>(),
        fall_accel in 0.0f32..1.0, terminal in 1.0f32..5.0,
        aerial_friction in 0.0f32..0.2, air_drift_max in 0.1f32..3.0,
        over_drift_step in 0.0f32..0.3,
    ) {
        compare_phys(phase, false, delay, 0.0, svx, f32::from_bits(svy), fall_accel, terminal, 0.0, 0.0, aerial_friction, air_drift_max, over_drift_step, 1.0);
    }

    #[test]
    fn arbitrary_air_phys_full_range(
        phase in 0i32..5,
        delay in 0i32..=3,
        svx in any::<u32>(), svy in any::<u32>(),
        fall_accel in 0.0f32..1.0, terminal in 1.0f32..5.0,
        aerial_friction in 0.0f32..0.2, air_drift_max in 0.1f32..3.0,
        over_drift_step in 0.0f32..0.3,
    ) {
        compare_phys(phase, false, delay, 0.0, f32::from_bits(svx), f32::from_bits(svy), fall_accel, terminal, 0.0, 0.0, aerial_friction, air_drift_max, over_drift_step, 1.0);
    }

    #[test]
    fn arbitrary_air_drift_recovery(
        svx in -6.0f32..6.0, aerial_friction in 0.0f32..0.2,
        air_drift_max in 0.1f32..3.0, x1fc in 0.0f32..0.3,
    ) {
        compare_air_drift_recovery(svx, aerial_friction, air_drift_max, x1fc);
    }

    #[test]
    fn arbitrary_air_drift_recovery_full_range(
        svx in any::<u32>(), aerial_friction in 0.0f32..0.2,
        air_drift_max in 0.1f32..3.0, x1fc in 0.0f32..0.3,
    ) {
        compare_air_drift_recovery(f32::from_bits(svx), aerial_friction, air_drift_max, x1fc);
    }
}

#[test]
fn air_phys_crosses_the_drift_maximum_in_both_directions() {
    // Explicit boundary cases (not left to proptest's own random sampling):
    // svx well past a small air_drift_max, on both signs, and a case
    // exactly at the boundary (still the under-max friction branch, since
    // the source's own comparison is strict `>`).
    for (phase, ground) in (0..5).flat_map(|p| [(p, false)]) {
        compare_phys(
            phase, ground, 0, 0.0, 5.0, 0.0, 0.1, 3.0, 0.0, 0.0, 0.05, 1.0, 0.02, 1.0,
        );
        compare_phys(
            phase, ground, 0, 0.0, -5.0, 0.0, 0.1, 3.0, 0.0, 0.0, 0.05, 1.0, 0.02, 1.0,
        );
        compare_phys(
            phase, ground, 0, 0.0, 1.0, 0.0, 0.1, 3.0, 0.0, 0.0, 0.05, 1.0, 0.02, 1.0,
        );
    }
}

#[test]
fn start_anim_only_sets_release_from_input_no_transition() {
    for ground in [true, false] {
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                0,
                ground,
                false,
                5,
                0,
                0,
                0,
                999.0,
                true,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        // Start's own Anim never transitions or touches releaseLag itself;
        // only isRelease responds to a held B.
        assert_eq!(msid, 0);
        assert_eq!(lag, 5);
        assert_eq!(release, 1);
        assert!(!reflecting);
    }
}

#[test]
fn loop_anim_counts_release_lag_down_and_exits_to_end_once_released() {
    for ground in [true, false] {
        // Held B: isRelease stays false, releaseLag still counts down.
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                1,
                ground,
                true,
                3,
                0,
                0,
                0,
                999.0,
                false,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(lag, 2);
        assert_eq!(release, 0);
        assert_eq!(msid, 0);

        // Released with releaseLag already at 0: End enters this frame.
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                1,
                ground,
                false,
                0,
                1,
                0,
                0,
                999.0,
                false,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(lag, 0);
        assert_eq!(release, 1);
        assert_eq!(msid, if ground { MS_END_GROUND } else { MS_END_AIR });
        let _ = (turn, cmd0, reflecting, hits);
    }
}

#[test]
fn turn_step_flips_facing_on_the_first_step_only_and_exits_through_hit_check() {
    for ground in [true, false] {
        // First step: turnFrames counts down from x9c and the flip fires
        // (cmd0 0 -> 1).
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                2,
                ground,
                false,
                5,
                0,
                3,
                0,
                999.0,
                false,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(turn, 2);
        assert_eq!(cmd0, 1);
        assert_eq!(msid, 0);

        // A later step (cmd0 already 1) does not flip again, and turnFrames
        // reaching 0 dispatches through Hit_Check (End here: released with
        // no remaining lag).
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                2,
                ground,
                false,
                0,
                1,
                1,
                1,
                999.0,
                false,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(turn, 0);
        assert_eq!(cmd0, 1);
        assert_eq!(msid, if ground { MS_END_GROUND } else { MS_END_AIR });
        let _ = (lag, release, reflecting, hits);
    }
}

#[test]
fn hit_anim_clip_end_dispatches_through_hit_check() {
    for ground in [true, false] {
        // Not yet released: clip end re-creates the reflect hit and returns
        // to Loop.
        let (mut msid, mut lag, mut release, mut turn, mut cmd0, mut reflecting, mut hits) =
            (0, 0, 0, 0, 0, false, 0);
        unsafe {
            oracle_down_anim(
                3,
                ground,
                true,
                4,
                0,
                0,
                0,
                999.0,
                false,
                &mut msid,
                &mut lag,
                &mut release,
                &mut turn,
                &mut cmd0,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
        assert!(reflecting);
        assert_eq!(hits, 1);
        let _ = (lag, release, turn, cmd0);
    }
}

#[test]
fn end_anim_dispatches_wait_or_fall_by_ground_or_air() {
    for ground in [true, false] {
        let (mut wait_calls, mut fall_calls) = (0, 0);
        unsafe {
            oracle_down_end_anim(ground, false, &mut wait_calls, &mut fall_calls);
        }
        if ground {
            assert_eq!((wait_calls, fall_calls), (1, 0));
        } else {
            assert_eq!((wait_calls, fall_calls), (0, 1));
        }
        let (mut wait_calls, mut fall_calls) = (0, 0);
        unsafe {
            oracle_down_end_anim(ground, true, &mut wait_calls, &mut fall_calls);
        }
        assert_eq!((wait_calls, fall_calls), (0, 0));
    }
}

#[test]
fn loop_iasa_turn_check_short_circuits_ground_jump_cancel_and_air_jump() {
    for ground in [true, false] {
        // Turn wins outright; the jump/aerial-jump predicate never runs.
        let (mut msid, mut turn_calls, mut jump_calls, mut pass_calls, mut reflecting) =
            (0, 0, 0, 0, false);
        let result = unsafe {
            oracle_down_loop_iasa(
                ground,
                true,
                true,
                &mut msid,
                &mut turn_calls,
                &mut jump_calls,
                &mut pass_calls,
                &mut reflecting,
            )
        };
        assert_eq!(result, 1);
        assert_eq!(turn_calls, 1);
        assert_eq!(jump_calls, 0);
        assert!(reflecting);
        assert_eq!(msid, if ground { MS_TURN_GROUND } else { MS_TURN_AIR });

        // Turn declines; the jump/aerial-jump predicate then runs exactly
        // once.
        let (mut msid, mut turn_calls, mut jump_calls, mut pass_calls, mut reflecting) =
            (0, 0, 0, 0, false);
        let result = unsafe {
            oracle_down_loop_iasa(
                ground,
                false,
                false,
                &mut msid,
                &mut turn_calls,
                &mut jump_calls,
                &mut pass_calls,
                &mut reflecting,
            )
        };
        assert_eq!(result, 0);
        assert_eq!(turn_calls, 1);
        assert_eq!(jump_calls, 1);
        assert_eq!(msid, 0);
        let _ = (pass_calls, reflecting);
    }
}

#[test]
fn hit_check_end_requires_both_released_and_lag_elapsed() {
    for ground in [true, false] {
        for (lag, release, expect_end) in
            [(0, 1, true), (0, 0, false), (1, 1, false), (1, 0, false)]
        {
            let (mut msid, mut reflecting, mut hits) = (0, false, 0);
            let result = unsafe {
                oracle_down_hit_check(ground, lag, release, &mut msid, &mut reflecting, &mut hits)
            };
            if expect_end {
                assert_eq!(result, 0);
                assert_eq!(msid, if ground { MS_END_GROUND } else { MS_END_AIR });
                assert!(!reflecting);
                assert_eq!(hits, 0);
            } else {
                assert_eq!(result, 1);
                assert_eq!(msid, if ground { MS_LOOP_GROUND } else { MS_LOOP_AIR });
                assert!(reflecting);
                assert_eq!(hits, 1);
            }
        }
    }
}

#[test]
fn turn_check_entry_flips_facing_immediately_and_sets_the_countdown() {
    for ground in [true, false] {
        let (mut msid, mut reflecting, mut facing, mut turn, mut cmd0) = (0, false, 0.0, 0, 0);
        unsafe {
            oracle_down_turn_check(
                ground,
                6.0,
                1.0,
                &mut msid,
                &mut reflecting,
                &mut facing,
                &mut turn,
                &mut cmd0,
            );
        }
        assert_eq!(msid, if ground { MS_TURN_GROUND } else { MS_TURN_AIR });
        assert!(reflecting);
        assert_eq!(facing, -1.0);
        assert_eq!(turn, 5);
        assert_eq!(cmd0, 1);
    }
}

#[test]
fn platform_drop_keeps_the_phase_and_creates_a_fresh_reflect_hit() {
    for (phase, expected_msid) in [(0, MS_START_AIR), (1, MS_LOOP_AIR)] {
        let (mut msid, mut svy, mut reflecting, mut hits) = (0, 0.0, false, 0);
        unsafe {
            oracle_down_pass(
                phase,
                12.0,
                -1.5,
                &mut msid,
                &mut svy,
                &mut reflecting,
                &mut hits,
            );
        }
        assert_eq!(msid, expected_msid);
        same_bits("self_vel_y", svy, -1.5);
        assert!(reflecting);
        assert_eq!(hits, 1);
    }
}

#[test]
fn conversions_preserve_reflecting_by_phase() {
    // phase: 0 Start, 1 Loop, 2 Turn, 3 Hit, 4 End. Start/End never touch
    // `reflecting`; Loop/Turn/Hit all set it, but only Loop's own
    // conversion (`ftFx_SpecialLw_CreateReflectHit`) calls the real
    // `ftColl_CreateReflectHit` -- Turn's (`ftFox_SpecialLw_SetReflectVars`)
    // and Hit's (`ftFx_SpecialLwHit_SetCall`) just set the flag/callback
    // directly, matching the pinned source exactly.
    let refl_by_phase = [false, true, true, true, false];
    let hit_calls_by_phase = [false, true, false, false, false];
    for (phase, expect_reflecting) in refl_by_phase.into_iter().enumerate() {
        let phase = phase as i32;
        let expect_hit_calls = hit_calls_by_phase[phase as usize];
        for ground_side in [true, false] {
            // The ground Coll callbacks convert when `ft_80082708` returns
            // false; the air ones convert when `ft_80081D0C` returns true
            // (`oracle_down_conversion` shares one `coll_result` script
            // value between both scripted predicates).
            let converts = !ground_side;
            let (mut msid, mut reflecting, mut hits) = (0, false, 0);
            let result = unsafe {
                oracle_down_conversion(
                    phase,
                    ground_side,
                    converts,
                    &mut msid,
                    &mut reflecting,
                    &mut hits,
                )
            };
            assert_eq!(result, 1, "phase {phase} ground_side {ground_side}");
            assert_ne!(msid, 0);
            assert_eq!(reflecting, expect_reflecting, "phase {phase}");
            assert_eq!(hits > 0, expect_hit_calls, "phase {phase}");

            // The opposite predicate value means no conversion this frame.
            let (mut msid, mut reflecting, mut hits) = (0, false, 0);
            let result = unsafe {
                oracle_down_conversion(
                    phase,
                    ground_side,
                    !converts,
                    &mut msid,
                    &mut reflecting,
                    &mut hits,
                )
            };
            assert_eq!(result, 0);
            assert_eq!(msid, 0);
        }
    }
}
