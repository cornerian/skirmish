//! End-to-end Run animation rate/frame and Slippi ids (`docs/run.md`) in a
//! synthetic world built on the existing locomotion fixture, extending the
//! walk batch's model (`tests/game_walk.rs`) to Run.
#[path = "support/run.rs"]
mod run_support;

use skirmish::{
    fighter::locomotion::run_animation_rate,
    game::{Action, Controller, Match, State, data::MatchData},
};

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let p = serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(p);
    }
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.spawns = [[-40.0, 0.0], [40.0, 0.0]];
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    run_support::profile(data)
}

fn game() -> Match {
    Match::new(data(), 42).unwrap()
}

fn step(game: &mut Match, buttons: u16, stick: [f32; 2]) -> State {
    game.step([
        Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            buttons,
            stick,
        },
        Controller::default(),
    ])
    .unwrap()
    .clone()
}

/// Hold full-forward stick from Wait until `Action::Dash`'s own
/// `action_frame >= dash_run_frame` transition (`dash_run_frame` 8 in the
/// fixture) reaches Run. Returns the entry frame's state.
fn ramp_to_run(game: &mut Match) -> State {
    let mut state = step(game, 0, [1.0, 0.0]);
    assert_eq!(state.fighters[0].action, Action::Dash);
    for _ in 0..20 {
        state = step(game, 0, [1.0, 0.0]);
        if state.fighters[0].action == Action::Run {
            return state;
        }
    }
    panic!("expected to reach Run within 20 frames");
}

#[test]
fn dash_to_run_transition_enters_at_frame_zero_with_last_rate_one() {
    let mut game = game();
    let entered = ramp_to_run(&mut game);
    assert_eq!(entered.fighters[0].locomotion.run.frame, 0.0);
    assert_eq!(entered.fighters[0].locomotion.run.last_rate, 1.0);
}

#[test]
fn first_run_frame_advances_by_one_then_later_frames_match_the_previous_velocity() {
    let mut game = game();
    let mut previous = ramp_to_run(&mut game);
    // last_rate at entry is always 1.0 (ChangeMotionState's own rate=1
    // argument), so the very next Run frame advances the float frame by
    // exactly 1.0, independent of velocity.
    let mut current = step(&mut game, 0, [1.0, 0.0]);
    assert_eq!(current.fighters[0].action, Action::Run);
    assert_eq!(current.fighters[0].locomotion.run.frame, 1.0);
    for _ in 0..40 {
        if current.fighters[0].action != Action::Run {
            break;
        }
        let p = &previous.fighters[0];
        let c = &current.fighters[0];
        let expected_rate = run_animation_rate(
            p.ground_velocity,
            p.facing,
            0.0,
            run_support::ANIMATION.scaling,
            1.0,
        );
        assert_eq!(
            c.locomotion.run.last_rate.to_bits(),
            expected_rate.to_bits()
        );
        let mut expected_frame = p.locomotion.run.frame + p.locomotion.run.last_rate;
        while expected_frame >= run_support::ANIMATION.length {
            expected_frame -= run_support::ANIMATION.length;
        }
        assert_eq!(c.locomotion.run.frame.to_bits(), expected_frame.to_bits());
        previous = current.clone();
        current = step(&mut game, 0, [1.0, 0.0]);
    }
}

#[test]
fn animation_frame_wraps_at_the_length() {
    let mut game = game();
    let mut previous = ramp_to_run(&mut game);
    let mut wrapped = false;
    for _ in 0..60 {
        let current = step(&mut game, 0, [1.0, 0.0]);
        if current.fighters[0].action != Action::Run {
            break;
        }
        assert!(current.fighters[0].locomotion.run.frame < run_support::ANIMATION.length);
        if current.fighters[0].locomotion.run.frame < previous.fighters[0].locomotion.run.frame {
            wrapped = true;
        }
        previous = current;
    }
    assert!(
        wrapped,
        "expected the Run animation frame to wrap at least once"
    );
}

