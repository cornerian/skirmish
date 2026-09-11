//! Regression coverage for the 2026-09-11 run-corrections batch
//! (`docs/run.md`): RunTurn's flip check now reads the per-entry facing
//! (`Fighter.locomotion.run_turn_facing`) instead of a fixed resource
//! constant, RunBrake models its own velocity-gated marker freeze
//! (`ftCo_RunBrake.c:49-77`), and Run's `run.x0` turn-run lockout
//! (`ftCo_Run.c:72,96-98,125-126`) gates RunTurn/RunBrake entry from Run in
//! both `game::locomotion::update_actions` and `game::dash::
//! update_dash_or_run`'s Run arm.
use skirmish::game::{Action, Controller, Match, State, data::MatchData, locomotion::Parameters};

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
    data
}

/// `data()` with every fighter's locomotion parameters passed through
/// `mutate`, mirroring `tests/game_locomotion.rs`'s own per-test parameter
/// tweaks.
fn data_with(mutate: impl Fn(&mut Parameters)) -> MatchData {
    let mut data = data();
    for fighter in &mut data.fighters {
        let p = fighter.locomotion.as_mut().unwrap();
        mutate(p);
    }
    data
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

fn reach_run(game: &mut Match) {
    for _ in 0..20 {
        if step(game, 0, [1.0, 0.0]).fighters[0].action == Action::Run {
            return;
        }
    }
    panic!("dash did not reach Run: {:?}", game.state());
}

fn parameters(game: &Match) -> Parameters {
    *game.data().fighters[0].locomotion.as_ref().unwrap()
}

// --- Correction 1: RunTurn's flip reads the per-entry facing -----------

/// `ftCo_TurnRun_Anim` (`ftCo_TurnRun.c:57-77`): while the marker is set,
/// freeze (rate 0, held `action_frame`); once `facing_at_entry * gr_vel <=
/// 0.01`, resume and flip. Entered from Run while facing +1.0, the entry
/// facing captured by `start_run_turn` -- `run_turn_facing` -- is +1.0, so
/// the flip only fires once ground velocity has decayed to (or past) zero;
/// a fixed `run_turn_velocity_scale` of 1.0 would coincidentally agree here
/// (the bug this correction fixes only shows up at a -1.0 entry, covered by
/// the sibling test below).
#[test]
fn run_turn_started_facing_positive_flips_only_after_velocity_crosses_zero() {
    let mut game = game();
    reach_run(&mut game);
    assert_eq!(game.state().fighters[0].facing, 1.0);
    let p = parameters(&game);
    let entered = step(&mut game, 0, [p.turn_threshold, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunTurn);
    assert_eq!(entered.fighters[0].locomotion.run_turn_facing, 1.0);

    let mut before = entered;
    let mut flipped = false;
    for _ in 0..40 {
        let state = step(&mut game, 0, [-1.0, 0.0]);
        if state.fighters[0].locomotion.run_turn_waiting {
            // Frozen: rate 0 holds action_frame at the marker frame.
            assert_eq!(state.fighters[0].action_frame, p.run_turn_flip_frame);
            assert_eq!(state.fighters[0].facing, 1.0);
        }
        if state.fighters[0].facing == -1.0 && before.fighters[0].facing == 1.0 {
            // The flip fires once the *previous* frame's ground velocity
            // (Anim precedes Phys within a frame) satisfies the per-entry
            // check: entry_facing(1.0) * gr_vel <= 0.01.
            assert!(1.0 * before.fighters[0].ground_velocity <= 0.01);
            flipped = true;
            break;
        }
        before = state;
    }
    assert!(flipped, "expected the reversal to flip facing");
    assert_eq!(game.state().fighters[0].locomotion.run_turn_facing, 1.0);
}

/// The same reversal, but entered while facing -1.0 (reached by a first
/// full run-turn reversal from the +1.0 baseline). Before this correction,
/// `Parameters::run_turn_velocity_scale` was a fixed per-match constant
/// (1.0 in this fixture) applied regardless of entry facing, so a -1.0
/// entry incorrectly flipped as soon as *any* positive-velocity carryover
/// crossed 0.01 in the wrong direction -- `facing_at_entry * gr_vel <=
/// 0.01` with `facing_at_entry = -1.0` instead requires `gr_vel >= -0.01`,
/// the opposite sign test. This is exactly the discrepancy `docs/run.md`
/// reported and this batch fixes.
#[test]
fn run_turn_started_facing_negative_flips_only_after_velocity_crosses_zero() {
    let mut game = game();
    reach_run(&mut game);
    let p = parameters(&game);
    step(&mut game, 0, [p.turn_threshold, 0.0]);
    // Hold the reversal through the first flip and back into Run facing -1.0.
    let mut reached_negative_run = false;
    for _ in 0..60 {
        let state = step(&mut game, 0, [-1.0, 0.0]);
        if state.fighters[0].action == Action::Run && state.fighters[0].facing == -1.0 {
            reached_negative_run = true;
            break;
        }
    }
    assert!(reached_negative_run, "expected to reach Run facing -1.0");
    assert_eq!(game.state().fighters[0].locomotion.run_lockout, 0.0);

    // Reverse again (stick positive, boundary at -turn_threshold since
    // facing is now -1.0): stick * facing <= turn_threshold.
    let entered = step(&mut game, 0, [-p.turn_threshold, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunTurn);
    assert_eq!(entered.fighters[0].locomotion.run_turn_facing, -1.0);

    let mut before = entered;
    let mut flipped = false;
    for _ in 0..40 {
        let state = step(&mut game, 0, [1.0, 0.0]);
        if state.fighters[0].locomotion.run_turn_waiting {
            assert_eq!(state.fighters[0].action_frame, p.run_turn_flip_frame);
            assert_eq!(state.fighters[0].facing, -1.0);
        }
        if state.fighters[0].facing == 1.0 && before.fighters[0].facing == -1.0 {
            // entry_facing(-1.0) * gr_vel <= 0.01 <=> gr_vel >= -0.01.
            assert!(-before.fighters[0].ground_velocity <= 0.01);
            flipped = true;
            break;
        }
        before = state;
    }
    assert!(flipped, "expected the second reversal to flip facing back");
}

// --- Correction 2: RunBrake's velocity-gated marker freeze --------------

/// `ftCo_RunBrake_Anim` (`ftCo_RunBrake.c:49-77`): while the marker is set
/// and not yet frozen, freeze once `|gr_vel| >= run_brake_freeze_speed`;
/// once frozen, resume once `|gr_vel| <= run_brake_freeze_speed`. The
/// `frames` countdown (`run_brake_frames`) keeps counting down every frame
/// regardless of the freeze.
#[test]
fn run_brake_freeze_holds_frames_while_fast_and_resumes_when_slow() {
    let mut data = data();
    for fighter in &mut data.fighters {
        let p = fighter.locomotion.as_mut().unwrap();
        p.run_brake_marker_frame = Some(1);
        p.run_brake_freeze_speed = Some(1.5);
    }
    let mut game = Match::new(data, 42).unwrap();
    reach_run(&mut game);
    let entered = step(&mut game, 0, [0.0, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunBrake);
    assert!(
        entered.fighters[0].ground_velocity.abs() >= 1.5,
        "test assumes RunBrake enters above the freeze speed"
    );

    let held_frame = entered.fighters[0].action_frame;
    let mut previous_frames = entered.fighters[0].locomotion.run_brake_frames;
    let mut saw_frozen = false;
    let mut saw_resumed = false;
    for _ in 0..8 {
        let state = step(&mut game, 0, [0.0, 0.0]);
        if state.fighters[0].action != Action::RunBrake {
            break;
        }
        // The countdown keeps ticking down regardless of the freeze.
        assert!(state.fighters[0].locomotion.run_brake_frames < previous_frames);
        previous_frames = state.fighters[0].locomotion.run_brake_frames;
        if state.fighters[0].locomotion.run_brake_frozen {
            saw_frozen = true;
            assert_eq!(
                state.fighters[0].action_frame, held_frame,
                "frozen frames must hold action_frame"
            );
        } else if saw_frozen {
            saw_resumed = true;
            assert!(state.fighters[0].action_frame > held_frame);
            assert!(state.fighters[0].ground_velocity.abs() <= 1.5);
        }
    }
    assert!(saw_frozen, "expected the brake to freeze while fast");
    assert!(
        saw_resumed,
        "expected the brake to resume once velocity dropped under the freeze speed"
    );
}

/// When the freeze speed is never reached within the brake's own frame
/// countdown, `frames` still forces the exit to `Wait` while the animation
/// stays frozen the entire time (`ftCo_RunBrake.c:68-77`'s countdown is
/// unconditional; the freeze never resumes on its own).
#[test]
fn run_brake_freeze_that_never_resumes_still_ends_on_the_frame_countdown() {
    let mut data = data();
    for fighter in &mut data.fighters {
        let p = fighter.locomotion.as_mut().unwrap();
        p.run_brake_marker_frame = Some(1);
        p.run_brake_freeze_speed = Some(0.01);
    }
    let mut game = Match::new(data, 42).unwrap();
    reach_run(&mut game);
    let entered = step(&mut game, 0, [0.0, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunBrake);
    let held_frame = entered.fighters[0].action_frame;

    let mut ended = false;
    for _ in 0..8 {
        let state = step(&mut game, 0, [0.0, 0.0]);
        if state.fighters[0].action != Action::RunBrake {
            assert_eq!(state.fighters[0].action, Action::Wait);
            ended = true;
            break;
        }
        assert!(state.fighters[0].locomotion.run_brake_frozen);
        assert_eq!(state.fighters[0].action_frame, held_frame);
    }
    assert!(
        ended,
        "expected the frame countdown to end the brake despite the freeze"
    );
}

/// `run_brake_marker_frame`/`run_brake_freeze_speed` absent (the fixture's
/// own default) keeps the pre-batch behavior: no freeze at all,
/// `action_frame` advances every RunBrake frame.
#[test]
fn run_brake_without_a_marker_keeps_the_old_unfrozen_behaviour() {
    let mut game = game();
    reach_run(&mut game);
    let entered = step(&mut game, 0, [0.0, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunBrake);
    let mut previous_frame = entered.fighters[0].action_frame;
    for _ in 0..8 {
        let state = step(&mut game, 0, [0.0, 0.0]);
        assert!(!state.fighters[0].locomotion.run_brake_frozen);
        if state.fighters[0].action != Action::RunBrake {
            break;
        }
        assert_eq!(state.fighters[0].action_frame, previous_frame + 1);
        previous_frame = state.fighters[0].action_frame;
    }
}

// --- Correction 3: run.x0's turn-run lockout ----------------------------

/// `ftCo_Run_IASA` (`ftCo_Run.c:125-126`): while `run.x0 > 0.0` (freshly set
/// by a RunTurn-to-Run re-entry, `fn_800CA644`'s `arg0 = x430`), the whole
/// rest of the IASA chain -- including RunTurn and RunBrake entry -- is
/// skipped. `run.x0` counts down by 1.0 per Run animation frame
/// (`ftCo_Run.c:96-98`), so with `run_turn_lockout_frames = Some(5.0)` Run
/// stays current for exactly 4 further frames (lockout 4, 3, 2, 1) before
/// unlocking on the 5th.
#[test]
fn run_turn_lockout_blocks_turn_and_brake_entry_for_exactly_its_frame_count() {
    let mut game = Match::new(data_with(|p| p.run_turn_lockout_frames = Some(5.0)), 42).unwrap();
    reach_run(&mut game);
    let p = parameters(&game);
    step(&mut game, 0, [p.turn_threshold, 0.0]);
    let mut reached_run = None;
    for _ in 0..40 {
        let state = step(&mut game, 0, [-1.0, 0.0]);
        if state.fighters[0].action == Action::Run {
            reached_run = Some(state);
            break;
        }
    }
    let reached_run = reached_run.expect("expected the run-turn to re-enter Run");
    assert_eq!(reached_run.fighters[0].locomotion.run_lockout, 5.0);

    // Neutral stick would ordinarily brake immediately (see
    // `dash_to_run_has_no_lockout`); here it must not while locked out.
    let mut brake = game.clone();
    for expected_lockout in [4.0, 3.0, 2.0, 1.0] {
        let state = step(&mut brake, 0, [0.0, 0.0]);
        assert_eq!(state.fighters[0].action, Action::Run);
        assert_eq!(state.fighters[0].locomotion.run_lockout, expected_lockout);
    }
    let unlocked = step(&mut brake, 0, [0.0, 0.0]);
    assert_eq!(unlocked.fighters[0].action, Action::RunBrake);
    assert_eq!(unlocked.fighters[0].locomotion.run_lockout, 0.0);

    // A reversed stick is equally blocked from entering RunTurn. Facing is
    // -1.0 at this point (the first reversal above already flipped it), so
    // the boundary stick is -turn_threshold: stick * facing <= turn_threshold.
    let mut turn = game;
    for expected_lockout in [4.0, 3.0, 2.0, 1.0] {
        let state = step(&mut turn, 0, [-p.turn_threshold, 0.0]);
        assert_eq!(state.fighters[0].action, Action::Run);
        assert_eq!(state.fighters[0].locomotion.run_lockout, expected_lockout);
    }
    let unlocked = step(&mut turn, 0, [-p.turn_threshold, 0.0]);
    assert_eq!(unlocked.fighters[0].action, Action::RunTurn);
}

/// `fn_800CA5F0` (Dash's own IASA transition into Run) always supplies
/// `arg0 = 0.0`: an ordinary Dash-to-Run entry never sets the lockout, even
/// when `run_turn_lockout_frames` is configured.
#[test]
fn dash_to_run_has_no_lockout() {
    let mut game = Match::new(data_with(|p| p.run_turn_lockout_frames = Some(5.0)), 42).unwrap();
    reach_run(&mut game);
    assert_eq!(game.state().fighters[0].locomotion.run_lockout, 0.0);
    let brake = step(&mut game, 0, [0.0, 0.0]);
    assert_eq!(brake.fighters[0].action, Action::RunBrake);
}

// --- Checkpoints and invalid resources -----------------------------------

/// A checkpoint taken mid-lockout (still braking-blocked) restores
/// `run_lockout` exactly and replays identically.
#[test]
fn checkpoints_restore_the_run_lockout_and_brake_freeze_exactly() {
    let mut game = Match::new(
        data_with(|p| {
            p.run_turn_lockout_frames = Some(5.0);
            p.run_brake_marker_frame = Some(1);
            p.run_brake_freeze_speed = Some(1.5);
        }),
        42,
    )
    .unwrap();
    reach_run(&mut game);
    let p = parameters(&game);
    step(&mut game, 0, [p.turn_threshold, 0.0]);
    for _ in 0..40 {
        if step(&mut game, 0, [-1.0, 0.0]).fighters[0].action == Action::Run {
            break;
        }
    }
    assert_eq!(game.state().fighters[0].locomotion.run_lockout, 5.0);
    let checkpoint = game.checkpoint();
    let mut expected = Vec::new();
    for _ in 0..12 {
        expected.push(step(&mut game, 0, [0.0, 0.0]));
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, 0, [0.0, 0.0]), expected);
    }
}

#[test]
fn invalid_run_correction_resources_are_rejected_without_constructing_a_match() {
    // run_turn_lockout_frames must be finite and non-negative.
    for bad in [-1.0, f32::NAN, f32::INFINITY] {
        let data = data_with(|p| p.run_turn_lockout_frames = Some(bad));
        assert!(Match::new(data, 42).is_err());
    }
    // run_brake_marker_frame/run_brake_freeze_speed must be paired.
    let data = data_with(|p| p.run_brake_marker_frame = Some(0));
    assert!(Match::new(data, 42).is_err());
    let data = data_with(|p| p.run_brake_freeze_speed = Some(1.0));
    assert!(Match::new(data, 42).is_err());
    // The marker frame must be inside the brake's own animation.
    let data = data_with(|p| {
        p.run_brake_marker_frame = Some(p.run_brake_animation_frames);
        p.run_brake_freeze_speed = Some(1.0);
    });
    assert!(Match::new(data, 42).is_err());
    // The freeze speed must be finite and non-negative.
    for bad in [-1.0, f32::NAN, f32::INFINITY] {
        let data = data_with(|p| {
            p.run_brake_marker_frame = Some(0);
            p.run_brake_freeze_speed = Some(bad);
        });
        assert!(Match::new(data, 42).is_err());
    }
}
