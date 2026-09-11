//! Input-to-state tests of ordinary and resource-driven multijump ftCo movement
//! branches in a synthetic world.
//! Explicit timings are test resources, not extracted character attributes.
use skirmish::{
    collision::ecb,
    game::{
        Action, BUTTON_A, BUTTON_X, Controller, Match, State,
        data::{CollisionBox, MatchData},
        locomotion::MultiJump,
    },
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
    data.rules.top_ko_min_knockback = Some(0.5);
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.spawns = [[-40.0, 0.0], [40.0, 0.0]];
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data
}

fn game() -> Match {
    Match::new(data(), 42).unwrap()
}

fn multi_jump_data() -> MatchData {
    let mut data = data();
    let parameters = data.fighters[0].locomotion.as_mut().unwrap();
    parameters.max_jumps = 6;
    parameters.multi_jump = Some(MultiJump {
        turn_frames: 4,
        backward_turn_threshold: 0.2,
        horizontal_velocity: 1.25,
        air_drift_threshold: 0.3,
        air_drift_acceleration_multiplier: 0.5,
        air_drift_max_velocity_multiplier: 0.5,
        vertical_velocities: [3.0, 2.75, 2.5, 2.25, 2.0],
        animation_frames: [8; 5],
        repeat_input_frames: [3; 5],
    });
    data
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
fn launch(game: &mut Match, buttons: u16, stick: [f32; 2]) -> State {
    for _ in 0..20 {
        let state = step(game, buttons, stick);
        if !state.fighters[0].grounded {
            return state;
        }
    }
    panic!("ground jump did not launch: {:?}", game.state());
}

fn reach_run(game: &mut Match) {
    for _ in 0..20 {
        if step(game, 0, [1.0, 0.0]).fighters[0].action == Action::Run {
            return;
        }
    }
    panic!("dash did not reach Run: {:?}", game.state());
}

// ftCo_Dash_Enter writes gr_accel2; Fighter_procUpdate integrates it after
// ApplyGroundMovement projected the old speed for this frame's displacement.
#[test]
fn dash_entry_then_held_input_runs_without_reapplying_initial_speed() {
    let mut game = game();
    let initial = game.state().fighters[0].position[0];
    let first = step(&mut game, 0, [1.0, 0.0]);
    assert_eq!(first.fighters[0].action, Action::Dash);
    assert_eq!(first.fighters[0].position[0].to_bits(), initial.to_bits());
    assert_eq!(first.fighters[0].ground_velocity, 2.0);
    let second = step(&mut game, 0, [1.0, 0.0]);
    assert!(second.fighters[0].position[0] > initial);
    assert!(second.fighters[0].ground_velocity > 2.0);
    for _ in 0..12 {
        step(&mut game, 0, [1.0, 0.0]);
    }
    assert_eq!(game.state().fighters[0].action, Action::Run);
    assert_eq!(game.state().fighters[0].locomotion.tilt_x_age, 254);
    let moving = game.state().fighters[0].ground_velocity;
    let brake = step(&mut game, 0, [0.0; 2]);
    assert_eq!(brake.fighters[0].action, Action::RunBrake);
    assert!(brake.fighters[0].ground_velocity < moving);
    for _ in 0..12 {
        step(&mut game, 0, [0.0; 2]);
    }
    assert_eq!(game.state().fighters[0].action, Action::Wait);
}

// Run tests TurnRun before RunBrake. TurnRun stores the entry facing, pauses at
// its script marker until velocity crosses x0.01, then flips and resumes Run.
#[test]
fn reversed_run_turns_after_deceleration_and_checkpoints_the_frozen_marker() {
    let mut game = game();
    reach_run(&mut game);
    let boundary = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .turn_threshold;
    let mut brake = game.clone();
    assert_eq!(
        step(&mut brake, 0, [boundary.next_up(), 0.0]).fighters[0].action,
        Action::RunBrake
    );
    let entered = step(&mut game, 0, [boundary, 0.0]);
    assert_eq!(entered.fighters[0].action, Action::RunTurn);
    assert_eq!(entered.fighters[0].facing, 1.0);
    assert_eq!(entered.fighters[0].locomotion.run_turn_facing, 1.0);

    for _ in 0..20 {
        if game.state().fighters[0].locomotion.run_turn_waiting {
            break;
        }
        step(&mut game, 0, [-1.0, 0.0]);
    }
    let marker = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .run_turn_flip_frame;
    assert!(game.state().fighters[0].locomotion.run_turn_waiting);
    assert_eq!(game.state().fighters[0].action_frame, marker);
    let checkpoint = game.checkpoint();
    let mut expected = Vec::new();
    for _ in 0..80 {
        let before = game.state().fighters[0].clone();
        let state = step(&mut game, 0, [-1.0, 0.0]);
        if state.fighters[0].facing == 1.0 {
            assert_eq!(state.fighters[0].action_frame, marker);
        } else if before.facing == 1.0 {
            assert!(before.ground_velocity <= 0.01);
        }
        expected.push(state);
        if game.state().fighters[0].action == Action::Run {
            break;
        }
    }
    assert_eq!(game.state().fighters[0].action, Action::Run);
    assert_eq!(game.state().fighters[0].facing, -1.0);
    assert!(game.state().fighters[0].ground_velocity < 0.0);
    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, 0, [-1.0, 0.0]), expected);
    }
}

