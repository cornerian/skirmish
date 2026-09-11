//! End-to-end grounded tilts (forward variants, up, down with its buffered
//! repeat) and their input chains in an explicitly synthetic native world.
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Match, State, data::MatchData,
    shield,
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

/// Fighter 0 (facing +X at -2) attacks; fighter 1 (facing -X at +2) idles.
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
    }
    tilt_support::profile(grab_support::profile(data))
}

fn staling(mut data: MatchData) -> MatchData {
    data.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02, 0.01],
        debug_bypass: false,
    });
    data
}

fn input(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn step(game: &mut Match, attacker: Controller) -> State {
    game.step([attacker, Controller::default()])
        .unwrap()
        .clone()
}

fn from_wait(stick: [f32; 2]) -> Action {
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, input(BUTTON_A, stick)).fighters[0].action
}

#[test]
fn forward_tilt_variants_follow_the_folded_stick_angle_and_supplied_availability() {
    for (stick, expected) in [
        ([1.0, 0.0], Action::AttackS3S),
        ([0.5, 0.0], Action::AttackS3S),
        ([1.0, 0.5], Action::AttackS3Hi),
        ([1.0, 0.3], Action::AttackS3HiS),
        ([1.0, -0.3], Action::AttackS3LwS),
        ([1.0, -0.5], Action::AttackS3Lw),
        ([0.49, 0.0], Action::Jab),
        ([0.6, 0.6], Action::AttackHi3),
        ([-1.0, 0.0], Action::Jab),
    ] {
        assert_eq!(from_wait(stick), expected, "stick {stick:?}");
    }
    // Fighter 1 faces -X, so its forward stick is negative.
    let mut mirrored = Match::new(data(), 42).unwrap();
    let state = mirrored
        .step([Controller::default(), input(BUTTON_A, [-1.0, 0.4])])
        .unwrap()
        .clone();
    assert_eq!(state.fighters[1].action, Action::AttackS3HiS);

    let mut without_high = data();
    without_high.fighters[0]
        .tilts
        .as_mut()
        .unwrap()
        .forward
        .high = None;
    let mut game = Match::new(without_high, 42).unwrap();
    assert_eq!(
        step(&mut game, input(BUTTON_A, [1.0, 0.5])).fighters[0].action,
        Action::AttackS3HiS
    );
    let mut straight_only = data();
    let forward = &mut straight_only.fighters[0].tilts.as_mut().unwrap().forward;
    forward.high = None;
    forward.high_slight = None;
    forward.low_slight = None;
    forward.low = None;
    for stick in [[1.0, 0.5], [1.0, -0.5], [1.0, 0.0]] {
        let mut game = Match::new(straight_only.clone(), 42).unwrap();
        assert_eq!(
            step(&mut game, input(BUTTON_A, stick)).fighters[0].action,
            Action::AttackS3S,
            "stick {stick:?}"
        );
    }
}

