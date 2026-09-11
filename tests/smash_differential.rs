//! Smash input predicates, the forward-smash facing and angle selection, the
//! windowless KneeBend up smash and the complete charge state machine checked
//! against the pinned C bodies.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::aerial::stick_angle;
use skirmish::fighter::grab::{fresh_down, fresh_up};
use skirmish::fighter::smash::{
    ChargeState, charge_damage, charge_input, charge_tick, down_smash_input, forward_smash_input,
    fresh_cstick_smash_x, stick_sign, up_smash_input,
};
use skirmish::fighter::tilt::{ForwardVariant, forward_variant};

const HSD_PAD_A: u32 = 0x100;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_forward_smash(
        values: *const f32,
        rules: *const f32,
        pressed: u32,
        tilt_x_age: u32,
        available: u32,
        motion: *mut i32,
        facing: *mut f32,
    ) -> i32;
    fn oracle_up_smash(
        values: *const f32,
        rules: *const f32,
        pressed: u32,
        tilt_y_age: u32,
        motion: *mut i32,
    ) -> i32;
    fn oracle_up_smash_no_window(
        values: *const f32,
        rules: *const f32,
        pressed: u32,
        tilt_y_age: u32,
        motion: *mut i32,
    ) -> i32;
    fn oracle_down_smash(
        values: *const f32,
        rules: *const f32,
        pressed: u32,
        tilt_y_age: u32,
        motion: *mut i32,
    ) -> i32;
    fn oracle_charge_lifecycle(
        hold: f32,
        mul: f32,
        rate: f32,
        held: u32,
        frames: i32,
        damage: f32,
        states: *mut i32,
        frames_out: *mut f32,
        rate_sets: *mut i32,
        last_rate: *mut f32,
    ) -> f32;
    fn oracle_charge_input(state: i32, held: u32, rate: f32, rate_out: *mut f32) -> i32;
    fn oracle_charge_tick(state: i32, frames: f32, hold: f32, frames_out: *mut f32) -> i32;
    fn oracle_charge_damage(state: i32, damage: f32, frames: f32, hold: f32, mul: f32) -> f32;
    fn oracle_charge_clear() -> i32;
}

fn state_code(state: ChargeState) -> i32 {
    match state {
        ChargeState::None => 0,
        ChargeState::PreCharge => 1,
        ChargeState::Charging => 2,
        ChargeState::Release => 3,
    }
}

fn state_from(code: i32) -> ChargeState {
    match code {
        0 => ChargeState::None,
        1 => ChargeState::PreCharge,
        2 => ChargeState::Charging,
        3 => ChargeState::Release,
        other => panic!("unexpected smash state {other}"),
    }
}

/// sticks: main stick, C-stick, previous C-stick; rules: dash threshold,
/// dash window, xB8, xBC, xC0, xC4.
fn compare_forward(
    [stick, cstick, previous]: [[f32; 2]; 3],
    facing: f32,
    rules: [f32; 6],
    pressed: u32,
    age: u8,
    available: u32,
) {
    let values = [
        stick[0],
        stick[1],
        cstick[0],
        cstick[1],
        previous[0],
        previous[1],
        facing,
    ];
    let (mut motion, mut facing_after) = (0, 0.0);
    // SAFETY: the adapter reads seven and six floats and writes one integer
    // and one float.
    let expected = unsafe {
        oracle_forward_smash(
            values.as_ptr(),
            rules.as_ptr(),
            pressed,
            u32::from(age),
            available,
            &mut motion,
            &mut facing_after,
        )
    };
    let a = pressed & HSD_PAD_A != 0;
    let selected = if forward_smash_input(a, stick[0], age, rules[0], rules[1] as u8) {
        Some((stick_sign(stick[0]), stick_angle(stick)))
    } else if fresh_cstick_smash_x(previous[0], cstick[0], rules[0]) {
        Some((stick_sign(cstick[0]), stick_angle(cstick)))
    } else {
        None
    };
    assert_eq!(
        selected.is_some(),
        expected != 0,
        "{stick:?} {cstick:?} {previous:?}"
    );
    let Some((sign, angle)) = selected else {
        assert_eq!(motion, 0);
        assert_eq!(facing_after.to_bits(), facing.to_bits());
        return;
    };
    assert_eq!(
        facing_after.to_bits(),
        sign.to_bits(),
        "{stick:?} {cstick:?}"
    );
    let expected_motion = match forward_variant(
        angle,
        [rules[2], rules[3], rules[4], rules[5]],
        [
            available & 1 != 0,
            available & 2 != 0,
            available & 4 != 0,
            available & 8 != 0,
        ],
    ) {
        ForwardVariant::High => 58,
        ForwardVariant::HighSlight => 59,
        ForwardVariant::Straight => 60,
        ForwardVariant::LowSlight => 61,
        ForwardVariant::Low => 62,
    };
    assert_eq!(
        motion, expected_motion,
        "{stick:?} {cstick:?} {rules:?} {available}"
    );
}

