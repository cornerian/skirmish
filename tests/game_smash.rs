//! End-to-end grounded smashes (forward variants with stick-sign facing, up,
//! down, C-stick entry, the charge state machine, charged damage, the
//! charging-victim knockback multiplier and TransN root motion) in an
//! explicitly synthetic native world.
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Match, State,
    damage::Armor,
    data::MatchData,
    shield,
    smash::{ChargeState, Rules},
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

/// Fighter 0 (facing +X at -2) attacks; fighter 1 (facing -X at +20) idles
/// out of reach so hitlag never freezes the attacker's samples.
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
    data.stage.spawns = [[-2.0, 0.0], [20.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
    }
    smash_support::profile(tilt_support::profile(grab_support::profile(data)))
}

/// Both fighters within the forward hitbox's reach.
fn close() -> MatchData {
    let mut data = data();
    data.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
    data.rules.knockback_speed = 0.15;
    data
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

fn cstick(cstick: [f32; 2]) -> Controller {
    Controller {
        cstick,
        ..Default::default()
    }
}

fn step(game: &mut Match, attacker: Controller) -> State {
    step_both(game, attacker, Controller::default())
}

fn step_both(game: &mut Match, attacker: Controller, defender: Controller) -> State {
    game.step([attacker, defender]).unwrap().clone()
}

fn from_wait(controller: Controller) -> State {
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, controller)
}

fn magnitude([x, y]: [f32; 2]) -> f32 {
    (x * x + y * y).sqrt()
}

#[test]
fn forward_smash_variants_follow_the_stick_angle_and_adopt_the_stick_sign() {
    for (stick, expected) in [
        ([1.0, 0.0], Action::AttackS4S),
        ([0.8, 0.0], Action::AttackS4S),
        ([1.0, 0.5], Action::AttackS4Hi),
        ([1.0, 0.3], Action::AttackS4HiS),
        ([1.0, -0.3], Action::AttackS4LwS),
        ([1.0, -0.5], Action::AttackS4Lw),
        // Below the dash magnitude the tilt chain answers instead.
        ([0.79, 0.0], Action::AttackS3S),
    ] {
        let state = from_wait(input(BUTTON_A, stick));
        assert_eq!(state.fighters[0].action, expected, "stick {stick:?}");
        assert_eq!(state.fighters[0].facing, 1.0);
        assert_eq!(state.fighters[0].action_frame, 1);
    }
    // A backward stick turns the fighter around inside the same frame.
    let reversed = from_wait(input(BUTTON_A, [-1.0, 0.5]));
    assert_eq!(reversed.fighters[0].action, Action::AttackS4Hi);
    assert_eq!(reversed.fighters[0].facing, -1.0);
    assert_eq!(reversed.fighters[0].smash.charge, ChargeState::None);
    // Z is the logical A press with a held shoulder: the Wait chain's catch
    // check answers first, so the folded press shows from the down tilt's
    // narrower chain below.
    assert_eq!(
        from_wait(input(BUTTON_Z, [1.0, 0.0])).fighters[0].action,
        Action::Catch
    );

    // The stick must be fresh: past the dash window a smash-magnitude stick
    // with A is a tilt (the tilt age counts from the walk deadzone).
    let mut aged = Match::new(data(), 42).unwrap();
    for _ in 0..5 {
        assert_eq!(
            step(&mut aged, input(0, [0.6, 0.0])).fighters[0].action,
            Action::Walk
        );
    }
    assert_eq!(
        step(&mut aged, input(BUTTON_A, [1.0, 0.0])).fighters[0].action,
        Action::AttackS3S
    );

    let mut without_high = data();
    without_high.fighters[0]
        .smashes
        .as_mut()
        .unwrap()
        .forward
        .high = None;
    let mut game = Match::new(without_high, 42).unwrap();
    assert_eq!(
        step(&mut game, input(BUTTON_A, [1.0, 0.5])).fighters[0].action,
        Action::AttackS4HiS
    );
}

