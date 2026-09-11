//! Fox/Falco up special (Fire Fox/Fire Bird) checked against the complete
//! pinned C (`ftfoxspecialhi.c`), plus the two external helpers it calls
//! that live in other already-pinned files (`lbVector_AngleXY` via the
//! `lbvector` snapshot, `ftCo_8009A134` via the `pass` snapshot). Compares
//! the charge/hold arithmetic, the stick-driven launch angle and its two
//! thresholds, the grounded-vs-aerial launch decision (including the
//! platform check), Travel's duration-independent-of-animation countdown
//! and its post-`duration_end` reverse acceleration, the grounded/aerial
//! Coll decisions (including the Bound rebound gate and the redirect this
//! port leaves unmodeled), Landing/Fall (including the frame-13 regression
//! fixed in this batch, see `docs/fox-up-special.md`), and Bound
//! bit-exactly.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
#![allow(clippy::too_many_arguments)]

use proptest::prelude::*;
use skirmish::fighter::Movement;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_hold_enter(
        ground: bool,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        max_jumps: i32,
        x54: f32,
        x58: f32,
        out_msid: *mut i32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_gravity_delay: *mut f32,
    );
    fn oracle_hold_phys(
        ground: bool,
        gravity_delay_in: f32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        ground_friction: f32,
        walk_max_vel: f32,
        x5c: f32,
        x60: f32,
        terminal_velocity: f32,
        out_gravity_delay: *mut f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_hold_anim(
        ground: bool,
        frames_remaining: bool,
        air_actual: bool,
        stick_x: f32,
        stick_y: f32,
        x64: f32,
        x68: f32,
        x74: f32,
        x88: f32,
        floor_nx: f32,
        floor_ny: f32,
        on_platform: bool,
        facing_in: f32,
        max_jumps: i32,
        out_msid: *mut i32,
        out_travel_frames: *mut f32,
        out_rotate_model: *mut f32,
        out_facing: *mut f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
        out_jumps_used: *mut i32,
    );
    fn oracle_hold_coll(
        ground: bool,
        coll_result: bool,
        ledge_check: bool,
        ledge_common: bool,
        out_msid: *mut i32,
    ) -> i32;
    fn oracle_travel_anim(
        ground: bool,
        travel_frames_in: f32,
        grounded_actual: bool,
        out_travel_frames: *mut f32,
        out_msid: *mut i32,
        out_start: *mut f32,
    ) -> i32;
    fn oracle_travel_phys(
        ground: bool,
        unk_in: f32,
        x70: f32,
        x78: f32,
        gr_vel_in: f32,
        self_vel_x_in: f32,
        self_vel_y_in: f32,
        facing: f32,
        rotate_model: f32,
        out_unk: *mut f32,
        out_gr_vel: *mut f32,
        out_self_vel_x: *mut f32,
        out_self_vel_y: *mut f32,
    );
    fn oracle_travel_coll_ground(
        unk2_in: f32,
        coll_result: bool,
        floor_contact: bool,
        floor_nx: f32,
        floor_ny: f32,
        facing: f32,
        out_unk2: *mut f32,
        out_rotate_model: *mut f32,
        out_msid: *mut i32,
    ) -> i32;
    fn oracle_travel_coll_air(
        unk2_in: f32,
        x6c: i32,
        x94: f32,
        on_platform: bool,
        check_ground_ledge: bool,
        ledge_common: bool,
        env_flags: u32,
        floor_nx: f32,
        floor_ny: f32,
        ceil_nx: f32,
        ceil_ny: f32,
        lwall_nx: f32,
        lwall_ny: f32,
        rwall_nx: f32,
        rwall_ny: f32,
        self_vel_x: f32,
        self_vel_y: f32,
        out_facing: *mut f32,
        out_rotate_model: *mut f32,
    ) -> i32;
    fn oracle_landing_fall_anim(
        landing: bool,
        frames_remaining: bool,
        x8c: f32,
        x90: f32,
        out_wait_calls: *mut i32,
        out_fallspecial_calls: *mut i32,
        out_mobility: *mut f32,
        out_landing_lag: *mut f32,
    );
    fn oracle_landing_fall_phys(
        landing: bool,
        gr_vel_in: f32,
        x7c: f32,
        ground_friction: f32,
        out_gr_vel: *mut f32,
        out_fall_phys_calls: *mut i32,
    );
    fn oracle_landing_coll(
        coll_result: bool,
        x8c: f32,
        x90: f32,
        out_fallspecial_calls: *mut i32,
        out_mobility: *mut f32,
        out_landing_lag: *mut f32,
    );
    fn oracle_fall_coll(
        check_ground_ledge: bool,
        ledge_common: bool,
        out_msid: *mut i32,
        out_start: *mut f32,
    ) -> i32;
    fn oracle_bound_enter(
        self_vel_x_in: f32,
        x84: f32,
        floor_contact: bool,
        floor_nx: f32,
        floor_ny: f32,
        out_msid: *mut i32,
        out_self_vel_x: *mut f32,
        out_cmd_var0: *mut i32,
    );
    fn oracle_bound_anim(
        cmd_var0: i32,
        ground_actual: bool,
        frames_remaining: bool,
        x8c: f32,
        x90: f32,
        max_jumps: i32,
        out_wait_calls: *mut i32,
        out_fallspecial_calls: *mut i32,
        out_jumps_used: *mut i32,
    ) -> i32;
    fn oracle_bound_phys(
        ground_actual: bool,
        trans_n_y: f32,
        gr_vel_in: f32,
        ground_friction: f32,
        walk_max_vel: f32,
        out_self_vel_y: *mut f32,
        out_gr_vel: *mut f32,
        out_drift_clamp_calls: *mut i32,
    );
    fn oracle_bound_coll(
        ground_actual: bool,
        check_ground_ledge: bool,
        ledge_common: bool,
        ft_800827a0_result: bool,
        out_gr_vel: *mut f32,
    ) -> i32;
}

const MS_HOLD: i32 = 2000;
const MS_HOLD_AIR: i32 = 2001;
const MS_TRAVEL_GROUND: i32 = 2002;
const MS_TRAVEL_AIR: i32 = 2003;
const MS_LANDING: i32 = 2004;
const MS_FALL: i32 = 2005;
const MS_BOUND: i32 = 2006;

