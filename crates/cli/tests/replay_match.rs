//! File-backed harness regressions. Expected recordings are produced by this
//! same native implementation, so success does not certify Melee fidelity.
use peppi::frame::mutable;
use serde_json::Value;
use skirmish::game::{Action, BUTTON_A, BUTTON_X, Controller, Event, State};
use skirmish_replay::{
    Checkpoint,
    match_validation::{self as replay_match, Initialization, Outcome, Report},
    observation,
    slippi::{Port, Replay, Timeline, Version},
};
use std::{fs, process::Command};

#[path = "../../../tests/support/aerial.rs"]
mod aerial_support;
#[path = "../../peppi-adapter/tests/support/mod.rs"]
mod support;

const FIRST: i32 = -123;
const PORTS: [Port; 2] = [Port::P1, Port::P3];
const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

struct Recording {
    initialization: Initialization,
    inputs: Vec<[Controller; 2]>,
    states: Vec<State>,
}

impl Recording {
    fn new() -> Self {
        let mut data: skirmish::game::data::MatchData = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        data.rules.countdown_frames = 0;
        data.stage.floor.left = -100.0;
        data.stage.floor.right = 100.0;
        data.stage.blast = [-500.0, 500.0, -100.0, 200.0];
        let initialization = Initialization {
            data,
            seed: 42,
            ports: PORTS,
            next_frame: FIRST,
            warmup: Vec::new(),
        };
        let mut inputs = vec![IDLE; 56];
        inputs[0][0].buttons = BUTTON_A;
        for input in &mut inputs[10..14] {
            input[0].stick[0] = 0.5;
        }
        for input in &mut inputs[14..18] {
            input[0].buttons = BUTTON_X;
            input[0].stick[0] = 0.25;
        }
        for input in &mut inputs[44..48] {
            input[0].stick[0] = -0.5;
        }
        inputs[52][0].buttons = BUTTON_A;
        // Independently execute the complete script once. No replay or future
        // expected observation is available to the simulator during this run.
        let mut game = replay_match::initialize(&initialization).unwrap();
        let states = inputs
            .iter()
            .map(|&input| game.step(input).unwrap().clone())
            .collect();
        Self {
            initialization,
            inputs,
            states,
        }
    }

    fn bytes(
        &self,
        mut fixture: support::Fixture,
        edit: impl FnOnce(&mut mutable::Frame),
    ) -> Vec<u8> {
        fixture.frame_ids = (0..self.inputs.len()).map(|i| FIRST + i as i32).collect();
        support::replay_bytes_with(&fixture, |_, frames| {
            for (player, port) in frames.ports.iter_mut().enumerate() {
                for (row, (inputs, state)) in self.inputs.iter().zip(&self.states).enumerate() {
                    let input = inputs[player];
                    let pre = &mut port.leader.pre;
                    pre.joystick.x.set(row, Some(input.stick[0]));
                    pre.joystick.y.set(row, Some(input.stick[1]));
                    pre.buttons.set(row, Some(u32::from(input.buttons)));
                    pre.buttons_physical.set(row, Some(input.buttons));
                    pre.cstick.x.set(row, Some(input.cstick[0]));
                    pre.cstick.y.set(row, Some(input.cstick[1]));
                    pre.triggers.set(row, Some(input.trigger));
                    pre.triggers_physical.l.set(row, Some(0.0));
                    pre.triggers_physical.r.set(row, Some(0.0));
                    let fighter = &state.fighters[player];
                    let post = &mut port.leader.post;
                    post.state.set(
                        row,
                        Some(observation::action_state(fighter).expect(
                            "the synthetic recording uses only mapped common action states",
                        )),
                    );
                    post.state_age
                        .as_mut()
                        .unwrap()
                        .set(row, Some(fighter.action_frame as f32));
                    post.position.x.set(row, Some(fighter.position[0]));
                    post.position.y.set(row, Some(fighter.position[1]));
                    post.direction.set(row, Some(fighter.facing));
                    post.percent.set(row, Some(fighter.percent));
                    post.shield.set(row, Some(fighter.shield.health));
                    post.stocks.set(row, Some(fighter.stocks));
                    post.airborne
                        .as_mut()
                        .unwrap()
                        .set(row, Some(u8::from(!fighter.grounded)));
                    post.jumps.as_mut().unwrap().set(
                        row,
                        Some(2_u8.saturating_sub(fighter.locomotion.jumps_used)),
                    );
                    if let Some(velocity) = &mut post.velocities {
                        velocity.self_x_air.set(row, Some(fighter.velocity[0]));
                        velocity.self_y.set(row, Some(fighter.velocity[1]));
                        velocity.knockback_x.set(row, Some(fighter.knockback[0]));
                        velocity.knockback_y.set(row, Some(fighter.knockback[1]));
                        velocity
                            .self_x_ground
                            .set(row, Some(fighter.ground_velocity));
                    }
                    if let Some(hitlag) = &mut post.hitlag {
                        hitlag.set(row, Some(fighter.hitlag));
                    }
                }
            }
            edit(frames);
        })
    }

