//! Native bone-driven grab, capture, and four-direction throw scenarios.

#[path = "support/grab.rs"]
mod grab_resources;

use skirmish::{
    collision::ecb,
    game::{
        Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Event, Match, State,
        data::{CollisionBox, MatchData},
        grab,
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    grab_resources::profile(data)
}

fn with_locomotion(mut data: MatchData) -> MatchData {
    let locomotion = serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(locomotion);
    }
    data
}

fn input(player: usize, buttons: u16, stick: [f32; 2], cstick: [f32; 2]) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = Controller {
        buttons,
        stick,
        cstick,
        trigger: 0.0,
    };
    inputs
}

fn step(game: &mut Match, inputs: [Controller; 2]) -> State {
    game.step(inputs).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..120 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game, IDLE);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn held(mut data: MatchData) -> Match {
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    let mut game = Match::new(data, 7).unwrap();
    let caught = step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert!(caught.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));
    until(&mut game, |state| {
        state.fighters[0].action == Action::CatchWait
    });
    game
}

#[test]
fn catch_contact_uses_the_sampled_bone_pose_and_miss_recovers() {
    let mut animated = data();
    animated.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let parameters = animated.fighters[0].grab.as_mut().unwrap();
    for frame in &mut parameters.catch.frames {
        frame.bones[1].translation[0] = 2.0;
        if let Some(grabbox) = frame.grabboxes.first_mut() {
            grabbox.start = [0.0; 3];
            grabbox.end = [0.0; 3];
            grabbox.radius = 0.1;
        }
    }
    let mut reaches = Match::new(animated.clone(), 0).unwrap();
    let caught = step(&mut reaches, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert_eq!(caught.fighters[0].grab.victim, Some(1));

    for frame in &mut animated.fighters[0].grab.as_mut().unwrap().catch.frames {
        frame.bones[1].translation[0] = 0.0;
    }
    let mut misses = Match::new(animated, 0).unwrap();
    step(&mut misses, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    for _ in 0..8 {
        step(&mut misses, IDLE);
    }
    assert_eq!(misses.state().fighters[0].action, Action::Wait);
    assert!(
        misses
            .state()
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
}

#[test]
fn dash_and_run_use_dash_catch_poses_preserve_momentum_and_replay_misses() {
    let mut resource = with_locomotion(data());
    resource.stage.spawns = [[-2.0, 0.0], [6.0, 0.0]];
    for frame in &mut resource.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .catch_dash
        .frames
    {
        if let Some(grabbox) = frame.grabboxes.first_mut() {
            frame.bones[1].translation[0] = 6.0;
            grabbox.start[0] = 0.0;
            grabbox.end[0] = 0.0;
            grabbox.radius = 0.2;
        }
    }
    let mut contact = Match::new(resource, 12).unwrap();
    let dashed = step(&mut contact, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(dashed.fighters[0].action, Action::Dash);
    let caught = step(&mut contact, input(0, BUTTON_Z, [1.0, 0.0], [0.0; 2]));
    assert_eq!(caught.fighters[0].action, Action::CatchDashPull);
    assert_eq!(caught.fighters[1].action, Action::CapturePulled);
    assert!(caught.fighters[0].position[0] > dashed.fighters[0].position[0]);
    assert!(caught.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));
    let waiting = until(&mut contact, |state| {
        state.fighters[0].action == Action::CatchWait
    });
    assert_eq!(waiting.fighters[1].action, Action::CaptureWait);

    let mut resource = with_locomotion(data());
    resource.stage.spawns = [[-8.0, 0.0], [8.0, 0.0]];
    let mut miss = Match::new(resource, 12).unwrap();
    step(&mut miss, input(0, 0, [1.0, 0.0], [0.0; 2]));
    let checkpoint = miss.checkpoint();
    let entered = step(&mut miss, input(0, BUTTON_Z, [1.0, 0.0], [0.0; 2]));
    assert_eq!(entered.fighters[0].action, Action::CatchDash);
    miss.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        step(&mut miss, input(0, BUTTON_Z, [1.0, 0.0], [0.0; 2])),
        entered
    );
    let recovered = until(&mut miss, |state| state.fighters[0].action == Action::Wait);
    assert_eq!(recovered.fighters[0].grab, grab::State::default());

    let mut run_resource = with_locomotion(data());
    run_resource.stage.floor.left = -100.0;
    run_resource.stage.floor.right = 100.0;
    run_resource.stage.spawns = [[-20.0, 0.0], [20.0, 0.0]];
    run_resource.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    let mut run = Match::new(run_resource, 12).unwrap();
    step(&mut run, input(0, 0, [1.0, 0.0], [0.0; 2]));
    for _ in 0..12 {
        if run.state().fighters[0].action == Action::Run {
            break;
        }
        step(&mut run, input(0, 0, [1.0, 0.0], [0.0; 2]));
    }
    assert_eq!(run.state().fighters[0].action, Action::Run);
    assert_eq!(
        step(&mut run, input(0, BUTTON_Z, [1.0, 0.0], [0.0; 2])).fighters[0].action,
        Action::CatchDash
    );
}

#[test]
fn turn_grab_applies_the_pending_facing_before_standing_catch_contact() {
    let mut resource = with_locomotion(data());
    resource.stage.spawns = [[1.0, 0.0], [-1.0, 0.0]];
    let mut game = Match::new(resource, 13).unwrap();

    let turning = step(&mut game, input(0, 0, [-1.0, 0.0], [0.0; 2]));
    assert_eq!(turning.fighters[0].action, Action::Turn);
    assert_eq!(turning.fighters[0].facing, 1.0);
    let caught = step(&mut game, input(0, BUTTON_Z, [-1.0, 0.0], [0.0; 2]));
    assert_eq!(caught.fighters[0].facing, -1.0);
    assert_eq!(caught.fighters[0].action, Action::CatchPull);
    assert_eq!(caught.fighters[1].action, Action::CapturePulled);
    assert!(caught.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));

    let mut completed = Match::new(with_locomotion(data()), 13).unwrap();
    step(&mut completed, input(0, 0, [-1.0, 0.0], [0.0; 2]));
    until(&mut completed, |state| {
        state.fighters[0].locomotion.turn_has_turned
    });
    assert_eq!(completed.state().fighters[0].facing, -1.0);
    let entered = step(&mut completed, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert_eq!(entered.fighters[0].action, Action::Catch);
    assert_eq!(entered.fighters[0].facing, -1.0);
}

#[test]
fn crouch_startup_accepts_the_source_standing_catch_transition() {
    let mut resource = with_locomotion(data());
    resource.stage.spawns = [[-8.0, 0.0], [8.0, 0.0]];
    let mut game = Match::new(resource, 14).unwrap();

    let crouched = step(&mut game, input(0, 0, [0.0, -1.0], [0.0; 2]));
    assert_eq!(crouched.fighters[0].action, Action::Squat);
    let caught = step(&mut game, input(0, BUTTON_Z, [0.0, -1.0], [0.0; 2]));
    assert_eq!(caught.fighters[0].action, Action::Catch);
    assert_eq!(caught.fighters[0].grab, grab::State::default());
}

#[test]
fn all_four_throw_directions_release_into_the_shared_damage_pipeline() {
    let cases = [
        ([1.0, 0.0], Action::ThrowF, Action::ThrownF, [1, 0]),
        ([-1.0, 0.0], Action::ThrowB, Action::ThrownB, [-1, 0]),
        ([0.0, 1.0], Action::ThrowHi, Action::ThrownHi, [0, 1]),
        ([0.0, -1.0], Action::ThrowLw, Action::ThrownLw, [0, -1]),
    ];
    for (stick, holder_action, victim_action, direction) in cases {
        let mut game = held(data());
        let entered = step(&mut game, input(0, 0, stick, [0.0; 2]));
        assert_eq!(entered.fighters[0].action, holder_action);
        assert_eq!(entered.fighters[1].action, victim_action);
        let released = until(&mut game, |state| state.fighters[1].percent > 0.0);
        assert_eq!(released.fighters[1].percent, 8.0);
        assert_eq!(released.fighters[1].facing, -1.0);
        assert!(
            released
                .fighters
                .iter()
                .all(|fighter| fighter.grab == grab::State::default())
        );
        if direction[0] != 0 {
            assert_eq!(
                released.fighters[1].knockback[0].signum(),
                direction[0] as f32
            );
        }
        if direction[1] != 0 {
            assert_eq!(
                released.fighters[1].knockback[1].signum(),
                direction[1] as f32
            );
        }
        assert!(released.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                damage: 8.0,
                ..
            }
        )));
    }
}

