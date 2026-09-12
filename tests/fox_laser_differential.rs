//! The fired laser item checked against the pinned C (`itfoxlaser.c` for
//! spawn/motion/shield-bounce/reflected, `item.c`'s `Item_80269F14` for the
//! Reflector hand-off's owner-swap and damage-scaling arithmetic): spawn
//! position (`ftLib_80086990`'s ECB-midpoint formula, through
//! `Item_InitRaySpawnPosition`), per-frame velocity recompute
//! (`Item_UpdateRayAnimation`), the shield-bounce velocity mirror (the real
//! `lbVector_Mirror`), the Reflector item-side callback's facing snap and
//! `angle += pi`, and the reflect damage-scaling truncation/cap. Each
//! comparison pins the extracted callback's exact behaviour with an inline
//! Rust formula, matching `fox_neutral_special_differential.rs`'s own
//! style -- `tests/game_fox_neutral_special.rs` and
//! `game_fox_neutral_special_reflect.rs` separately exercise the Rust
//! engine's own mirror end to end.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_laser_spawn(
        owner_x: f32,
        owner_y: f32,
        ecb_top: f32,
        ecb_bottom: f32,
        angle_in: f32,
        speed_in: f32,
        kind_in: i32,
        lifetime_attr: f32,
        out_pos_x: *mut f32,
        out_pos_y: *mut f32,
        out_angle: *mut f32,
        out_speed: *mut f32,
        out_facing_dir: *mut f32,
        out_lifetime: *mut f32,
    );
    fn oracle_laser_motion(
        speed_in: f32,
        angle_in: f32,
        out_vel_x: *mut f32,
        out_vel_y: *mut f32,
        out_facing_dir: *mut f32,
    );
    fn oracle_laser_shield_bounce(
        vel_x_in: f32,
        vel_y_in: f32,
        normal_x_in: f32,
        normal_y_in: f32,
        out_vel_x: *mut f32,
        out_vel_y: *mut f32,
        out_angle: *mut f32,
    );
    fn oracle_laser_reflected(
        facing_dir_in: f32,
        xc68_in: f32,
        angle_in: f32,
        out_facing_dir: *mut f32,
        out_angle: *mut f32,
    );
    fn oracle_reflect_owner_and_damage(
        hitbox_damage_in: f32,
        damage_mul_in: f32,
        damage_cap_in: u32,
        out_owner_swapped: *mut i32,
        out_reflect_called: *mut i32,
        out_scaled_damage: *mut u32,
    );
}