    fn compare(&self, bytes: &[u8]) -> Report {
        let replay = load(bytes);
        let mut game = replay_match::initialize(&self.initialization).unwrap();
        let checkpoint = Checkpoint {
            next_frame: FIRST,
            state: game.checkpoint(),
        };
        replay_match::validate(
            &replay,
            &mut game,
            &checkpoint,
            PORTS,
            Timeline::LastRecorded,
        )
        .unwrap()
    }
}

fn load(bytes: &[u8]) -> Replay {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("synthetic-native-match.slp");
    fs::write(&path, bytes).unwrap();
    Replay::read(fs::File::open(path).unwrap()).unwrap()
}

fn matched(report: &Report, first: i32, length: usize) {
    assert!(report.is_match(), "{report:?}");
    assert!(matches!(
        report.outcome,
        Outcome::Matched { first_frame, last_frame, checked_frames }
            if first_frame == first
                && last_frame == first + length as i32 - 1
                && checked_frames == length as u64
    ));
}

#[test]
fn file_backed_native_run_matches_walking_jump_landing_and_combat_observations() {
    let recording = Recording::new();
    for action in [Action::Walk, Action::Jump, Action::Landing, Action::Jab] {
        assert!(
            recording
                .states
                .iter()
                .any(|s| s.fighters[0].action == action)
        );
    }
    assert!(recording.states.iter().any(|s| {
        s.events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
            && s.fighters[1].hitlag > 0.0
    }));
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    let report = recording.compare(&bytes);
    matched(&report, FIRST, recording.inputs.len());
    assert_eq!(report.policy, "fighter-post-v2");
    assert_eq!(report.ports, PORTS);
    assert_eq!(report.checkpoint_next_frame, FIRST);
    assert_eq!(report.replay.bytes, bytes.len());
    assert_eq!(report.resources_sha256.len(), 64);
}

#[test]
fn file_backed_cstick_aerial_and_l_cancel_match_and_changed_selection_diverges() {
    let mut data = aerial_support::data();
    data.stage.spawns[0] = [-10.0, 4.0];
    data.fighters[0].movement.gravity = 0.5;
    data.fighters[0].movement.terminal_velocity = 2.0;
    data.fighters[0].aerials.as_mut().unwrap().moves[1].flags[0].reverse_facing = true;
    let initialization = Initialization {
        data,
        seed: 42,
        ports: PORTS,
        next_frame: FIRST,
        warmup: vec![],
    };
    let mut inputs = vec![IDLE; 56];
    inputs[2][0].cstick = [1.0, 0.0];
    inputs[2][0].trigger = 0.25;
    let mut game = replay_match::initialize(&initialization).unwrap();
    let states: Vec<_> = inputs
        .iter()
        .map(|&input| game.step(input).unwrap().clone())
        .collect();
    for action in [Action::AttackAirF, Action::LandingAirF, Action::Wait] {
        assert!(states.iter().any(|s| s.fighters[0].action == action));
    }
    let recording = Recording {
        initialization,
        inputs,
        states,
    };
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.cstick.x.set(2, Some(-1.0));
    });
    assert!(
        matches!(recording.compare(&changed).outcome, Outcome::Mismatch { frame, checked_frames: 2, .. } if frame == FIRST + 2)
    );
}

