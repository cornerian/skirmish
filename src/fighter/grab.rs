//! Throw-direction input predicates retained from `ftCo_Throw.c`.

use serde::{Deserialize, Serialize};

const MASH_BUTTONS: u32 = 0x8000_0F00;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MashState {
    pub axes: [i8; 2],
    pub shake_enabled: bool,
    pub shaking: bool,
    pub shake_frame: u8,
    pub shake_frames: u8,
}

/// Complete `ftCommon_GrabMash` timer, axis-latch, and shake-state mutation.
pub fn mash(
    timer: &mut f32,
    state: &mut MashState,
    pressed_buttons: u32,
    stick: [f32; 2],
    penalty: f32,
    threshold: f32,
) -> bool {
    let mut result = false;
    if pressed_buttons & MASH_BUTTONS != 0 {
        *timer -= penalty;
        result = true;
    }
    let previous = state.axes;
    for (value, axis) in stick.into_iter().zip(&mut state.axes) {
        if value < -threshold {
            *axis = -1;
        }
        if value > threshold {
            *axis = 1;
        }
    }
    if state.axes != previous {
        *timer -= penalty;
        result = true;
    }
    if result && state.shake_enabled {
        state.shaking = true;
        state.shake_frame = state.shake_frame.wrapping_add(1);
        if state.shake_frame >= state.shake_frames {
            state.shake_frame = 0;
        }
    } else {
        state.shaking = false;
    }
    result
}

/// `fn_800DA4C0`: a freshly pressed A button requests CatchAttack.
pub const fn pummel_pressed(pressed_buttons: u16) -> bool {
    pressed_buttons & 0x100 != 0
}

/// Victim-weight branch retained from `ftCo_800DD4B0`. Multiplication precedes
/// division; an independent direction does not inspect either coefficient.
pub fn throw_animation_rate(independent: bool, victim_weight: f32, scale: f32) -> f32 {
    if independent {
        1.0
    } else {
        1.0 / (victim_weight * scale)
    }
}

/// Matrix lookups excluded, this is complete `fn_800DAD18`: compute the three
/// bone-alignment deltas, test its strict scaled-height branch, then translate
/// the captured fighter in the source operation order.
pub fn capture_alignment(
    mut position: [f32; 3],
    holder_anchor: [f32; 3],
    victim_anchor: [f32; 3],
    height_threshold: f32,
    model_scale_y: f32,
) -> ([f32; 3], bool) {
    let delta = [
        holder_anchor[0] - victim_anchor[0],
        holder_anchor[1] - victim_anchor[1],
        holder_anchor[2] - victim_anchor[2],
    ];
    let lifted = delta[1] > height_threshold * model_scale_y;
    position[0] += delta[0];
    position[1] += delta[1];
    position[2] += delta[2];
    (position, lifted)
}

/// `ftCo_800DA824`: the real grab-escape capture timer, driven by a live
/// match's standing and handicap rather than the flattened
/// `EscapeRules::timer_base` path (`docs/grab-escape-timer.md`). `base`,
/// `handicap_scale`, `handicap_max`, `rank_scale`, `rank_max` and
/// `percent_scale` are the six `ftCommonData` constants (`x354`, `x358`,
/// `x35C`, `x360`, `x364`, `x368`); keep this exact f32 evaluation order:
/// `slot = standing + 1`; `value = rank_max - slot`; `value = rank_scale *
/// value`; `temp = handicap_max - handicap`; `temp = handicap_scale * temp +
/// base`; `temp += value`; `return percent * percent_scale + temp`.
#[allow(clippy::too_many_arguments)]
pub fn escape_timer(
    base: f32,
    handicap_scale: f32,
    handicap_max: f32,
    rank_scale: f32,
    rank_max: f32,
    percent_scale: f32,
    percent: f32,
    standing: u8,
    handicap: u8,
) -> f32 {
    let slot = standing as f32 + 1.0;
    let value = rank_max - slot;
    let value = rank_scale * value;
    let handicap = handicap as f32;
    let temp = handicap_max - handicap;
    let temp = handicap_scale * temp + base;
    let temp = temp + value;
    percent * percent_scale + temp
}

/// A main-stick horizontal threshold crossing has priority over vertical throws.
pub fn fresh_horizontal(current: f32, previous: f32, threshold: f32) -> bool {
    (previous < threshold && current >= threshold)
        || (previous > -threshold && current <= -threshold)
}