/// values: stick y, C-stick y, previous C-stick y; rules: threshold, window.
fn compare_vertical(values: [f32; 3], threshold: f32, window: f32, pressed: u32, age: u8) {
    let up_rules = [threshold, window];
    let down_rules = [-threshold, window];
    let (mut up_motion, mut no_window_motion, mut down_motion) = (0, 0, 0);
    // SAFETY: the adapters read three and two floats and write one integer.
    let (up, no_window, down) = unsafe {
        (
            oracle_up_smash(
                values.as_ptr(),
                up_rules.as_ptr(),
                pressed,
                u32::from(age),
                &mut up_motion,
            ),
            oracle_up_smash_no_window(
                values.as_ptr(),
                up_rules.as_ptr(),
                pressed,
                u32::from(age),
                &mut no_window_motion,
            ),
            oracle_down_smash(
                values.as_ptr(),
                down_rules.as_ptr(),
                pressed,
                u32::from(age),
                &mut down_motion,
            ),
        )
    };
    let a = pressed & HSD_PAD_A != 0;
    let [stick_y, cstick_y, previous_y] = values;
    let up_actual = up_smash_input(a, stick_y, age, threshold, window)
        || fresh_up(cstick_y, previous_y, threshold);
    assert_eq!(up_actual, up != 0, "{values:?} {threshold} {window} {age}");
    assert_eq!(up_motion, if up != 0 { 63 } else { 0 });
    let no_window_actual = (a && stick_y >= threshold) || fresh_up(cstick_y, previous_y, threshold);
    assert_eq!(
        no_window_actual,
        no_window != 0,
        "{values:?} {threshold} {age}"
    );
    assert_eq!(no_window_motion, if no_window != 0 { 63 } else { 0 });
    let down_actual = down_smash_input(a, stick_y, age, -threshold, window)
        || fresh_down(cstick_y, previous_y, -threshold);
    assert_eq!(
        down_actual,
        down != 0,
        "{values:?} {threshold} {window} {age}"
    );
    assert_eq!(down_motion, if down != 0 { 64 } else { 0 });
}

fn compare_lifecycle(hold: f32, mul: f32, rate: f32, held: u32, frames: usize, damage: f32) {
    let mut states = vec![0; frames];
    let (mut frames_out, mut rate_sets, mut last_rate) = (0.0, 0, 0.0);
    // SAFETY: the adapter writes `frames` integers and three scalars.
    let expected = unsafe {
        oracle_charge_lifecycle(
            hold,
            mul,
            rate,
            held,
            frames as i32,
            damage,
            states.as_mut_ptr(),
            &mut frames_out,
            &mut rate_sets,
            &mut last_rate,
        )
    };
    let mut state = ChargeState::PreCharge;
    let mut counted = 0.0;
    let mut sets = 0;
    let mut rate_after = -1.0;
    for (frame, &expected_state) in states.iter().enumerate() {
        let before = state;
        state = charge_tick(state, &mut counted, hold);
        if before == ChargeState::Charging && state == ChargeState::Release {
            sets += 1;
            rate_after = rate;
        }
        let a_held = held >> frame & 1 != 0;
        let before = state;
        state = charge_input(state, a_held);
        match (before, state) {
            (ChargeState::PreCharge, ChargeState::Charging) => {
                sets += 1;
                rate_after = 0.0;
            }
            (ChargeState::Charging, ChargeState::Release) => {
                sets += 1;
                rate_after = rate;
            }
            _ => {}
        }
        assert_eq!(
            state,
            state_from(expected_state),
            "frame {frame} held {held:#b}"
        );
    }
    assert_eq!(counted.to_bits(), frames_out.to_bits(), "held {held:#b}");
    assert_eq!(sets, rate_sets, "held {held:#b}");
    assert_eq!(rate_after.to_bits(), last_rate.to_bits(), "held {held:#b}");
    let actual = charge_damage(damage, state, counted, hold, mul);
    assert_eq!(actual.to_bits(), expected.to_bits(), "held {held:#b}");
}