#[test]
fn cstick_throws_and_source_priority_select_main_horizontal_first() {
    let mut cstick = held(data());
    let entered = step(&mut cstick, input(0, 0, [0.0; 2], [0.0, 1.0]));
    assert_eq!(entered.fighters[0].action, Action::ThrowHi);

    let mut priority = held(data());
    let entered = step(&mut priority, input(0, 0, [-1.0, -1.0], [1.0, 1.0]));
    assert_eq!(entered.fighters[0].action, Action::ThrowB);
}

#[test]
fn held_stick_does_not_become_a_fresh_throw_when_pull_enters_wait() {
    let mut game = Match::new(data(), 0).unwrap();
    step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    let waiting = step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(waiting.fighters[0].action, Action::CatchWait);
    step(&mut game, input(0, 0, [0.0; 2], [0.0; 2]));
    let fresh = step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(fresh.fighters[0].action, Action::ThrowF);
}

#[test]
fn capture_ignores_victim_actions_and_checkpoint_replays_the_pair_exactly() {
    let mut game = held(data());
    let held_position = game.state().fighters[1].position;
    for buttons in [BUTTON_A, BUTTON_X, BUTTON_A | BUTTON_X] {
        let state = step(&mut game, input(1, buttons, [1.0, 1.0], [1.0, 1.0]));
        assert_eq!(state.fighters[1].action, Action::CaptureWait);
        assert_eq!(state.fighters[1].position, held_position);
        assert_eq!(state.fighters[1].grab.captor, Some(0));
    }

    let checkpoint = game.checkpoint();
    let suffix = [input(0, 0, [1.0, 0.0], [0.0; 2]), IDLE, IDLE, IDLE, IDLE];
    let expected = suffix.map(|inputs| step(&mut game, inputs));
    game.restore_checkpoint(&checkpoint).unwrap();
    for (inputs, expected) in suffix.into_iter().zip(expected) {
        assert_eq!(step(&mut game, inputs), expected);
    }
}

