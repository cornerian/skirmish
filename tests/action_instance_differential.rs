//! Original fighter action-instance transitions over arbitrary retained state.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::{action_instance, instance::Counter};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_action_transition(state: *mut u16, identity: u8);
    fn oracle_action_restart(state: *mut u16);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn transitions_match_original_c(
        previous in any::<u8>(), id in any::<u16>(),
        next in 1_u16..=u16::MAX, identity in any::<u8>(),
    ) {
        let mut expected = [u16::from(previous), id, next];
        // SAFETY: the adapter reads and writes exactly three initialized halves.
        unsafe { oracle_action_transition(expected.as_mut_ptr(), identity) };
        let mut actual = action_instance::State::default();
        actual.motion_identity = previous;
        actual.id = id;
        let mut counter = Counter::from_next(next).unwrap();
        action_instance::queue(&mut actual, identity);
        action_instance::flush(&mut actual, &mut counter);
        prop_assert_eq!(
            [u16::from(actual.motion_identity), actual.id, counter.next_value()],
            expected,
        );
    }

    #[test]
    fn explicit_restarts_match_original_c(id in any::<u16>(), next in 1_u16..=u16::MAX) {
        let mut expected = [id, next];
        // SAFETY: the adapter reads and writes exactly two initialized halves.
        unsafe { oracle_action_restart(expected.as_mut_ptr()) };
        let mut actual = action_instance::State::default();
        actual.id = id;
        let mut counter = Counter::from_next(next).unwrap();
        action_instance::restart(&mut actual, &mut counter);
        prop_assert_eq!([actual.id, counter.next_value()], expected);
    }
}