#[test]
fn up_down_and_cstick_smashes_precede_tilts_and_keep_the_chain_facing() {
    assert_eq!(
        from_wait(input(BUTTON_A, [0.0, 1.0])).fighters[0].action,
        Action::AttackHi4
    );
    assert_eq!(
        from_wait(input(BUTTON_A, [0.0, 0.7])).fighters[0].action,
        Action::AttackHi4
    );
    assert_eq!(
        from_wait(input(BUTTON_A, [0.0, 0.69])).fighters[0].action,
        Action::AttackHi3
    );
    assert_eq!(
        from_wait(input(BUTTON_A, [0.0, -1.0])).fighters[0].action,
        Action::AttackLw4
    );
    assert_eq!(
        from_wait(input(BUTTON_A, [0.0, -0.69])).fighters[0].action,
        Action::AttackLw3
    );
    // Forward smashes come first: a diagonal beyond both thresholds.
    assert_eq!(
        from_wait(input(BUTTON_A, [0.8, 0.8])).fighters[0].action,
        Action::AttackS4Hi
    );
    // A fresh C-stick crossing needs no button.
    for (stick, expected) in [
        ([1.0, 0.0], Action::AttackS4S),
        ([-0.8, 0.3], Action::AttackS4HiS),
        ([0.0, 0.7], Action::AttackHi4),
        ([0.0, -0.7], Action::AttackLw4),
        ([0.0, 0.69], Action::Wait),
    ] {
        let state = from_wait(cstick(stick));
        assert_eq!(state.fighters[0].action, expected, "cstick {stick:?}");
    }
    assert_eq!(from_wait(cstick([-0.8, 0.3])).fighters[0].facing, -1.0);

    // The stick-age window is strict: age 3 passes and age 4 fails (the
    // held down stick crouches meanwhile).
    for (held, expected) in [(3, Action::AttackLw4), (4, Action::AttackLw3)] {
        let mut aged = Match::new(data(), 42).unwrap();
        for _ in 0..held {
            assert_eq!(
                step(&mut aged, input(0, [0.0, -1.0])).fighters[0].action,
                Action::Squat
            );
        }
        assert_eq!(
            step(&mut aged, input(BUTTON_A, [0.0, -1.0])).fighters[0].action,
            expected,
            "held {held}"
        );
    }

    // Turn evaluates the chain with the post-turn facing; up and down
    // smashes keep it.
    let mut turning = Match::new(data(), 42).unwrap();
    let turn = step(&mut turning, input(0, [-1.0, 0.0]));
    assert_eq!(turn.fighters[0].action, Action::Turn);
    assert!(!turn.fighters[0].locomotion.turn_has_turned);
    let smashed = step(&mut turning, input(BUTTON_A, [0.0, 1.0]));
    assert_eq!(smashed.fighters[0].action, Action::AttackHi4);
    assert_eq!(smashed.fighters[0].facing, -1.0);
    let mut turning_down = Match::new(data(), 42).unwrap();
    step(&mut turning_down, input(0, [-1.0, 0.0]));
    let smashed = step(&mut turning_down, cstick([0.0, -1.0]));
    assert_eq!(smashed.fighters[0].action, Action::AttackLw4);
    assert_eq!(smashed.fighters[0].facing, -1.0);

    // Every grounded chain state reaches the smashes.
    let mut crouched = Match::new(data(), 42).unwrap();
    for _ in 0..7 {
        step(&mut crouched, input(0, [0.0, -1.0]));
    }
    assert_eq!(crouched.state().fighters[0].action, Action::SquatWait);
    assert_eq!(
        step(&mut crouched, cstick([0.0, -1.0])).fighters[0].action,
        Action::AttackLw4
    );
    let mut walking = Match::new(data(), 42).unwrap();
    assert_eq!(
        step(&mut walking, input(0, [0.3, 0.0])).fighters[0].action,
        Action::Walk
    );
    assert_eq!(
        step(&mut walking, cstick([0.0, 1.0])).fighters[0].action,
        Action::AttackHi4
    );

    // A same-frame shield press yields to the C-stick smash.
    assert_eq!(
        from_wait(Controller {
            buttons: BUTTON_L,
            cstick: [1.0, 0.0],
            ..Default::default()
        })
        .fighters[0]
            .action,
        Action::AttackS4S
    );
}

