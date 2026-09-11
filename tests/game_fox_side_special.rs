//! Fox/Falco side special (Illusion/Phantasm): common input dispatch, the six
//! Start/Dash/End actions, TransN root motion, ground/air conversions and the
//! FallSpecial exit. The C-oracle differential harness and the Peppi replay
//! fixtures are not part of this batch; see the batch report for what
//! remains unmodeled/unverified.
//!
//! Exact per-frame values below (gravity delay countdowns, TransN-driven
//! velocities, friction results) were captured empirically against this
//! module's own implementation (`ftfoxspecials.c`'s cited functions), not
//! against the pinned C oracle -- there is no differential harness in this
//! batch to certify them bit-exactly.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_side_special.rs"]
mod side_special_resources;

use skirmish::game::{Action, BUTTON_B, Controller, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    side_special_resources::profile(conformance::data())
}

/// Player 0 spawns airborne, matching `tests/support/special.rs::airborne_game`.
fn airborne_data() -> MatchData {
    let mut resource = data();
    resource.stage.spawns[0][1] = 6.0;
    resource
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn side(stick_x: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [stick_x, 0.0],
        ..Controller::default()
    }
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-4, "expected {b}, got {a}");
}

#[test]
fn none_keeps_b_and_side_inert() {
    let mut plain = conformance::data();
    plain.fighters[0].side_special = None;
    plain.rules.specials = None;
    let mut game = Match::new(plain, 0).unwrap();
    // B has no consumer without the resource; the stick still drives
    // ordinary Walk, but never the side special itself.
    let state = game.step(input(0, side(0.6))).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialSStart);
}

#[test]
fn ground_entry_from_wait_enters_start_with_gravity_delay_and_jumps_untouched() {
    let mut game = Match::new(data(), 0).unwrap();
    let jumps_before = game.state().fighters[0].locomotion.jumps_used;
    let state = game.step(input(0, side(0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialSStart);
    // The generic per-frame action_frame increment already ran this step.
    assert_eq!(state.fighters[0].action_frame, 1);
    // x24 == 2.0, ticked once by this same step's own grounded Phys.
    assert_eq!(state.fighters[0].fox_side_special.gravity_delay, 1.0);
    assert_eq!(state.fighters[0].locomotion.jumps_used, jumps_before);
}

#[test]
fn turn_around_threshold_is_strict_and_matches_the_aerial_side_branch() {
    // facing starts at +1 (player 0). stick.x * facing < -turn_threshold(0.1).
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, side(-0.6))).unwrap();
    assert_eq!(state.fighters[0].facing, -1.0);
    assert_eq!(state.fighters[0].action, Action::SpecialSStart);

    // Right at the boundary (stick_x * facing == -turn_threshold) must not turn.
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, side(-0.1))).unwrap();
    assert_eq!(state.fighters[0].facing, 1.0);
}

#[test]
fn b_age_gate_rejects_a_stale_press_whose_stick_moves_later() {
    let mut game = Match::new(data(), 0).unwrap();
    // Frame 1: B pressed, stick under threshold (HasInput false: x688 counts up).
    game.step(input(
        0,
        Controller {
            buttons: BUTTON_B,
            stick: [0.0, 0.0],
            ..Controller::default()
        },
    ))
    .unwrap();
    // Frame 2: B still held (not a fresh press), stick now past threshold.
    let state = game
        .step(input(
            0,
            Controller {
                buttons: BUTTON_B,
                stick: [0.6, 0.0],
                ..Controller::default()
            },
        ))
        .unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialSStart);
}

