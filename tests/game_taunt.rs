//! End-to-end taunts (AppealSR/AppealSL, `docs/taunt.md`) and the Wait-chain
//! spot dodge in an explicitly synthetic native world with invented escape,
//! tilt, smash, dash, jab, grab, edge/teeter and taunt profiles.
#[path = "support/dash.rs"]
mod dash_support;
#[path = "support/edge.rs"]
mod edge_support;
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/jab.rs"]
mod jab_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/taunt.rs"]
mod taunt_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::{
    fighter::dash::transition_friction,
    game::{
        Action, BUTTON_A, BUTTON_DPAD_DOWN, BUTTON_DPAD_LEFT, BUTTON_DPAD_RIGHT, BUTTON_DPAD_UP,
        BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Error, Match, State,
        data::MatchData,
        taunt::{Taunt, TauntAnimation, TauntFrame},
    },
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: skirmish::game::shield::Rules,
    attributes: skirmish::game::shield::Attributes,
}

/// Fighter 0 (facing +X at -2) is the primary subject; fighter 1 (facing -X
/// at +2) idles unless a test drives it directly for the facing-left cases.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.rules.knockback_speed = 0.0;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
        fighter.movement.landing_frames = 6;
        fighter.movement.normal_landing_lag = Some(3.0);
    }
    let data = smash_support::profile(tilt_support::profile(escape_support::profile(
        grab_support::profile(data),
    )));
    let data = edge_support::profile(dash_support::profile(jab_support::profile(data)));
    taunt_support::profile(data)
}