// RunBrake exposes TurnRun only after cmd_vars[0] is set by its animation and
// passes its current animation time into the destination motion state.
#[test]
fn run_brake_turn_waits_for_its_marker_and_preserves_animation_time() {
    let mut game = game();
    reach_run(&mut game);
    assert_eq!(
        step(&mut game, 0, [0.0; 2]).fighters[0].action,
        Action::RunBrake
    );
    let parameters = game.data().fighters[0].locomotion.as_ref().unwrap();
    let gate = parameters.run_brake_turn_frame;
    let boundary = parameters.turn_threshold;
    let early = step(&mut game, 0, [boundary, 0.0]);
    assert_eq!(early.fighters[0].action, Action::RunBrake);
    assert_eq!(early.fighters[0].action_frame, gate);
    let checkpoint = game.checkpoint();
    let mut expected = Vec::new();
    for _ in 0..80 {
        let state = step(&mut game, 0, [-1.0, 0.0]);
        expected.push(state);
        if game.state().fighters[0].action == Action::Run {
            break;
        }
    }
    assert_eq!(expected[0].fighters[0].action, Action::RunTurn);
    assert_eq!(expected[0].fighters[0].action_frame, gate + 1);
    assert_eq!(expected[0].fighters[0].locomotion.run_turn_facing, 1.0);
    assert_eq!(game.state().fighters[0].action, Action::Run);
    assert_eq!(game.state().fighters[0].facing, -1.0);
    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, 0, [-1.0, 0.0]), expected);
    }
}

// fighter.c starts smash ages at the small common deadzone, not dash threshold.
#[test]
fn slow_tilt_misses_dash_window_but_neutral_rearms_it() {
    let mut game = game();
    for x in [0.3, 0.4, 0.5, 0.6, 0.7, 1.0] {
        step(&mut game, 0, [x, 0.0]);
    }
    assert_eq!(game.state().fighters[0].action, Action::Walk);
    assert!(game.state().fighters[0].locomotion.tilt_x_age >= 4);
    step(&mut game, 0, [0.0; 2]);
    assert_eq!(
        step(&mut game, 0, [1.0, 0.0]).fighters[0].action,
        Action::Dash
    );
}

// ftCo_Turn_Anim_Inner returns after decrementing, even when it just reached 0.
#[test]
fn standing_turn_delays_facing_but_smash_turn_flips_next_callback() {
    let mut basic = game();
    assert_eq!(
        step(&mut basic, 0, [-0.5, 0.0]).fighters[0].action,
        Action::Turn
    );
    for _ in 0..3 {
        assert_eq!(step(&mut basic, 0, [-0.5, 0.0]).fighters[0].facing, 1.0);
    }
    assert_eq!(step(&mut basic, 0, [-0.5, 0.0]).fighters[0].facing, -1.0);
    let mut smash = game();
    let first = step(&mut smash, 0, [-1.0, 0.0]);
    assert_eq!(first.fighters[0].action, Action::Turn);
    assert_eq!(first.fighters[0].facing, 1.0);
    let second = step(&mut smash, 0, [-1.0, 0.0]);
    assert_eq!(second.fighters[0].action, Action::Dash);
    assert_eq!(second.fighters[0].facing, -1.0);
    assert!(second.fighters[0].ground_velocity < 0.0);
}