#[allow(clippy::excessive_precision)]
const HALF_PI: f32 = 1.5707963705062866;

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

/// Total-order distance between two finite `f32` bit patterns (the
/// standard "monotonic bit pattern" ULP trick), used only by
/// [`close_bits`].
fn ulp_distance(a: f32, b: f32) -> u32 {
    fn key(x: f32) -> i32 {
        let bits = x.to_bits() as i32;
        if bits < 0 {
            i32::MIN.wrapping_sub(bits)
        } else {
            bits
        }
    }
    key(a).wrapping_sub(key(b)).unsigned_abs()
}

/// This crate uses the `libm` crate's own `atan2f`/`cosf`/`sinf`/`acosf`
/// pervasively (`src/fighter/{aerial,escape_air,damage}.rs`, this move),
/// deliberately: it gives every platform the exact same replay-affecting
/// arithmetic regardless of the local system's C library, which matters
/// far more for this project than matching any one build's `libm`.
/// Measured directly against this host's C compiler (see the batch's own
/// validation notes): `libm`'s `atan2f`/`cosf`/`sinf`/`acosf` each disagree
/// from the system `libm` by a handful of ULPs on their own, growing a
/// little further once chained through a further multiply/subtract (as
/// every comparison below actually uses them); 32 ULPs is generous
/// headroom for that chain while staying many orders of magnitude tighter
/// than any translation bug this suite has actually caught (a wrong
/// operand grouping or formula shows up as a completely different value,
/// not a few dozen ULPs), which is an inherent cross-implementation rounding
/// gap for transcendental functions, not a translation bug -- the same reason
/// `tests/game_ground_launch.rs`'s own trig-derived expectation already
/// uses a tolerance rather than bit-exact equality. Every value derived
/// through a `cosf`/`sinf`/`atan2f`/`acosf` call (directly or via
/// `mirror_angle_xy`) is compared with this instead of [`same_bits`];
/// everything else (branch codes, plain arithmetic, captured calls) stays
/// bit-exact.
fn close_bits(label: &str, actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "{label}: {actual:?} != {expected:?}");
        return;
    }
    assert!(
        actual.is_finite() == expected.is_finite(),
        "{label}: {actual:?} != {expected:?}"
    );
    if !expected.is_finite() {
        same_bits(label, actual, expected);
        return;
    }
    // Both this test's inputs and the trig chains they feed are otherwise
    // unrestricted binary32 patterns (denormals and magnitudes far outside
    // any value a real gameplay attribute or stick reading could take);
    // deep in subnormal range a fixed ULP count is no longer a meaningful
    // yardstick (subnormal encoding spaces representable values far closer
    // together in absolute terms), so once both sides already round to
    // physically-zero for gameplay purposes, they are equivalent outright.
    if actual.abs() < 1e-30 && expected.abs() < 1e-30 {
        return;
    }
    // A `cosf`/`sinf` result near its own zero crossing is the sharpest
    // case: both functions have unit-magnitude derivative there, so a
    // handful of ULPs of disagreement in the *angle* (or in `cosf`/`sinf`
    // themselves) becomes a merely-small ABSOLUTE difference in the
    // result that is nonetheless an enormous RELATIVE/ULP one, since the
    // result itself is near zero -- observed up to a few times 1e-5 for
    // this suite's own (already attribute-range-bounded) inputs. No
    // uniform ULP bound can absorb an effect whose relative size is
    // unbounded as the crossing is approached exactly, so this is an
    // absolute floor instead, comfortably above the observed noise and
    // comfortably below anything an actual formula/grouping bug would
    // produce (those move the result by a similar order of magnitude to
    // its own operands, not by a fraction of a thousandth).
    if (actual - expected).abs() <= 0.001 {
        return;
    }
    let distance = ulp_distance(actual, expected);
    assert!(
        distance <= 32,
        "{label}: {actual:?} ({:#010x}) != {expected:?} ({:#010x}), {distance} ULPs apart",
        actual.to_bits(),
        expected.to_bits(),
    );
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

/// `lbVector_AngleXY`, mirroring `up.rs::angle_xy` (private to the crate).
fn mirror_angle_xy(a: [f32; 2], b: [f32; 2]) -> f32 {
    let len_a = (a[0] * a[0] + a[1] * a[1]).sqrt();
    let len_b = (b[0] * b[0] + b[1] * b[1]).sqrt();
    let product = len_a * len_b;
    // `if (lena_lenb)`: non-zero, not positive -- see `up.rs::angle_xy`'s
    // identical fix and comment for why this matters (a NaN product from
    // an overflowing vector must propagate, not silently become 0.0).
    if product != 0.0 {
        let cosine = ((a[0] * b[0] + a[1] * b[1]) / product).clamp(-1.0, 1.0);
        libm::acosf(cosine)
    } else {
        0.0
    }
}

/// `ftCommon_UpdateFacing`, mirroring `up.rs::face_stick`.
fn mirror_face_stick(stick_x: f32) -> f32 {
    if stick_x >= 0.0 { 1.0 } else { -1.0 }
}

/// `ftFx_SpecialAirHi_Enter`, mirroring `up.rs::enter_aerial_launch`.
/// Returns (angle, facing_out, self_vel).
fn mirror_aerial_launch(
    stick: [f32; 2],
    x64: f32,
    x74: f32,
    x88: f32,
    facing_in: f32,
) -> (f32, f32, [f32; 2]) {
    let magnitude = stick[0].abs() + stick[1].abs();
    let (angle, facing) = if magnitude >= x64 {
        let facing = if stick[0].abs() > x88 {
            mirror_face_stick(stick[0])
        } else {
            facing_in
        };
        (libm::atan2f(stick[1], stick[0] * facing), facing)
    } else {
        (HALF_PI, facing_in)
    };
    let self_vel = [facing * (x74 * libm::cosf(angle)), x74 * libm::sinf(angle)];
    (angle, facing, self_vel)
}