fn stick(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn buttons(buttons: u16) -> Controller {
    stick(buttons, [0.0, 0.0])
}

fn dpad_up() -> Controller {
    buttons(BUTTON_DPAD_UP)
}

fn step(game: &mut Match, first: Controller) -> State {
    game.step([first, Controller::default()]).unwrap().clone()
}

fn step2(game: &mut Match, first: Controller, second: Controller) -> State {
    game.step([first, second]).unwrap().clone()
}

/// Drive fighter 0 with `drive(frame_index)` until it reaches `target`,
/// returning that state. Panics after `max` steps.
fn enter_action(
    game: &mut Match,
    drive: impl Fn(u32) -> Controller,
    target: Action,
    max: u32,
) -> State {
    let mut state = step(game, drive(0));
    for frame in 1..max {
        if state.fighters[0].action == target {
            return state;
        }
        state = step(game, drive(frame));
    }
    panic!(
        "fighter 0 never reached {target:?}, stayed at {:?}",
        state.fighters[0].action
    );
}

/// X press then release for a short hop: steps until Landing.
fn enter_landing(game: &mut Match) -> State {
    step(game, stick(BUTTON_X, [0.0, 0.0]));
    for _ in 0..40 {
        let state = step(game, buttons(0));
        if state.fighters[0].action == Action::Landing {
            return state;
        }
    }
    unreachable!("the short hop must land within forty steps");
}

/// Walk fighter 0 into Ottotto against a nearby `floor.right`.
fn enter_ottotto(game: &mut Match) -> State {
    let mut state = step(game, stick(0, [0.5, 0.0]));
    for _ in 0..40 {
        if state.fighters[0].action == Action::Ottotto {
            return state;
        }
        state = step(game, stick(0, [0.5, 0.0]));
    }
    panic!("the fighter should have entered Ottotto");
}

#[test]
fn taunt_fires_after_the_shield_check_and_before_the_jump_from_every_chain_it_lists() {
    // Wait: a fresh D-pad-up press taunts directly.
    let mut game = Match::new(data(), 42).unwrap();
    let entry = step(&mut game, dpad_up());
    assert_eq!(entry.fighters[0].action, Action::AppealSR);
    assert_eq!(entry.fighters[0].action_frame, 1);

    // Walk.
    let mut game = Match::new(data(), 42).unwrap();
    let walking = enter_action(&mut game, |_| stick(0, [0.3, 0.0]), Action::Walk, 5);
    assert_eq!(walking.fighters[0].action, Action::Walk);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // Squat (the transitional crouch-entry state itself).
    let mut game = Match::new(data(), 42).unwrap();
    let squat = step(&mut game, stick(0, [0.0, -1.0]));
    assert_eq!(squat.fighters[0].action, Action::Squat);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // SquatWait: keep holding down until the crouch settles.
    let mut game = Match::new(data(), 42).unwrap();
    let squat_wait = enter_action(&mut game, |_| stick(0, [0.0, -1.0]), Action::SquatWait, 12);
    assert_eq!(squat_wait.fighters[0].action, Action::SquatWait);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // Turn (post-turn facing): fighter 0 starts facing +1; a stick opposite
    // it starts Turn, and after `standing_turn_frames` it flips to -1 before
    // this taunt fires, so the resulting motion must reflect the new facing.
    let mut game = Match::new(data(), 42).unwrap();
    let mut state = step(&mut game, stick(0, [-1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Turn);
    let mut turned = false;
    for _ in 0..6 {
        state = step(&mut game, buttons(0));
        if state.fighters[0].facing == -1.0 {
            turned = true;
            break;
        }
        assert_eq!(state.fighters[0].action, Action::Turn);
    }
    assert!(turned, "the turn should complete and flip facing");
    let after_turn = step(&mut game, dpad_up());
    // Facing -1 with the fixture's left motion supplied picks AppealSL.
    assert_eq!(after_turn.fighters[0].action, Action::AppealSL);

    // Run: dash past `dash_run_frame` while still holding outward.
    let mut game = Match::new(data(), 42).unwrap();
    let running = enter_action(&mut game, |_| stick(0, [1.0, 0.0]), Action::Run, 12);
    assert_eq!(running.fighters[0].action, Action::Run);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // Landing's interruptible frame (normal_landing_lag == 3.0).
    let mut game = Match::new(data(), 42).unwrap();
    enter_landing(&mut game);
    step(&mut game, buttons(0));
    step(&mut game, buttons(0));
    let interruptible = step(&mut game, buttons(0));
    assert_eq!(interruptible.fighters[0].action, Action::Landing);
    assert!(interruptible.fighters[0].action_frame >= 3);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // An interruptible smash pose (AttackS4S's interrupt_from is 7; the
    // charge command freezes the animation at pose 2 for a few frames after
    // the releasing A press, so drive by actual `action_frame` instead of a
    // fixed step count).
    let mut game = Match::new(data(), 42).unwrap();
    let entry = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::AttackS4S);
    let mut interruptible = entry;
    for _ in 0..20 {
        if interruptible.fighters[0].action_frame > 7 {
            break;
        }
        interruptible = step(&mut game, buttons(0));
        assert_eq!(interruptible.fighters[0].action, Action::AttackS4S);
    }
    assert!(interruptible.fighters[0].action_frame > 7);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );

    // Ottotto.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    resource.stage.spawns[1] = [-1.0, 0.0];
    let mut game = Match::new(resource, 42).unwrap();
    let teetering = enter_ottotto(&mut game);
    assert_eq!(teetering.fighters[0].action, Action::Ottotto);
    assert_eq!(
        step(&mut game, dpad_up()).fighters[0].action,
        Action::AppealSR
    );
}

/// Removes the right taunt's root motion and the ground friction that would
/// otherwise keep decaying `ground_velocity` after the friction step, so the
/// transition-friction effect on `Fighter::ground_velocity` (applied during
/// `update_actions`, before physics integrates the frame) is observable
/// bit-exact in the post-step state instead of being overwritten by root
/// motion or further decayed by ordinary ground friction.
fn friction_observable_resource() -> MatchData {
    let mut resource = data();
    for fighter in &mut resource.fighters {
        fighter.movement.ground_friction = 0.0;
        let taunt = fighter.taunt.as_mut().unwrap();
        taunt.right.root_translations = None;
    }
    resource
}

#[test]
fn taunt_from_dash_falls_through_the_block_42_friction_tail_but_run_does_not() {
    // Dash: block_42's shared taunt check falls through to `ftCo_Dash_IASA`'s
    // own x54 friction tail (`dash.rs`'s `apply_transition_friction`).
    let mut game = Match::new(friction_observable_resource(), 42).unwrap();
    let dashing = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(dashing.fighters[0].action, Action::Dash);
    let before = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(before.fighters[0].action, Action::Dash);
    let pre_velocity = before.fighters[0].ground_velocity;
    let entry = step(&mut game, dpad_up());
    assert_eq!(entry.fighters[0].action, Action::AppealSR);
    let expected = transition_friction(pre_velocity, dash_support::RULES.transition_friction, 1.0);
    assert_eq!(entry.fighters[0].ground_velocity, expected);
    assert_ne!(
        expected, pre_velocity,
        "the friction step must have changed it"
    );

    // Run: the same shared block_42 check fires, but `ftCo_Run_IASA` has no
    // friction tail, so ground_velocity is untouched by the taunt entry.
    let mut game = Match::new(friction_observable_resource(), 42).unwrap();
    let running = enter_action(&mut game, |_| stick(0, [1.0, 0.0]), Action::Run, 12);
    assert_eq!(running.fighters[0].action, Action::Run);
    let pre_velocity = running.fighters[0].ground_velocity;
    let entry = step(&mut game, dpad_up());
    assert_eq!(entry.fighters[0].action, Action::AppealSR);
    assert_eq!(entry.fighters[0].ground_velocity, pre_velocity);
}

#[test]
fn facing_left_picks_appealsl_when_supplied_else_appealsr() {
    // Fighter 1 faces -X by default; the shared fixture supplies a left
    // motion, so it should pick AppealSL.
    let mut game = Match::new(data(), 42).unwrap();
    let state = step2(&mut game, Controller::default(), dpad_up());
    assert_eq!(state.fighters[1].action, Action::AppealSL);

    // With only the right motion supplied, facing left still picks AppealSR.
    let mut resource = data();
    for fighter in &mut resource.fighters {
        let taunt = fighter.taunt.as_ref().unwrap();
        fighter.taunt = Some(Taunt {
            right: taunt.right.clone(),
            left: None,
        });
    }
    let mut game = Match::new(resource, 42).unwrap();
    let state = step2(&mut game, Controller::default(), dpad_up());
    assert_eq!(state.fighters[1].action, Action::AppealSR);
}

#[test]
fn every_other_dpad_bit_is_inert() {
    for flag in [BUTTON_DPAD_LEFT, BUTTON_DPAD_RIGHT, BUTTON_DPAD_DOWN] {
        let mut game = Match::new(data(), 42).unwrap();
        let state = step(&mut game, buttons(flag));
        assert_eq!(state.fighters[0].action, Action::Wait, "flag {flag:#x}");
    }
}

#[test]
fn taunts_own_chain_allows_catch_smash_tilt_jab_spot_dodge_and_shield() {
    fn interruptible_taunt(game: &mut Match) -> State {
        step(game, dpad_up());
        step(game, buttons(0))
    }

    // Catch (A + a held shoulder).
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, buttons(BUTTON_A | BUTTON_Z));
    assert_eq!(state.fighters[0].action, Action::Catch);

    // Smash: a fresh full-magnitude stick + A outranks the tilt/jab checks.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AttackS4S);

    // Tilt: below the smash thresholds (0.7/0.8) but at or above the tilt
    // ones (0.5), matching `tests/game_tilt.rs`'s own up-tilt case.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, stick(BUTTON_A, [0.6, 0.6]));
    assert_eq!(state.fighters[0].action, Action::AttackHi3);

    // Jab: a plain A press with a neutral stick.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, buttons(BUTTON_A));
    assert_eq!(state.fighters[0].action, Action::Jab);

    // The Wait-chain spot dodge: L held with a fresh downward stick.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, stick(BUTTON_L, [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::EscapeN);

    // Shield: L held with a neutral stick.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, buttons(BUTTON_L));
    assert_eq!(state.fighters[0].action, Action::GuardOn);
}

