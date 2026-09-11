//! `ftCo_Run_Anim`'s animation-rate selection and `run.x0` countdown, plus
//! `ftCo_Run_Enter`'s literal field assignments (`tests/oracle/run.c`,
//! `tests/oracle/original/run.c`) checked against
//! `fighter::locomotion::run_animation_rate`. `run_animation_rate` is a
//! pure chained float comparison/arithmetic and compared over the full
//! binary32 domain, including NaN, the same NaN-safety the sibling
//! `walkcommon_differential.rs` claims for `walk_animation_rate`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::locomotion::run_animation_rate;

#[repr(C)]
struct RunAnimResult {
    rate: f32,
    run_x0_after: f32,
}

#[repr(C)]
struct RunEnterResult {
    msid: i32,
    start: f32,
    speed: f32,
    x0: f32,
    x4: f32,
}

#[repr(C)]
struct RunIasaResult {
    special_called: i32,
    attack100_called: i32,
    x6824_called: i32,
    x68c0_called: i32,
    x8a38_called: i32,
    attackdash_called: i32,
    attackdash_setmv0_called: i32,
    x91a4c_called: i32,
    x91b90_called: i32,
    x91b9c_called: i32,
    de9d8_called: i32,
    jump_called: i32,
    turn_called: i32,
    brake_called: i32,
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_run_anim(
        gr_vel: f32,
        run_x4: f32,
        facing: f32,
        scaling: f32,
        friction_mul: f32,
        run_x0: f32,
    ) -> RunAnimResult;
    fn oracle_run_enter(x0: f32, gr_vel: f32) -> RunEnterResult;
    #[allow(clippy::too_many_arguments)]
    fn oracle_run_iasa(
        run_x0: f32,
        special: i32,
        attack100: i32,
        x6824: i32,
        x68c0: i32,
        x8a38: i32,
        attackdash: i32,
        x91a4c: i32,
        de9d8: i32,
        jump: i32,
        turn: i32,
        brake: i32,
    ) -> RunIasaResult;
}

fn exact(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{actual:?} != NaN");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn compare_anim(
    gr_vel: f32,
    run_x4: f32,
    facing: f32,
    scaling: f32,
    friction_mul: f32,
    run_x0: f32,
) {
    let expected_rate = run_animation_rate(gr_vel, facing, run_x4, scaling, friction_mul);
    // ftCo_Run.c:96-98: decremented by 1.0 only while strictly positive,
    // never otherwise clamped -- not modeled by any Rust helper (this
    // codebase does not track run.x0; see docs/run.md), so this is checked
    // directly against the literal source line rather than a Rust mirror.
    let expected_x0_after = if run_x0 > 0.0 { run_x0 - 1.0 } else { run_x0 };
    // SAFETY: six scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe { oracle_run_anim(gr_vel, run_x4, facing, scaling, friction_mul, run_x0) };
    exact(actual.rate, expected_rate);
    exact(actual.run_x0_after, expected_x0_after);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_run_anim(
        gr_vel in any::<u32>().prop_map(f32::from_bits),
        run_x4 in any::<u32>().prop_map(f32::from_bits),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        scaling in any::<u32>().prop_map(f32::from_bits),
        friction_mul in any::<u32>().prop_map(f32::from_bits),
        run_x0 in prop_oneof![
            Just(0.0f32),
            (0.0f32..1000.0f32),
            any::<u32>().prop_map(f32::from_bits),
        ],
    ) {
        compare_anim(gr_vel, run_x4, facing, scaling, friction_mul, run_x0);
    }
}

#[test]
fn exact_boundaries() {
    // velocity exactly 0: v * facing == 0.0 <= 0.0, rate 0 regardless of facing.
    compare_anim(0.0, 0.0, 1.0, 4.0, 1.0, 0.0);
    compare_anim(0.0, 0.0, -1.0, 4.0, 1.0, 0.0);
    // negative velocity, facing also negative: the product is positive, so
    // this is a nonzero rate from |velocity| despite the negative sign.
    compare_anim(-4.0, 0.0, -1.0, 4.0, 1.0, 0.0);
    // negative velocity opposing facing: zero rate.
    compare_anim(-4.0, 0.0, 1.0, 4.0, 1.0, 0.0);
    // scaling tiny: a large but finite rate.
    compare_anim(1.0, 0.0, 1.0, f32::MIN_POSITIVE, 1.0, 0.0);
    compare_anim(4.0, 0.0, 1.0, 1e-30, 1.0, 0.0);
    // friction multiplier below 1.0 selects run_x4, including its sign.
    compare_anim(4.0, -4.0, 1.0, 4.0, 0.5, 0.0);
    // run.x0 countdown: positive decrements by exactly one, zero/negative untouched.
    compare_anim(4.0, 0.0, 1.0, 4.0, 1.0, 3.0);
    compare_anim(4.0, 0.0, 1.0, 4.0, 1.0, 1.0);
    compare_anim(4.0, 0.0, 1.0, 4.0, 1.0, 0.0);
    compare_anim(4.0, 0.0, 1.0, 4.0, 1.0, -2.0);
    // NaN inputs.
    compare_anim(f32::NAN, 0.0, 1.0, 4.0, 1.0, 0.0);
    compare_anim(4.0, 0.0, 1.0, f32::NAN, 1.0, 0.0);
    compare_anim(4.0, 0.0, f32::NAN, 4.0, 1.0, 0.0);
}