#[test]
fn first_late_post_mismatch_reports_the_matched_prefix_and_expected_bits() {
    let recording = Recording::new();
    let row = 37;
    let changed = recording.states[row].fighters[1].position[0] + 0.25;
    let bytes = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[1]
            .leader
            .post
            .position
            .x
            .set(row, Some(changed));
    });
    let report = recording.compare(&bytes);
    assert!(!report.is_match());
    match report.outcome {
        Outcome::Mismatch {
            frame,
            checked_frames,
            difference,
        } => {
            assert_eq!(frame, FIRST + row as i32);
            assert_eq!(checked_frames, row as u64);
            assert_eq!(difference.port, Port::P3);
            assert_eq!(difference.field, "position.x");
            assert!(
                difference
                    .expected
                    .contains(&format!("{:08x}", changed.to_bits()))
            );
            assert_ne!(difference.expected, difference.actual);
        }
        other => panic!("expected a late position mismatch, got {other:?}"),
    }
}

#[test]
fn every_reported_post_field_detects_its_first_file_backed_difference() {
    let recording = Recording::new();
    let row = 37;
    let player = 1;
    let fighter = &recording.states[row].fighters[player];
    for field in observation::fields(Version(3, 18, 0)) {
        let bytes = recording.bytes(support::Fixture::default(), |frames| {
            let post = &mut frames.ports[player].leader.post;
            match field {
                "action_state" => post.state.set(row, Some(u16::MAX)),
                "action_age" => post
                    .state_age
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.action_frame as f32 + 0.5)),
                "position.x" => post.position.x.set(row, Some(fighter.position[0] + 0.25)),
                "position.y" => post.position.y.set(row, Some(fighter.position[1] - 0.25)),
                "direction" => post.direction.set(row, Some(-fighter.facing)),
                "percent" => post.percent.set(row, Some(fighter.percent + 0.25)),
                "shield" => post.shield.set(row, Some(fighter.shield.health + 0.25)),
                "stocks" => post.stocks.set(row, Some(fighter.stocks.saturating_sub(1))),
                "airborne" => post
                    .airborne
                    .as_mut()
                    .unwrap()
                    .set(row, Some(u8::from(fighter.grounded))),
                "jumps_remaining" => post.jumps.as_mut().unwrap().set(
                    row,
                    Some(
                        2_u8.saturating_sub(fighter.locomotion.jumps_used)
                            .saturating_add(1),
                    ),
                ),
                "velocities.self_x_air" => post
                    .velocities
                    .as_mut()
                    .unwrap()
                    .self_x_air
                    .set(row, Some(fighter.velocity[0] + 0.25)),
                "velocities.self_y" => post
                    .velocities
                    .as_mut()
                    .unwrap()
                    .self_y
                    .set(row, Some(fighter.velocity[1] + 0.25)),
                "velocities.knockback_x" => post
                    .velocities
                    .as_mut()
                    .unwrap()
                    .knockback_x
                    .set(row, Some(fighter.knockback[0] + 0.25)),
                "velocities.knockback_y" => post
                    .velocities
                    .as_mut()
                    .unwrap()
                    .knockback_y
                    .set(row, Some(fighter.knockback[1] + 0.25)),
                "velocities.self_x_ground" => post
                    .velocities
                    .as_mut()
                    .unwrap()
                    .self_x_ground
                    .set(row, Some(fighter.ground_velocity + 0.25)),
                "hitlag" => post
                    .hitlag
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.hitlag + 0.25)),
                _ => unreachable!(),
            }
        });
        let report = recording.compare(&bytes);
        assert!(
            matches!(
                report.outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames,
                    ref difference,
                } if frame == FIRST + row as i32
                    && checked_frames == row as u64
                    && difference.port == Port::P3
                    && difference.field == field
            ),
            "{field}: {report:?}"
        );
    }
}