#[test]
fn taunts_own_chain_excludes_jump_dash_squat_turn_and_plain_walk_stick() {
    fn interruptible_taunt(game: &mut Match) -> State {
        step(game, dpad_up());
        step(game, buttons(0))
    }

    // X (jump) does nothing; the taunt keeps running.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, buttons(BUTTON_X));
    assert_eq!(state.fighters[0].action, Action::AppealSR);

    // A dash-magnitude stick alone (no A) does nothing.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AppealSR);

    // A downward stick alone (no shoulder) stays in the taunt: the Wait-
    // chain spot dodge requires the shoulder held too.
    let mut game = Match::new(data(), 42).unwrap();
    interruptible_taunt(&mut game);
    let state = step(&mut game, stick(0, [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::AppealSR);
}

#[test]
fn taunt_root_motion_matches_the_supplied_samples_and_ends_into_wait() {
    let mut game = Match::new(data(), 42).unwrap();
    let entry = step(&mut game, dpad_up());
    assert_eq!(entry.fighters[0].action, Action::AppealSR);
    // The right taunt's own root translations: [0.0, 0.5, 0.25, 0.0].
    let roots = [0.5, 0.25, 0.0];
    let mut position = entry.fighters[0].position[0];
    for (index, root) in roots.into_iter().enumerate() {
        let state = step(&mut game, buttons(0));
        assert_eq!(state.fighters[0].action, Action::AppealSR, "sample {index}");
        assert_eq!(state.fighters[0].ground_velocity, root, "sample {index}");
        position += root;
        assert!(
            (state.fighters[0].position[0] - position).abs() < 1e-4,
            "sample {index}"
        );
    }
    let waiting = step(&mut game, buttons(0));
    assert_eq!(waiting.fighters[0].action, Action::Wait);
}

#[test]
fn taunt_clamps_at_a_floor_end_instead_of_sliding_through() {
    use skirmish::game::edge::EdgeSide;
    let mut resource = data();
    resource.stage.floor.right = 0.6;
    resource.stage.spawns = [[0.0, 0.0], [-0.5, 0.0]];
    let mut game = Match::new(resource, 42).unwrap();
    let entry = step(&mut game, dpad_up());
    assert_eq!(entry.fighters[0].action, Action::AppealSR);
    let mut last = entry;
    for _ in 0..4 {
        last = step(&mut game, buttons(0));
        if last.fighters[0].edge_contact.is_some() {
            break;
        }
    }
    assert_eq!(last.fighters[0].edge_contact, Some(EdgeSide::Right));
    assert!(last.fighters[0].grounded);
    assert_eq!(last.fighters[0].position[0], 0.6);
}

#[test]
fn wait_chain_spot_dodge_dodges_immediately_only_from_wait_and_taunt_with_a_fresh_main_stick() {
    // A held shoulder with the stick already down (fresh, inside the window)
    // dodges on that very first frame, without a GuardOn frame in between.
    let mut game = Match::new(data(), 42).unwrap();
    let state = step(&mut game, stick(BUTTON_L, [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::EscapeN);

    // Once the stick has aged past the window, the same input raises
    // GuardOn as usual (the ordinary shield check, not the dodge).
    let mut game = Match::new(data(), 42).unwrap();
    for _ in 0..5 {
        step(&mut game, stick(0, [0.0, -0.9]));
    }
    let state = step(&mut game, stick(BUTTON_L, [0.0, -0.9]));
    assert_eq!(state.fighters[0].action, Action::GuardOn);

    // A held C-stick alone is not part of this predicate: GuardOn, not a
    // dodge. Hold it a few frames first so it is stale for the fixture's
    // own down-smash C-stick click, which would otherwise preempt the
    // shield entry the same way a fresh smash always does.
    let mut game = Match::new(data(), 42).unwrap();
    let cstick_down = Controller {
        cstick: [0.0, -1.0],
        ..Default::default()
    };
    for _ in 0..6 {
        step(&mut game, cstick_down);
    }
    let state = step(
        &mut game,
        Controller {
            buttons: BUTTON_L,
            cstick: [0.0, -1.0],
            ..Default::default()
        },
    );
    assert_eq!(state.fighters[0].action, Action::GuardOn);

    // Walk's chain has no `ftCo_80099794` call: the same combined input from
    // Walk still raises GuardOn on that frame instead of dodging.
    let mut game = Match::new(data(), 42).unwrap();
    let walking = step(&mut game, stick(0, [0.3, 0.0]));
    assert_eq!(walking.fighters[0].action, Action::Walk);
    let state = step(&mut game, stick(BUTTON_L, [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::GuardOn);

    // From an interruptible AppealS frame, the same fresh combination
    // dodges immediately too.
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, dpad_up());
    step(&mut game, buttons(0));
    let state = step(&mut game, stick(BUTTON_L, [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::EscapeN);
}

#[test]
fn checkpoints_restore_taunt_state_exactly() {
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, dpad_up());
    step(&mut game, buttons(0));
    let checkpoint = game.checkpoint();
    let expected = step(&mut game, buttons(0));

    let mut replay = Match::new(data(), 42).unwrap();
    replay.step([dpad_up(), Controller::default()]).unwrap();
    replay.step([buttons(0), Controller::default()]).unwrap();
    replay.restore_checkpoint(&checkpoint).unwrap();
    let restored = step(&mut replay, buttons(0));
    assert_eq!(restored, expected);
}

#[test]
fn invalid_taunt_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut Taunt)) {
        let mut resource = data();
        let mut taunt = resource.fighters[0].taunt.clone().unwrap();
        edit(&mut taunt);
        resource.fighters[0].taunt = Some(taunt);
        match Match::new(resource, 42) {
            Err(Error::Data(_)) => {}
            other => panic!("expected a data error, got {other:?}"),
        }
    }
    rejected(|t| t.right.frames.clear());
    rejected(|t| {
        t.right.flags.pop();
    });
    rejected(|t| t.right.flags[0].allow_interrupt = true);
    rejected(|t| t.right.flags[1].repeat_ready = true);
    rejected(|t| {
        t.right.root_translations = Some(vec![0.0, 0.5]);
    });
    rejected(|t| {
        t.right.root_translations = Some(vec![f32::NAN, 0.5, 0.25, 0.0]);
    });
    rejected(|t| {
        if let Some(left) = t.left.as_mut() {
            left.flags[0].allow_interrupt = true;
        }
    });
}

#[test]
fn none_keeps_dpad_up_inert() {
    let mut resource = data();
    for fighter in &mut resource.fighters {
        fighter.taunt = None;
    }
    let mut game = Match::new(resource, 42).unwrap();
    let state = step(&mut game, dpad_up());
    assert_eq!(state.fighters[0].action, Action::Wait);
}

// Silence unused-import warnings for helpers only some configurations reach.
#[allow(dead_code)]
fn _unused(_: TauntAnimation, _: TauntFrame) {}