fn compare_input(state: ChargeState, held: u32, rate: f32) {
    let mut rate_out = 0.0;
    // SAFETY: the adapter writes one float.
    let expected = unsafe { oracle_charge_input(state_code(state), held, rate, &mut rate_out) };
    let actual = charge_input(state, held & HSD_PAD_A != 0);
    assert_eq!(actual, state_from(expected), "{state:?} {held:#x}");
    let expected_rate = match (state, actual) {
        (ChargeState::PreCharge, ChargeState::Charging) => 0.0,
        (ChargeState::Charging, ChargeState::Release) => rate,
        _ => -1.0,
    };
    assert_eq!(
        rate_out.to_bits(),
        expected_rate.to_bits(),
        "{state:?} {held:#x}"
    );
}

fn compare_tick(state: ChargeState, frames: f32, hold: f32) {
    let mut frames_out = 0.0;
    // SAFETY: the adapter writes one float.
    let expected = unsafe { oracle_charge_tick(state_code(state), frames, hold, &mut frames_out) };
    let mut counted = frames;
    let actual = charge_tick(state, &mut counted, hold);
    assert_eq!(actual, state_from(expected), "{state:?} {frames} {hold}");
    assert_eq!(
        counted.to_bits(),
        frames_out.to_bits(),
        "{state:?} {frames} {hold}"
    );
}

fn compare_damage(state: ChargeState, damage: f32, frames: f32, hold: f32, mul: f32) {
    // SAFETY: the adapter takes scalars only.
    let expected = unsafe { oracle_charge_damage(state_code(state), damage, frames, hold, mul) };
    let actual = charge_damage(damage, state, frames, hold, mul);
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{state:?} {damage} {frames} {hold} {mul}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_forward_inputs_match(
        stick in prop::array::uniform2(-1.0_f32..=1.0),
        cstick in prop::array::uniform2(-1.0_f32..=1.0),
        previous in prop::array::uniform2(-1.0_f32..=1.0),
        facing in prop_oneof![Just(1.0_f32), Just(-1.0_f32)],
        threshold in 0.0_f32..=1.0,
        window in 0_u8..8,
        angles in prop::array::uniform4(-1.6_f32..=1.6),
        pressed in any::<u32>(),
        age in 0_u8..8,
        available in 0_u32..16,
    ) {
        let rules = [threshold, f32::from(window), angles[0], angles[1], angles[2], angles[3]];
        compare_forward([stick, cstick, previous], facing, rules, pressed, age, available);
    }

    #[test]
    fn arbitrary_vertical_inputs_match(
        values in prop::array::uniform3(-1.0_f32..=1.0),
        threshold in 0.0_f32..=1.0,
        window in 0.0_f32..=8.0,
        pressed in any::<u32>(),
        age in 0_u8..8,
    ) {
        compare_vertical(values, threshold, window, pressed, age);
    }

    #[test]
    fn arbitrary_charge_lifecycles_match(
        hold in 1.0_f32..=12.0,
        mul in 0.0_f32..=3.0,
        rate in 0.5_f32..=2.0,
        held in any::<u32>(),
        frames in 1_usize..=20,
        damage in 0.0_f32..=40.0,
    ) {
        compare_lifecycle(hold, mul, rate, held, frames, damage);
    }

    #[test]
    fn arbitrary_charge_steps_match(
        state in prop_oneof![
            Just(ChargeState::None),
            Just(ChargeState::PreCharge),
            Just(ChargeState::Charging),
            Just(ChargeState::Release),
        ],
        held in any::<u32>(),
        rate in 0.0_f32..=2.0,
        frames in 0.0_f32..=70.0,
        hold in 0.5_f32..=70.0,
        damage in 0.0_f32..=40.0,
        mul in 0.0_f32..=3.0,
    ) {
        compare_input(state, held, rate);
        compare_tick(state, frames, hold);
        compare_damage(state, damage, frames, hold, mul);
    }
}

