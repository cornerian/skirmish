//! File-backed harness regressions. Expected recordings are produced by this
//! same native implementation, so success does not certify Melee fidelity.
use peppi::frame::mutable;
use serde_json::Value;
use skirmish::game::{Action, BUTTON_A, BUTTON_B, BUTTON_X, BUTTON_Z, Controller, Event, State};
use skirmish_replay::{
    Checkpoint,
    match_validation::{self as replay_match, Initialization, Outcome, Report},
    observation,
    slippi::{Port, Replay, Timeline, Version},
};
use std::{fs, process::Command};

#[path = "../../../tests/support/aerial.rs"]
mod aerial_support;
#[path = "../../../tests/support/death.rs"]
mod death_support;
#[path = "../../../tests/support/grab.rs"]
mod grab_support;
#[path = "../../../tests/support/ledge.rs"]
mod ledge_support;
#[path = "../../../tests/support/special.rs"]
mod special_support;
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
    fn from_script(
        data: skirmish::game::data::MatchData,
        seed: u32,
        inputs: Vec<[Controller; 2]>,
    ) -> Self {
        let initialization = Initialization {
            data,
            seed,
            ports: PORTS,
            next_frame: FIRST,
            warmup: Vec::new(),
        };
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

    fn new() -> Self {
        let mut data: skirmish::game::data::MatchData = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        data.rules.countdown_frames = 0;
        data.stage.floor.left = -100.0;
        data.stage.floor.right = 100.0;
        data.stage.blast = [-500.0, 500.0, -100.0, 200.0];
        let mut inputs = vec![IDLE; 56];
        inputs[0][0].buttons = BUTTON_A;
        for input in &mut inputs[10..14] {
            input[0].stick[0] = 0.5;
        }
        for input in &mut inputs[14..18] {
            input[0].buttons = BUTTON_X;
            input[0].stick[0] = 0.25;
        }
        // Re-arm the tap between attempts so one input crosses the threshold
        // after the jump has started descending.
        for row in [20, 22, 24, 26, 28, 30] {
            inputs[row][0].stick[1] = -1.0;
        }
        for input in &mut inputs[44..48] {
            input[0].stick[0] = -0.5;
        }
        inputs[52][0].buttons = BUTTON_A;
        // Independently execute the complete script once. No replay or future
        // expected observation is available to the simulator during this run.
        Self::from_script(data, 42, inputs)
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
                        Some(observation::action_state(fighter, Some(2)).expect(
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
                    post.character
                        .set(row, Some(observation::internal_character(2).unwrap()));
                    post.last_attack_landed
                        .set(row, Some(fighter.combo.last_attack_landed as u8));
                    post.combo_count.set(row, Some(fighter.combo.count as u8));
                    post.last_hit_by.set(
                        row,
                        Some(
                            fighter
                                .combo
                                .last_hit_by
                                .map_or(6, |source| PORTS[source] as u8),
                        ),
                    );
                    if let Some(instance) = &mut post.last_hit_by_instance {
                        instance.set(row, Some(fighter.combo.last_hit_by_instance));
                    }
                    if let Some(instance) = &mut post.instance_id {
                        instance.set(row, Some(fighter.action_instance.id));
                    }
                    if let Some(animation) = &mut post.animation_index {
                        animation.set(row, observation::animation_index(fighter, Some(2)));
                    }
                    post.stocks.set(row, Some(fighter.stocks));
                    post.airborne
                        .as_mut()
                        .unwrap()
                        .set(row, Some(u8::from(!fighter.grounded)));
                    post.jumps.as_mut().unwrap().set(
                        row,
                        Some(2_u8.saturating_sub(fighter.locomotion.jumps_used)),
                    );
                    post.ground.as_mut().unwrap().set(
                        row,
                        Some(
                            fighter
                                .last_ground_line
                                .map_or(u16::MAX, |line| line as u16),
                        ),
                    );
                    post.l_cancel
                        .as_mut()
                        .unwrap()
                        .set(row, Some(fighter.l_cancel_status));
                    let native_flags = observation::state_flags(fighter);
                    if let Some(flags) = &mut post.state_flags {
                        flags.0.set(row, Some(native_flags[0]));
                        flags.1.set(row, Some(native_flags[1]));
                        flags.2.set(row, Some(native_flags[2]));
                        flags.3.set(row, Some(native_flags[3]));
                        flags.4.set(row, Some(native_flags[4]));
                    }
                    post.misc_as
                        .as_mut()
                        .unwrap()
                        .set(row, Some(fighter.hitstun as f32));
                    if let Some(hurtbox_state) = &mut post.hurtbox_state {
                        hurtbox_state.set(row, Some(observation::hurtbox_state(fighter)));
                    }
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
    assert!(recording.states.iter().any(|state| {
        !state.fighters[0].grounded && state.fighters[0].last_ground_line == Some(0)
    }));
    assert!(recording.states.iter().any(|s| {
        s.events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
            && s.fighters[1].hitlag > 0.0
    }));
    assert!(
        recording
            .states
            .iter()
            .any(|state| state.fighters[0].fast_fall)
    );
    assert!(recording.states.iter().any(|state| {
        state.fighters[1].hitlag > 0.0
            && state.fighters[1].hitstun > 0
            && state.fighters[1].hitstun as f32 > 0.0
    }));
    assert!(recording.states.iter().any(|state| {
        state.fighters[0].combo.last_attack_landed == 1
            && state.fighters[0].combo.count == 1
            && state.fighters[0].combo.victim == Some(1)
            && state.fighters[1].combo.last_hit_by == Some(0)
    }));
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    let report = recording.compare(&bytes);
    matched(&report, FIRST, recording.inputs.len());
    assert_eq!(report.policy, "fighter-post-v9");
    assert_eq!(report.ports, PORTS);
    assert_eq!(report.checkpoint_next_frame, FIRST);
    assert_eq!(report.replay.bytes, bytes.len());
    assert_eq!(report.resources_sha256.len(), 64);
}

#[test]
fn physical_b_drives_file_backed_neutral_special_and_detects_its_removal() {
    let mut data = special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 8];
    inputs[0][0].buttons = BUTTON_B;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::SpecialN);
    assert!(recording.states.iter().any(|state| {
        state.events.iter().any(|event| {
            matches!(
                event,
                Event::Hit {
                    attacker: 0,
                    victim: 1,
                    damage: 10.0,
                    ..
                }
            )
        })
    }));

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame: FIRST,
            checked_frames: 0,
            ..
        }
    ));
}