#[test]
fn up_down_and_jab_follow_the_shared_order_from_every_grounded_state() {
    assert_eq!(from_wait([0.0, 1.0]), Action::AttackHi3);
    assert_eq!(from_wait([0.0, -1.0]), Action::AttackLw3);
    assert_eq!(from_wait([0.0, 0.0]), Action::Jab);

    let mut walking = Match::new(data(), 42).unwrap();
    assert_eq!(
        step(&mut walking, input(0, [0.3, 0.0])).fighters[0].action,
        Action::Walk
    );
    assert_eq!(
        step(&mut walking, input(BUTTON_A, [1.0, 0.0])).fighters[0].action,
        Action::AttackS3S
    );

    // Turn evaluates the chain with the post-turn facing before flipping.
    let mut turning = Match::new(data(), 42).unwrap();
    let turn = step(&mut turning, input(0, [-1.0, 0.0]));
    assert_eq!(turn.fighters[0].action, Action::Turn);
    assert!(!turn.fighters[0].locomotion.turn_has_turned);
    let tilted = step(&mut turning, input(BUTTON_A, [-1.0, 0.0]));
    assert_eq!(tilted.fighters[0].action, Action::AttackS3S);
    assert_eq!(tilted.fighters[0].facing, -1.0);

    let mut squatting = Match::new(data(), 42).unwrap();
    assert_eq!(
        step(&mut squatting, input(0, [0.0, -1.0])).fighters[0].action,
        Action::Squat
    );
    assert_eq!(
        step(&mut squatting, input(BUTTON_A, [0.0, -1.0])).fighters[0].action,
        Action::AttackLw3
    );

    let mut crouched = Match::new(data(), 42).unwrap();
    for _ in 0..7 {
        step(&mut crouched, input(0, [0.0, -1.0]));
    }
    assert_eq!(crouched.state().fighters[0].action, Action::SquatWait);
    assert_eq!(
        step(&mut crouched, input(BUTTON_A, [0.0, -1.0])).fighters[0].action,
        Action::AttackLw3
    );

    let mut rising = Match::new(data(), 42).unwrap();
    for _ in 0..7 {
        step(&mut rising, input(0, [0.0, -1.0]));
    }
    assert_eq!(
        step(&mut rising, input(0, [0.0, 0.0])).fighters[0].action,
        Action::SquatRv
    );
    assert_eq!(
        step(&mut rising, input(BUTTON_A, [0.0, 0.0])).fighters[0].action,
        Action::Jab
    );
}

#[test]
fn tilts_end_in_wait_or_squat_wait_and_open_their_chains_only_when_flagged() {
    // Forward tilt: seven samples, Wait chain from sample four.
    for (buttons, stick, expected) in [
        (BUTTON_X, [0.0, 0.0], Action::JumpSquat),
        (BUTTON_L, [0.0, 0.0], Action::GuardOn),
        (BUTTON_Z, [0.0, 0.0], Action::Catch),
        (BUTTON_A, [0.0, 1.0], Action::AttackHi3),
        (0, [1.0, 0.0], Action::Dash),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, input(BUTTON_A, [1.0, 0.0]));
        for _ in 1..4 {
            let state = step(&mut game, input(buttons, stick));
            assert_eq!(state.fighters[0].action, Action::AttackS3S, "{buttons:#x}");
        }
        let released = step(&mut game, input(0, [0.0, 0.0]));
        assert_eq!(released.fighters[0].action, Action::AttackS3S);
        assert_eq!(released.fighters[0].action_frame, 5);
        let state = step(&mut game, input(buttons, stick));
        assert_eq!(state.fighters[0].action, expected, "{buttons:#x} {stick:?}");
    }
    let mut finishing = Match::new(data(), 42).unwrap();
    step(&mut finishing, input(BUTTON_A, [0.0, 1.0]));
    for _ in 1..6 {
        assert_eq!(
            step(&mut finishing, input(0, [0.0, 0.0])).fighters[0].action,
            Action::AttackHi3
        );
    }
    assert_eq!(
        step(&mut finishing, input(0, [0.0, 0.0])).fighters[0].action,
        Action::Wait
    );

    // Down tilt: its chain excludes shield, grab and specials.
    for (buttons, stick, expected) in [
        (BUTTON_L, [0.0, 0.0], Action::AttackLw3),
        (BUTTON_Z, [0.0, 0.0], Action::AttackLw3),
        (BUTTON_X, [0.0, 0.0], Action::JumpSquat),
        (BUTTON_A, [1.0, 0.0], Action::AttackS3S),
        (0, [-1.0, 0.0], Action::Turn),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, input(BUTTON_A, [0.0, -1.0]));
        for _ in 1..4 {
            step(&mut game, input(0, [0.0, 0.0]));
        }
        let state = step(&mut game, input(buttons, stick));
        assert_eq!(state.fighters[0].action, expected, "{buttons:#x} {stick:?}");
    }
    // Holding down keeps the SquatWait that ftCo_800D638C enters; a neutral
    // stick would continue into SquatRv on the same callback.
    let mut finishing = Match::new(data(), 42).unwrap();
    step(&mut finishing, input(BUTTON_A, [0.0, -1.0]));
    for _ in 1..6 {
        step(&mut finishing, input(0, [0.0, 0.0]));
    }
    assert_eq!(
        step(&mut finishing, input(0, [0.0, -1.0])).fighters[0].action,
        Action::SquatWait
    );
    let mut released = Match::new(data(), 42).unwrap();
    step(&mut released, input(BUTTON_A, [0.0, -1.0]));
    for _ in 1..6 {
        step(&mut released, input(0, [0.0, 0.0]));
    }
    assert_eq!(
        step(&mut released, input(0, [0.0, 0.0])).fighters[0].action,
        Action::SquatRv
    );
}