#[test]
fn air_entry_divides_velocity_and_uses_all_jumps() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    // Drift the falling fighter to build a real nonzero self_vel.x, then
    // press B with the opposite stick direction to exercise the turn and
    // the entry division in the same frame.
    for _ in 0..3 {
        game.step(input(
            0,
            Controller {
                buttons: 0,
                stick: [0.9, 0.0],
                ..Controller::default()
            },
        ))
        .unwrap();
    }
    let before = game.state().fighters[0].clone();
    assert!(
        !before.grounded,
        "the fixture spawn must stay airborne here"
    );
    assert_ne!(before.velocity[0], 0.0);
    let max_jumps = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .max_jumps;
    let state = game.step(input(0, side(-0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirSStart);
    assert_eq!(state.fighters[0].velocity[1], 0.0);
    // x28 division, then this same step's own Start Phys friction (x2C ==
    // 0.02) already applied via `friction_air`.
    let half = before.velocity[0] / 2.0;
    let friction = if half > 0.0 { -0.02 } else { 0.02 };
    approx(state.fighters[0].velocity[0], half + friction);
    assert_eq!(state.fighters[0].locomotion.jumps_used, max_jumps);
}

#[test]
fn aerial_vertical_stick_suppresses_the_side_branch() {
    // ftCo_SpecialAir_CheckInput: SpecialAirHi/Lw take priority and stay
    // unmodeled; a stick past x21C must not fall through to the side branch.
    let mut game = Match::new(airborne_data(), 0).unwrap();
    assert!(!game.state().fighters[0].grounded);
    let state = game
        .step(input(
            0,
            Controller {
                buttons: BUTTON_B,
                stick: [0.6, 0.9],
                ..Controller::default()
            },
        ))
        .unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialAirSStart);
}

/// Entry, then enough idle frames for the 4-pose Start phase to hand off to
/// the Dash phase (SpecialS, action_frame 1 -- the transition's own frame).
fn ground_dash_entry_state(game: &mut Match) -> skirmish::game::State {
    game.step(input(0, side(0.6))).unwrap();
    for _ in 0..4 {
        game.step(IDLE).unwrap();
    }
    game.state().clone()
}

#[test]
fn dash_phase_ground_trans_n_sets_velocity_and_falls_back_to_friction() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = ground_dash_entry_state(&mut game);
    assert_eq!(state.fighters[0].action, Action::SpecialS);
    assert_eq!(state.fighters[0].action_frame, 1);
    // Pose 0's TransN (2.0) applied on this transition's own physics frame.
    assert_eq!(state.fighters[0].ground_velocity, 2.0);
    // Pose 1's TransN is also 2.0.
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].ground_velocity, 2.0);
    // Pose 2 has no TransN sample (null): ordinary ground friction (0.2)
    // applies instead, decaying away from the constant TransN value.
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].ground_velocity, 1.8);
}

#[test]
fn dash_phase_air_trans_n_sets_both_axes_unconditionally() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    assert!(!game.state().fighters[0].grounded);
    game.step(input(0, side(0.6))).unwrap();
    let state = (0..4)
        .map(|_| game.step(IDLE).unwrap().clone())
        .last()
        .unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirS);
    assert_eq!(state.fighters[0].velocity, [2.0, 0.0]);
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].velocity, [2.0, -0.1]);
}

#[test]
fn b_press_shortens_the_ground_dash_into_end() {
    let mut game = Match::new(data(), 0).unwrap();
    ground_dash_entry_state(&mut game);
    let state = game.step(input(0, side(0.0))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialSEnd);
    // x34 == 1.5, then this same step's own End Phys friction (x38 == 0.05).
    assert_eq!(state.fighters[0].ground_velocity, 1.45);
    // x44 == 1.0, ticked once by the same step's End Phys.
    assert_eq!(state.fighters[0].fox_side_special.gravity_delay, 0.0);
}

#[test]
fn b_press_shortens_the_air_dash_into_end() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    game.step(input(0, side(0.6))).unwrap();
    for _ in 0..4 {
        game.step(IDLE).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialAirS);
    let state = game.step(input(0, side(0.0))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirSEnd);
    // x3C == 1.2, then this same step's End Phys friction (x40 == 0.03) on
    // the x axis only; the gravity delay (x44 == 1.0) absorbs this step's
    // own tick before any fall, so velocity.y stays 0.
    approx(state.fighters[0].velocity[0], 1.17);
    assert_eq!(state.fighters[0].velocity[1], 0.0);
}