// Turn IASA tests attacks under the future facing, then restores facing before
// checking jump. A fresh smash during an ordinary turn arms the later dash.
#[test]
fn turn_attacks_use_future_facing_and_interrupt_before_the_dash_check() {
    for frames in [0, 3] {
        let mut game = game();
        step(&mut game, 0, [-0.5, 0.0]);
        for _ in 0..frames {
            step(&mut game, 0, [-0.5, 0.0]);
        }
        let jab = step(&mut game, BUTTON_A, [-1.0, 0.0]);
        assert_eq!(jab.fighters[0].action, Action::Jab);
        assert_eq!(jab.fighters[0].facing, -1.0);
    }
    let mut game = game();
    step(&mut game, 0, [-0.5, 0.0]);
    step(&mut game, 0, [0.0; 2]);
    step(&mut game, 0, [-1.0, 0.0]);
    step(&mut game, 0, [-1.0, 0.0]);
    assert_eq!(
        step(&mut game, 0, [-1.0, 0.0]).fighters[0].action,
        Action::Dash
    );
}

#[test]
fn walk_gate_uses_facing_when_stick_is_between_walk_and_turn_thresholds() {
    let mut game = game();
    let waiting = step(&mut game, 0, [-0.25, 0.0]);
    assert_eq!(waiting.fighters[0].action, Action::Wait);
    assert_eq!(waiting.fighters[0].ground_velocity, 0.0);
    assert_eq!(
        step(&mut game, 0, [0.25, 0.0]).fighters[0].action,
        Action::Walk
    );
}

// Squat uses strict < entry; SquatRv uses strict > release and a distinct phase.
#[test]
fn crouch_thresholds_have_hysteresis_and_do_not_reverse_while_held() {
    let mut game = game();
    assert_eq!(
        step(&mut game, 0, [0.0, -0.7]).fighters[0].action,
        Action::Wait
    );
    assert_eq!(
        step(&mut game, 0, [0.0, -0.71]).fighters[0].action,
        Action::Squat
    );
    for _ in 0..10 {
        step(&mut game, 0, [0.0, -1.0]);
    }
    assert_eq!(game.state().fighters[0].action, Action::SquatWait);
    assert_eq!(
        step(&mut game, 0, [0.0, -0.5]).fighters[0].action,
        Action::SquatWait
    );
    assert_eq!(
        step(&mut game, 0, [0.0, -0.49]).fighters[0].action,
        Action::SquatRv
    );
    for _ in 0..4 {
        step(&mut game, 0, [0.0; 2]);
    }
    assert_eq!(game.state().fighters[0].action, Action::Wait);
}

// KneeBend stores which input initiated the jump. Holding another jump source
// does not prevent a short hop when the original source was released.
#[test]
fn short_hop_release_tracks_button_or_stick_source_and_launch_deadline() {
    for (buttons, stick, released_buttons, released_stick) in [
        (BUTTON_X, [0.0; 2], 0, [0.0, 1.0]),
        (0, [0.0, 1.0], BUTTON_X, [0.0; 2]),
    ] {
        let mut game = game();
        step(&mut game, buttons, stick);
        step(&mut game, released_buttons, released_stick);
        let expected = game.data().fighters[0].movement.short_hop_vertical_velocity;
        assert_eq!(
            launch(&mut game, released_buttons, released_stick).fighters[0].velocity[1],
            expected
        );
    }
    let mut game = game();
    let startup = game.data().fighters[0].movement.jump_startup_frames;
    for _ in 0..startup {
        step(&mut game, BUTTON_X, [0.0; 2]);
    }
    let expected = game.data().fighters[0].movement.jump_vertical_velocity;
    assert_eq!(
        step(&mut game, 0, [0.0; 2]).fighters[0].velocity[1],
        expected
    );
}