#[test]
fn physical_z_drives_file_backed_grab_capture_and_detects_its_removal() {
    let mut data = grab_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    let mut inputs = vec![IDLE; 8];
    inputs[0][0].buttons = BUTTON_Z;
    let recording = Recording::from_script(data, 11, inputs);
    assert!(recording.states[0].events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));
    assert_eq!(recording.states[0].fighters[0].action, Action::CatchPull);
    assert_eq!(
        recording.states[0].fighters[1].action,
        Action::CapturePulledLw
    );
    assert!(
        recording
            .states
            .iter()
            .any(|state| state.fighters[0].action == Action::CatchWait)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame: FIRST,
            checked_frames: 0,
            ..
        }
    ));
}

#[test]
fn physical_l_drives_file_backed_shield_state_and_detects_its_removal() {
    #[derive(serde::Deserialize)]
    struct ShieldProfile {
        rules: skirmish::game::shield::Rules,
        attributes: skirmish::game::shield::Attributes,
    }
    let mut data: skirmish::game::data::MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("../../../tests/fixtures/game/shield.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.shield = Some(profile.rules);
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
    }
    let mut inputs = vec![IDLE; 4];
    inputs[0][0].buttons = skirmish::game::BUTTON_L;
    inputs[1][0].buttons = skirmish::game::BUTTON_L;
    let recording = Recording::from_script(data, 19, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::GuardOn);
    assert_ne!(
        observation::state_flags(&recording.states[0].fighters[0])[2] & 0x80,
        0
    );
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame: FIRST,
            checked_frames: 0,
            ..
        }
    ));
}

