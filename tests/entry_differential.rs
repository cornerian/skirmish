//! Match-start warp-in (Entry/EntryStart/EntryEnd) timers and Y curve,
//! checked bit-exactly against the pinned `ft_0C31.c` (`entry.functions.json`,
//! `docs/match-start.md`), including the `x6BC` divisor EntryEnd's own Phys
//! uses instead of `x6C0`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::entry::{amplitude, end_progress, start_progress};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_entry_anim(
        timer: *mut i32,
        trophy_scale: f32,
        scale_y: f32,
        start_frames: i32,
        out_timer: *mut i32,
        out_x20: *mut f32,
        out_x24: *mut f32,
    ) -> i32;
    fn oracle_entry_start_frame(
        timer: *mut i32,
        x4: f32,
        x20: f32,
        start_frames: i32,
        end_frames: i32,
        out_timer: *mut i32,
        out_x28: *mut f32,
        out_y: *mut f32,
    ) -> i32;
    fn oracle_entry_end_frame(
        timer: *mut i32,
        x4: f32,
        x20: f32,
        start_frames: i32,
        flag_bit4: bool,
        invincibility_frames: i32,
        out_timer: *mut i32,
        out_x28: *mut f32,
        out_y: *mut f32,
        out_invincibility_applied: *mut i32,
        out_invincibility_value: *mut i32,
    ) -> i32;
    fn oracle_entry_start_enter(
        trophy_scale: f32,
        scale_y: f32,
        start_frames: i32,
        out_timer: *mut i32,
        out_x24: *mut f32,
        out_x20: *mut f32,
    );
    fn oracle_entry_end_enter(
        x4: f32,
        x20: f32,
        end_frames: i32,
        out_timer: *mut i32,
        out_y: *mut f32,
    );
    /// Chains `ftCo_Entry_Anim`'s own transition (timer == 0) directly into
    /// `ftCo_EntryStart_Phys` within the same call -- the exact per-frame
    /// order of the Entry -> EntryStart transition frame itself, which none
    /// of the proptests above exercise (`oracle_entry_start_frame` models a
    /// *steady-state* EntryStart frame, re-running `ftCo_EntryStart_Anim`'s
    /// own decrement first, not the fresh-transition frame's `t = 1 /
    /// start_frames`).
    fn oracle_entry_transition_frame(
        trophy_scale: f32,
        scale_y: f32,
        x4: f32,
        start_frames: i32,
        out_timer: *mut i32,
        out_x20: *mut f32,
        out_y: *mut f32,
    );
}

/// Both NaN, or exactly the same bits: NaN payload propagation through
/// arithmetic is unspecified (an existing hardening idiom used by several
/// other differential tests, e.g. `ground_launch_differential.rs`), so a
/// bit-exact comparison must not fail two independently computed NaNs
/// against each other.
fn same_float(a: f32, b: f32) {
    if a.is_nan() || b.is_nan() {
        assert!(a.is_nan() && b.is_nan(), "{a} != {b}");
    } else {
        assert_eq!(a.to_bits(), b.to_bits(), "{a} != {b}");
    }
}