// ft_did_jump counts edges; ordinary JumpAerial applies gravity on entry,
// unlike the first ground-jump physics callback.
#[test]
fn air_jump_needs_repress_exhausts_and_landing_restores_it() {
    let mut game = game();
    launch(&mut game, BUTTON_X, [0.0; 2]);
    for _ in 0..3 {
        step(&mut game, BUTTON_X, [0.0; 2]);
    }
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 1);
    step(&mut game, 0, [0.0; 2]);
    let expected = game.data().fighters[0].movement.jump_vertical_velocity
        - game.data().fighters[0].movement.gravity;
    let second = step(&mut game, BUTTON_X, [0.0; 2]);
    assert_eq!(second.fighters[0].action, Action::JumpAerial);
    assert_eq!(second.fighters[0].velocity[1], expected);
    assert_eq!(second.fighters[0].locomotion.jumps_used, 2);
    step(&mut game, 0, [0.0; 2]);
    let third = step(&mut game, BUTTON_X, [0.0; 2]);
    assert!(third.fighters[0].velocity[1] < expected);
    assert_eq!(third.fighters[0].locomotion.jumps_used, 2);
    for _ in 0..100 {
        if step(&mut game, 0, [0.0; 2]).fighters[0].grounded {
            break;
        }
    }
    assert!(game.state().fighters[0].grounded);
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 0);
    for _ in 0..10 {
        step(&mut game, 0, [0.0; 2]);
    }
    assert_eq!(
        launch(&mut game, BUTTON_X, [0.0; 2]).fighters[0]
            .locomotion
            .jumps_used,
        1
    );
}

#[test]
fn multijumps_use_fresh_then_held_input_after_each_command_marker() {
    let mut game = Match::new(multi_jump_data(), 42).unwrap();
    launch(&mut game, BUTTON_X, [0.0; 2]);
    step(&mut game, 0, [0.0; 2]);
    let first = step(&mut game, BUTTON_X, [0.0; 2]);
    assert_eq!(first.fighters[0].action, Action::JumpAerial);
    assert_eq!(first.fighters[0].action_frame, 1);
    assert_eq!(first.fighters[0].locomotion.jumps_used, 2);
    assert_eq!(first.fighters[0].velocity[1], 3.0 - 0.2);

    let checkpoint = game.checkpoint();
    let mut expected = Vec::new();
    let mut impulses = vec![first.fighters[0].velocity[1]];
    let mut jumps_used = 2;
    for _ in 0..20 {
        let state = step(&mut game, BUTTON_X, [0.0; 2]);
        if state.fighters[0].locomotion.jumps_used > jumps_used {
            jumps_used = state.fighters[0].locomotion.jumps_used;
            impulses.push(state.fighters[0].velocity[1]);
        }
        expected.push(state);
        if game.state().fighters[0].locomotion.jumps_used == 6 {
            break;
        }
    }
    assert_eq!(expected[0].fighters[0].action_frame, 2);
    assert_eq!(expected[0].fighters[0].locomotion.jumps_used, 2);
    assert_eq!(expected[1].fighters[0].action_frame, 3);
    assert_eq!(expected[1].fighters[0].locomotion.jumps_used, 2);
    assert_eq!(expected[2].fighters[0].action_frame, 1);
    assert_eq!(expected[2].fighters[0].locomotion.jumps_used, 3);
    assert_eq!(expected[2].fighters[0].velocity[1], 2.75 - 0.2);
    assert_eq!(impulses, [2.8, 2.55, 2.3, 2.05, 1.8]);
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 6);

    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, BUTTON_X, [0.0; 2]), expected);
    }
    for _ in 0..12 {
        step(&mut game, BUTTON_X, [0.0; 2]);
    }
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 6);
    assert_eq!(game.state().fighters[0].action, Action::Fall);
    for _ in 0..240 {
        if step(&mut game, 0, [0.0; 2]).fighters[0].grounded {
            break;
        }
    }
    assert!(game.state().fighters[0].grounded);
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 0);
}