#[test]
fn report_fields_follow_the_slippi_version_without_silent_missing_checks() {
    let recording = Recording::new();
    for version in [Version(2, 0, 0), Version(3, 5, 0), Version(3, 8, 0)] {
        let bytes = recording.bytes(
            support::Fixture {
                version,
                ..Default::default()
            },
            |_| {},
        );
        let report = recording.compare(&bytes);
        matched(&report, FIRST, recording.inputs.len());
        assert_eq!(report.fields, observation::fields(version));
        assert_eq!(
            report.fields.contains(&"velocities.self_x_air"),
            version.gte(3, 5)
        );
        assert_eq!(report.fields.contains(&"hitlag"), version.gte(3, 8));
    }
}

#[test]
fn recorded_pre_state_and_rng_never_reinitialize_the_simulator() {
    let recording = Recording::new();
    let bytes = recording.bytes(support::Fixture::default(), |frames| {
        for row in 0..recording.inputs.len() {
            frames
                .start
                .as_mut()
                .unwrap()
                .random_seed
                .set(row, Some(u32::MAX));
            for port in &mut frames.ports {
                let pre = &mut port.leader.pre;
                pre.random_seed.set(row, Some(0xdead_beef));
                pre.position.x.set(row, Some(99_999.0));
                pre.position.y.set(row, Some(-88_888.0));
                pre.direction.set(row, Some(-13.0));
                pre.percent.as_mut().unwrap().set(row, Some(777.0));
                pre.state.set(row, Some(u16::MAX));
            }
        }
    });
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
}

#[test]
fn changed_controller_input_changes_simulation_without_changing_expected_posts() {
    let recording = Recording::new();
    let row = 10;
    let bytes = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.joystick.x.set(row, Some(0.75));
    });
    assert!(matches!(
        recording.compare(&bytes).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + row as i32 && checked_frames == row as u64
    ));
}

#[test]
fn unsupported_inputs_fail_at_their_frame_after_the_matching_prefix() {
    let recording = Recording::new();
    let row = 19;
    for case in 0..5 {
        let bytes = recording.bytes(support::Fixture::default(), |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            match case {
                0 => pre.cstick.x.set(row, Some(1.01)),
                1 => pre.triggers.set(row, Some(-0.25)),
                2 => pre.triggers_physical.r.set(row, Some(1.5)),
                3 => pre.buttons_physical.set(row, Some(0x200)),
                _ => pre.buttons.set(row, Some(0x200)),
            }
        });
        let report = recording.compare(&bytes);
        assert!(!report.is_match());
        assert!(
            matches!(
                report.outcome,
                Outcome::Error { frame: Some(frame), checked_frames, .. }
                    if frame == FIRST + row as i32 && checked_frames == row as u64
            ),
            "case {case}: {report:?}"
        );
    }
}