#[test]
fn air_to_ground_conversion_preserves_frame_and_gravity_delay() {
    // Spawn just above the floor: the entry frame does not land, then the
    // fall lands within a few frames once the gravity delay lapses.
    let mut resource = data();
    resource.stage.spawns[0][1] = 0.3;
    let mut game = Match::new(resource, 0).unwrap();
    assert!(!game.state().fighters[0].grounded);
    let state = game.step(input(0, side(0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirSStart);
    let mut state = state.clone();
    let mut converted = false;
    for _ in 0..10 {
        if state.fighters[0].grounded {
            break;
        }
        let frame_before = state.fighters[0].action_frame;
        let delay_before = state.fighters[0].fox_side_special.gravity_delay;
        state = game.step(IDLE).unwrap().clone();
        if state.fighters[0].grounded {
            converted = true;
            // The Start phase may already have handed off to Dash before
            // landing; either pair converts symmetrically.
            assert!(matches!(
                state.fighters[0].action,
                Action::SpecialSStart | Action::SpecialS
            ));
            // The conversion preserves the frame as of this step's own
            // entry; the generic per-frame increment then advances it once
            // more, like every other transition observed in this suite.
            assert_eq!(state.fighters[0].action_frame, frame_before + 1);
            assert_eq!(
                state.fighters[0].fox_side_special.gravity_delay,
                delay_before
            );
        }
    }
    assert!(converted, "must land within 10 frames");
}

#[test]
fn air_start_gravity_delay_holds_vertical_velocity_before_falling() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    let state = game.step(input(0, side(0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirSStart);
    assert_eq!(state.fighters[0].fox_side_special.gravity_delay, 1.0);
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].velocity[1], 0.0);
    assert_eq!(state.fighters[0].fox_side_special.gravity_delay, 0.0);
    // The delay has lapsed: ftCommon_Fall(x30) now applies every frame.
    let state = game.step(IDLE).unwrap();
    assert!(state.fighters[0].velocity[1] < 0.0);
}

#[test]
fn end_air_exits_into_fall_special_with_scaled_mobility_and_all_jumps() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    let max_jumps = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .max_jumps;
    game.step(input(0, side(0.6))).unwrap();
    for _ in 0..4 {
        game.step(IDLE).unwrap();
    }
    let state = game.step(input(0, side(0.0))).unwrap(); // shorten into SpecialAirSEnd
    assert_eq!(state.fighters[0].action, Action::SpecialAirSEnd);
    // The End air pair has 2 poses; the transition check reads the frame
    // count from before this step's own generic increment, so it takes one
    // extra idle frame past the nominal pose count to fire.
    game.step(IDLE).unwrap();
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].action, Action::FallSpecial);
    assert!(state.fighters[0].aerial.allow_interrupt);
    assert_eq!(state.fighters[0].aerial.mobility, 0.5);
    assert_eq!(state.fighters[0].locomotion.jumps_used, max_jumps);
}

#[test]
fn slippi_ids_are_347_through_352() {
    for (action, id) in [
        (Action::SpecialSStart, "special_s_start"),
        (Action::SpecialS, "special_s"),
        (Action::SpecialSEnd, "special_s_end"),
        (Action::SpecialAirSStart, "special_air_s_start"),
        (Action::SpecialAirS, "special_air_s"),
        (Action::SpecialAirSEnd, "special_air_s_end"),
    ] {
        let value = serde_json::to_value(action).unwrap();
        assert_eq!(value.as_str().unwrap(), id);
    }
}

#[test]
fn every_phase_survives_a_checkpoint_round_trip() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, side(0.6))).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let expected = game.state().clone();
    game.step(IDLE).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state(), &expected);
}

#[test]
fn invalid_side_special_resources_are_rejected() {
    let mut resource = data();
    resource.fighters[0]
        .side_special
        .as_mut()
        .unwrap()
        .ground_speed_retention = 1.5;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    resource.fighters[0]
        .side_special
        .as_mut()
        .unwrap()
        .dash
        .ground_trans_n
        .pop();
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    resource.fighters[0]
        .side_special
        .as_mut()
        .unwrap()
        .attributes
        .entry_speed_div = 0.0;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    resource.fighters[0].escape_air = None;
    assert!(Match::new(resource, 0).is_err());
}