#[test]
fn fresh_pummel_has_priority_over_throw_and_replays_its_single_captured_hit() {
    let mut resource = data();
    resource.fighters[0].grab.as_mut().unwrap().pummel.poses[1][1].translation[0] += 2.0;
    let mut game = held(resource);
    let held_position = game.state().fighters[1].position;

    let entered = step(&mut game, input(0, BUTTON_A, [1.0, 0.0], [0.0; 2]));
    assert_eq!(entered.fighters[0].action, Action::CatchAttack);
    assert_eq!(entered.fighters[1].action, Action::CaptureWait);
    assert_eq!(entered.fighters[0].grab.victim, Some(1));
    let checkpoint = game.checkpoint();

    let hit = step(&mut game, IDLE);
    assert_eq!(hit.fighters[1].percent, 3.0);
    assert_eq!(hit.fighters[1].action, Action::CaptureDamage);
    assert!(hit.fighters[0].grab.pummel_hit);
    assert!(hit.fighters.iter().all(|fighter| fighter.hitlag == 2.0));
    assert!(hit.fighters[1].position[0] > held_position[0] + 1.0);
    assert!(hit.events.contains(&Event::Hit {
        attacker: 0,
        victim: 1,
        damage: 3.0,
        knockback: 0.0,
    }));

    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(step(&mut game, IDLE), hit);
    let waiting = until(&mut game, |state| {
        state.fighters[0].action == Action::CatchWait
    });
    assert_eq!(waiting.fighters[1].percent, 3.0);
    assert!(!waiting.fighters[0].grab.pummel_hit);
    assert_eq!(waiting.fighters[0].grab.victim, Some(1));

    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let second = until(&mut game, |state| state.fighters[1].percent == 6.0);
    assert_eq!(second.fighters[0].action, Action::CatchAttack);
    assert_eq!(second.fighters[0].grab.victim, Some(1));
}