proptest! {
    /// `ftCo_800C6408`'s amplitude formula, scoped to `scale_y == 1.0`
    /// (Skirmish keeps no separate uniform fighter scale, matching
    /// `game::entry::enter_start`'s own assumption).
    #[test]
    fn entry_start_enter_amplitude_matches_the_1_497345_literal(
        trophy_scale in prop::num::f32::ANY,
        start_frames in 1u32..=10_000,
    ) {
        let (mut timer, mut x24, mut x20) = (0i32, 0.0f32, 0.0f32);
        unsafe {
            oracle_entry_start_enter(
                trophy_scale, 1.0, start_frames as i32, &mut timer, &mut x24, &mut x20,
            );
        }
        prop_assert_eq!(timer as u32, start_frames);
        same_float(x24, trophy_scale);
        same_float(x20, amplitude(trophy_scale));
    }

    /// `ftCo_Entry_Anim`: the transition (if any) runs first, and the
    /// unconditional trailing decrement always lands on whatever timer
    /// value is current *after* that -- EntryStart's own fresh `start_frames`
    /// on a transition frame, or the plain countdown otherwise.
    #[test]
    fn entry_anim_transition_and_shared_timer_decrement(
        timer_in in 0u32..2_000,
        trophy_scale in -10.0f32..10.0,
        start_frames in 1u32..=10_000,
    ) {
        let mut timer = timer_in as i32;
        let (mut out_timer, mut out_x20, mut out_x24) = (0i32, 0.0f32, 0.0f32);
        let transitioned = unsafe {
            oracle_entry_anim(
                &mut timer, trophy_scale, 1.0, start_frames as i32,
                &mut out_timer, &mut out_x20, &mut out_x24,
            )
        };
        if timer_in == 0 {
            prop_assert_ne!(transitioned, 0);
            prop_assert_eq!(out_timer as u32, start_frames - 1);
            same_float(out_x24, trophy_scale);
            same_float(out_x20, amplitude(trophy_scale));
        } else {
            prop_assert_eq!(transitioned, 0);
            prop_assert_eq!(out_timer as u32, timer_in - 1);
        }
    }

    /// `ftCo_EntryStart_Anim` (decrement-then-check) followed by whichever
    /// Phys is current afterward: `ftCo_EntryStart_Phys`'s `(x6BC - timer) /
    /// x6BC`, or -- on the exact frame it transitions -- `ftCo_EntryEnd_Phys`
    /// at full progress (`x6BC` divisor, matching the design note's cited
    /// fact that EntryEnd's own Phys divides by `x6BC`, not `x6C0`).
    #[test]
    fn entry_start_frame_matches_the_progress_fraction(
        timer_in in 1u32..2_000,
        x4 in -1_000.0f32..1_000.0,
        x20 in -1_000.0f32..1_000.0,
        start_frames in 1u32..=2_000,
        end_frames in 1u32..=2_000,
    ) {
        let mut timer = timer_in as i32;
        let (mut out_timer, mut out_x28, mut out_y) = (0i32, 0.0f32, 0.0f32);
        let transitioned = unsafe {
            oracle_entry_start_frame(
                &mut timer, x4, x20, start_frames as i32, end_frames as i32,
                &mut out_timer, &mut out_x28, &mut out_y,
            )
        };
        if timer_in == 1 {
            prop_assert_ne!(transitioned, 0);
            prop_assert_eq!(out_timer as u32, end_frames);
            let t = end_progress(end_frames, start_frames);
            same_float(out_x28, x20 * t);
            same_float(out_y, x4 + x20 * t);
        } else {
            prop_assert_eq!(transitioned, 0);
            prop_assert_eq!(out_timer as u32, timer_in - 1);
            let t = start_progress(out_timer as u32, start_frames);
            same_float(out_x28, x20 * t);
            same_float(out_y, x4 + x20 * t);
        }
    }

    /// `ftCo_EntryEnd_Anim` (decrement-then-check; the invincibility branch
    /// and `ftCommon_8007D92C` exit only fire once timer reaches 0) followed
    /// by `ftCo_EntryEnd_Phys` on a non-exit frame: `timer / x6BC`.
    #[test]
    fn entry_end_frame_matches_the_x6bc_divisor_and_exit_gate(
        timer_in in 1u32..2_000,
        x4 in -1_000.0f32..1_000.0,
        x20 in -1_000.0f32..1_000.0,
        start_frames in 1u32..=2_000,
        flag_bit4 in any::<bool>(),
        invincibility_frames in 0u32..=1_000,
    ) {
        let mut timer = timer_in as i32;
        let (mut out_timer, mut out_x28, mut out_y, mut applied, mut value) =
            (0i32, 0.0f32, 0.0f32, 0i32, 0i32);
        let exited = unsafe {
            oracle_entry_end_frame(
                &mut timer, x4, x20, start_frames as i32, flag_bit4, invincibility_frames as i32,
                &mut out_timer, &mut out_x28, &mut out_y, &mut applied, &mut value,
            )
        };
        if timer_in == 1 {
            prop_assert_ne!(exited, 0);
            prop_assert_eq!(out_timer, 0);
            if flag_bit4 {
                prop_assert_eq!(applied, 1);
                prop_assert_eq!(value as u32, invincibility_frames);
            } else {
                prop_assert_eq!(applied, 0);
            }
        } else {
            prop_assert_eq!(exited, 0);
            prop_assert_eq!(out_timer as u32, timer_in - 1);
            let t = end_progress(out_timer as u32, start_frames);
            same_float(out_x28, x20 * t);
            same_float(out_y, x4 + x20 * t);
        }
    }
}