#[test]
fn adapters_retain_the_complete_source_functions_and_boundaries() {
    let s4 = include_str!("oracle/original/attack_s4.c");
    let hi4 = include_str!("oracle/original/attack_hi4.c");
    let lw4 = include_str!("oracle/original/attack_lw4.c");
    let charge = include_str!("oracle/original/smash_charge.c");
    assert!(s4.contains("bool ftCo_AttackS4_CheckInput(Fighter_GObj* gobj)"));
    assert!(
        s4.contains("void decideFighter(HSD_GObj* gobj, float stick_x_sign, float stick_angle)")
    );
    assert!(hi4.contains("bool ftCo_AttackHi4_CheckInputNoD0(HSD_GObj* gobj)"));
    assert!(lw4.contains("bool ftCo_AttackLw4_CheckInput(Fighter_GObj* gobj)"));
    assert!(charge.contains("void ftCo_800DF0D0(Fighter_GObj* gobj)"));
    assert!(charge.contains("f32 ftCo_800DEEB8(Fighter* fp, f32 arg1)"));
    let rules = [0.8, 4.0, 0.4, 0.2, -0.2, -0.4];
    let neutral = [0.0, 0.0];
    for (stick, age) in [
        ([0.8, 0.0], 0),
        ([0.79, 0.0], 0),
        ([-0.8, 0.0], 3),
        ([1.0, 0.0], 4),
        ([1.0, 0.5], 0),
        ([1.0, 0.3], 0),
        ([1.0, -0.3], 0),
        ([1.0, -0.5], 0),
        ([-0.0, 0.0], 0),
        ([f32::NAN, 0.0], 0),
        ([-1.0, f32::NAN], 0),
    ] {
        for available in [15, 0, 14, 7] {
            compare_forward(
                [stick, neutral, neutral],
                1.0,
                rules,
                HSD_PAD_A,
                age,
                available,
            );
            compare_forward([stick, neutral, neutral], -1.0, rules, 0, age, available);
        }
    }
    // C-stick crossings: both frames use absolute values, and a crossing
    // while A is pressed with a stale main stick still smashes.
    for (cstick, previous, pressed) in [
        ([0.8, 0.0], [0.0, 0.0], 0),
        ([-0.8, 0.5], [0.79, 0.0], 0),
        ([0.8, 0.0], [-0.8, 0.0], 0),
        ([0.79, 0.0], [0.0, 0.0], 0),
        ([1.0, -0.5], [0.0, 0.0], HSD_PAD_A),
        ([-0.0, 0.0], [0.0, 0.0], 0),
    ] {
        compare_forward([[1.0, 0.0], cstick, previous], 1.0, rules, pressed, 7, 15);
    }
    for values in [
        [0.7, 0.0, 0.0],
        [0.69, 0.0, 0.0],
        [-0.7, 0.0, 0.0],
        [0.0, 0.7, 0.69],
        [0.0, 0.7, 0.7],
        [0.0, -0.7, -0.69],
        [0.0, -0.7, -0.7],
        [f32::NAN, f32::NAN, 0.0],
    ] {
        for age in [0, 3, 4, 200] {
            compare_vertical(values, 0.7, 4.0, HSD_PAD_A, age);
            compare_vertical(values, 0.7, 4.0, 0, age);
        }
        compare_vertical(values, 0.7, 4.5, HSD_PAD_A, 4);
    }
    // Tap, partial, full and over-held charges.
    compare_lifecycle(10.0, 1.5, 1.0, 0, 6, 10.0);
    compare_lifecycle(10.0, 1.5, 1.0, 0b1111, 8, 10.0);
    compare_lifecycle(10.0, 1.5, 1.0, u32::MAX, 14, 10.0);
    compare_lifecycle(2.5, 1.5, 1.0, u32::MAX, 6, 10.0);
    compare_tick(ChargeState::Charging, 9.0, 10.0);
    compare_tick(ChargeState::Charging, 9.5, 10.0);
    compare_tick(ChargeState::Charging, 10.0, 10.0);
    compare_tick(ChargeState::Release, 10.0, 10.0);
    compare_damage(ChargeState::Release, 10.0, 3.0, 10.0, 1.5);
    compare_damage(ChargeState::Charging, 10.0, 3.0, 10.0, 1.5);
    compare_damage(ChargeState::Release, 10.0, 0.0, 10.0, 1.5);
    compare_input(ChargeState::PreCharge, HSD_PAD_A, 1.0);
    compare_input(ChargeState::PreCharge, 0, 1.0);
    compare_input(ChargeState::Charging, 0, 1.0);
    compare_input(ChargeState::Release, 0, 1.0);
    // SAFETY: the adapter takes no arguments.
    assert_eq!(
        unsafe { oracle_charge_clear() },
        state_code(ChargeState::None)
    );
}