#[test]
fn file_backed_death_flags_cover_disappearance_sleep_and_return_to_play() {
    for (mode, expected_action, delayed) in [
        (0, Action::DeadUp, false),
        (1, Action::DeadUpStar, true),
        (2, Action::DeadUpFall, true),
    ] {
        let mut data = death_support::profile(aerial_support::conformance::data());
        data.rules.countdown_frames = 0;
        data.rules.stocks = 3;
        data.rules.respawn_frames = 2;
        data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
        data.stage.floor.left = -50.0;
        data.stage.floor.right = 50.0;
        data.stage.blast = [-80.0, 80.0, -40.0, 5.0];
        data.rules.knockback_decay = 0.0;
        data.rules.knockback_speed = 1.0;
        data.rules.hitlag.base = 0.0;
        data.rules.hitlag.damage_scale = 0.0;
        let death = data.rules.death.as_mut().unwrap();
        death.force_normal_top[1] = mode == 0;
        death.screen_chance_percent = if mode == 2 { 100 } else { 0 };
        for fighter in &mut data.fighters {
            fighter.movement.gravity = 0.0;
            for frame in &mut fighter.jab.frames {
                for hit in &mut frame.hitboxes {
                    hit.angle_degrees = 90.0;
                    hit.growth = 0;
                    hit.base = 2;
                }
            }
        }

        let mut inputs = vec![IDLE; 40];
        inputs[0][0].buttons = BUTTON_A;
        let recording = Recording::from_script(data, 17, inputs);
        let started = recording
            .states
            .iter()
            .position(|state| {
                state
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::DeathStarted { player: 1, .. }))
            })
            .expect("script must reach the top blast zone");
        assert_eq!(
            recording.states[started].fighters[1].action,
            expected_action
        );
        assert_eq!(
            observation::state_flags(&recording.states[started].fighters[1])[4] & 0x40 != 0,
            !delayed
        );
        let first_dead = recording.states[started..]
            .iter()
            .position(|state| observation::state_flags(&state.fighters[1])[4] & 0x40 != 0)
            .map(|row| started + row)
            .expect("every death path must eventually set x221F_b1");
        assert_eq!(first_dead > started, delayed);
        let respawn = recording.states[first_dead..]
            .iter()
            .position(|state| state.fighters[1].action == Action::Respawn)
            .map(|row| first_dead + row)
            .expect("death script must reach the inactive respawn delay");
        assert_eq!(
            observation::state_flags(&recording.states[respawn].fighters[1])[4],
            0x50
        );
        let active = recording.states[respawn + 1..]
            .iter()
            .position(|state| state.fighters[1].action != Action::Respawn)
            .map(|row| respawn + 1 + row)
            .expect("script must return the fighter to active play");
        assert_eq!(
            observation::state_flags(&recording.states[active].fighters[1])[4],
            0
        );
        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let corrupted = recording.bytes(support::Fixture::default(), |frames| {
            frames.ports[1]
                .leader
                .post
                .state_flags
                .as_mut()
                .unwrap()
                .4
                .set(first_dead, Some(0));
        });
        assert!(matches!(
            recording.compare(&corrupted).outcome,
            Outcome::Mismatch {
                frame,
                checked_frames,
                ref difference,
            } if frame == FIRST + first_dead as i32
                && checked_frames == first_dead as u64
                && difference.port == PORTS[1]
                && difference.field == "state_flags.dead"
        ));
        let corrupted = recording.bytes(support::Fixture::default(), |frames| {
            frames.ports[1]
                .leader
                .post
                .state_flags
                .as_mut()
                .unwrap()
                .4
                .set(respawn, Some(0x40));
        });
        assert!(matches!(
            recording.compare(&corrupted).outcome,
            Outcome::Mismatch {
                frame,
                checked_frames,
                ref difference,
            } if frame == FIRST + respawn as i32
                && checked_frames == respawn as u64
                && difference.port == PORTS[1]
                && difference.field == "state_flags.sleep"
        ));
    }
}