/// `ftFx_SpecialAirHi_AirToGround`, mirroring `up.rs::enter_from_ground_hold`.
/// Returns (grounded_launch, angle, facing_out, gr_vel, self_vel).
fn mirror_ground_launch_decision(
    stick: [f32; 2],
    floor_normal: [f32; 2],
    on_platform: bool,
    x64: f32,
    x74: f32,
    x88: f32,
    facing_in: f32,
) -> (bool, f32, f32, f32, [f32; 2]) {
    let magnitude = stick[0].abs() + stick[1].abs();
    // Negated less-than, not `>=` -- see `up.rs::enter_from_ground_hold`'s
    // identical comment (NaN-sensitive).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let along_ground =
        !(magnitude < x64) && !(mirror_angle_xy(floor_normal, stick) < HALF_PI) && !on_platform;
    if along_ground {
        let facing = mirror_face_stick(stick[0]);
        let angle = libm::atan2f(-floor_normal[0] * facing, floor_normal[1]);
        (true, angle, facing, x74 * facing, [0.0, 0.0])
    } else {
        let (angle, facing, self_vel) = mirror_aerial_launch(stick, x64, x74, x88, facing_in);
        (false, angle, facing, 0.0, self_vel)
    }
}

// ---- Hold: Enter, Phys. ----

fn compare_hold_enter(ground: bool, gr_vel_in: f32, self_vel_x_in: f32, x54: f32, x58: f32) {
    let max_jumps = 5;
    let (mut msid, mut gr_vel, mut svx, mut svy, mut delay) = (0, 0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_hold_enter(
            ground,
            gr_vel_in,
            self_vel_x_in,
            max_jumps,
            x54,
            x58,
            &mut msid,
            &mut gr_vel,
            &mut svx,
            &mut svy,
            &mut delay,
        );
    }
    same_bits("gravity_delay", delay, x54);
    if ground {
        assert_eq!(msid, MS_HOLD);
        same_bits("gr_vel", gr_vel, gr_vel_in / x58);
    } else {
        assert_eq!(msid, MS_HOLD_AIR);
        same_bits("self_vel_x", svx, self_vel_x_in / x58);
        same_bits("self_vel_y", svy, 0.0);
    }
}