/// Up throw uses a positive threshold crossing.
pub fn fresh_up(current: f32, previous: f32, threshold: f32) -> bool {
    previous < threshold && current >= threshold
}

/// Down throw's common-data threshold is signed and normally negative.
pub fn fresh_down(current: f32, previous: f32, threshold: f32) -> bool {
    previous > threshold && current <= threshold
}

/// Complete `ftCo_Catch_CheckInput` input branch after its item and tether
/// gates: a fresh logical A press while the logical shoulder is held.
/// `Fighter_procInput` folds physical Z into both of those logical bits.
pub fn shield_grab(shoulder_held: bool, a_pressed: bool) -> bool {
    shoulder_held && a_pressed
}

/// Complete `ftCo_800D8B9C`: a fresh logical A press inside the guard `x24`
/// dash-grab buffer starts CatchDash; otherwise the buffer counts down.
pub fn dash_shield_grab(a_pressed: bool, buffer: &mut f32) -> bool {
    if a_pressed && *buffer != 0.0 {
        return true;
    }
    if *buffer != 0.0 {
        *buffer -= 1.0;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowDirection {
    Forward,
    Backward,
    Up,
    Down,
}

/// `ftCo_800DD1E4` direction priority for main and C-stick crossings.
pub fn direction(
    current: [f32; 2],
    previous: [f32; 2],
    ccurrent: [f32; 2],
    cprevious: [f32; 2],
    facing: f32,
    thresholds: [f32; 3],
) -> Option<ThrowDirection> {
    let [horizontal, up, down] = thresholds;
    if fresh_horizontal(current[0], previous[0], horizontal) {
        Some(if current[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_horizontal(ccurrent[0], cprevious[0], horizontal) {
        Some(if ccurrent[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_up(current[1], previous[1], up) || fresh_up(ccurrent[1], cprevious[1], up) {
        Some(ThrowDirection::Up)
    } else if fresh_down(current[1], previous[1], down)
        || fresh_down(ccurrent[1], cprevious[1], down)
    {
        Some(ThrowDirection::Down)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_grab_needs_both_logical_inputs() {
        assert!(shield_grab(true, true));
        assert!(!shield_grab(false, true));
        assert!(!shield_grab(true, false));
    }

    #[test]
    fn dash_shield_grab_counts_down_only_while_armed() {
        let mut buffer = 2.0;
        assert!(!dash_shield_grab(false, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(dash_shield_grab(true, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(!dash_shield_grab(false, &mut buffer));
        assert_eq!(buffer, 0.0);
        assert!(!dash_shield_grab(true, &mut buffer));
        assert_eq!(buffer, 0.0);
        let mut negative = -0.5;
        assert!(dash_shield_grab(true, &mut negative));
        assert!(!dash_shield_grab(false, &mut negative));
        assert_eq!(negative, -1.5);
    }

    #[test]
    fn escape_timer_at_the_real_fox_constants_and_replay_settings_matches_the_flattened_value() {
        // Fox's real `ftCommonData` constants (`x354..x368`), extracted to
        // `/mnt/archive/datasets/melee/skirmish-gameplay/v2/rules.json`'s
        // `grab.escape_formula`: base 30.0, handicap_scale 8.0, handicap_max
        // 9.0, rank_scale 15.0, rank_max 4.0, percent_scale 1.6.
        let (base, handicap_scale, handicap_max, rank_scale, rank_max, percent_scale) =
            (30.0, 8.0, 9.0, 15.0, 4.0, 1.6);
        // The parity replay's settings: handicap rule off (9), best
        // standing (0). Derivation, in the source's own order: `slot =
        // standing + 1 = 1.0`; `value = rank_scale * (rank_max - slot) =
        // 15.0 * (4.0 - 1.0) = 45.0`; `temp = handicap_scale * (handicap_max
        // - handicap) + base = 8.0 * (9.0 - 9.0) + 30.0 = 30.0`; `temp +=
        // value = 75.0`; at percent 0, the timer is exactly `temp`.
        let at_zero_percent = escape_timer(
            base,
            handicap_scale,
            handicap_max,
            rank_scale,
            rank_max,
            percent_scale,
            0.0,
            0,
            9,
        );
        assert_eq!(at_zero_percent, 75.0);
        // This is exactly the flattened `timer_base` the same exporter
        // wrote for Fox (`docs/grab-escape-timer.md`), and `percent_scale`
        // is exactly the flattened `timer_percent_scale`, so the two paths
        // agree at these settings for every percent, not only zero.
        let at_forty_percent = escape_timer(
            base,
            handicap_scale,
            handicap_max,
            rank_scale,
            rank_max,
            percent_scale,
            40.0,
            0,
            9,
        );
        assert_eq!(at_forty_percent, 75.0 + 40.0 * 1.6);
    }

    #[test]
    fn threshold_crossings_retain_source_strictness() {
        assert!(fresh_horizontal(0.7, 0.69, 0.7));
        assert!(!fresh_horizontal(0.7, 0.7, 0.7));
        assert!(fresh_horizontal(-0.7, -0.69, 0.7));
        assert!(fresh_up(0.6, 0.59, 0.6));
        assert!(!fresh_up(0.6, 0.6, 0.6));
        assert!(fresh_down(-0.6, -0.59, -0.6));
        assert!(!fresh_down(-0.6, -0.6, -0.6));
    }

    #[test]
    fn pummel_uses_only_the_fresh_a_bit() {
        assert!(!pummel_pressed(0));
        assert!(!pummel_pressed(0x200));
        assert!(pummel_pressed(0x100));
        assert!(pummel_pressed(0x110));
    }

    #[test]
    fn throw_rate_preserves_the_source_branch_and_operation_order() {
        assert_eq!(throw_animation_rate(true, f32::NAN, f32::NAN), 1.0);
        let weight = f32::from_bits(0x5104_7A75);
        let scale = f32::from_bits(0x2F2F_5F4B);
        assert_eq!(
            throw_animation_rate(false, weight, scale).to_bits(),
            0x3E34_8831
        );
        assert_eq!(((1.0 / weight) / scale).to_bits(), 0x3E34_8830);
    }

    #[test]
    fn capture_alignment_uses_the_source_strict_scaled_height_branch() {
        assert_eq!(
            capture_alignment([1.0, 2.0, 3.0], [5.0, 6.0, 7.0], [2.0, 4.0, 6.0], 1.0, 2.0,),
            ([4.0, 4.0, 4.0], false),
        );
        assert!(capture_alignment([0.0; 3], [0.0, 2.000_000_2, 0.0], [0.0; 3], 1.0, 2.0).1);
        assert!(!capture_alignment([0.0; 3], [0.0; 3], [0.0; 3], f32::NAN, 1.0).1);
    }

    #[test]
    fn grab_mash_preserves_strict_latches_double_penalty_and_shake_wrap() {
        let mut timer = 10.0;
        let mut state = MashState {
            shake_enabled: true,
            shake_frame: 1,
            shake_frames: 3,
            ..Default::default()
        };
        assert!(!mash(&mut timer, &mut state, 0, [0.5, -0.5], 2.0, 0.5));
        assert_eq!(timer, 10.0);
        assert!(mash(&mut timer, &mut state, 0x100, [0.6, 0.0], 2.0, 0.5));
        assert_eq!(timer, 6.0);
        assert_eq!(state.axes, [1, 0]);
        assert_eq!(state.shake_frame, 2);
        assert!(!mash(&mut timer, &mut state, 0, [0.0; 2], 2.0, 0.5));
        assert_eq!(state.axes, [1, 0]);
        assert!(!state.shaking);
        assert!(mash(&mut timer, &mut state, 1 << 31, [-0.6, 0.6], 2.0, 0.5));
        assert_eq!(timer, 2.0);
        assert_eq!(state.axes, [-1, 1]);
        assert_eq!(state.shake_frame, 0);
    }

    #[test]
    fn horizontal_main_and_cstick_precede_vertical_selection() {
        let thresholds = [0.7, 0.6, -0.6];
        assert_eq!(
            direction(
                [-0.7, 0.8],
                [0.0; 2],
                [0.8, 0.8],
                [0.0; 2],
                -1.0,
                thresholds,
            ),
            Some(ThrowDirection::Forward)
        );
        assert_eq!(
            direction([0.0, -0.7], [0.0; 2], [0.8, 0.8], [0.0; 2], 1.0, thresholds,),
            Some(ThrowDirection::Forward)
        );
    }
}