#[test]
fn multijump_turn_rotates_bone_physics_and_flips_facing_at_halfway() {
    let mut data = multi_jump_data();
    data.fighters[0].bones[1].translation[0] = 8.0;
    data.fighters[0].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    let mut game = Match::new(data, 42).unwrap();
    launch(&mut game, BUTTON_X, [0.0; 2]);
    step(&mut game, 0, [0.0; 2]);
    let entered = step(&mut game, BUTTON_X, [-1.0, 0.0]);
    assert_eq!(entered.fighters[0].facing, 1.0);
    assert_eq!(entered.fighters[0].locomotion.multi_jump_turn_remaining, 3);
    assert!(entered.fighters[0].locomotion.multi_jump_yaw < 0.0);
    assert_eq!(entered.fighters[0].velocity[0], -1.15);
    assert!(entered.fighters[0].ecb.desired.right[0] < 8.0);
    let checkpoint = game.checkpoint();
    let halfway = step(&mut game, 0, [-1.0, 0.0]);
    assert_eq!(halfway.fighters[0].locomotion.multi_jump_turn_remaining, 2);
    assert_eq!(halfway.fighters[0].facing, -1.0);
    assert!(halfway.fighters[0].ecb.desired.left[0] > -8.0);
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(step(&mut game, 0, [-1.0, 0.0]), halfway);
}

#[test]
fn held_up_does_not_spend_air_jump_but_a_new_up_flick_does() {
    let mut game = game();
    launch(&mut game, 0, [0.0, 1.0]);
    for _ in 0..3 {
        step(&mut game, 0, [0.0, 1.0]);
    }
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 1);
    step(&mut game, 0, [0.0; 2]);
    assert_eq!(
        step(&mut game, 0, [0.0, 1.0]).fighters[0].action,
        Action::JumpAerial
    );
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 2);
}

#[test]
fn late_air_jump_relocks_the_ecb_and_keeps_its_action_past_the_apex() {
    let mut resource = data();
    resource.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .air_jump_animation_frames = 80;
    let mut game = Match::new(resource, 42).unwrap();
    launch(&mut game, BUTTON_X, [0.0; 2]);
    for _ in 0..11 {
        step(&mut game, 0, [0.0; 2]);
    }
    assert_eq!(game.state().fighters[0].ecb_lock, 0);
    assert!(!game.state().fighters[0].ecb.bottom_locked);
    let jump = step(&mut game, BUTTON_X, [0.0; 2]);
    assert_eq!(jump.fighters[0].ecb_lock, 9);
    assert!(jump.fighters[0].ecb.bottom_locked);
    assert_eq!(jump.fighters[0].ground_velocity, 0.0);
    for _ in 0..13 {
        step(&mut game, 0, [0.0; 2]);
    }
    assert!(!game.state().fighters[0].grounded);
    assert!(game.state().fighters[0].velocity[1] < 0.0);
    assert_eq!(game.state().fighters[0].action, Action::JumpAerial);
}

#[test]
fn a_gradual_up_tilt_misses_the_tap_jump_window() {
    let mut game = game();
    for y in [0.3, 0.4, 0.5, 0.6, 0.7, 1.0] {
        step(&mut game, 0, [0.0, y]);
    }
    assert_eq!(game.state().fighters[0].action, Action::Wait);
    step(&mut game, 0, [0.0; 2]);
    assert_eq!(
        step(&mut game, 0, [0.0, 1.0]).fighters[0].action,
        Action::JumpSquat
    );
}

#[test]
fn airborne_spawn_has_only_the_remaining_aerial_jump() {
    let mut data = data();
    data.stage.spawns[0][1] = 20.0;
    let mut game = Match::new(data, 42).unwrap();
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 1);
    assert_eq!(
        step(&mut game, BUTTON_X, [0.0; 2]).fighters[0].action,
        Action::JumpAerial
    );
    step(&mut game, 0, [0.0; 2]);
    let before = game.state().fighters[0].velocity[1];
    assert!(step(&mut game, BUTTON_X, [0.0; 2]).fighters[0].velocity[1] < before);
}

#[test]
fn walking_off_an_edge_consumes_the_ground_jump() {
    let mut data = data();
    data.stage.floor.left = -2.0;
    data.stage.floor.right = 2.0;
    data.stage.spawns = [[1.9, 0.0], [-1.9, 0.0]];
    let mut game = Match::new(data, 42).unwrap();
    for _ in 0..10 {
        if !step(&mut game, 0, [0.5, 0.0]).fighters[0].grounded {
            break;
        }
    }
    assert!(!game.state().fighters[0].grounded);
    assert_eq!(game.state().fighters[0].locomotion.jumps_used, 1);
    let jump = step(&mut game, BUTTON_X, [0.0; 2]);
    assert_eq!(jump.fighters[0].action, Action::JumpAerial);
    assert_eq!(jump.fighters[0].locomotion.jumps_used, 2);
}