fn compare_hold_phys(
    ground: bool,
    gravity_delay_in: f32,
    gr_vel_in: f32,
    self_vel_x_in: f32,
    self_vel_y_in: f32,
    ground_friction: f32,
    x5c: f32,
    x60: f32,
    terminal_velocity: f32,
) {
    let (mut delay, mut gr_vel, mut svx, mut svy) = (0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_hold_phys(
            ground,
            gravity_delay_in,
            gr_vel_in,
            self_vel_x_in,
            self_vel_y_in,
            ground_friction,
            f32::INFINITY, // walk_max_vel: the above-walk-speed multiplier
            // is fixed at 1.0 in this adapter (see its header), so this
            // just keeps the untested branch permanently unreached.
            x5c,
            x60,
            terminal_velocity,
            &mut delay,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    if ground {
        // `ftFx_SpecialHiHold_Phys` never touches the gravity delay.
        same_bits("gravity_delay", delay, gravity_delay_in);
        same_bits(
            "gr_vel",
            gr_vel,
            mirror_friction_ground(gr_vel_in, ground_friction),
        );
    } else {
        let expected_delay = if gravity_delay_in != 0.0 {
            gravity_delay_in - 1.0
        } else {
            gravity_delay_in
        };
        same_bits("gravity_delay", delay, expected_delay);
        let expected_svy = if gravity_delay_in != 0.0 {
            self_vel_y_in
        } else {
            mirror_fall(self_vel_y_in, x60, terminal_velocity)
        };
        same_bits("self_vel_y", svy, expected_svy);
        same_bits("self_vel_x", svx, mirror_friction_air(self_vel_x_in, x5c));
    }
}

// ---- Hold Anim end: the launch decision graph. ----

fn compare_hold_anim(
    air_actual: bool,
    stick: [f32; 2],
    x64: f32,
    x68: f32,
    x74: f32,
    x88: f32,
    floor_normal: [f32; 2],
    on_platform: bool,
    facing_in: f32,
) {
    let max_jumps = 4;
    let (mut msid, mut travel_frames, mut rotate_model, mut facing) = (0, 0.0, 0.0, 0.0);
    let (mut gr_vel, mut svx, mut svy, mut jumps) = (0.0, 0.0, 0.0, 0);
    unsafe {
        oracle_hold_anim(
            !air_actual,
            false, // frames_remaining: always fire the transition
            air_actual,
            stick[0],
            stick[1],
            x64,
            x68,
            x74,
            x88,
            floor_normal[0],
            floor_normal[1],
            on_platform,
            facing_in,
            max_jumps,
            &mut msid,
            &mut travel_frames,
            &mut rotate_model,
            &mut facing,
            &mut gr_vel,
            &mut svx,
            &mut svy,
            &mut jumps,
        );
    }
    if air_actual {
        let (angle, expected_facing, self_vel) =
            mirror_aerial_launch(stick, x64, x74, x88, facing_in);
        assert_eq!(msid, MS_TRAVEL_AIR);
        same_bits("travel_frames", travel_frames, x68);
        close_bits("rotate_model", rotate_model, angle);
        same_bits("facing", facing, expected_facing);
        close_bits("self_vel_x", svx, self_vel[0]);
        close_bits("self_vel_y", svy, self_vel[1]);
        assert_eq!(jumps, max_jumps, "aerial launch restores every jump");
        return;
    }
    let (grounded, _, _, _, _) =
        mirror_ground_launch_decision(stick, floor_normal, on_platform, x64, x74, x88, facing_in);
    // The grounded-vs-declined gate compares an `acosf`-derived angle
    // against exactly `HALF_PI`: at the natural "stick perpendicular to
    // the floor" input (cosine exactly 0), a correctly-rounded `acosf`
    // must return `HALF_PI` itself, but `libm::acosf(0.0)` measurably
    // returns one ULP short of it (confirmed directly against both this
    // host's C compiler and Rust's own `f32::acos`), which can flip which
    // branch this test's own prediction expects right at that boundary --
    // not a translation bug, the same documented `close_bits` gap applied
    // to a branch condition instead of a returned value. When the
    // predicted angle is within a few ULPs of the gate (or the magnitude
    // is within a few ULPs of `x64`, the same class of gap), trust
    // whichever branch the oracle actually took and validate only that
    // branch's own arithmetic (recomputed directly below, independent of
    // `mirror_ground_launch_decision`'s own branch choice), rather than
    // asserting the branch choice itself. Recomputed directly (not taken
    // from the tuple above), since that angle is conditionally either
    // branch's own launch angle, not this decision's own gate value.
    let angle_margin = (mirror_angle_xy(floor_normal, stick) - HALF_PI).abs();
    let magnitude_margin = ((stick[0].abs() + stick[1].abs()) - x64).abs();
    let boundary_ambiguous = angle_margin < 1e-5 || magnitude_margin < 1e-5;
    let oracle_grounded = msid == MS_TRAVEL_GROUND;
    if !boundary_ambiguous {
        assert_eq!(
            oracle_grounded, grounded,
            "grounded-launch decision: angle margin {angle_margin}, magnitude margin {magnitude_margin}"
        );
    }
    if oracle_grounded {
        let facing_out = mirror_face_stick(stick[0]);
        let expected_angle = libm::atan2f(-floor_normal[0] * facing_out, floor_normal[1]);
        same_bits("facing", facing, facing_out);
        close_bits("rotate_model", rotate_model, expected_angle);
        same_bits("gr_vel", gr_vel, x74 * facing_out);
        assert_eq!(jumps, 0, "the grounded launch never touches jumps");
    } else {
        let (expected_angle, expected_facing, self_vel) =
            mirror_aerial_launch(stick, x64, x74, x88, facing_in);
        same_bits("facing", facing, expected_facing);
        close_bits("rotate_model", rotate_model, expected_angle);
        close_bits("self_vel_x", svx, self_vel[0]);
        close_bits("self_vel_y", svy, self_vel[1]);
        same_bits("gr_vel", gr_vel, 0.0);
        assert_eq!(jumps, max_jumps, "the declined launch restores every jump");
    }
}

// ---- Hold Coll: ground loss, air landing/ledge. ----

fn compare_hold_coll(ground: bool, coll_result: bool, ledge_check: bool, ledge_common: bool) {
    let mut msid = 0;
    let code =
        unsafe { oracle_hold_coll(ground, coll_result, ledge_check, ledge_common, &mut msid) };
    if ground {
        assert_eq!(code, if coll_result { 0 } else { 1 });
        if !coll_result {
            assert_eq!(msid, MS_HOLD_AIR);
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
            assert_eq!(msid, MS_HOLD);
        }
    }
}

// ---- Travel: Anim, Phys, Coll (ground and air). ----

fn compare_travel_anim(ground: bool, travel_frames_in: f32, grounded_actual: bool) {
    let (mut travel_frames, mut msid, mut start) = (0.0, 0, 0.0);
    let code = unsafe {
        oracle_travel_anim(
            ground,
            travel_frames_in,
            grounded_actual,
            &mut travel_frames,
            &mut msid,
            &mut start,
        )
    };
    same_bits("travel_frames", travel_frames, travel_frames_in - 1.0);
    if travel_frames_in - 1.0 <= 0.0 {
        assert_eq!(code, if grounded_actual { 1 } else { 2 });
        assert_eq!(msid, if grounded_actual { MS_LANDING } else { MS_FALL });
        same_bits("start", start, 0.0);
    } else {
        assert_eq!(code, 0);
    }
}

fn compare_travel_phys(
    ground: bool,
    unk_in: f32,
    x70: f32,
    x78: f32,
    gr_vel_in: f32,
    self_vel_x_in: f32,
    self_vel_y_in: f32,
    facing: f32,
    rotate_model: f32,
) {
    let (mut unk, mut gr_vel, mut svx, mut svy) = (0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_travel_phys(
            ground,
            unk_in,
            x70,
            x78,
            gr_vel_in,
            self_vel_x_in,
            self_vel_y_in,
            facing,
            rotate_model,
            &mut unk,
            &mut gr_vel,
            &mut svx,
            &mut svy,
        );
    }
    let expected_unk = unk_in + 1.0;
    same_bits("unk", unk, expected_unk);
    let past_end = expected_unk >= x70;
    if ground {
        let expected_gr_vel = if past_end {
            mirror_friction_ground(gr_vel_in, x78)
        } else {
            gr_vel_in
        };
        same_bits("gr_vel", gr_vel, expected_gr_vel);
    } else if past_end {
        // `facing * (x78 * cosf(rotateModel))`, kept bit-exact to the
        // source's own grouping (see `up.rs`'s matching comment).
        let expected_svx = -((facing * (x78 * libm::cosf(rotate_model))) - self_vel_x_in);
        let expected_svy = -((x78 * libm::sinf(rotate_model)) - self_vel_y_in);
        close_bits("self_vel_x", svx, expected_svx);
        close_bits("self_vel_y", svy, expected_svy);
    } else {
        same_bits("self_vel_x", svx, self_vel_x_in);
        same_bits("self_vel_y", svy, self_vel_y_in);
    }
}

fn compare_travel_coll_ground(
    unk2_in: f32,
    coll_result: bool,
    floor_contact: bool,
    floor_normal: [f32; 2],
    facing: f32,
) {
    let (mut unk2, mut rotate_model, mut msid) = (0.0, 0.0, 0);
    let code = unsafe {
        oracle_travel_coll_ground(
            unk2_in,
            coll_result,
            floor_contact,
            floor_normal[0],
            floor_normal[1],
            facing,
            &mut unk2,
            &mut rotate_model,
            &mut msid,
        )
    };
    same_bits("unk2", unk2, unk2_in + 1.0);
    assert_eq!(code, if coll_result { 0 } else { 1 });
    if !coll_result {
        assert_eq!(msid, MS_TRAVEL_AIR);
        // Ground loss returns before the floor-rotate branch runs.
        same_bits("rotate_model", rotate_model, 0.0);
        return;
    }
    if floor_contact {
        let expected = libm::atan2f(-floor_normal[0] * facing, floor_normal[1]);
        close_bits("rotate_model", rotate_model, expected);
    } else {
        same_bits("rotate_model", rotate_model, 0.0);
    }
}

fn compare_travel_coll_air(
    unk2_in: f32,
    x6c: i32,
    x94: f32,
    on_platform: bool,
    check_ground_ledge: bool,
    ledge_common: bool,
    env_flags: u32,
    floor_normal: [f32; 2],
    ceiling_normal: [f32; 2],
    left_wall_normal: [f32; 2],
    right_wall_normal: [f32; 2],
    self_vel: [f32; 2],
) {
    const FLOOR: u32 = 0x18000;
    const CEILING: u32 = 0x6000;
    const LEFT_WALL: u32 = 0x3F;
    const RIGHT_WALL: u32 = 0xFC0;

    let (mut out_facing, mut out_rotate_model) = (0.0, 0.0);
    let code = unsafe {
        oracle_travel_coll_air(
            unk2_in,
            x6c,
            x94,
            on_platform,
            check_ground_ledge,
            ledge_common,
            env_flags,
            floor_normal[0],
            floor_normal[1],
            ceiling_normal[0],
            ceiling_normal[1],
            left_wall_normal[0],
            left_wall_normal[1],
            right_wall_normal[0],
            right_wall_normal[1],
            self_vel[0],
            self_vel[1],
            &mut out_facing,
            &mut out_rotate_model,
        )
    };

    let is_bound = check_ground_ledge && (unk2_in >= x6c as f32 || !on_platform);
    // `0.01745329238474369f`: the pinned source's own degrees-to-radians
    // literal, kept bit-exact rather than reconstructed.
    #[allow(clippy::excessive_precision)]
    let gate = 0.01745329238474369_f32 * (90.0 + x94);
    if is_bound {
        let no_floor = (env_flags & FLOOR) == 0;
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let angle_ok = !(mirror_angle_xy(floor_normal, self_vel) < gate);
        if no_floor || angle_ok {
            assert_eq!(code, 1, "expected Bound entry");
            same_bits("facing", out_facing, 1.0);
            return;
        }
        // else: falls through to facingDir directly, same as below.
        let facing = if self_vel[0] >= 0.0 { 1.0 } else { -1.0 };
        let expected_rotate = libm::atan2f(self_vel[1], self_vel[0] * facing);
        assert_eq!(code, 2, "expected the shallow-floor graze redirect");
        same_bits("facing", out_facing, facing);
        close_bits("rotate_model", out_rotate_model, expected_rotate);
        return;
    }
    // Reached whenever `is_bound` above is false -- including when
    // `check_ground_ledge` was true but `IsBound` itself was false, since
    // the real source only ever *skips* this check via the `goto
    // facingDir` inside the `is_bound` branch above (which jumps straight
    // past this `ftCliffCommon_80081298` call into its body), not via any
    // condition on `check_ground_ledge` itself.
    if !ledge_common {
        let (var, has_surface) = if (env_flags & CEILING) != 0 {
            (mirror_angle_xy(ceiling_normal, self_vel), true)
        } else if (env_flags & LEFT_WALL) != 0 {
            (mirror_angle_xy(left_wall_normal, self_vel), true)
        } else if (env_flags & RIGHT_WALL) != 0 {
            (mirror_angle_xy(right_wall_normal, self_vel), true)
        } else {
            (0.0, false)
        };
        if has_surface && var < gate {
            let facing = if self_vel[0] >= 0.0 { 1.0 } else { -1.0 };
            let expected_rotate = libm::atan2f(self_vel[1], self_vel[0] * facing);
            assert_eq!(code, 2, "expected the wall/ceiling redirect");
            same_bits("facing", out_facing, facing);
            close_bits("rotate_model", out_rotate_model, expected_rotate);
            return;
        }
    }
    assert_eq!(code, 0, "expected nothing to happen");
    same_bits("facing", out_facing, 1.0);
    same_bits("rotate_model", out_rotate_model, 0.0);
}

// ---- Landing/Fall: Anim, Phys, Coll. ----

fn compare_landing_fall_anim(landing: bool, frames_remaining: bool, x8c: f32, x90: f32) {
    let (mut wait_calls, mut fallspecial_calls, mut mobility, mut landing_lag) = (0, 0, 0.0, 0.0);
    unsafe {
        oracle_landing_fall_anim(
            landing,
            frames_remaining,
            x8c,
            x90,
            &mut wait_calls,
            &mut fallspecial_calls,
            &mut mobility,
            &mut landing_lag,
        );
    }
    if frames_remaining {
        assert_eq!(wait_calls, 0);
        assert_eq!(fallspecial_calls, 0);
        return;
    }
    if landing {
        assert_eq!(wait_calls, 1);
        assert_eq!(fallspecial_calls, 0);
    } else {
        assert_eq!(wait_calls, 0);
        assert_eq!(fallspecial_calls, 1);
        same_bits("mobility", mobility, x8c);
        same_bits("landing_lag", landing_lag, x90);
    }
}

fn compare_landing_fall_phys(landing: bool, gr_vel_in: f32, x7c: f32, ground_friction: f32) {
    let (mut gr_vel, mut fall_phys_calls) = (0.0, 0);
    unsafe {
        oracle_landing_fall_phys(
            landing,
            gr_vel_in,
            x7c,
            ground_friction,
            &mut gr_vel,
            &mut fall_phys_calls,
        );
    }
    if landing {
        same_bits("gr_vel", gr_vel, mirror_friction_ground(gr_vel_in, x7c));
        assert_eq!(fall_phys_calls, 0);
    } else {
        // `ftFx_SpecialHiFall_Phys` is the shared ordinary-air pipeline
        // (`ft_80084DB0`); only dispatch is confirmed here, see the C
        // adapter's own header for why its arithmetic is out of scope.
        same_bits("gr_vel", gr_vel, gr_vel_in);
        assert_eq!(fall_phys_calls, 1);
    }
}

fn compare_landing_coll(coll_result: bool, x8c: f32, x90: f32) {
    let (mut fallspecial_calls, mut mobility, mut landing_lag) = (0, 0.0, 0.0);
    unsafe {
        oracle_landing_coll(
            coll_result,
            x8c,
            x90,
            &mut fallspecial_calls,
            &mut mobility,
            &mut landing_lag,
        );
    }
    assert_eq!(fallspecial_calls, if coll_result { 0 } else { 1 });
    if !coll_result {
        same_bits("mobility", mobility, x8c);
        same_bits("landing_lag", landing_lag, x90);
    }
}

/// The frame-13 Fall->Landing regression fixed in this batch
/// (`ftFx_SpecialHiFall_Coll` -> `ftFx_SpecialHiFall_Enter`); see
/// `docs/fox-up-special.md`.
fn compare_fall_coll(check_ground_ledge: bool, ledge_common: bool) {
    let (mut msid, mut start) = (0, 0.0);
    let code = unsafe { oracle_fall_coll(check_ground_ledge, ledge_common, &mut msid, &mut start) };
    if check_ground_ledge {
        assert_eq!(code, 1);
        assert_eq!(msid, MS_LANDING);
        same_bits("start", start, 13.0);
    } else {
        assert_eq!(code, if ledge_common { 2 } else { 0 });
        assert_eq!(msid, 0);
    }
}

// ---- Bound: Enter, Anim, Phys, Coll. ----

fn compare_bound_enter(self_vel_x_in: f32, x84: f32, floor_contact: bool, floor_normal: [f32; 2]) {
    let (mut msid, mut svx, mut cmd0) = (0, 0.0, 0);
    unsafe {
        oracle_bound_enter(
            self_vel_x_in,
            x84,
            floor_contact,
            floor_normal[0],
            floor_normal[1],
            &mut msid,
            &mut svx,
            &mut cmd0,
        );
    }
    assert_eq!(msid, MS_BOUND);
    same_bits("self_vel_x", svx, self_vel_x_in * x84);
    assert_eq!(cmd0, 0);
}

fn compare_bound_anim(
    cmd_var0_set: bool,
    ground_actual: bool,
    frames_remaining: bool,
    x8c: f32,
    x90: f32,
) {
    let max_jumps = 3;
    let (mut wait_calls, mut fallspecial_calls, mut jumps) = (0, 0, 0);
    let code = unsafe {
        oracle_bound_anim(
            i32::from(cmd_var0_set),
            ground_actual,
            frames_remaining,
            x8c,
            x90,
            max_jumps,
            &mut wait_calls,
            &mut fallspecial_calls,
            &mut jumps,
        )
    };
    if cmd_var0_set && !ground_actual {
        assert_eq!(code, 1);
        assert_eq!(fallspecial_calls, 1);
        assert_eq!(jumps, max_jumps);
        return;
    }
    if frames_remaining {
        assert_eq!(code, 0);
        return;
    }
    if !ground_actual {
        assert_eq!(code, 1);
        assert_eq!(fallspecial_calls, 1);
        assert_eq!(jumps, max_jumps);
    } else {
        assert_eq!(code, 2);
        assert_eq!(wait_calls, 1);
    }
}

fn compare_bound_phys(ground_actual: bool, trans_n_y: f32, gr_vel_in: f32, ground_friction: f32) {
    let (mut svy, mut gr_vel, mut drift_calls) = (0.0, 0.0, 0);
    unsafe {
        oracle_bound_phys(
            ground_actual,
            trans_n_y,
            gr_vel_in,
            ground_friction,
            f32::INFINITY,
            &mut svy,
            &mut gr_vel,
            &mut drift_calls,
        );
    }
    if ground_actual {
        same_bits(
            "gr_vel",
            gr_vel,
            mirror_friction_ground(gr_vel_in, ground_friction),
        );
        assert_eq!(drift_calls, 0);
    } else {
        same_bits("self_vel_y", svy, trans_n_y);
        assert_eq!(drift_calls, 1, "ftCommon_8007CF58 must run once in the air");
    }
}

fn compare_bound_coll(
    ground_actual: bool,
    check_ground_ledge: bool,
    ledge_common: bool,
    ft_800827a0: bool,
) {
    let mut gr_vel = 0.0;
    let code = unsafe {
        oracle_bound_coll(
            ground_actual,
            check_ground_ledge,
            ledge_common,
            ft_800827a0,
            &mut gr_vel,
        )
    };
    if !ground_actual {
        let expected = if check_ground_ledge {
            1
        } else if ledge_common {
            2
        } else {
            0
        };
        assert_eq!(code, expected);
    } else {
        assert_eq!(code, if ft_800827a0 { 0 } else { 3 });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_hold_enter(
        ground in any::<bool>(),
        gr_vel in any::<u32>(), svx in any::<u32>(),
        x54 in any::<u32>(), x58 in any::<u32>(),
    ) {
        let x58 = f32::from_bits(x58);
        prop_assume!(x58 != 0.0);
        compare_hold_enter(ground, f32::from_bits(gr_vel), f32::from_bits(svx), f32::from_bits(x54), x58);
    }

    #[test]
    fn arbitrary_hold_phys(
        ground in any::<bool>(),
        delay in any::<u32>(), gr_vel in any::<u32>(), svx in any::<u32>(), svy in any::<u32>(),
        friction in any::<u32>(), x5c in any::<u32>(), x60 in any::<u32>(), terminal in any::<u32>(),
    ) {
        compare_hold_phys(
            ground, f32::from_bits(delay), f32::from_bits(gr_vel), f32::from_bits(svx), f32::from_bits(svy),
            f32::from_bits(friction), f32::from_bits(x5c), f32::from_bits(x60), f32::from_bits(terminal),
        );
    }

    // `x64`/`x68`/`x74`/`x88` are `UpSpecial::attributes` fields, gated by
    // `up::validate` to finite magnitudes no larger than 1_000_000.0
    // before any resource carrying them is reachable in a real match;
    // stick/floor-normal components are bounded to a generous multiple of
    // their real range instead of every binary32 pattern for the same
    // reason `arbitrary_travel_coll_air` below already does. Unrestricted
    // magnitudes here would let `x74 * cosf(angle)`-style chains land
    // arbitrarily close to a zero crossing, where a libm-vs-libm angle
    // difference of a handful of ULPs (see `close_bits`'s own doc) blows
    // up into an arbitrarily large *relative* difference in the result
    // through simple calculus (`cos`/`sin` both have unit-magnitude
    // derivative at their own zero crossings) -- not a translation bug,
    // just not a meaningful thing for a fixed tolerance to absorb.
    #[test]
    fn arbitrary_hold_anim(
        air_actual in any::<bool>(),
        stick_x in -1000.0f32..1000.0, stick_y in -1000.0f32..1000.0,
        x64 in 0.0f32..1000.0, x68 in 0.0f32..1000.0, x74 in -1000.0f32..1000.0, x88 in 0.0f32..1000.0,
        floor_nx in -10.0f32..10.0, floor_ny in -10.0f32..10.0,
        on_platform in any::<bool>(),
        facing_in in prop_oneof![Just(1.0f32), Just(-1.0f32)],
    ) {
        compare_hold_anim(
            air_actual, [stick_x, stick_y],
            x64, x68, x74, x88,
            [floor_nx, floor_ny], on_platform, facing_in,
        );
    }

    #[test]
    fn arbitrary_hold_coll(
        ground in any::<bool>(), coll in any::<bool>(), ledge_check in any::<bool>(), ledge_common in any::<bool>(),
    ) {
        compare_hold_coll(ground, coll, ledge_check, ledge_common);
    }

    #[test]
    fn arbitrary_travel_anim(
        ground in any::<bool>(), travel_frames in any::<u32>(), grounded_actual in any::<bool>(),
    ) {
        compare_travel_anim(ground, f32::from_bits(travel_frames), grounded_actual);
    }

    // Bounded for the same reason `arbitrary_hold_anim` above is: `x78 *
    // cosf(rotate_model)` is the same style of zero-crossing-sensitive
    // chain, and `x70`/`x78` are validated `UpSpecial::attributes` fields.
    #[test]
    fn arbitrary_travel_phys(
        ground in any::<bool>(),
        unk in -1000.0f32..1000.0, x70 in 0.0f32..1000.0, x78 in -1000.0f32..1000.0,
        gr_vel in -1000.0f32..1000.0, svx in -1000.0f32..1000.0, svy in -1000.0f32..1000.0,
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        rotate_model in -100.0f32..100.0,
    ) {
        compare_travel_phys(
            ground, unk, x70, x78,
            gr_vel, svx, svy,
            facing, rotate_model,
        );
    }

    #[test]
    fn arbitrary_travel_coll_ground(
        unk2 in any::<u32>(), coll in any::<bool>(), floor_contact in any::<bool>(),
        floor_nx in any::<u32>(), floor_ny in any::<u32>(),
        facing in prop_oneof![Just(1.0f32), Just(-1.0f32)],
    ) {
        compare_travel_coll_ground(
            f32::from_bits(unk2), coll, floor_contact,
            [f32::from_bits(floor_nx), f32::from_bits(floor_ny)], facing,
        );
    }

    #[test]
    fn arbitrary_travel_coll_air(
        unk2 in 0i32..40, x6c in 0i32..40, x94 in 0.0f32..90.0,
        on_platform in any::<bool>(), check_ground_ledge in any::<bool>(), ledge_common in any::<bool>(),
        env_flags in prop_oneof![Just(0u32), Just(0x18000), Just(0x6000), Just(0x1), Just(0x40)],
        floor_nx in -1.0f32..1.0, floor_ny in -1.0f32..1.0,
        self_vel_x in -10.0f32..10.0, self_vel_y in -10.0f32..10.0,
    ) {
        compare_travel_coll_air(
            unk2 as f32, x6c, x94, on_platform, check_ground_ledge, ledge_common, env_flags,
            [floor_nx, floor_ny], [floor_nx, floor_ny], [floor_nx, floor_ny], [floor_nx, floor_ny],
            [self_vel_x, self_vel_y],
        );
    }

    #[test]
    fn arbitrary_landing_fall_anim(
        landing in any::<bool>(), frames_remaining in any::<bool>(),
        x8c in any::<u32>(), x90 in any::<u32>(),
    ) {
        compare_landing_fall_anim(landing, frames_remaining, f32::from_bits(x8c), f32::from_bits(x90));
    }

    #[test]
    fn arbitrary_landing_fall_phys(
        landing in any::<bool>(), gr_vel in any::<u32>(), x7c in any::<u32>(), friction in any::<u32>(),
    ) {
        compare_landing_fall_phys(landing, f32::from_bits(gr_vel), f32::from_bits(x7c), f32::from_bits(friction));
    }

    #[test]
    fn arbitrary_landing_coll(coll in any::<bool>(), x8c in any::<u32>(), x90 in any::<u32>()) {
        compare_landing_coll(coll, f32::from_bits(x8c), f32::from_bits(x90));
    }

    #[test]
    fn arbitrary_fall_coll(check_ground_ledge in any::<bool>(), ledge_common in any::<bool>()) {
        compare_fall_coll(check_ground_ledge, ledge_common);
    }

    #[test]
    fn arbitrary_bound_enter(
        svx in any::<u32>(), x84 in any::<u32>(), floor_contact in any::<bool>(),
        floor_nx in any::<u32>(), floor_ny in any::<u32>(),
    ) {
        compare_bound_enter(f32::from_bits(svx), f32::from_bits(x84), floor_contact, [f32::from_bits(floor_nx), f32::from_bits(floor_ny)]);
    }

    #[test]
    fn arbitrary_bound_anim(
        cmd_var0_set in any::<bool>(), ground_actual in any::<bool>(), frames_remaining in any::<bool>(),
        x8c in any::<u32>(), x90 in any::<u32>(),
    ) {
        compare_bound_anim(cmd_var0_set, ground_actual, frames_remaining, f32::from_bits(x8c), f32::from_bits(x90));
    }

    #[test]
    fn arbitrary_bound_phys(
        ground_actual in any::<bool>(), trans_n_y in any::<u32>(), gr_vel in any::<u32>(), friction in any::<u32>(),
    ) {
        compare_bound_phys(ground_actual, f32::from_bits(trans_n_y), f32::from_bits(gr_vel), f32::from_bits(friction));
    }

    #[test]
    fn arbitrary_bound_coll(
        ground_actual in any::<bool>(), check_ground_ledge in any::<bool>(), ledge_common in any::<bool>(), ft_800827a0 in any::<bool>(),
    ) {
        compare_bound_coll(ground_actual, check_ground_ledge, ledge_common, ft_800827a0);
    }
}

#[test]
fn boundaries() {
    for &ground in &[true, false] {
        compare_hold_enter(ground, 0.0, 0.0, 2.0, 2.0);
        compare_hold_enter(ground, f32::NAN, f32::INFINITY, 2.0, 3.0);
        compare_hold_phys(ground, 2.0, 5.0, -5.0, 5.0, 0.02, 0.1, 0.05, 3.0);
        compare_hold_phys(ground, 0.0, 5.0, -5.0, 5.0, 0.02, 0.1, 0.05, 3.0);
    }
    // The straight-up default and the two stick thresholds.
    for &(air_actual, stick, x64, x88) in &[
        (true, [0.0f32, 0.0f32], 0.2875, 0.2875),
        (true, [0.0, 0.9], 0.2875, 0.2875),
        (true, [0.9, 0.0], 0.2875, 0.2875),
        (true, [-0.9, 0.0], 0.2875, 0.2875),
        (true, [0.9, 0.9], 0.2875, 0.95), // magnitude clears x64, |x| below x88
        (true, [0.9, 0.9], 0.2875, 0.2),  // magnitude clears x64, |x| above x88
        (false, [0.9, -0.9], 0.2875, 0.2875), // grounded launch, into the floor test below
    ] {
        compare_hold_anim(
            air_actual,
            stick,
            x64,
            20.0,
            3.0,
            x88,
            [0.0, 1.0],
            false,
            1.0,
        );
    }
    // Grounded launch: along the floor vs. into the floor vs. a platform.
    compare_hold_anim(
        false,
        [0.9, 0.0],
        0.2875,
        20.0,
        3.0,
        0.2875,
        [0.0, 1.0],
        false,
        1.0,
    );
    compare_hold_anim(
        false,
        [-0.9, 0.0],
        0.2875,
        20.0,
        3.0,
        0.2875,
        [0.0, 1.0],
        false,
        1.0,
    );
    compare_hold_anim(
        false,
        [0.9, 0.0],
        0.2875,
        20.0,
        3.0,
        0.2875,
        [0.0, 1.0],
        true,
        1.0,
    );
    for &ground in &[true, false] {
        for &coll in &[true, false] {
            for &ledge_check in &[true, false] {
                for &ledge_common in &[true, false] {
                    compare_hold_coll(ground, coll, ledge_check, ledge_common);
                }
            }
        }
    }
    for &ground in &[true, false] {
        for &travel_frames in &[0.0f32, 1.0, 2.0, -1.0] {
            for &grounded_actual in &[true, false] {
                compare_travel_anim(ground, travel_frames, grounded_actual);
            }
        }
        compare_travel_phys(ground, 0.0, 10.0, 2.0, 3.0, 3.0, -3.0, 1.0, 0.7);
        compare_travel_phys(ground, 15.0, 10.0, 2.0, 3.0, 3.0, -3.0, -1.0, 0.7);
    }
    compare_travel_coll_ground(0.0, true, true, [0.0, 1.0], 1.0);
    compare_travel_coll_ground(0.0, true, false, [0.0, 1.0], 1.0);
    compare_travel_coll_ground(0.0, false, true, [0.0, 1.0], 1.0);
    // The bound decision: past bounce_frames, off-platform, and the
    // shallow-floor graze this port approximates as always-Bound.
    compare_travel_coll_air(
        10.0,
        5,
        30.0,
        true,
        true,
        false,
        0x18000,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -5.0],
    );
    compare_travel_coll_air(
        0.0,
        5,
        30.0,
        false,
        true,
        false,
        0x18000,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -5.0],
    );
    compare_travel_coll_air(
        0.0,
        5,
        30.0,
        true,
        true,
        false,
        0x18000,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.1, -0.01],
    );
    compare_travel_coll_air(
        0.0,
        5,
        30.0,
        true,
        false,
        false,
        0x6000,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -5.0],
    );
    compare_travel_coll_air(
        0.0,
        5,
        30.0,
        true,
        false,
        true,
        0x6000,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -5.0],
    );
    compare_travel_coll_air(
        0.0,
        5,
        30.0,
        true,
        false,
        false,
        0,
        [0.0, 1.0],
        [0.0, -1.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -5.0],
    );
    for &landing in &[true, false] {
        for &frames_remaining in &[true, false] {
            compare_landing_fall_anim(landing, frames_remaining, 0.8, 4.0);
        }
        compare_landing_fall_phys(landing, 5.0, 0.05, 0.02);
    }
    for &coll in &[true, false] {
        compare_landing_coll(coll, 0.8, 4.0);
    }
    for &check_ground_ledge in &[true, false] {
        for &ledge_common in &[true, false] {
            compare_fall_coll(check_ground_ledge, ledge_common);
        }
    }
    compare_bound_enter(5.0, 0.5, true, [0.0, 1.0]);
    compare_bound_enter(-5.0, 0.5, false, [0.0, 1.0]);
    for &ground_actual in &[true, false] {
        for &frames_remaining in &[true, false] {
            for &cmd_var0_set in &[true, false] {
                compare_bound_anim(cmd_var0_set, ground_actual, frames_remaining, 0.8, 4.0);
            }
        }
        compare_bound_phys(ground_actual, -1.5, 5.0, 0.02);
        for &check_ground_ledge in &[true, false] {
            for &ledge_common in &[true, false] {
                for &ft_800827a0 in &[true, false] {
                    compare_bound_coll(
                        ground_actual,
                        check_ground_ledge,
                        ledge_common,
                        ft_800827a0,
                    );
                }
            }
        }
    }
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let up = include_str!("oracle/original/ftfoxspecialhi.c");
    for header in [
        "void ftFx_SpecialHi_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialAirHiStart_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialAirHi_AirToGround(HSD_GObj* gobj)",
        "void ftFx_SpecialAirHi_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialHi_Phys(HSD_GObj* gobj)",
        "void ftFx_SpecialAirHi_Phys(HSD_GObj* gobj)",
        "void ftFx_SpecialAirHi_Coll(HSD_GObj* gobj)",
        "void ftFx_SpecialHiFall_Coll(HSD_GObj* gobj)",
        "void ftFx_SpecialHiFall_Enter(HSD_GObj* gobj)",
        "void ftFx_SpecialHiBound_Enter(HSD_GObj* gobj)",
    ] {
        assert!(up.contains(header), "{header}");
    }
    let adapter = include_str!("oracle/fox_specialhi.c");
    assert!(adapter.contains("#include \"ftfoxspecialhi_original.inc\""));
    assert!(adapter.contains("#include \"up_special_angle_original.inc\""));
    assert!(adapter.contains("#include \"up_special_platform_original.inc\""));
}