#[test]
fn file_backed_cstick_asdi_reaches_simulation_and_changed_input_diverges() {
    let mut recording = Recording::new();
    recording.initialization.data.rules.damage.displacement =
        Some(skirmish::game::damage::HitlagDisplacementRules {
            axis_thresholds: [0.5; 2],
            minimum_stick_magnitude: 0.5,
            sdi_window: 3,
            sdi_distance: 2.0,
            asdi_distance: 0.75,
        });
    for input in &mut recording.inputs[2..5] {
        input[1].cstick = [0.5, 0.0];
        input[1].trigger = 0.25;
    }
    recording.inputs[12][0].buttons = skirmish::game::BUTTON_L;
    let mut game = replay_match::initialize(&recording.initialization).unwrap();
    recording.states = recording
        .inputs
        .iter()
        .map(|&input| game.step(input).unwrap().clone())
        .collect();
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Hit connects at row1, with three hitlag ticks; row4 is its ASDI expiry.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[1].leader.pre.cstick.x.set(4, Some(-0.5));
    });
    assert!(matches!(recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. } if frame == FIRST + 4 && checked_frames == 4));
}

#[test]
fn followers_and_invalid_port_selection_are_explicit_setup_errors() {
    let recording = Recording::new();
    for (follower, ports) in [
        (true, PORTS),
        (false, [Port::P1, Port::P2]),
        (false, [Port::P1; 2]),
    ] {
        let bytes = recording.bytes(
            support::Fixture {
                follower,
                ..Default::default()
            },
            |_| {},
        );
        let replay = load(&bytes);
        let mut game = replay_match::initialize(&recording.initialization).unwrap();
        let checkpoint = Checkpoint {
            next_frame: FIRST,
            state: game.checkpoint(),
        };
        assert!(
            replay_match::validate(
                &replay,
                &mut game,
                &checkpoint,
                ports,
                Timeline::LastRecorded
            )
            .is_err()
        );
    }
}

#[test]
fn cpu_and_team_metadata_are_rejected_before_advancing_the_native_match() {
    let recording = Recording::new();
    for teams in [false, true] {
        let bytes = support::replay_bytes_with(&support::Fixture::default(), |start, _| {
            if teams {
                start.is_teams = true;
            } else {
                start.players[0].r#type = peppi::game::PlayerType::Cpu;
                start.players[0].cpu_level = Some(9);
            }
        });
        let replay = load(&bytes);
        let mut game = replay_match::initialize(&recording.initialization).unwrap();
        let before = serde_json::to_vec(game.state()).unwrap();
        let checkpoint = Checkpoint {
            next_frame: FIRST,
            state: game.checkpoint(),
        };
        let error = replay_match::validate(
            &replay,
            &mut game,
            &checkpoint,
            PORTS,
            Timeline::LastRecorded,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(if teams { "team" } else { "human" }),
            "{error:#}"
        );
        assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    }
}

#[test]
fn finalized_policy_compares_only_its_explicit_suffix_and_reports_that_scope() {
    let recording = Recording::new();
    let prefix = 32;
    let mut watermarks = vec![FIRST - 7; recording.inputs.len()];
    *watermarks.last_mut().unwrap() = FIRST + prefix as i32 - 1;
    let bytes = recording.bytes(
        support::Fixture {
            finalized_frames: Some(watermarks),
            ..Default::default()
        },
        |frames| {
            // This mismatch lies in the unconfirmed tail. It must fail the
            // last-recorded policy while remaining outside finalized-only.
            frames.ports[0].leader.post.percent.set(40, Some(222.0));
        },
    );
    let replay = load(&bytes);
    for start in [0, 16] {
        let mut initialization = recording.initialization.clone();
        initialization.warmup = recording.inputs[..start].to_vec();
        initialization.next_frame = FIRST + start as i32;
        let mut game = replay_match::initialize(&initialization).unwrap();
        let checkpoint = Checkpoint {
            next_frame: initialization.next_frame,
            state: game.checkpoint(),
        };
        let report = replay_match::validate(
            &replay,
            &mut game,
            &checkpoint,
            PORTS,
            Timeline::FinalizedOnly,
        )
        .unwrap();
        matched(&report, initialization.next_frame, prefix - start);
        assert_eq!(report.replay.timeline, Timeline::FinalizedOnly);
        assert_eq!(report.replay.selected_frames, prefix);
        assert_eq!(report.replay.surviving_frames, recording.inputs.len());
        assert_eq!(report.replay.last_frame, Some(FIRST + prefix as i32 - 1));
    }
    assert!(matches!(
        recording.compare(&bytes).outcome,
        Outcome::Mismatch { frame, checked_frames: 40, .. } if frame == FIRST + 40
    ));
}