#[test]
fn checkpoints_restore_the_run_frame_and_last_rate_exactly() {
    let mut game = game();
    ramp_to_run(&mut game);
    for _ in 0..3 {
        step(&mut game, 0, [1.0, 0.0]);
    }
    let checkpoint = game.checkpoint();
    let saved = game.state().fighters[0].locomotion.run;
    let mut expected = vec![];
    for _ in 0..10 {
        expected.push(step(&mut game, 0, [1.0, 0.0]));
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state().fighters[0].locomotion.run.frame, saved.frame);
    assert_eq!(
        game.state().fighters[0].locomotion.run.last_rate,
        saved.last_rate
    );
    for expected in expected {
        assert_eq!(step(&mut game, 0, [1.0, 0.0]), expected);
    }
}

#[test]
fn invalid_run_resources_are_rejected_without_constructing_a_match() {
    let mut zero_length = data();
    zero_length.fighters[0]
        .movement
        .run_animation
        .as_mut()
        .unwrap()
        .length = 0.0;
    assert!(Match::new(zero_length, 42).is_err());

    let mut negative_scaling = data();
    negative_scaling.fighters[0]
        .movement
        .run_animation
        .as_mut()
        .unwrap()
        .scaling = -1.0;
    assert!(Match::new(negative_scaling, 42).is_err());

    let mut non_finite = data();
    non_finite.fighters[0]
        .movement
        .run_animation
        .as_mut()
        .unwrap()
        .length = f32::NAN;
    assert!(Match::new(non_finite, 42).is_err());
}

#[test]
fn without_the_resource_run_keeps_a_single_integer_frame() {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let p = serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(p);
    }
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.spawns = [[-40.0, 0.0], [40.0, 0.0]];
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    let mut game = Match::new(data, 42).unwrap();
    let mut state = step(&mut game, 0, [1.0, 0.0]);
    assert_eq!(state.fighters[0].action, Action::Dash);
    for _ in 0..20 {
        state = step(&mut game, 0, [1.0, 0.0]);
        if state.fighters[0].action == Action::Run {
            break;
        }
    }
    assert_eq!(state.fighters[0].action, Action::Run);
    assert_eq!(state.fighters[0].locomotion.run.frame, 0.0);
    let entry_frame = state.fighters[0].action_frame;
    let next = step(&mut game, 0, [1.0, 0.0]);
    assert_eq!(next.fighters[0].locomotion.run.frame, 0.0);
    assert_eq!(next.fighters[0].action_frame, entry_frame + 1);
}

/// `ftCo_Run_Anim`'s zero-rate branch (`vel * facing <= 0.0`) is exercised
/// directly by `run_animation_rate`'s own unit test
/// (`src/fighter/locomotion.rs`). Reaching it through an actual Run frame in
/// this integration harness would need `ground_velocity` to oppose `facing`
/// while `Action::Run` stays current; `game::locomotion::update_actions`'s
/// own `Action::Run` arm only remains in Run while `stick * facing >
/// turn_threshold` and `|stick| >= run_threshold` (else RunTurn or
/// RunBrake), which -- combined with `ftCo_Run_Phys`'s acceleration toward
/// `stick * dash_max_velocity` -- drives `ground_velocity` to the same sign
/// as the (possibly just-flipped) `facing` before Run is reachable again.
/// This test drives exactly that reversal (ramp to Run, then hold a hard
/// reverse stick through a RunTurn flip back into Run) and confirms the
/// product stays positive on every observed Run frame, documenting rather
/// than asserting the zero-rate branch is unreachable here.
#[test]
fn moving_against_facing_within_run_is_not_reachable_through_a_run_turn_reversal() {
    let mut game = game();
    ramp_to_run(&mut game);
    let mut saw_run_again = false;
    for _ in 0..80 {
        let state = step(&mut game, 0, [-1.0, 0.0]);
        if state.fighters[0].action == Action::Run {
            saw_run_again = true;
            assert!(
                state.fighters[0].ground_velocity * state.fighters[0].facing > 0.0,
                "expected ground_velocity to already agree with the flipped facing"
            );
        }
    }
    assert!(
        saw_run_again,
        "expected the reversal to route back through Run"
    );
}