// Fighter input history is global: hitlag freezes animation, not stick ages.
#[test]
fn holding_up_during_hitlag_does_not_become_a_fresh_jump_after_damage() {
    let mut data = data();
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    data.rules.hitlag.base = 8.0;
    let mut game = Match::new(data, 42).unwrap();
    step(&mut game, BUTTON_A, [0.0; 2]);
    step(&mut game, 0, [0.0; 2]);
    assert!(game.state().fighters[1].hitlag > 4.0);
    let held_up = [
        Controller::default(),
        Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            buttons: 0,
            stick: [0.0, 1.0],
        },
    ];
    while game.state().fighters[1].hitlag > 0.0 {
        game.step(held_up).unwrap();
    }
    assert!(game.state().fighters[1].locomotion.tilt_y_age >= 4);
    for _ in 0..80 {
        let state = game.step(held_up).unwrap();
        assert!(!matches!(
            state.fighters[1].action,
            Action::JumpSquat | Action::Jump | Action::JumpAerial
        ));
    }
    assert_eq!(game.state().fighters[1].action, Action::Wait);
    game.step([Controller::default(); 2]).unwrap();
    assert_eq!(
        game.step(held_up).unwrap().fighters[1].action,
        Action::JumpSquat
    );
}

#[test]
fn checkpoints_restore_pending_turn_and_input_window_history() {
    let mut game = game();
    step(&mut game, 0, [-0.5, 0.0]);
    step(&mut game, 0, [-0.5, 0.0]);
    let checkpoint = game.checkpoint();
    let inputs = [
        (0, [-0.5, 0.0]),
        (0, [-0.5, 0.0]),
        (0, [-1.0, 0.0]),
        (BUTTON_X, [0.0, 0.0]),
        (0, [0.0, 0.0]),
        (0, [0.0, 0.0]),
        (BUTTON_X, [0.0, 0.0]),
    ];
    let expected: Vec<_> = inputs
        .iter()
        .map(|&(b, s)| serde_json::to_vec(&step(&mut game, b, s)).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    for ((b, s), expected) in inputs.into_iter().zip(expected) {
        assert_eq!(
            serde_json::to_vec(&step(&mut game, b, s)).unwrap(),
            expected
        );
    }
}

#[test]
fn legacy_profiles_and_invalid_locomotion_resources_are_explicit() {
    let mut legacy = data();
    legacy.fighters[0].locomotion = None;
    let mut game = Match::new(legacy, 42).unwrap();
    assert_eq!(
        step(&mut game, 0, [1.0, 0.0]).fighters[0].action,
        Action::Walk
    );
    for bad in [f32::NAN, f32::INFINITY, -1.0] {
        let mut invalid = data();
        invalid.fighters[0]
            .locomotion
            .as_mut()
            .unwrap()
            .dash_initial_velocity = bad;
        assert!(Match::new(invalid, 42).is_err());
    }
    let mut invalid = data();
    invalid.fighters[0].locomotion.as_mut().unwrap().max_jumps = 3;
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = multi_jump_data();
    invalid.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .multi_jump
        .as_mut()
        .unwrap()
        .vertical_velocities[0] = 0.0;
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = multi_jump_data();
    let multi = invalid.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .multi_jump
        .as_mut()
        .unwrap();
    multi.repeat_input_frames[0] = multi.animation_frames[0];
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = data();
    let p = invalid.fighters[0].locomotion.as_mut().unwrap();
    p.run_brake_turn_frame = p.run_brake_animation_frames;
    assert!(Match::new(invalid, 42).is_err());
    // run_brake_marker_frame/run_brake_freeze_speed must be paired.
    let mut invalid = data();
    invalid.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .run_brake_marker_frame = Some(0);
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = data();
    invalid.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .run_brake_freeze_speed = Some(1.0);
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = data();
    let p = invalid.fighters[0].locomotion.as_mut().unwrap();
    p.run_brake_marker_frame = Some(p.run_brake_animation_frames);
    p.run_brake_freeze_speed = Some(1.0);
    assert!(Match::new(invalid, 42).is_err());
    let mut invalid = data();
    invalid.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .run_turn_lockout_frames = Some(-1.0);
    assert!(Match::new(invalid, 42).is_err());
}
