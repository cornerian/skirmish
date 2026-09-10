//! Pinned original retained-combo and floor-push branches over arbitrary data.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::combo::{self, Rules, State};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_combo_record(state: *mut u16, attack_id: u16, push_count: i32, push_frames: u32);
    fn oracle_combo_update(state: *mut u16, victim_hitstun: i32, victim_escape: u16);
    fn oracle_combo_push(
        state: *mut u16,
        position: *mut f32,
        facing: f32,
        normal: *const f32,
        grounded: i32,
        holding: i32,
        strong_count: i32,
        distances: *const f32,
    );
}

fn rules(push_count: i32, strong_push_count: i32, push_frames: u32, distances: [f32; 2]) -> Rules {
    Rules {
        push_count,
        strong_push_count,
        escape_frames: 0,
        push_distance: distances,
        push_frames,
    }
}

fn retained(state: &State) -> u16 {
    match state.victim {
        None => 0,
        Some(1) => 1,
        Some(_) => 2,
    }
}

fn exact(actual: f32, expected: f32) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

fn record_case(state: &mut State, attack_id: u16, push_count: i32, push_frames: u32) {
    let mut expected = [
        state.last_attack_landed,
        state.count,
        state.push_timer,
        retained(state),
    ];
    // SAFETY: four initialized writable halves; the adapter uses only scalar
    // values and stack-local pointer identities.
    unsafe { oracle_combo_record(expected.as_mut_ptr(), attack_id, push_count, push_frames) };
    combo::record(
        state,
        1,
        attack_id,
        &rules(push_count, push_count, push_frames, [0.0; 2]),
    );
    assert_eq!(
        [
            state.last_attack_landed,
            state.count,
            state.push_timer,
            retained(state),
        ],
        expected
    );
}

#[test]
fn curated_pointer_attack_and_push_boundaries_match() {
    for victim in [None, Some(1), Some(2)] {
        for attack_id in [0, 1, 7, u16::MAX] {
            for (push_count, push_frames) in [
                (i32::MIN, 0),
                (-1, u32::MAX),
                (0, u32::from(u16::MAX)),
                (1, u32::from(u16::MAX) + 1),
                (i32::MAX, u32::MAX),
            ] {
                let mut state = State {
                    last_attack_landed: 7,
                    count: u16::MAX,
                    victim,
                    ..State::default()
                };
                record_case(&mut state, attack_id, push_count, push_frames);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn generated_states_match_original_c(
        last in any::<u16>(), count in any::<u16>(), push_timer in any::<u16>(),
        retained_kind in 0_u8..3, attack_id in any::<u16>(), push_count in any::<i32>(),
        push_frames in any::<u32>(), escape in any::<u16>(), victim_escape in any::<u16>(),
        victim_hitstun in any::<bool>(), grounded in any::<bool>(), holding in any::<bool>(),
        strong_count in any::<i32>(), position_bits in any::<[u32; 2]>(),
        facing_bits in any::<u32>(), normal_bits in any::<[u32; 2]>(),
        distance_bits in any::<[u32; 2]>(),
    ) {
        let victim = match retained_kind { 0 => None, 1 => Some(1), _ => Some(2) };
        let mut state = State {
            last_attack_landed: last,
            count,
            victim,
            escape_timer: escape,
            push_timer,
            last_hit_by: None,
        };
        record_case(&mut state, attack_id, push_count, push_frames);

        let mut update_expected = [state.escape_timer, u16::from(state.victim.is_some())];
        // SAFETY: two initialized writable halves and scalar victim state.
        unsafe {
            oracle_combo_update(
                update_expected.as_mut_ptr(),
                i32::from(victim_hitstun),
                victim_escape,
            )
        };
        combo::update_retention(&mut state, victim_hitstun, victim_escape);
        prop_assert_eq!(state.escape_timer, update_expected[0]);
        prop_assert_eq!(u16::from(state.victim.is_some()), update_expected[1]);

        let mut expected_state = [state.count, state.push_timer];
        let mut expected_position = position_bits.map(f32::from_bits);
        let mut actual_position = expected_position;
        let facing = f32::from_bits(facing_bits);
        let normal = normal_bits.map(f32::from_bits);
        let distances = distance_bits.map(f32::from_bits);
        // SAFETY: both arrays contain two initialized elements; the adapter
        // writes only the state and position arrays.
        unsafe {
            oracle_combo_push(
                expected_state.as_mut_ptr(),
                expected_position.as_mut_ptr(),
                facing,
                normal.as_ptr(),
                i32::from(grounded),
                i32::from(holding),
                strong_count,
                distances.as_ptr(),
            )
        };
        combo::push(
            &mut state,
            &mut actual_position,
            facing,
            normal,
            grounded,
            holding,
            &rules(1, strong_count, 1, distances),
        );
        prop_assert_eq!([state.count, state.push_timer], expected_state);
        exact(actual_position[0], expected_position[0]);
        exact(actual_position[1], expected_position[1]);
    }
}