#[test]
fn entry_end_enter_writes_the_full_amplitude_position() {
    let (mut timer, mut y) = (0i32, 0.0f32);
    unsafe {
        oracle_entry_end_enter(10.0, 1.347_610_5, 30, &mut timer, &mut y);
    }
    assert_eq!(timer, 30);
    assert_eq!(y, 11.347_61);
}

#[test]
fn boundary_timers_and_frame_counts() {
    for (timer_in, start_frames) in [(0u32, 1u32), (0, 10_000), (1, 1)] {
        let mut timer = timer_in as i32;
        let (mut out_timer, mut out_x20, mut out_x24) = (0i32, 0.0f32, 0.0f32);
        let transitioned = unsafe {
            oracle_entry_anim(
                &mut timer,
                0.9,
                1.0,
                start_frames as i32,
                &mut out_timer,
                &mut out_x20,
                &mut out_x24,
            )
        };
        if timer_in == 0 {
            assert_ne!(transitioned, 0);
            assert_eq!(out_timer as u32, start_frames - 1);
        } else {
            assert_eq!(transitioned, 0);
        }
    }
}

proptest! {
    /// The Entry -> EntryStart transition frame itself: `ftCo_Entry_Anim`'s
    /// transition (setting `x20`/timer fresh) followed, in the very same
    /// call, by `ftCo_EntryStart_Phys` using that fresh state -- exactly
    /// `game::entry::update_animation`'s `Action::Entry` arm calling
    /// `enter_start` and then falling through to `move_fighter`'s
    /// `Action::EntryStart` arm within the same simulated frame. Pinned
    /// against the real-replay parity loop's own finding (docs/parity.md):
    /// `fox-ys.slp`/`fox-fod.slp` show a real recording whose position.y at
    /// this exact frame differs from what this formula (and the retail
    /// `main.dol`'s own disassembled `fdivs`/`fmuls`/`fadds` sequence, and
    /// this oracle) all agree on -- proving the gap is not a Skirmish
    /// arithmetic bug, since Skirmish already matches the compiled decomp
    /// source bit-exactly here.
    #[test]
    fn entry_transition_frame_matches_the_oracle_bit_exactly(
        trophy_scale in prop::num::f32::ANY,
        x4 in -1_000.0f32..1_000.0,
        start_frames in 2u32..=2_000,
    ) {
        let (mut out_timer, mut out_x20, mut out_y) = (0i32, 0.0f32, 0.0f32);
        unsafe {
            oracle_entry_transition_frame(
                trophy_scale, 1.0, x4, start_frames as i32,
                &mut out_timer, &mut out_x20, &mut out_y,
            );
        }
        prop_assert_eq!(out_timer as u32, start_frames - 1);
        let amp = amplitude(trophy_scale);
        same_float(out_x20, amp);
        let t = start_progress(start_frames - 1, start_frames);
        same_float(out_y, x4 + amp * t);
    }
}

/// The exact fox-ys.slp P2 case (docs/parity.md): trophy_scale 0.9, spawn
/// y 28.0, start_frames 30. Confirms the oracle (and therefore Skirmish,
/// which the proptest above already ties to the oracle bit-exactly) lands
/// on `0x41e05bff`, one ULP below the recording's own `0x41e05c00` --
/// pinned here so the exact reported gap cannot silently drift.
#[test]
fn entry_transition_frame_reproduces_the_fox_ys_p2_one_ulp_gap() {
    let (mut out_timer, mut out_x20, mut out_y) = (0i32, 0.0f32, 0.0f32);
    unsafe {
        oracle_entry_transition_frame(
            0.9, // exact pack value: 0.899_999_976_158_142_1 as f64, i.e. f32(0.9)
            1.0,
            28.0,
            30,
            &mut out_timer,
            &mut out_x20,
            &mut out_y,
        );
    }
    assert_eq!(out_timer, 29);
    assert_eq!(out_y.to_bits(), 0x41e05bff);
    assert_ne!(
        out_y.to_bits(),
        0x41e05c00,
        "recording's own bits, not reproducible from this arithmetic"
    );
}