fn same_bits(label: &str, actual: f32, expected: f32) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{label}: {actual} (0x{:08x}) != {expected} (0x{:08x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

/// Every `while (angle < 0.0F) angle += M_TAU;` / `while (angle > M_TAU)
/// angle -= M_TAU;` loop in the pinned source (`normalizeAngle`,
/// `Item_BounceRayOffShield`, `Item_ResetRayAfterReflection`): `angle` is
/// `f32` throughout, `M_TAU` is the `f64` constant `M_PI * 2.0`, so each
/// step promotes `angle` to `f64`, adds/subtracts the full-precision
/// constant, and narrows back to `f32` -- a single f64 accumulation across
/// every iteration (rather than narrowing once at the end) rounds
/// differently for inputs far outside `[0, tau)` that take several
/// iterations to settle, which is exactly the case this test's own
/// unrestricted `angle` range exercises.
fn normalize_angle_f32(mut angle: f32) -> f32 {
    const TAU: f64 = core::f64::consts::PI * 2.0;
    while (angle as f64) < 0.0 {
        angle = (angle as f64 + TAU) as f32;
    }
    while (angle as f64) > TAU {
        angle = (angle as f64 - TAU) as f32;
    }
    angle
}

/// This crate uses the `libm` crate's own `cosf`/`sinf`/`atan2f`
/// pervasively (`src/game/projectile.rs`, matching `src/fighter/{aerial,
/// escape_air,damage}.rs`'s own established precedent), deliberately: it
/// gives every platform the same replay-affecting arithmetic regardless of
/// the local system's C library. `libm`'s own implementations disagree from
/// this host's system `libm` (which the pinned C oracle actually calls) by
/// a handful of ULPs on their own -- an inherent cross-implementation
/// rounding gap for transcendental functions, not a translation bug, the
/// same reasoning `fox_up_special_differential.rs`'s own `close_bits`
/// already documents for the identical functions. Only genuinely
/// trig-derived comparisons use this; the angle-normalization loops above
/// (no trig call at all) stay bit-exact via `normalize_angle_f32`.
fn close_bits(label: &str, actual: f32, expected: f32) {
    let distance = (actual.to_bits() as i64 - expected.to_bits() as i64).unsigned_abs();
    assert!(
        distance <= 8,
        "{label}: {actual:?} ({:#010x}) != {expected:?} ({:#010x}), {distance} ULPs apart",
        actual.to_bits(),
        expected.to_bits(),
    );
}

fn compare_spawn(
    owner_x: f32,
    owner_y: f32,
    ecb_top: f32,
    ecb_bottom: f32,
    angle: f32,
    speed: f32,
) {
    let (mut px, mut py, mut oa, mut os, mut facing, mut lifetime) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    unsafe {
        oracle_laser_spawn(
            owner_x,
            owner_y,
            ecb_top,
            ecb_bottom,
            angle,
            speed,
            54,
            35.0,
            &mut px,
            &mut py,
            &mut oa,
            &mut os,
            &mut facing,
            &mut lifetime,
        );
    }
    // `ftLib_80086990`: `owner.cur_pos + (0, 0.5 * (ecb.top + ecb.bottom), 0)`.
    same_bits("pos_x", px, owner_x);
    same_bits("pos_y", py, owner_y + 0.5 * (ecb_top + ecb_bottom));
    // `normalizeAngle`: clamp into `[0, tau]`.
    same_bits("angle", oa, normalize_angle_f32(angle));
    same_bits("speed", os, speed);
    same_bits("lifetime", lifetime, 35.0);
    let expected_facing = {
        let a = normalize_angle_f32(angle) as f64;
        if a < core::f64::consts::PI / 2.0 || a > core::f64::consts::PI * 3.0 / 2.0 {
            1.0f32
        } else {
            -1.0f32
        }
    };
    assert_eq!(facing, expected_facing, "right_facing at angle {oa}");
}

fn compare_motion(speed: f32, angle: f32) {
    let (mut vx, mut vy, mut facing) = (0.0, 0.0, 0.0);
    unsafe {
        oracle_laser_motion(speed, angle, &mut vx, &mut vy, &mut facing);
    }
    close_bits("vel_x", vx, speed * libm::cosf(angle));
    close_bits("vel_y", vy, speed * libm::sinf(angle));
    assert_eq!(facing, if vx > 0.0 { 1.0 } else { -1.0 });
}

/// `lbVector_Mirror`: `a -= 2 * <a, n> * n` (x/y only).
fn expected_mirror(vel: [f32; 2], normal: [f32; 2]) -> [f32; 2] {
    let f = (normal[0] * vel[0] + normal[1] * vel[1]) * -2.0;
    [vel[0] + normal[0] * f, vel[1] + normal[1] * f]
}

fn compare_shield_bounce(vel: [f32; 2], normal: [f32; 2]) {
    let (mut vx, mut vy, mut angle) = (0.0, 0.0, 0.0);
    unsafe {
        oracle_laser_shield_bounce(
            vel[0], vel[1], normal[0], normal[1], &mut vx, &mut vy, &mut angle,
        );
    }
    let expected = expected_mirror(vel, normal);
    // Pure multiply/add: bit-exact, matching the real `lbVector_Mirror`
    // linked directly (not stubbed).
    same_bits("vel_x", vx, expected[0]);
    same_bits("vel_y", vy, expected[1]);
    // `atan2f`-derived: ULP tolerance (see `close_bits`).
    close_bits(
        "angle",
        angle,
        normalize_angle_f32(libm::atan2f(expected[1], expected[0])),
    );
}

fn compare_reflected(facing_dir: f32, xc68: f32, angle: f32) {
    let (mut out_facing, mut out_angle) = (0.0, 0.0);
    unsafe {
        oracle_laser_reflected(facing_dir, xc68, angle, &mut out_facing, &mut out_angle);
    }
    assert_eq!(
        out_facing, xc68,
        "facing snaps to the reflecting fighter's own xC68 side"
    );
    // No trig call at all (`angle += M_PI`, the `f64` constant, promoting
    // the f32 field for the addition exactly as C's own `+=` does): bit-
    // exact via the precise per-iteration normalization loop.
    let after_add = (angle as f64 + core::f64::consts::PI) as f32;
    same_bits("angle", out_angle, normalize_angle_f32(after_add));
}

fn compare_reflect_damage(hitbox_damage: f32, damage_mul: f32, damage_cap: u32) {
    let (mut owner_swapped, mut reflect_called, mut scaled) = (0, 0, 0);
    unsafe {
        oracle_reflect_owner_and_damage(
            hitbox_damage,
            damage_mul,
            damage_cap,
            &mut owner_swapped,
            &mut reflect_called,
            &mut scaled,
        );
    }
    assert_eq!(
        owner_swapped, 1,
        "Item_80269F14 always swaps the owner for a non-M_Ball item"
    );
    assert_eq!(reflect_called, 1);
    // `item.c:1613-1619`: `hit.damage * xC6C + 0.99f`, truncated toward
    // zero, capped at the (here test-controlled) global maximum.
    let unclamped = (hitbox_damage * damage_mul + 0.99).max(0.0) as u32;
    let expected = unclamped.min(damage_cap);
    assert_eq!(scaled, expected);
}

#[test]
fn adapter_statements_are_verbatim_in_the_pinned_sources() {
    let adapter = include_str!("oracle/fox_laser.c");
    let source = include_str!("oracle/original/it_kinds_inlines.h");
    for (begin, end) in [
        (
            "/* BEGIN VERBATIM INIT RAY SPAWN FIELDS */\n",
            "/* END VERBATIM INIT RAY SPAWN FIELDS */",
        ),
        (
            "/* BEGIN VERBATIM INIT RAY SPAWN POSITION */\n",
            "/* END VERBATIM INIT RAY SPAWN POSITION */",
        ),
        (
            "/* BEGIN VERBATIM UPDATE RAY ANIMATION */\n",
            "/* END VERBATIM UPDATE RAY ANIMATION */",
        ),
        (
            "/* BEGIN VERBATIM BOUNCE RAY OFF SHIELD */\n",
            "/* END VERBATIM BOUNCE RAY OFF SHIELD */",
        ),
        (
            "/* BEGIN VERBATIM RESET RAY AFTER REFLECTION */\n",
            "/* END VERBATIM RESET RAY AFTER REFLECTION */",
        ),
    ] {
        let block = adapter
            .split(begin)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap();
        assert!(source.contains(block), "not verbatim: {begin}");
    }
}

#[test]
fn known_values_match() {
    compare_spawn(0.0, 0.0, 3.0, 0.0, 0.0, 7.0);
    compare_spawn(-2.0, 4.0, 16.0, 0.0, core::f32::consts::PI, 7.0);
    compare_motion(7.0, 0.0);
    compare_motion(7.0, core::f32::consts::PI);
    compare_shield_bounce([7.0, 0.0], [-1.0, 0.0]);
    compare_shield_bounce([4.0, 3.0], [0.0, -1.0]);
    compare_reflected(1.0, -1.0, 0.0);
    compare_reflected(-1.0, -1.0, 1.5);
    // damage_mul = 1.0 rounds identically either way (the exact case the
    // native regression, `game_fox_neutral_special_reflect.rs`, avoids for
    // this reason); damage_mul = 1.5 is the one that actually distinguishes
    // the fixed truncation formula from the bug this batch found and fixed.
    compare_reflect_damage(3.0, 1.0, 999);
    compare_reflect_damage(3.0, 1.5, 999);
    compare_reflect_damage(3.0, 1.5, 4);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn spawn_matches_arbitrary_positions(
        owner_x in -50.0f32..50.0,
        owner_y in -50.0f32..50.0,
        ecb_top in 0.0f32..20.0,
        ecb_bottom in -5.0f32..5.0,
        angle in 0.0f32..(core::f32::consts::TAU),
        speed in 0.0f32..20.0,
    ) {
        compare_spawn(owner_x, owner_y, ecb_top, ecb_bottom, angle, speed);
    }

    #[test]
    fn motion_matches_arbitrary_angles(
        speed in 0.0f32..20.0,
        angle in -10.0f32..10.0,
    ) {
        compare_motion(speed, angle);
    }

    #[test]
    fn shield_bounce_matches_arbitrary_vectors(
        vx in -20.0f32..20.0,
        vy in -20.0f32..20.0,
        normal in prop_oneof![
            Just([1.0f32, 0.0f32]),
            Just([-1.0f32, 0.0f32]),
            Just([0.0f32, 1.0f32]),
            Just([0.0f32, -1.0f32]),
        ],
    ) {
        compare_shield_bounce([vx, vy], normal);
    }

    #[test]
    fn reflected_matches_arbitrary_facings(
        facing_dir in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        xc68 in prop_oneof![Just(1.0f32), Just(-1.0f32)],
        angle in -10.0f32..10.0,
    ) {
        compare_reflected(facing_dir, xc68, angle);
    }

    #[test]
    fn reflect_damage_matches_arbitrary_scaling(
        hitbox_damage in 0.0f32..20.0,
        damage_mul in 0.0f32..3.0,
        damage_cap in 0u32..999,
    ) {
        compare_reflect_damage(hitbox_damage, damage_mul, damage_cap);
    }
}