#[test]
fn jump_squat_takes_a_windowless_up_smash_after_the_catch() {
    let mut game = Match::new(data(), 42).unwrap();
    for _ in 0..6 {
        step(&mut game, input(0, [0.0, 0.7]));
    }
    assert_eq!(
        step(&mut game, input(BUTTON_X, [0.0, 0.7])).fighters[0].action,
        Action::JumpSquat
    );
    // The stick has aged past the window, which KneeBend ignores.
    let smashed = step(&mut game, input(BUTTON_A | BUTTON_X, [0.0, 0.7]));
    assert_eq!(smashed.fighters[0].action, Action::AttackHi4);

    let mut cstick_jump = Match::new(data(), 42).unwrap();
    step(&mut cstick_jump, input(BUTTON_X, [0.0, 0.0]));
    assert_eq!(
        step(&mut cstick_jump, cstick([0.0, 1.0])).fighters[0].action,
        Action::AttackHi4
    );

    let mut grabbing = Match::new(data(), 42).unwrap();
    step(&mut grabbing, input(BUTTON_X, [0.0, 0.0]));
    assert_eq!(
        step(&mut grabbing, input(BUTTON_Z, [0.0, 1.0])).fighters[0].action,
        Action::Catch
    );

    let mut below = Match::new(data(), 42).unwrap();
    step(&mut below, input(BUTTON_X, [0.0, 0.0]));
    assert_eq!(
        step(&mut below, input(BUTTON_A, [0.0, 0.69])).fighters[0].action,
        Action::JumpSquat
    );
}

