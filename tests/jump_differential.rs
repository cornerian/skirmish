//! Ground jump direction test (`ftCo_Jump_Enter`, `ftCo_Jump.c:157-161`)
//! checked against the complete pinned source predicate. `ftCo_JumpAerial_
//! Enter_Basic` (`ftCo_JumpAerial.c:169-171`, `190-192`) uses the exact same
//! test, so this single oracle covers both call sites; `src/game/locomotion.rs`
//! calls the shared `fighter::locomotion::jump_backward` helper checked here
//! from both `ground_jump` and `try_aerial_jump`.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::locomotion::jump_backward;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_jump_enter(stick_x: f32, facing: f32, x78: f32) -> i32;
}

// Sentinel motion ids, mirroring `tests/oracle/jump.c`.
const MS_JUMP_F: i32 = 25;
const MS_JUMP_B: i32 = 26;

fn compare(stick_x: f32, facing: f32, x78: f32) {
    // SAFETY: the adapter accepts three scalar binary32 values and owns its
    // thread-local state.
    let msid = unsafe { oracle_jump_enter(stick_x, facing, x78) };
    assert!(
        msid == MS_JUMP_F || msid == MS_JUMP_B,
        "unexpected msid {msid} for stick_x {stick_x} facing {facing} x78 {x78}"
    );
    let expected = msid == MS_JUMP_B;
    assert_eq!(
        jump_backward(stick_x, facing, x78),
        expected,
        "stick_x {stick_x} facing {facing} x78 {x78}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_binary32_inputs_match(
        stick_x in any::<u32>(),
        facing in any::<u32>(),
        x78 in any::<u32>(),
    ) {
        compare(f32::from_bits(stick_x), f32::from_bits(facing), f32::from_bits(x78));
    }
}

#[test]
fn complete_predicate_and_exceptional_boundaries_match() {
    let source = include_str!("oracle/original/jump.c");
    let adapter = include_str!("oracle/jump.c");
    assert!(source.contains("void ftCo_Jump_Enter(Fighter_GObj* gobj)"));
    assert!(adapter.contains("ftCo_Jump_Enter(&gobj)"));
    for (stick_x, facing, x78) in [
        // Exactly at the boundary: equality is backward.
        (-0.3, 1.0, 0.3),
        (-0.3 + f32::EPSILON, 1.0, 0.3),
        (-0.3 - f32::EPSILON, 1.0, 0.3),
        // Facing negates the effective stick direction.
        (0.3, -1.0, 0.3),
        (-0.3, -1.0, 0.3),
        (f32::NAN, 1.0, 0.3),
        (1.0, f32::NAN, 0.3),
        (1.0, 1.0, f32::NAN),
        (f32::INFINITY, 1.0, 0.3),
        (f32::NEG_INFINITY, 1.0, 0.3),
        (1.0, 1.0, f32::INFINITY),
        (-0.0, 1.0, 0.0),
        (0.0, 1.0, -0.0),
        (0.0, 0.0, 0.3),
    ] {
        compare(stick_x, facing, x78);
    }
}