#[test]
fn down_tilt_buffers_an_early_press_and_repeats_once_the_script_allows() {
    let mut buffered = Match::new(staling(data()), 42).unwrap();
    let wait_id = buffered.state().fighters[0].action_instance.id;
    let entry = step(&mut buffered, input(BUTTON_A, [0.0, -1.0]));
    assert_eq!(entry.fighters[0].action, Action::AttackLw3);
    // Ft_MF_SkipAttackCount keeps the Wait instance; the stale identity moves.
    assert_eq!(entry.fighters[0].action_instance.id, wait_id);
    assert_eq!(entry.fighters[0].staling.identity.move_id, 8);
    let stale_instance = entry.fighters[0].staling.identity.attack_instance;
    // A held press is not fresh; release it, then press again on sample 2,
    // before the repeat flag, to arm the buffer.
    let released = step(&mut buffered, input(0, [0.0, 0.0]));
    assert_eq!(released.fighters[0].action_frame, 2);
    assert!(!released.fighters[0].tilt.repeat_buffered);
    let early = step(&mut buffered, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(early.fighters[0].action, Action::AttackLw3);
    assert_eq!(early.fighters[0].action_frame, 3);
    assert!(early.fighters[0].tilt.repeat_buffered);
    // Sample 3 raises the repeat flag: the animation callback restarts.
    let repeated = step(&mut buffered, input(0, [0.0, 0.0]));
    assert_eq!(repeated.fighters[0].action, Action::AttackLw3);
    assert_eq!(repeated.fighters[0].action_frame, 1);
    assert!(!repeated.fighters[0].tilt.repeat_buffered);
    assert_ne!(repeated.fighters[0].action_instance.id, wait_id);
    assert_eq!(repeated.fighters[0].staling.identity.move_id, 8);
    assert_ne!(
        repeated.fighters[0].staling.identity.attack_instance,
        stale_instance
    );

    let mut direct = Match::new(data(), 42).unwrap();
    step(&mut direct, input(BUTTON_A, [0.0, -1.0]));
    for _ in 1..3 {
        step(&mut direct, input(0, [0.0, 0.0]));
    }
    let state = step(&mut direct, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AttackLw3);
    assert_eq!(state.fighters[0].action_frame, 1);

    let mut plain = Match::new(data(), 42).unwrap();
    step(&mut plain, input(BUTTON_A, [0.0, -1.0]));
    for frame in 1..6 {
        let state = step(&mut plain, input(0, [0.0, 0.0]));
        assert_eq!(state.fighters[0].action, Action::AttackLw3);
        assert_eq!(state.fighters[0].action_frame, frame + 1);
    }
    let ended = step(&mut plain, input(0, [0.0, -1.0]));
    assert_eq!(ended.fighters[0].action, Action::SquatWait);
}

#[test]
fn a_with_a_held_shoulder_grabs_from_the_wait_family_like_z() {
    for controller in [
        input(BUTTON_A | BUTTON_L, [0.0, 0.0]),
        Controller {
            buttons: BUTTON_A,
            trigger: 0.4,
            ..Default::default()
        },
        input(BUTTON_Z, [0.0, 0.0]),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        assert_eq!(
            step(&mut game, controller).fighters[0].action,
            Action::Catch
        );
    }
    // ftCo_SquatWait_IASA never reaches the catch check: Z is the logical A
    // press and the held stick selects the down tilt instead.
    let mut crouched = Match::new(data(), 42).unwrap();
    for _ in 0..7 {
        step(&mut crouched, input(0, [0.0, -1.0]));
    }
    assert_eq!(
        step(&mut crouched, input(BUTTON_Z, [0.0, -1.0])).fighters[0].action,
        Action::AttackLw3
    );
    let mut running = Match::new(data(), 42).unwrap();
    for _ in 0..9 {
        step(&mut running, input(0, [1.0, 0.0]));
    }
    assert_eq!(running.state().fighters[0].action, Action::Run);
    assert_eq!(
        step(&mut running, input(BUTTON_A | BUTTON_L, [1.0, 0.0])).fighters[0].action,
        Action::CatchDash
    );
}

#[test]
fn checkpoints_restore_tilts_and_the_buffered_repeat() {
    for stick in [[1.0, 0.3], [0.0, 1.0], [0.0, -1.0]] {
        let mut game = Match::new(staling(data()), 42).unwrap();
        step(&mut game, input(BUTTON_A, stick));
        step(&mut game, input(BUTTON_A, [0.0, -1.0]));
        let checkpoint = game.checkpoint();
        let inputs: Vec<_> = (0..12)
            .map(|frame| {
                [
                    input(
                        if frame % 3 == 0 { BUTTON_A } else { 0 },
                        if frame % 4 == 2 {
                            [1.0, 0.0]
                        } else {
                            [0.0, -1.0]
                        },
                    ),
                    Controller::default(),
                ]
            })
            .collect();
        let expected: Vec<_> = inputs
            .iter()
            .map(|&input| serde_json::to_vec(game.step(input).unwrap()).unwrap())
            .collect();
        game.restore_checkpoint(&checkpoint).unwrap();
        for (input, expected) in inputs.into_iter().zip(expected) {
            assert_eq!(
                serde_json::to_vec(game.step(input).unwrap()).unwrap(),
                expected,
                "{stick:?}"
            );
        }
    }
}

#[test]
fn invalid_tilt_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    assert!(Match::new(data(), 0).is_ok());
    assert!(Match::new(staling(data()), 0).is_ok());
    rejected(|d| d.rules.tilt.as_mut().unwrap().forward_stick_threshold = 1.5);
    rejected(|d| d.rules.tilt.as_mut().unwrap().angle_limit = 0.0);
    rejected(|d| d.rules.tilt.as_mut().unwrap().forward_high = f32::NAN);
    rejected(|d| d.rules.tilt.as_mut().unwrap().up_stick_threshold = 2.0);
    rejected(|d| d.rules.tilt.as_mut().unwrap().down_stick_threshold = 0.5);
    rejected(|d| {
        d.fighters[0].tilts.as_mut().unwrap().up.flags.pop();
    });
    rejected(|d| {
        d.fighters[1].tilts.as_mut().unwrap().forward.straight.flags[2].repeat_ready = true;
    });
    rejected(|d| {
        d.rules.staling = staling(data()).rules.staling;
        d.fighters[0]
            .tilts
            .as_mut()
            .unwrap()
            .forward
            .low
            .as_mut()
            .unwrap()
            .attack
            .move_id = Some(9);
    });
    rejected(|d| {
        d.fighters[0].tilts.as_mut().unwrap().down.attack.frames[1].hitboxes[0].radius = -1.0;
    });
    rejected(|d| d.fighters[0].tilts = None);
    rejected(|d| d.rules.tilt = None);
    rejected(|d| d.fighters[1].locomotion = None);
}