/// Drive a straight forward smash from Wait and return the victim's percent
/// and the attacker's cached hit after the first hitbox sample.
fn charged(data: MatchData, held_frames: usize) -> (State, Vec<(ChargeState, f32, u32)>) {
    let mut game = Match::new(data, 42).unwrap();
    let mut trace = Vec::new();
    let mut record = |state: &State| {
        let fighter = &state.fighters[0];
        trace.push((
            fighter.smash.charge,
            fighter.smash.frames,
            fighter.action_frame,
        ));
    };
    let mut state = step(&mut game, input(BUTTON_A, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AttackS4S);
    record(&state);
    let mut frame = 1;
    while state.fighters[0].action == Action::AttackS4S
        && state.fighters[0]
            .hitboxes
            .iter()
            .all(|track| track.group.is_none())
    {
        let buttons = if frame < held_frames { BUTTON_A } else { 0 };
        state = step(&mut game, input(buttons, [0.0, 0.0]));
        record(&state);
        frame += 1;
        assert!(frame < 60, "no hitbox");
    }
    assert!(
        state.fighters[0]
            .hitboxes
            .iter()
            .any(|track| track.group.is_some())
    );
    (state, trace)
}

#[test]
fn a_released_press_leaves_the_charge_unarmed_and_the_damage_unscaled() {
    let (hit, trace) = charged(close(), 1);
    assert_eq!(
        trace[..4],
        [
            (ChargeState::None, 0.0, 1),
            (ChargeState::None, 0.0, 2),
            // The pose-2 command arms, and the released A disarms it.
            (ChargeState::None, 0.0, 3),
            (ChargeState::None, 0.0, 4),
        ]
    );
    assert_eq!(hit.fighters[1].percent, 10.0);
    assert_eq!(hit.fighters[0].staling.hits[0].unwrap().base_damage, 10);
}

#[test]
fn a_held_press_charges_at_a_frozen_pose_until_release_or_the_hold_limit() {
    // Held through the limit: ten counted frames, then automatic release.
    let (hit, trace) = charged(close(), 30);
    assert_eq!(trace[1], (ChargeState::None, 0.0, 2));
    assert_eq!(trace[2], (ChargeState::Charging, 0.0, 2));
    for (frame, entry) in trace[3..12].iter().enumerate() {
        assert_eq!(*entry, (ChargeState::Charging, frame as f32 + 1.0, 2));
    }
    assert_eq!(trace[12], (ChargeState::Release, 10.0, 3));
    assert_eq!(hit.fighters[0].smash.charge, ChargeState::Release);
    assert_eq!(hit.fighters[1].percent, 15.0);
    assert_eq!(hit.fighters[0].staling.hits[0].unwrap().base_damage, 15);

    // Released after four counted frames: 10 * (0.5 * 0.4 + 1). The tick
    // precedes the input phase, so the release frame still counts.
    let (hit, trace) = charged(close(), 6);
    assert_eq!(trace[5], (ChargeState::Charging, 3.0, 2));
    assert_eq!(trace[6], (ChargeState::Release, 4.0, 3));
    assert_eq!(hit.fighters[1].percent, 12.0);

    // Three frames give 11.5: the stale count truncates, the stored damage
    // keeps the fraction and the fresh queue applies no penalty.
    let (hit, trace) = charged(staling(close()), 5);
    assert_eq!(trace[5], (ChargeState::Release, 3.0, 3));
    let cached = hit.fighters[0].staling.hits[0].unwrap();
    assert_eq!(cached.base_damage, 11);
    assert_eq!(cached.damage, 11.5);
    assert_eq!(hit.fighters[1].percent, 11.5);
    assert_eq!(hit.fighters[0].staling.identity.move_id, 9);
}

#[test]
fn a_charging_victim_takes_scaled_knockback_before_armor() {
    fn launch(data: MatchData, charging: bool) -> f32 {
        let mut game = Match::new(data, 42).unwrap();
        let defender = |frame: usize| {
            if charging && frame < 40 {
                input(BUTTON_A, [0.0, 1.0])
            } else {
                Controller::default()
            }
        };
        let entry = step_both(&mut game, input(BUTTON_A, [1.0, 0.0]), defender(0));
        assert_eq!(entry.fighters[0].action, Action::AttackS4S);
        if charging {
            assert_eq!(entry.fighters[1].action, Action::AttackHi4);
        }
        let mut frame = 1;
        loop {
            let state = step_both(&mut game, input(0, [0.0, 0.0]), defender(frame));
            frame += 1;
            if state.fighters[1].hitlag > 0.0 || state.fighters[1].percent > 0.0 {
                if charging {
                    assert_eq!(state.fighters[1].smash.charge, ChargeState::None);
                }
                return magnitude(state.fighters[1].knockback);
            }
            assert!(frame < 20, "no hit");
        }
    }
    // The stored vector is the launch velocity: knockback times the speed
    // rule, so armor subtracts 3 * 0.15 from it.
    let plain = launch(close(), false);
    let scaled = launch(close(), true);
    assert!(plain > 1.0, "{plain}");
    assert!((scaled - plain * 0.5).abs() < 1e-4, "{scaled} vs {plain}");
    let armor_velocity = 3.0 * 0.15;
    let mut armored = close();
    armored.fighters[1].armor = Some(Armor {
        armor0: 3.0,
        armor1: 1.0,
        minimum_knockback: 0.0,
    });
    let reduced = launch(armored, true);
    assert!(
        (reduced - (scaled - armor_velocity)).abs() < 1e-4,
        "{reduced} vs {scaled}"
    );
    assert!((reduced - (plain - armor_velocity) * 0.5).abs() > 0.1);
}

#[test]
fn forward_smashes_follow_their_root_motion_and_open_the_wait_chain_when_flagged() {
    let mut game = Match::new(data(), 42).unwrap();
    let entry = step(&mut game, input(BUTTON_A, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].position[0], -2.0);
    let mut expected = -2.0;
    for (sample, root) in smash_support::straight_roots().iter().enumerate().skip(1) {
        let state = step(&mut game, input(0, [0.0, 0.0]));
        expected += root;
        assert_eq!(
            state.fighters[0].action,
            Action::AttackS4S,
            "sample {sample}"
        );
        assert_eq!(state.fighters[0].position[0], expected, "sample {sample}");
        assert_eq!(state.fighters[0].action_frame, sample as u32 + 1);
    }
    assert_eq!(
        step(&mut game, input(0, [0.0, 0.0])).fighters[0].action,
        Action::Wait
    );

    // The mirrored fighter steps toward -X.
    let mut mirrored = Match::new(data(), 42).unwrap();
    let entry = step_both(
        &mut mirrored,
        Controller::default(),
        input(BUTTON_A, [-1.0, 0.0]),
    );
    assert_eq!(entry.fighters[1].action, Action::AttackS4S);
    for _ in 1..6 {
        step(&mut mirrored, Controller::default());
    }
    assert_eq!(mirrored.state().fighters[1].position[0], 18.0);

    // A frozen charge pose contributes no TransN delta; the pose's delta
    // applies once when the animation reaches it and the ordinary samples
    // resume after the release.
    let mut frozen = data();
    frozen.fighters[0]
        .smashes
        .as_mut()
        .unwrap()
        .forward
        .straight
        .root_translations = Some(vec![0.0, 0.0, 1.0, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0]);
    let mut game = Match::new(frozen, 42).unwrap();
    step(&mut game, input(BUTTON_A, [1.0, 0.0]));
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    let armed = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(armed.fighters[0].smash.charge, ChargeState::Charging);
    assert_eq!(armed.fighters[0].position[0], -1.0);
    for _ in 0..3 {
        let held = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
        assert!(held.fighters[0].smash.frozen);
        assert_eq!(held.fighters[0].position[0], -1.0);
    }
    let released = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(released.fighters[0].smash.charge, ChargeState::Release);
    assert_eq!(released.fighters[0].position[0], -1.0);
    assert_eq!(released.fighters[0].action_frame, 3);
    let resumed = step(&mut game, input(0, [0.0, 0.0]));
    assert!(!resumed.fighters[0].smash.frozen);
    assert_eq!(resumed.fighters[0].position[0], -0.5);

    // Samples 0..6 ignore the chain; sample 7 opens the complete Wait chain.
    for (controller, expected) in [
        (input(BUTTON_X, [0.0, 0.0]), Action::JumpSquat),
        (input(BUTTON_L, [0.0, 0.0]), Action::GuardOn),
        (input(BUTTON_Z, [0.0, 0.0]), Action::Catch),
        (input(BUTTON_A, [0.0, 1.0]), Action::AttackHi4),
        (input(BUTTON_A, [0.0, 0.0]), Action::Jab),
        (cstick([0.0, -1.0]), Action::AttackLw4),
        (input(0, [1.0, 0.0]), Action::Dash),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, input(BUTTON_A, [1.0, 0.0]));
        // Release A on sample 1 so the pose-2 command finds it released.
        for _ in 1..6 {
            step(&mut game, input(0, [0.0, 0.0]));
        }
        // Sample 6 is the last locked pose.
        let locked = step(&mut game, controller);
        assert_eq!(
            locked.fighters[0].action,
            Action::AttackS4S,
            "{controller:?}"
        );
        assert_eq!(locked.fighters[0].action_frame, 7);
        let neutral = step(&mut game, Controller::default());
        assert_eq!(neutral.fighters[0].action, Action::AttackS4S);
        let state = step(&mut game, controller);
        assert_eq!(state.fighters[0].action, expected, "{controller:?}");
    }

    // A C-stick held through the window is not fresh; releasing and crossing
    // again re-enters the smash.
    let mut held = Match::new(data(), 42).unwrap();
    step(&mut held, cstick([1.0, 0.0]));
    for frame in 1..9 {
        let state = step(&mut held, cstick([1.0, 0.0]));
        assert_eq!(state.fighters[0].action, Action::AttackS4S, "frame {frame}");
        assert_eq!(state.fighters[0].action_frame, frame as u32 + 1);
    }
    step(&mut held, cstick([0.0, 0.0]));
    let mut again = Match::new(data(), 42).unwrap();
    step(&mut again, cstick([1.0, 0.0]));
    for _ in 1..7 {
        step(&mut again, cstick([1.0, 0.0]));
    }
    step(&mut again, cstick([0.0, 0.0]));
    let reentered = step(&mut again, cstick([1.0, 0.0]));
    assert_eq!(reentered.fighters[0].action, Action::AttackS4S);
    assert_eq!(reentered.fighters[0].action_frame, 1);

    // Down smash from the down tilt's interruptible block and the up smash
    // ending in Wait.
    let mut from_down_tilt = Match::new(data(), 42).unwrap();
    step(&mut from_down_tilt, input(BUTTON_A, [0.0, -0.5]));
    assert_eq!(from_down_tilt.state().fighters[0].action, Action::AttackLw3);
    for _ in 1..4 {
        step(&mut from_down_tilt, input(0, [0.0, 0.0]));
    }
    assert_eq!(
        step(&mut from_down_tilt, cstick([0.0, -1.0])).fighters[0].action,
        Action::AttackLw4
    );
    // The down tilt's chain has no catch, so Z reads as the logical A press.
    let mut z_from_down_tilt = Match::new(data(), 42).unwrap();
    step(&mut z_from_down_tilt, input(BUTTON_A, [0.0, -0.5]));
    for _ in 1..4 {
        step(&mut z_from_down_tilt, input(0, [0.0, 0.0]));
    }
    assert_eq!(
        step(&mut z_from_down_tilt, input(BUTTON_Z, [0.0, 1.0])).fighters[0].action,
        Action::AttackHi4
    );
    let mut finishing = Match::new(data(), 42).unwrap();
    step(&mut finishing, input(BUTTON_A, [0.0, 1.0]));
    for _ in 1..9 {
        assert_eq!(
            step(&mut finishing, input(0, [0.0, 0.0])).fighters[0].action,
            Action::AttackHi4
        );
    }
    assert_eq!(
        step(&mut finishing, input(0, [0.0, 0.0])).fighters[0].action,
        Action::Wait
    );
}