#[test]
fn file_backed_ledge_intangibility_uses_hurtbox_state_two() {
    let mut data: skirmish::game::data::MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -2.0;
    data.stage.floor.right = 2.0;
    data.stage.spawns = [[-1.9, 1.0], [0.0, 0.0]];
    data.stage.blast = [-20.0, 20.0, -30.0, 30.0];
    let data = ledge_support::profile(data);
    let mut inputs = vec![IDLE; 4];
    inputs[0][0].stick = [-1.0, 0.0];
    let recording = Recording::from_script(data, 13, inputs);
    let caught = &recording.states[0];
    assert!(caught.events.contains(&Event::LedgeCaught {
        player: 0,
        line: 0,
        side: skirmish::game::ledge::Side::Left,
    }));
    assert!(caught.fighters[0].intangibility > 0);
    assert_eq!(caught.fighters[0].invincibility, 0);
    assert_eq!(observation::hurtbox_state(&caught.fighters[0]), 2);
    assert_ne!(observation::state_flags(&caught.fighters[0])[1] & 0x04, 0);
    assert_eq!(caught.fighters[0].last_ground_line, None);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0]
            .leader
            .post
            .hurtbox_state
            .as_mut()
            .unwrap()
            .set(0, Some(1));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame: FIRST,
            checked_frames: 0,
            ref difference,
        } if difference.field == "hurtbox_state"
    ));
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
    assert!(
        states
            .iter()
            .any(|state| state.fighters[0].l_cancel_status == 1)
    );
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
fn file_backed_failed_l_cancel_is_reported_only_on_the_landing_frame() {
    let mut data = aerial_support::data();
    data.stage.spawns[0] = [-10.0, 4.0];
    data.fighters[0].movement.gravity = 0.5;
    data.fighters[0].movement.terminal_velocity = 2.0;
    let mut inputs = vec![IDLE; 24];
    inputs[2][0].cstick = [1.0, 0.0];
    let recording = Recording::from_script(data, 17, inputs);
    let row = recording
        .states
        .iter()
        .position(|state| state.fighters[0].l_cancel_status == 2)
        .expect("unshielded aerial landing must report failed l-cancel");
    assert_eq!(
        recording.states[row].fighters[0].action,
        Action::LandingAirF
    );
    assert!(
        recording.states[..row]
            .iter()
            .all(|state| state.fighters[0].l_cancel_status == 0)
    );
    assert_eq!(recording.states[row + 1].fighters[0].l_cancel_status, 0);
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
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
    for field in observation::fields(Version(3, 18, 0)) {
        let player = if matches!(field, "last_attack_landed" | "combo_count" | "instance_id") {
            0
        } else {
            1
        };
        let row = if field == observation::MISC_HITSTUN_FIELD {
            recording
                .states
                .iter()
                .position(|state| state.fighters[player].hitstun > 0)
                .unwrap()
        } else if matches!(
            field,
            "last_attack_landed" | "combo_count" | "last_hit_by" | "last_hit_by_instance"
        ) {
            recording
                .states
                .iter()
                .position(|state| {
                    state.fighters[0].combo.count != 0
                        && state.fighters[1].combo.last_hit_by.is_some()
                })
                .unwrap()
        } else {
            37
        };
        let fighter = &recording.states[row].fighters[player];
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
                "last_ground_id" => post.ground.as_mut().unwrap().set(row, Some(u16::MAX)),
                "l_cancel" => post
                    .l_cancel
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.l_cancel_status ^ 1)),
                "character" => post.character.set(row, Some(3)),
                "last_attack_landed" => post
                    .last_attack_landed
                    .set(row, Some((fighter.combo.last_attack_landed as u8) ^ 1)),
                "combo_count" => post
                    .combo_count
                    .set(row, Some((fighter.combo.count as u8) ^ 1)),
                "last_hit_by" => post.last_hit_by.set(row, Some(6)),
                "last_hit_by_instance" => post
                    .last_hit_by_instance
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.combo.last_hit_by_instance ^ 1)),
                "instance_id" => post
                    .instance_id
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.action_instance.id ^ 1)),
                "animation_index" => post.animation_index.as_mut().unwrap().set(
                    row,
                    Some(observation::animation_index(fighter, Some(2)).unwrap() ^ 1),
                ),
                "state_flags.protected" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .1
                    .set(row, Some(observation::state_flags(fighter)[1] ^ 0x04)),
                "state_flags.fast_fall" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .1
                    .set(row, Some(observation::state_flags(fighter)[1] ^ 0x08)),
                "state_flags.hitlag" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .1
                    .set(row, Some(observation::state_flags(fighter)[1] ^ 0x20)),
                "state_flags.shield" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .2
                    .set(row, Some(observation::state_flags(fighter)[2] ^ 0x80)),
                "state_flags.hitstun" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .3
                    .set(row, Some(observation::state_flags(fighter)[3] ^ 0x02)),
                "state_flags.dead" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .4
                    .set(row, Some(observation::state_flags(fighter)[4] ^ 0x40)),
                "state_flags.sleep" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .4
                    .set(row, Some(observation::state_flags(fighter)[4] ^ 0x10)),
                "misc_as.hitstun" => post
                    .misc_as
                    .as_mut()
                    .unwrap()
                    .set(row, Some(fighter.hitstun as f32 + 1.0)),
                "hurtbox_state" => post
                    .hurtbox_state
                    .as_mut()
                    .unwrap()
                    .set(row, Some(observation::hurtbox_state(fighter) ^ 1)),
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
                    && difference.port == PORTS[player]
                    && difference.field == field
            ),
            "{field}: {report:?}"
        );
    }
}

#[test]
fn report_fields_follow_the_slippi_version_without_silent_missing_checks() {
    let recording = Recording::new();
    for version in [
        Version(2, 0, 0),
        Version(2, 1, 0),
        Version(3, 5, 0),
        Version(3, 8, 0),
        Version(3, 11, 0),
        Version(3, 16, 0),
    ] {
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
        assert_eq!(
            report.fields.contains(&"animation_index"),
            version.gte(3, 11)
        );
        assert_eq!(report.fields.contains(&"hurtbox_state"), version.gte(2, 1));
        assert_eq!(
            report.fields.contains(&"last_hit_by_instance"),
            version.gte(3, 16)
        );
        assert_eq!(report.fields.contains(&"instance_id"), version.gte(3, 16));
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
                3 => pre.buttons_physical.set(row, Some(0x80)),
                _ => pre.buttons.set(row, Some(0x80)),
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
fn followers_invalid_ports_and_characters_are_explicit_setup_errors() {
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

    let bytes = support::replay_bytes_with(&support::Fixture::default(), |start, _| {
        start.players[0].character = 26;
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
            .contains("unsupported external character ID"),
        "{error:#}"
    );
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
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
        assert_eq!(report["policy"], "fighter-post-v9");
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