#[test]
fn captured_damage_pose_freezes_in_hitlag_then_returns_to_the_paired_wait() {
    let mut resource = data();
    let reaction = &mut resource.fighters[1]
        .grab
        .as_mut()
        .unwrap()
        .capture_damage_poses;
    *reaction = vec![resource.fighters[1].bones.clone(); 8];
    reaction[0][1].translation[0] += 1.0;
    let mut game = held(resource);
    let waiting_position = game.state().fighters[1].position;

    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let hit = step(&mut game, IDLE);
    assert_eq!(hit.fighters[1].action, Action::CaptureDamage);
    assert_eq!(hit.fighters[1].action_frame, 0);
    assert!(hit.fighters[1].position[0] < waiting_position[0] - 0.5);

    let checkpoint = game.checkpoint();
    let frozen_timer = hit.fighters[1].grab.escape_timer;
    let expected = (0..4).map(|_| step(&mut game, IDLE)).collect::<Vec<_>>();
    assert_eq!(expected[0].fighters[1].grab.escape_timer, frozen_timer);
    assert_eq!(expected[1].fighters[1].grab.escape_timer, frozen_timer);
    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, IDLE), expected);
    }
    assert_eq!(game.state().fighters[1].action_frame, 2);

    let holder_wait = until(&mut game, |state| {
        state.fighters[0].action == Action::CatchWait
    });
    assert_eq!(holder_wait.fighters[1].action, Action::CaptureDamage);
    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let restarted = until(&mut game, |state| state.fighters[1].percent == 6.0);
    assert_eq!(restarted.fighters[1].action, Action::CaptureDamage);
    assert_eq!(restarted.fighters[1].action_frame, 0);
    let pair_wait = until(&mut game, |state| {
        state.fighters[0].action == Action::CatchWait
            && state.fighters[1].action == Action::CaptureWait
    });
    assert_eq!(pair_wait.fighters[0].action, Action::CatchWait);
    assert_eq!(pair_wait.fighters[0].grab.victim, Some(1));
    assert_eq!(pair_wait.fighters[1].grab.captor, Some(0));
}

#[test]
fn held_a_during_catch_pull_does_not_turn_into_a_pummel() {
    let mut game = Match::new(data(), 0).unwrap();
    let held_a = input(0, BUTTON_A, [0.0; 2], [0.0; 2]);
    step(&mut game, input(0, BUTTON_A | BUTTON_Z, [0.0; 2], [0.0; 2]));
    for _ in 0..8 {
        if game.state().fighters[0].action == Action::CatchWait {
            break;
        }
        step(&mut game, held_a);
    }
    assert_eq!(game.state().fighters[0].action, Action::CatchWait);
    assert_eq!(
        step(&mut game, held_a).fighters[0].action,
        Action::CatchWait
    );
    step(&mut game, IDLE);
    assert_eq!(
        step(&mut game, held_a).fighters[0].action,
        Action::CatchAttack
    );
}