#[test]
fn checkpoints_restore_smashes_in_every_charge_phase() {
    for held in [1, 3, 6, 14] {
        let mut game = Match::new(staling(close()), 42).unwrap();
        step(&mut game, input(BUTTON_A, [1.0, 0.0]));
        for _ in 1..held {
            step(&mut game, input(BUTTON_A, [0.0, 0.0]));
        }
        let checkpoint = game.checkpoint();
        let inputs: Vec<_> = (0..16)
            .map(|frame| {
                [
                    input(
                        if frame < 2 || frame % 5 == 0 {
                            BUTTON_A
                        } else {
                            0
                        },
                        if frame % 4 == 2 {
                            [1.0, 0.0]
                        } else {
                            [0.0, 0.0]
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
                "held {held}"
            );
        }
    }
}

#[test]
fn invalid_smash_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    fn rules(d: &mut MatchData) -> &mut Rules {
        d.rules.smash.as_mut().unwrap()
    }
    assert!(Match::new(data(), 0).is_ok());
    assert!(Match::new(staling(data()), 0).is_ok());
    rejected(|d| rules(d).forward_high = 4.0);
    rejected(|d| rules(d).forward_low = f32::NAN);
    rejected(|d| rules(d).up_stick_threshold = 1.5);
    rejected(|d| rules(d).up_window = 0.0);
    rejected(|d| rules(d).down_stick_threshold = 0.5);
    rejected(|d| rules(d).down_window = f32::INFINITY);
    rejected(|d| rules(d).charging_knockback_multiplier = -1.0);
    rejected(|d| {
        d.fighters[0].smashes.as_mut().unwrap().up.flags.pop();
    });
    rejected(|d| {
        d.fighters[1].smashes.as_mut().unwrap().down.flags[2].repeat_ready = true;
    });
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .up
            .charge
            .as_mut()
            .unwrap()
            .frame = 9;
    });
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .up
            .charge
            .as_mut()
            .unwrap()
            .hold_frames = 0.0;
    });
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .down
            .charge
            .as_mut()
            .unwrap()
            .damage_multiplier = f32::NAN;
    });
    // Hitboxes may not precede or share the charge pose.
    rejected(|d| {
        let up = &mut d.fighters[0].smashes.as_mut().unwrap().up;
        let hit = up.attack.frames[smash_support::HIT_FROM].hitboxes[0].clone();
        up.attack.frames[2].hitboxes.push(hit);
    });
    rejected(|d| {
        d.fighters[0].smashes.as_mut().unwrap().up.root_translations = Some(vec![0.0; 9]);
    });
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .forward
            .straight
            .root_translations = Some(vec![0.0; 9]);
    });
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .forward
            .straight
            .root_translations = Some(vec![f32::NAN; 10]);
    });
    // Forward variants share one move identity only when staling counts it.
    let mut distinct = data();
    distinct.fighters[0]
        .smashes
        .as_mut()
        .unwrap()
        .forward
        .low
        .as_mut()
        .unwrap()
        .attack
        .move_id = Some(12);
    assert!(Match::new(distinct.clone(), 0).is_ok());
    assert!(Match::new(staling(distinct), 0).is_err());
    rejected(|d| {
        d.fighters[0].smashes.as_mut().unwrap().down.attack.frames[4].hitboxes[0].radius = -1.0;
    });
    rejected(|d| d.fighters[0].smashes = None);
    rejected(|d| d.rules.smash = None);
    rejected(|d| d.fighters[1].locomotion = None);
    // A charge multiplier that would overflow the hitlag counter.
    rejected(|d| {
        d.fighters[0]
            .smashes
            .as_mut()
            .unwrap()
            .up
            .charge
            .as_mut()
            .unwrap()
            .damage_multiplier = 1_000_000.0;
    });
}
