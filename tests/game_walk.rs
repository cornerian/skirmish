//! End-to-end walk kind selection, animation rate/frame and Slippi ids
//! (`docs/walk.md`) in a synthetic world built on the existing locomotion
//! fixture.
#[path = "support/tilt.rs"]
mod tilt_support;
#[path = "support/walk.rs"]
mod walk_support;

use skirmish::{
    fighter::locomotion::{walk_animation_rate, walk_retype_frame},
    game::{Action, Controller, Match, State, data::MatchData, locomotion::WalkKind},
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
    walk_support::profile(data)
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

/// Walk while holding `stick`, returning every consecutive pair of Walk
/// states observed within `frames` steps (both `Action::Walk`).
fn walk_pairs(game: &mut Match, stick: [f32; 2], frames: usize) -> Vec<(State, State)> {
    let mut pairs = vec![];
    let mut previous = step(game, 0, stick);
    for _ in 0..frames {
        let current = step(game, 0, stick);
        if previous.fighters[0].action == Action::Walk && current.fighters[0].action == Action::Walk
        {
            pairs.push((previous.clone(), current.clone()));
        }
        previous = current;
    }
    pairs
}

#[test]
fn walk_kind_at_entry_from_wait_is_slow_with_frame_zero() {
    let mut game = game();
    let entered = step(&mut game, 0, [0.75, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::Walk);
    assert_eq!(entered.fighters[0].locomotion.walk.kind, WalkKind::Slow);
    assert_eq!(entered.fighters[0].locomotion.walk.frame, 0.0);
    assert_eq!(entered.fighters[0].locomotion.walk.last_rate, 1.0);
}

#[test]
fn walk_ramp_reaches_middle_then_fast_with_a_stable_instance_id_and_bit_exact_remapped_frames() {
    let mut game = game();
    let entered = step(&mut game, 0, [0.75, 0.0]);
    let instance_id = entered.fighters[0].action_instance.id;
    let mut previous = entered;
    let mut seen = vec![WalkKind::Slow];
    for _ in 0..60 {
        let current = step(&mut game, 0, [0.75, 0.0]);
        let f = &current.fighters[0];
        assert_eq!(f.action, Action::Walk, "stayed in Walk throughout the ramp");
        assert_eq!(
            f.action_instance.id, instance_id,
            "Walk/Dash share motion identity 102"
        );
        let prev_walk = &previous.fighters[0].locomotion.walk;
        if f.locomotion.walk.kind != prev_walk.kind {
            // Reconstruct the intermediate frame ftWalkCommon_800DFEC8 saw:
            // this step's own Anim-phase advance from the previous frame,
            // before the retype ran.
            let lengths = walk_support::ANIMATION.lengths;
            let prev_length = lengths[prev_walk.kind as usize];
            let mut intermediate = prev_walk.frame + prev_walk.last_rate;
            while intermediate >= prev_length {
                intermediate -= prev_length;
            }
            let new_length = lengths[f.locomotion.walk.kind as usize];
            let expected = walk_retype_frame(intermediate, prev_length, new_length) as f32;
            assert_eq!(
                f.locomotion.walk.frame.to_bits(),
                expected.to_bits(),
                "remapped frame must match the pure helper bit-exactly"
            );
            seen.push(f.locomotion.walk.kind);
        }
        previous = current;
    }
    assert_eq!(seen, [WalkKind::Slow, WalkKind::Middle, WalkKind::Fast]);
}

#[test]
fn animation_rate_lags_one_frame_and_is_zero_while_moving_against_facing() {
    let mut game = game();
    for (previous, current) in walk_pairs(&mut game, [0.75, 0.0], 60) {
        let p = &previous.fighters[0];
        let c = &current.fighters[0];
        if c.locomotion.walk.kind != p.locomotion.walk.kind {
            // A retype this frame re-entered Walk (a real ChangeMotionState),
            // resetting last_rate to the entry default instead of advancing
            // through walk_animation_rate; covered bit-exactly by the ramp
            // test above.
            assert_eq!(c.locomotion.walk.last_rate, 1.0);
            continue;
        }
        let expected = walk_animation_rate(
            p.ground_velocity,
            p.facing,
            p.locomotion.walk.kind,
            walk_support::ANIMATION.rates,
            0.0,
            1.0,
        );
        assert_eq!(c.locomotion.walk.last_rate.to_bits(), expected.to_bits());
        if p.ground_velocity * p.facing <= 0.0 {
            assert_eq!(c.locomotion.walk.last_rate, 0.0);
        }
    }
}

#[test]
fn animation_frame_wraps_at_the_kind_length() {
    // A small, steady stick keeps the fighter in Slow (target well under
    // the middle threshold's velocity), long enough for its 10-frame
    // length to wrap several times.
    let mut game = game();
    let mut previous = step(&mut game, 0, [0.2, 0.0]);
    let mut wrapped = false;
    for _ in 0..400 {
        let current = step(&mut game, 0, [0.2, 0.0]);
        let f = &current.fighters[0];
        if f.action == Action::Walk {
            assert_eq!(f.locomotion.walk.kind, WalkKind::Slow);
            assert!(f.locomotion.walk.frame < walk_support::ANIMATION.lengths[0]);
            if previous.fighters[0].action == Action::Walk
                && f.locomotion.walk.frame < previous.fighters[0].locomotion.walk.frame
            {
                wrapped = true;
            }
        }
        previous = current;
    }
    assert!(
        wrapped,
        "expected the Slow animation frame to wrap at least once"
    );
}

#[test]
fn wait_chain_exit_on_a_reversed_or_deadzone_stick_leaves_the_walk_state_untouched() {
    let mut walk_then_wait = game();
    let walking = step(&mut walk_then_wait, 0, [0.75, 0.0]);
    assert_eq!(walking.fighters[0].action, Action::Walk);
    let kind = walking.fighters[0].locomotion.walk.kind;
    let frame = walking.fighters[0].locomotion.walk.frame;
    let last_rate = walking.fighters[0].locomotion.walk.last_rate;
    // Below walk_threshold (0.2) and above turn_threshold (-0.3): back to
    // Wait. Walk's own Anim phase still runs on this exit frame (it
    // precedes IASA), advancing by one more step before the chain leaves.
    let waited = step(&mut walk_then_wait, 0, [0.0, 0.0]);
    assert_eq!(waited.fighters[0].action, Action::Wait);
    assert_eq!(waited.fighters[0].locomotion.walk.kind, kind);
    let expected_frame = {
        let length = walk_support::ANIMATION.lengths[kind as usize];
        let mut frame = frame + last_rate;
        while frame >= length {
            frame -= length;
        }
        frame
    };
    assert_eq!(
        waited.fighters[0].locomotion.walk.frame.to_bits(),
        expected_frame.to_bits()
    );

    let mut reversed_game = game();
    step(&mut reversed_game, 0, [0.75, 0.0]);
    let reversed = step(&mut reversed_game, 0, [-1.0, 0.0]);
    assert_eq!(reversed.fighters[0].action, Action::Turn);
}

#[test]
fn a_tilt_press_preempts_the_retype_check_and_leaves_the_walk_kind_unchanged() {
    let mut game = Match::new(tilt_support::profile(data()), 42).unwrap();
    // Age the stick past the smash window so a fresh A press is a tilt, not
    // a smash (matching tests/game_smash.rs's own aging pattern), while
    // still walking.
    let mut last = step(&mut game, 0, [0.6, 0.0]);
    for _ in 0..5 {
        last = step(&mut game, 0, [0.6, 0.0]);
        assert_eq!(last.fighters[0].action, Action::Walk);
    }
    let kind = last.fighters[0].locomotion.walk.kind;
    let frame = last.fighters[0].locomotion.walk.frame;
    let attacked = step(&mut game, skirmish::game::BUTTON_A, [1.0, 0.0]);
    assert_eq!(attacked.fighters[0].action, Action::AttackS3S);
    // update_ground_attacks returns before the shared Wait/Walk arm (and its
    // retype check) ever runs; Walk's own Anim phase still advanced this
    // frame (it precedes IASA), one more step from `last`'s own rate.
    let expected_frame = {
        let length = walk_support::ANIMATION.lengths[kind as usize];
        let mut frame = frame + last.fighters[0].locomotion.walk.last_rate;
        while frame >= length {
            frame -= length;
        }
        frame
    };
    assert_eq!(attacked.fighters[0].locomotion.walk.kind, kind);
    assert_eq!(
        attacked.fighters[0].locomotion.walk.frame.to_bits(),
        expected_frame.to_bits()
    );
}

#[test]
fn checkpoints_restore_the_walk_kind_and_frame_exactly() {
    let mut game = game();
    for _ in 0..3 {
        step(&mut game, 0, [0.75, 0.0]);
    }
    let checkpoint = game.checkpoint();
    let saved = game.state().fighters[0].locomotion.walk;
    let mut expected = vec![];
    for _ in 0..10 {
        expected.push(step(&mut game, 0, [0.75, 0.0]));
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state().fighters[0].locomotion.walk.kind, saved.kind);
    assert_eq!(game.state().fighters[0].locomotion.walk.frame, saved.frame);
    assert_eq!(
        game.state().fighters[0].locomotion.walk.last_rate,
        saved.last_rate
    );
    for expected in expected {
        assert_eq!(step(&mut game, 0, [0.75, 0.0]), expected);
    }
}

#[test]
fn invalid_walk_resources_are_rejected_without_constructing_a_match() {
    // Rules without animation data, or vice versa.
    let mut missing_animation = data();
    missing_animation.fighters[0].movement.walk_animation = None;
    assert!(Match::new(missing_animation, 42).is_err());

    let mut missing_rules = data();
    missing_rules.rules.walk = None;
    assert!(Match::new(missing_rules, 42).is_err());

    // Zero/negative lengths or rates.
    let mut zero_length = data();
    zero_length.fighters[0]
        .movement
        .walk_animation
        .as_mut()
        .unwrap()
        .lengths[0] = 0.0;
    assert!(Match::new(zero_length, 42).is_err());

    let mut negative_rate = data();
    negative_rate.fighters[0]
        .movement
        .walk_animation
        .as_mut()
        .unwrap()
        .rates[1] = -1.0;
    assert!(Match::new(negative_rate, 42).is_err());

    // middle_threshold above fast_threshold.
    let mut inverted = data();
    inverted.rules.walk.as_mut().unwrap().middle_threshold = 0.9;
    inverted.rules.walk.as_mut().unwrap().fast_threshold = 0.1;
    assert!(Match::new(inverted, 42).is_err());
}

#[test]
fn without_the_resource_walk_keeps_a_single_kind_and_never_advances_the_float_frame() {
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
    for _ in 0..30 {
        let state = step(&mut game, 0, [0.75, 0.0]);
        if state.fighters[0].action == Action::Walk {
            assert_eq!(state.fighters[0].locomotion.walk.kind, WalkKind::Slow);
            assert_eq!(state.fighters[0].locomotion.walk.frame, 0.0);
        }
    }
}