/// `ftCo_Run_Enter`'s literal field assignments (`ftCo_Run.c:61-74`):
/// always `anim_start = 0.0F`, `anim_speed = 1.0F` (the two-argument
/// wrapper's own fixed call into `Enter_Full`), `run.x0 = arg0`,
/// `run.x4 = gr_vel`. Not a comparison against a Rust pure function (entry
/// is wiring, not arithmetic) -- this confirms the source's own literal
/// pass-through that `game::locomotion::enter_run` assumes (frame 0,
/// last_rate 1.0), complementing `tests/game_run.rs`'s own
/// dash-to-run/run-turn-to-run coverage.
#[test]
fn run_enter_always_reports_start_zero_speed_one_and_the_passed_x0_x4() {
    for (x0, gr_vel) in [
        (0.0f32, 0.0f32),
        (3.0, 2.5),
        (-1.0, -4.0),
        (f32::NAN, f32::INFINITY),
    ] {
        // SAFETY: two scalar inputs; the adapter owns all thread-local state.
        let result = unsafe { oracle_run_enter(x0, gr_vel) };
        assert_eq!(result.start, 0.0);
        assert_eq!(result.speed, 1.0);
        exact(result.x0, x0);
        exact(result.x4, gr_vel);
        // The captured motion id is some fixed constant on every call.
        assert_eq!(result.msid, unsafe { oracle_run_enter(0.0, 0.0) }.msid);
    }
}

#[test]
fn adapter_retains_the_complete_pinned_functions() {
    let source = include_str!("oracle/original/run.c");
    assert!(source.contains("void ftCo_Run_Enter(Fighter_GObj* gobj, float arg0)"));
    assert!(
        source
            .contains("void ftCo_Run_Enter_Full(Fighter_GObj* gobj, float arg0, float anim_start,")
    );
    assert!(source.contains("void ftCo_Run_Anim(Fighter_GObj* gobj)"));
    assert!(source.contains("void ftCo_Run_IASA(Fighter_GObj* gobj)"));
    assert!(include_str!("oracle/run.c").contains("#include \"run_original.inc\""));
}

/// `ftCo_Run_IASA`'s gate order (`ftCo_Run.c:101-128`), focused on the
/// `run.x0` lockout this batch adds (`Parameters::run_turn_lockout_frames`,
/// `State::run_lockout`): `RETURN_IF((run.x0 <= 0.0F) && fn_800C9D40(gobj))`
/// only reaches the turn-run check while `run.x0 <= 0.0F`;
/// `RETURN_IF(!(run.x0 <= 0.0F))` then returns immediately whenever
/// `run.x0 > 0.0F`, so `ftCo_RunBrake_CheckInput` is unreached too --
/// exactly the "both RunTurn and RunBrake are locked out" behavior
/// `game::locomotion::update_actions`'s `Action::Run` arm and
/// `game::dash::update_dash_or_run`'s Run arm both now gate on
/// `run_lockout <= 0.0`.
fn compare_iasa(run_x0: f32, jump: bool, turn: bool, brake: bool) {
    // Every earlier scripted check answers false so the chain reaches the
    // lockout gate; jump is always reached first regardless of run.x0.
    let jump_called = true;
    let turn_called = !jump && run_x0 <= 0.0;
    let brake_called = !jump && run_x0 <= 0.0 && !turn;
    // SAFETY: twelve scalar inputs; the adapter owns all thread-local state.
    let actual = unsafe {
        oracle_run_iasa(
            run_x0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            i32::from(jump),
            i32::from(turn),
            i32::from(brake),
        )
    };
    assert_eq!(actual.jump_called != 0, jump_called);
    assert_eq!(actual.turn_called != 0, turn_called);
    assert_eq!(actual.brake_called != 0, brake_called);
}

#[test]
fn run_iasa_lockout_gate_order_matches_c() {
    for run_x0 in [-1.0f32, 0.0, 1.0, 5.0, f32::NAN] {
        for jump in [false, true] {
            for turn in [false, true] {
                for brake in [false, true] {
                    compare_iasa(run_x0, jump, turn, brake);
                }
            }
        }
    }
}