#[test]
fn passive_timer_buttons_and_latched_stick_mash_release_into_cut_actions() {
    let mut resource = data();
    let escape = &mut resource.rules.grab.as_mut().unwrap().escape;
    escape.timer_base = 15.0;
    escape.timer_percent_scale = 0.0;
    escape.timer_decrement = 1.0;
    escape.mash_penalty = 3.0;
    let victim = &mut resource.fighters[1];
    victim.collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    for pose in &mut victim.grab.as_mut().unwrap().escape.capture_cut_poses {
        pose[1].translation[0] = 6.0;
    }
    let mut game = held(resource);
    let held_distance = game.state().fighters[1].position[0] - game.state().fighters[0].position[0];

    assert_eq!(game.state().fighters[1].grab.escape_timer, 15.0);
    step(&mut game, IDLE);
    assert_eq!(game.state().fighters[1].grab.escape_timer, 14.0);
    let checkpoint = game.checkpoint();
    let suffix = [
        input(1, BUTTON_L, [0.0; 2], [0.0; 2]),
        input(1, BUTTON_L, [0.0; 2], [0.0; 2]),
        IDLE,
        input(1, 0, [0.8, 0.0], [0.0; 2]),
        input(1, 0, [0.8, 0.0], [0.0; 2]),
        input(1, 0, [-0.8, 0.0], [0.0; 2]),
    ];
    let expected = suffix.map(|controllers| step(&mut game, controllers));
    assert_eq!(expected[0].fighters[1].grab.escape_timer, 10.0);
    assert_eq!(expected[1].fighters[1].grab.escape_timer, 9.0);
    assert_eq!(expected[3].fighters[1].grab.escape_timer, 4.0);
    assert_eq!(expected[4].fighters[1].grab.escape_timer, 3.0);
    let escaped = &expected[5];
    assert_eq!(escaped.fighters[0].action, Action::CatchCut);
    assert_eq!(escaped.fighters[1].action, Action::CaptureCut);
    assert!(
        escaped
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
    assert!(escaped.events.contains(&Event::GrabEscaped {
        holder: 0,
        victim: 1,
    }));
    assert!(escaped.fighters[0].ground_velocity < 0.0);
    assert!(escaped.fighters[1].ground_velocity > 0.0);
    assert_eq!(escaped.fighters[1].ecb.desired.right[0], 6.0);
    assert_eq!(escaped.fighters[1].ecb.current.right[0], 6.0);
    assert!(escaped.fighters[1].position[0] - escaped.fighters[0].position[0] > held_distance);

    game.restore_checkpoint(&checkpoint).unwrap();
    for (controllers, expected) in suffix.into_iter().zip(expected) {
        assert_eq!(step(&mut game, controllers), expected);
    }
    let recovered = until(&mut game, |state| {
        state
            .fighters
            .iter()
            .all(|fighter| fighter.action == Action::Wait)
    });
    assert!(
        recovered
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
}

#[test]
fn passive_capture_timer_releases_without_mash_input() {
    let mut resource = data();
    let escape = &mut resource.rules.grab.as_mut().unwrap().escape;
    escape.timer_base = 2.0;
    escape.timer_percent_scale = 0.0;
    escape.timer_decrement = 1.0;
    let mut game = held(resource);

    let retained = step(&mut game, IDLE);
    assert_eq!(retained.fighters[1].grab.escape_timer, 1.0);
    assert_eq!(retained.fighters[1].action, Action::CaptureWait);
    let escaped = step(&mut game, IDLE);
    assert_eq!(escaped.fighters[0].action, Action::CatchCut);
    assert_eq!(escaped.fighters[1].action, Action::CaptureCut);
    assert!(escaped.events.contains(&Event::GrabEscaped {
        holder: 0,
        victim: 1,
    }));
}

#[test]
fn capture_timer_scales_with_existing_percent_on_contact() {
    let mut resource = data();
    let escape = &mut resource.rules.grab.as_mut().unwrap().escape;
    escape.timer_base = 7.5;
    escape.timer_percent_scale = 1.25;
    resource.rules.knockback_speed = 0.0;
    let mut game = Match::new(resource, 11).unwrap();

    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let damaged = until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(damaged.fighters[1].percent > 0.0);
    let recovered = until(&mut game, |state| {
        state
            .fighters
            .iter()
            .all(|fighter| fighter.action == Action::Wait)
    });
    let expected = 7.5 + recovered.fighters[1].percent * 1.25;
    let caught = step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert!(caught.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));
    assert_eq!(
        caught.fighters[1].grab.escape_timer.to_bits(),
        expected.to_bits()
    );
}

#[test]
fn expired_timer_waits_for_capture_damage_to_finish() {
    let mut resource = data();
    resource.rules.grab.as_mut().unwrap().escape.timer_base = 3.0;
    resource
        .rules
        .grab
        .as_mut()
        .unwrap()
        .escape
        .timer_percent_scale = 0.0;
    resource.fighters[1]
        .grab
        .as_mut()
        .unwrap()
        .capture_damage_poses = vec![resource.fighters[1].bones.clone(); 5];
    let mut game = held(resource);

    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    step(&mut game, IDLE);
    let expired = until(&mut game, |state| {
        state.fighters[1].grab.escape_timer <= 0.0
    });
    assert_eq!(expired.fighters[1].action, Action::CaptureDamage);
    assert_eq!(expired.fighters[0].grab.victim, Some(1));
    let escaped = until(&mut game, |state| {
        state.events.contains(&Event::GrabEscaped {
            holder: 0,
            victim: 1,
        })
    });
    assert_eq!(escaped.fighters[0].action, Action::CatchCut);
    assert_eq!(escaped.fighters[1].action, Action::CaptureCut);
}

#[test]
fn simultaneous_catches_resolve_in_stable_player_order() {
    let mut game = Match::new(data(), 0).unwrap();
    let inputs = [Controller {
        buttons: BUTTON_Z,
        ..Controller::default()
    }; 2];
    let state = step(&mut game, inputs);
    assert_eq!(state.fighters[0].grab.victim, Some(1));
    assert_eq!(state.fighters[1].grab.captor, Some(0));
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| matches!(event, Event::Grabbed { .. }))
            .count(),
        1
    );
}