#[test]
fn empty_finalized_timeline_and_checkpoint_after_the_last_frame_cannot_match() {
    let recording = Recording::new();
    // Explicitly keep every bookend unfinalized even in this longer fixture.
    let watermarks = vec![FIRST - 7; recording.inputs.len()];
    let bytes = recording.bytes(
        support::Fixture {
            finalized_frames: Some(watermarks),
            ..Default::default()
        },
        |_| {},
    );
    let replay = load(&bytes);
    for (timeline, next_frame) in [
        (Timeline::FinalizedOnly, FIRST),
        (
            Timeline::LastRecorded,
            FIRST + recording.inputs.len() as i32,
        ),
    ] {
        let mut game = replay_match::initialize(&recording.initialization).unwrap();
        let before = serde_json::to_vec(game.state()).unwrap();
        let checkpoint = Checkpoint {
            next_frame,
            state: game.checkpoint(),
        };
        let error =
            replay_match::validate(&replay, &mut game, &checkpoint, PORTS, timeline).unwrap_err();
        assert!(error.to_string().contains("absent"), "{error:#}");
        assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    }
}

#[test]
fn explicit_mid_replay_checkpoint_continues_the_complete_remaining_suffix() {
    let recording = Recording::new();
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    let replay = load(&bytes);
    let start = 16;
    let mut initialization = recording.initialization;
    initialization.warmup = recording.inputs[..start].to_vec();
    initialization.next_frame = FIRST + start as i32;
    let mut game = replay_match::initialize(&initialization).unwrap();
    let checkpoint = Checkpoint {
        next_frame: initialization.next_frame,
        state: game.checkpoint(),
    };
    // Disturb the live instance. Validation must restore the supplied complete
    // checkpoint once, before advancing the first selected replay input.
    game.step(IDLE).unwrap();
    let report = replay_match::validate(
        &replay,
        &mut game,
        &checkpoint,
        PORTS,
        Timeline::LastRecorded,
    )
    .unwrap();
    matched(
        &report,
        initialization.next_frame,
        recording.inputs.len() - start,
    );
    assert_eq!(
        serde_json::to_vec(game.state()).unwrap(),
        serde_json::to_vec(recording.states.last().unwrap()).unwrap()
    );
}

#[test]
fn cli_runs_real_file_comparison_and_exits_unsuccessfully_on_a_late_difference() {
    let recording = Recording::new();
    let directory = tempfile::tempdir().unwrap();
    let replay_path = directory.path().join("native-match.slp");
    let initialization_path = directory.path().join("initialization.json");
    let initialization = serde_json::to_vec(&recording.initialization).unwrap();
    fs::write(&initialization_path, initialization).unwrap();
    for mismatch in [false, true] {
        let bytes = recording.bytes(support::Fixture::default(), |frames| {
            if mismatch {
                frames.ports[0].leader.post.percent.set(31, Some(123.0));
            }
        });
        fs::write(&replay_path, bytes).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_skirmish"))
            .arg("validate-replay")
            .arg(&replay_path)
            .arg("--initialization")
            .arg(&initialization_path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            !mismatch,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["policy"], "fighter-post-v2");
        assert_eq!(report["initialization_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            report["outcome"]["status"],
            if mismatch { "mismatch" } else { "matched" }
        );
        if mismatch {
            assert_eq!(report["outcome"]["frame"], FIRST + 31);
            assert_eq!(report["outcome"]["checked_frames"], 31);
        } else {
            assert_eq!(report["outcome"]["checked_frames"], recording.inputs.len());
        }
    }
}