#[test]
fn grounded_only_catch_rejects_airborne_targets() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [1.0, 4.0]];
    let mut game = Match::new(resource, 0).unwrap();
    step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    for _ in 0..5 {
        step(&mut game, IDLE);
    }
    assert!(
        game.state()
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
}

#[test]
fn blast_exit_breaks_the_pair_before_stock_loss_is_published() {
    let mut resource = data();
    resource.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .attachment
        .holder_point[0] = 100.0;
    let mut game = Match::new(resource, 0).unwrap();
    let state = step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert!(state.events.contains(&Event::Knockout {
        player: 1,
        stocks: 1,
    }));
    assert!(
        state
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
    assert_eq!(state.fighters[0].action, Action::Wait);
    assert_eq!(state.fighters[1].action, Action::Respawn);
}

#[test]
fn malformed_grab_resources_are_rejected() {
    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules.grab.as_mut().unwrap().down_threshold = 0.0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].grab.as_mut().unwrap().catch.frames[0]
        .grabboxes
        .clear();
    for frame in &mut bad.fighters[0].grab.as_mut().unwrap().catch.frames {
        frame.grabboxes.clear();
    }
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .catch_dash
        .frames
        .clear();
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .throws
        .forward
        .release_frame = 0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[1].grab = None;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .throws
        .forward
        .hit
        .angle_degrees = 362.0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].grab.as_mut().unwrap().pummel.poses.clear();
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].grab.as_mut().unwrap().pummel.hit_frame = 0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].grab.as_mut().unwrap().pummel.damage = 1_000;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .capture_damage_poses
        .clear();
    cases.push(bad);
    let mut bad = data();
    bad.rules.grab.as_mut().unwrap().escape.stick_threshold = 0.0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .escape
        .catch_cut_poses
        .clear();
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
