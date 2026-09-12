//! File-backed harness regressions. Expected recordings are produced by this
//! same native implementation, so success does not certify Melee fidelity.
use peppi::frame::mutable;
use serde_json::Value;
use skirmish::collision::stage;
use skirmish::game::data::{HitElement, MatchData, StageGeometry};
use skirmish::game::{
    Action, BUTTON_A, BUTTON_B, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Event, State,
};
use skirmish_replay::{
    Checkpoint,
    match_validation::{self as replay_match, Initialization, Outcome, Report},
    observation,
    slippi::{Port, Replay, Timeline, Version},
};
use std::{fs, process::Command};

#[path = "../../../tests/support/aerial.rs"]
mod aerial_support;
#[path = "../../../tests/support/dash.rs"]
mod dash_support;
#[path = "../../../tests/support/death.rs"]
mod death_support;
#[path = "../../../tests/support/edge.rs"]
mod edge_support;
#[path = "../../../tests/support/escape_air.rs"]
mod escape_air_support;
#[path = "../../../tests/support/escape.rs"]
mod escape_support;
#[path = "../../../tests/support/fox_down_special.rs"]
mod fox_down_special_support;
#[path = "../../../tests/support/fox_side_special.rs"]
mod fox_side_special_support;
#[path = "../../../tests/support/fox_up_special.rs"]
mod fox_up_special_support;
#[path = "../../../tests/support/grab.rs"]
mod grab_support;
#[path = "../../../tests/support/idle.rs"]
mod idle_support;
#[path = "../../../tests/support/jab.rs"]
mod jab_support;
#[path = "../../../tests/support/ledge.rs"]
mod ledge_support;
#[path = "../../../tests/support/run.rs"]
mod run_support;
#[path = "../../../tests/support/smash.rs"]
mod smash_support;
#[path = "../../../tests/support/special.rs"]
mod special_support;
#[path = "../../peppi-adapter/tests/support/mod.rs"]
mod support;
#[path = "../../../tests/support/taunt.rs"]
mod taunt_support;
#[path = "../../../tests/support/tilt.rs"]
mod tilt_support;
#[path = "../../../tests/support/walk.rs"]
mod walk_support;

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

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: skirmish::game::shield::Rules,
    attributes: skirmish::game::shield::Attributes,
}

fn powershield_data() -> skirmish::game::data::MatchData {
    let mut data: skirmish::game::data::MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    let mut profile: ShieldProfile =
        serde_json::from_str(include_str!("../../../tests/fixtures/game/shield.json")).unwrap();
    profile.rules.powershield_input_window = 3;
    profile.rules.powershield_reflect_frames = 3.0;
    profile.rules.powershield_frames = 2.0;
    profile.attributes.raise_frames = 10.0;
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
    }
    for frame in &mut data.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.element = HitElement::Inert;
        }
    }
    data
}

fn shield_drop_data() -> MatchData {
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("../../../tests/fixtures/game/shield.json")).unwrap();
    let locomotion: skirmish::game::locomotion::Parameters =
        serde_json::from_str(include_str!("../../../tests/fixtures/game/locomotion.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.shield = Some(profile.rules);
    data.stage.floor.left = -40.0;
    data.stage.floor.right = 40.0;
    data.stage.spawns = [[0.0, 0.0], [20.0, 0.0]];
    data.stage.blast = [-100.0, 100.0, -100.0, 100.0];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![stage::Line {
            start: [-40.0, 0.0],
            end: [40.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            material_flags: stage::PLATFORM as u16,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-40.0, -100.0],
            bounds_max: [40.0, 100.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion = Some(locomotion);
    }
    data
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
                    // Slippi's state_age for Walk/Run is fp->cur_anim_frame,
                    // a float animation frame; without walk_animation/
                    // run_animation, Walk/Run keep the pre-batch integer
                    // action_frame. `observation::observe`'s own doc
                    // comments spell out the general -1 rule (`simulation::
                    // advance`'s shared end-of-frame `action_frame += 1`
                    // always runs one frame ahead of Melee's own
                    // `cur_anim_frame`), which now already covers Dash and
                    // Turn too (`game::locomotion::start_dash`/`start_turn`
                    // model their extra `ftAnim_8006EBA4` entry call at the
                    // source instead); this harness-local duplicate must
                    // track that formula.
                    let action_age = if fighter.action == Action::Walk
                        && self.initialization.data.fighters[player]
                            .movement
                            .walk_animation
                            .is_some()
                    {
                        fighter.locomotion.walk.frame
                    } else if fighter.action == Action::Run
                        && self.initialization.data.fighters[player]
                            .movement
                            .run_animation
                            .is_some()
                    {
                        fighter.locomotion.run.frame
                    } else if matches!(fighter.action, Action::Entry | Action::EntryEnd) {
                        -1.0
                    } else if fighter.action == Action::EntryStart {
                        let age = fighter.action_frame - 1;
                        match self.initialization.data.fighters[player].entry {
                            Some(animation) => age.min(animation.start_frames - 1) as f32,
                            None => age as f32,
                        }
                    } else if matches!(
                        fighter.action,
                        Action::LandingFallSpecial
                            | Action::LandingAirN
                            | Action::LandingAirF
                            | Action::LandingAirB
                            | Action::LandingAirHi
                            | Action::LandingAirLw
                    ) {
                        // `observation::observe`'s own new tracked-rate
                        // branch: these landings play at `aerial.
                        // landing_rate`, not 1.0.
                        fighter.aerial.landing_elapsed
                    } else {
                        fighter.action_frame.saturating_sub(1) as f32
                    };
                    post.state_age.as_mut().unwrap().set(row, Some(action_age));
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
    assert_eq!(report.policy, "fighter-post-v11");
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
fn file_backed_shield_drop_matches_and_detects_the_first_changed_stick_frame() {
    let mut inputs = vec![IDLE; 6];
    inputs[0][0].buttons = BUTTON_L;
    inputs[1][0].buttons = BUTTON_L;
    inputs[1][0].stick[1] = -1.0;
    let recording = Recording::from_script(shield_drop_data(), 29, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::GuardOn);
    let dropped = &recording.states[1].fighters[0];
    assert_eq!(dropped.action, Action::Pass);
    assert!(!dropped.grounded);
    assert_eq!(dropped.skip_floor, Some(0));

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.joystick.y.set(1, Some(0.0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame,
            checked_frames: 1,
            ..
        } if frame == FIRST + 1
    ));
}

#[test]
fn file_backed_cstick_shield_jump_matches_and_detects_the_first_changed_frame() {
    let mut inputs = vec![IDLE; 8];
    for input in &mut inputs[..2] {
        input[0].buttons = BUTTON_L;
        input[0].cstick[1] = 0.8;
    }
    inputs[2][0].buttons = BUTTON_L;
    let recording = Recording::from_script(shield_drop_data(), 31, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::GuardOn);
    assert_eq!(recording.states[1].fighters[0].action, Action::JumpSquat);
    assert_eq!(
        recording.states[1].fighters[0].locomotion.jump_input,
        skirmish::game::locomotion::JumpInput::CStick
    );
    assert!(recording.states[2].fighters[0].short_hop);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.cstick.y.set(1, Some(0.0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch {
            frame,
            checked_frames: 1,
            ..
        } if frame == FIRST + 1
    ));
}

#[test]
fn file_backed_shield_escapes_match_and_detect_their_first_changed_input_frame() {
    // Fighter 0 faces +X on a flat platform. Each recording shields on the
    // first frame and evades on the first Guard callback.
    struct Case {
        name: &'static str,
        pre: fn(&mut [Controller; 2], usize),
        action: Action,
        state: u16,
        animation: u32,
        edit: fn(&mut mutable::Frame),
    }
    let cases = [
        Case {
            name: "fresh main-stick forward roll",
            pre: |input, row| {
                input[0].buttons = BUTTON_L;
                if row == 1 {
                    input[0].stick[0] = 1.0;
                }
            },
            action: Action::EscapeF,
            state: 233,
            animation: 42,
            edit: |frames| frames.ports[0].leader.pre.joystick.x.set(1, Some(0.0)),
        },
        Case {
            name: "held C-stick backward roll",
            pre: |input, _| {
                input[0].buttons = BUTTON_L;
                input[0].cstick[0] = -1.0;
            },
            action: Action::EscapeB,
            state: 234,
            animation: 43,
            edit: |frames| frames.ports[0].leader.pre.cstick.x.set(1, Some(0.0)),
        },
        Case {
            name: "held C-stick spot dodge",
            pre: |input, _| {
                input[0].buttons = BUTTON_L;
                input[0].cstick[1] = -1.0;
            },
            action: Action::EscapeN,
            state: 235,
            animation: 41,
            edit: |frames| frames.ports[0].leader.pre.cstick.y.set(1, Some(0.0)),
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 12];
        for (row, input) in inputs.iter_mut().enumerate().take(2) {
            (case.pre)(input, row);
        }
        let recording =
            Recording::from_script(escape_support::profile(shield_drop_data()), 37, inputs);
        assert_eq!(recording.states[0].fighters[0].action, Action::GuardOn);
        let entered = &recording.states[1].fighters[0];
        assert_eq!(entered.action, case.action, "{}", case.name);
        assert_eq!(
            observation::action_state(entered, Some(2)),
            Some(case.state)
        );
        assert_eq!(
            observation::animation_index(entered, Some(2)),
            Some(case.animation)
        );
        assert_eq!(
            observation::state_flags(entered)[2] & 0x80,
            0,
            "{}",
            case.name
        );
        // Sample 2 of every invented motion is intangible through x1988, which
        // Slippi reports ahead of the timed counters.
        let intangible = &recording.states[3].fighters[0];
        assert_eq!(intangible.action, case.action);
        assert_eq!(intangible.intangibility, 0);
        assert_eq!(observation::hurtbox_state(intangible), 2, "{}", case.name);
        assert_ne!(observation::state_flags(intangible)[1] & 0x04, 0);
        assert_eq!(
            observation::hurtbox_state(&recording.states[1].fighters[0]),
            0
        );
        assert_eq!(
            recording.states[9].fighters[0].action,
            Action::Wait,
            "{}",
            case.name
        );
        if case.action == Action::EscapeF {
            assert_eq!(recording.states[9].fighters[0].position[0], 4.5);
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let changed = recording.bytes(support::Fixture::default(), case.edit);
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames: 1,
                    ..
                } if frame == FIRST + 1
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn file_backed_shield_grabs_match_and_detect_their_first_changed_button_frame() {
    fn shield_grab_data() -> MatchData {
        let mut data = grab_support::profile(shield_drop_data());
        data.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
        data.rules.grab.as_mut().unwrap().shield_grab =
            Some(skirmish::game::grab::ShieldGrabRules {
                dash_buffer_frames: 3.0,
                dash_buffer_frame_limit: 4.0,
            });
        for fighter in &mut data.fighters {
            fighter.shield.as_mut().unwrap().raise_frames = 10.0;
        }
        data
    }
    struct Case {
        name: &'static str,
        pre: fn(&mut [Controller; 2], usize),
        frame: usize,
        action: Action,
        state: u16,
        grabbed: bool,
    }
    let cases = [
        Case {
            name: "A while holding the trigger",
            pre: |input, row| {
                input[0].buttons = BUTTON_L;
                if row == 1 {
                    input[0].buttons |= BUTTON_A;
                }
            },
            frame: 1,
            action: Action::CatchPull,
            state: 213,
            grabbed: true,
        },
        Case {
            name: "Z after releasing the trigger inside the minimum hold",
            pre: |input, row| {
                input[0].buttons = match row {
                    0 => BUTTON_L,
                    2 => BUTTON_Z,
                    _ => 0,
                };
            },
            frame: 2,
            action: Action::CatchPull,
            state: 213,
            grabbed: true,
        },
        Case {
            name: "A inside the late-dash shield buffer",
            pre: |input, row| {
                if row < 7 {
                    input[0].stick[0] = 1.0;
                }
                if row >= 6 {
                    input[0].buttons = BUTTON_L;
                }
                if row == 7 {
                    input[0].buttons |= BUTTON_A;
                }
            },
            frame: 7,
            action: Action::CatchDash,
            state: 214,
            grabbed: false,
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 14];
        for (row, input) in inputs.iter_mut().enumerate() {
            (case.pre)(input, row);
        }
        let recording = Recording::from_script(shield_grab_data(), 41, inputs);
        let entered = &recording.states[case.frame].fighters[0];
        assert_eq!(entered.action, case.action, "{}", case.name);
        assert_eq!(
            observation::action_state(entered, Some(2)),
            Some(case.state)
        );
        assert_eq!(
            observation::state_flags(entered)[2] & 0x80,
            0,
            "{}",
            case.name
        );
        assert_eq!(
            recording.states[case.frame]
                .events
                .contains(&Event::Grabbed {
                    holder: 0,
                    victim: 1,
                }),
            case.grabbed,
            "{}",
            case.name
        );
        if case.action == Action::CatchDash {
            let shielded = &recording.states[case.frame - 1].fighters[0];
            assert_eq!(shielded.action, Action::GuardOn);
            assert_eq!(shielded.shield.dash_grab_buffer, 3.0);
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let row = case.frame;
        let kept = u32::from(recording.inputs[row][0].buttons & !(BUTTON_A | BUTTON_Z));
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            frames.ports[0].leader.pre.buttons.set(row, Some(kept));
            frames.ports[0]
                .leader
                .pre
                .buttons_physical
                .set(row, Some(kept as u16));
        });
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames,
                    ..
                } if frame == FIRST + row as i32 && checked_frames == row as u64
            ),
            "{}",
            case.name
        );
    }
}

fn jump_direction_data() -> MatchData {
    let mut data = shield_drop_data();
    for fighter in &mut data.fighters {
        fighter.locomotion.as_mut().unwrap().jump_backward_threshold = Some(0.3);
    }
    data
}

#[test]
fn file_backed_jump_variants_match_and_detect_their_first_changed_direction_frame() {
    // Fighter 0 full hops from the platform floor at X=0 (button held rows
    // 0..1, so short_hop stays false); fighter 1 idles. `jump_backward_threshold`
    // is 0.3, an invented fixture value: `ftCo_Jump_Enter`'s own direction test
    // (`fp->input.lstick[0].x`) is dispatched from the Anim callback, which
    // runs before this same frame's own controller read updates `fp->input`
    // (`game::locomotion::ground_jump`'s own doc comment), so the ground
    // jump's launch (row 2, where JumpSquat's `jump_startup_frames` of 2
    // expires) consults row 1's stick, not its own; the aerial jump's own
    // launch (IASA-dispatched, unaffected) still consults that frame's own
    // stick sample.
    let data = jump_direction_data();
    struct Case {
        name: &'static str,
        stick_x: f32,
        double_jump_row: usize,
        checks: &'static [(usize, Action, u16, u32)],
    }
    let cases = [
        Case {
            name: "backward short hop into a backward double jump into the aerial fall",
            stick_x: -1.0,
            // A fresh press one frame after the ground jump launches, while
            // still ascending.
            double_jump_row: 3,
            checks: &[
                (2, Action::Jump, 26, 17),
                (3, Action::JumpAerial, 28, 19),
                // JumpAerial's own animation end (entry row 3 plus the
                // fixture's `air_jump_animation_frames` of 20) reports the
                // aerial fall regardless of direction.
                (23, Action::Fall, 32, 23),
            ],
        },
        Case {
            name: "forward short hop reaching the ordinary apex fall, then a forward double jump",
            stick_x: 1.0,
            // A fresh press after the full hop's own vertical velocity
            // (2.4) has decayed under the fixture's gravity (0.2) past zero,
            // so this double jump launches from Fall, not Jump.
            double_jump_row: 16,
            checks: &[
                (2, Action::Jump, 25, 16),
                (15, Action::Fall, 29, 20),
                (16, Action::JumpAerial, 27, 18),
            ],
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 30];
        inputs[0][0].buttons = BUTTON_X;
        inputs[1][0].buttons = BUTTON_X;
        inputs[1][0].stick[0] = case.stick_x;
        inputs[case.double_jump_row][0].buttons = BUTTON_X;
        inputs[case.double_jump_row][0].stick[0] = case.stick_x;
        let recording = Recording::from_script(data.clone(), 51, inputs);
        for &(row, action, state, animation) in case.checks {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "{} row {row}", case.name);
            assert_eq!(
                observation::action_state(fighter, Some(2)),
                Some(state),
                "{} row {row}",
                case.name
            );
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation),
                "{} row {row}",
                case.name
            );
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        // Flipping row 1's stick (the ground jump's actual direction input)
        // does not change row 1's own JumpSquat report, so the mismatch
        // appears one frame later, at the launch row.
        let flip_row = 1;
        let mismatch_row = 2;
        let flipped = -case.stick_x;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            frames.ports[0]
                .leader
                .pre
                .joystick
                .x
                .set(flip_row, Some(flipped));
        });
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames,
                    ..
                } if frame == FIRST + mismatch_row && checked_frames == mismatch_row as u64
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn file_backed_air_dodges_match_and_detect_their_first_changed_trigger_frame() {
    // Fighter 0 full hops from the platform floor at X=0 and air dodges.
    let data = escape_air_support::profile(shield_drop_data());
    struct Case {
        name: &'static str,
        dodge_row: usize,
        stick: [f32; 2],
        checks: &'static [(usize, Action, u16, u32)],
        intangible_row: Option<usize>,
    }
    let cases = [
        Case {
            name: "downward dodge landing straight into the special landing",
            dodge_row: 3,
            stick: [1.0, -1.0],
            checks: &[
                (3, Action::EscapeAir, 236, 44),
                (4, Action::LandingFallSpecial, 43, 36),
            ],
            intangible_row: None,
        },
        Case {
            name: "neutral dodge through FallSpecial",
            dodge_row: 4,
            stick: [0.0, 0.0],
            checks: &[
                (4, Action::EscapeAir, 236, 44),
                (12, Action::FallSpecial, 35, 26),
            ],
            intangible_row: Some(7),
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 30];
        inputs[0][0].buttons = BUTTON_X;
        inputs[1][0].buttons = BUTTON_X;
        inputs[case.dodge_row][0].buttons = BUTTON_L;
        inputs[case.dodge_row][0].stick = case.stick;
        let recording = Recording::from_script(data.clone(), 43, inputs);
        assert_eq!(recording.states[2].fighters[0].action, Action::Jump);
        for &(row, action, state, animation) in case.checks {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "{} row {row}", case.name);
            assert_eq!(observation::action_state(fighter, Some(2)), Some(state));
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation)
            );
        }
        if let Some(row) = case.intangible_row {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, Action::EscapeAir);
            assert_eq!(observation::hurtbox_state(fighter), 2, "{}", case.name);
        }
        assert!(
            recording
                .states
                .iter()
                .any(|state| state.fighters[0].action == Action::LandingFallSpecial),
            "{}",
            case.name
        );
        assert_eq!(
            recording.states[29].fighters[0].action,
            Action::Wait,
            "{}",
            case.name
        );

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let row = case.dodge_row;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            frames.ports[0].leader.pre.buttons.set(row, Some(0));
            frames.ports[0]
                .leader
                .pre
                .buttons_physical
                .set(row, Some(0));
        });
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames,
                    ..
                } if frame == FIRST + row as i32 && checked_frames == row as u64
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn file_backed_tilts_match_and_detect_their_first_changed_attack_frame() {
    let data = tilt_support::profile(shield_drop_data());
    struct Case {
        name: &'static str,
        stick: [f32; 2],
        checks: &'static [(usize, Action, u16, u32)],
        repeat: bool,
    }
    let cases = [
        Case {
            name: "straight forward tilt",
            stick: [1.0, 0.0],
            checks: &[(0, Action::AttackS3S, 53, 55), (7, Action::Wait, 14, 2)],
            repeat: false,
        },
        Case {
            name: "high forward tilt",
            stick: [1.0, 0.5],
            checks: &[(0, Action::AttackS3Hi, 51, 53)],
            repeat: false,
        },
        Case {
            name: "up tilt",
            stick: [0.0, 1.0],
            checks: &[(0, Action::AttackHi3, 56, 58), (6, Action::Wait, 14, 2)],
            repeat: false,
        },
        Case {
            name: "down tilt with a buffered repeat",
            stick: [0.0, -1.0],
            checks: &[
                (0, Action::AttackLw3, 57, 59),
                (3, Action::AttackLw3, 57, 59),
                (9, Action::SquatRv, 41, 34),
                (12, Action::Wait, 14, 2),
            ],
            repeat: true,
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 14];
        inputs[0][0].buttons = BUTTON_A;
        inputs[0][0].stick = case.stick;
        if case.repeat {
            // Released on row 1, pressed again on row 2 before the repeat flag.
            inputs[2][0].buttons = BUTTON_A;
        }
        let recording = Recording::from_script(data.clone(), 47, inputs);
        for &(row, action, state, animation) in case.checks {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "{} row {row}", case.name);
            assert_eq!(observation::action_state(fighter, Some(2)), Some(state));
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation)
            );
        }
        if case.repeat {
            assert_eq!(
                recording.states[3].fighters[0].action_frame, 1,
                "{}",
                case.name
            );
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let changed = recording.bytes(support::Fixture::default(), |frames| {
            frames.ports[0].leader.pre.buttons.set(0, Some(0));
            frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        });
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame: FIRST,
                    checked_frames: 0,
                    ..
                }
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn file_backed_smashes_match_and_detect_their_first_changed_input_frame() {
    let data = smash_support::profile(tilt_support::profile(shield_drop_data()));
    struct Case {
        name: &'static str,
        buttons: u16,
        stick: [f32; 2],
        cstick: [f32; 2],
        held_rows: usize,
        checks: &'static [(usize, Action, u16, u32, u32)],
        changed_row: usize,
    }
    let cases = [
        Case {
            // A held through row 5: the pose-2 command charges for three
            // frames and the release resumes the animation on row 6.
            name: "charged forward smash",
            buttons: BUTTON_A,
            stick: [1.0, 0.0],
            cstick: [0.0, 0.0],
            held_rows: 6,
            checks: &[
                (0, Action::AttackS4S, 60, 62, 1),
                (2, Action::AttackS4S, 60, 62, 2),
                (5, Action::AttackS4S, 60, 62, 2),
                (6, Action::AttackS4S, 60, 62, 3),
                (14, Action::Wait, 14, 2, 1),
            ],
            changed_row: 3,
        },
        Case {
            name: "C-stick down smash",
            buttons: 0,
            stick: [0.0, 0.0],
            cstick: [0.0, -1.0],
            held_rows: 1,
            checks: &[
                (0, Action::AttackLw4, 64, 66, 1),
                (8, Action::Wait, 14, 2, 1),
            ],
            changed_row: 0,
        },
        Case {
            name: "up smash",
            buttons: BUTTON_A,
            stick: [0.0, 1.0],
            cstick: [0.0, 0.0],
            held_rows: 1,
            checks: &[
                (0, Action::AttackHi4, 63, 65, 1),
                (9, Action::Wait, 14, 2, 1),
            ],
            changed_row: 0,
        },
    ];
    for case in cases {
        let mut inputs = vec![IDLE; 16];
        inputs[0][0].buttons = case.buttons;
        inputs[0][0].stick = case.stick;
        inputs[0][0].cstick = case.cstick;
        for row in inputs.iter_mut().take(case.held_rows).skip(1) {
            row[0].buttons = case.buttons;
        }
        let recording = Recording::from_script(data.clone(), 47, inputs);
        for &(row, action, state, animation, action_frame) in case.checks {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "{} row {row}", case.name);
            assert_eq!(
                fighter.action_frame, action_frame,
                "{} row {row}",
                case.name
            );
            assert_eq!(observation::action_state(fighter, Some(2)), Some(state));
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation)
            );
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        let row = case.changed_row;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            pre.buttons.set(row, Some(0));
            pre.buttons_physical.set(row, Some(0));
            pre.cstick.x.set(row, Some(0.0));
            pre.cstick.y.set(row, Some(0.0));
        });
        assert!(
            matches!(
                recording.compare(&changed).outcome,
                Outcome::Mismatch {
                    frame,
                    checked_frames,
                    ..
                } if frame == FIRST + row as i32 && checked_frames == row as u64
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn file_backed_jab_combos_match_and_detect_their_first_changed_press_frame() {
    let data = jab_support::profile(smash_support::profile(tilt_support::profile(
        shield_drop_data(),
    )));

    // press, hold, release, press: the release (count 1) makes the row 3
    // press fresh; row 3 is also the first pose whose follow_up_ready is
    // raised (pose 3 of the 5-pose first jab), so that fresh press both
    // latches and fires the second jab in the same row (a held run cannot
    // be used here: editing an earlier row of a hold would just shift the
    // fresh edge onto the next row and still fire, since the follow-up
    // latch does not require the firing row's own press to be fresh). A
    // release then a fresh press fires the third the moment the second
    // jab's own follow-up flag (pose 2 of its 6 poses) is reached: Slippi
    // states 44, 45, 46 with animations 46, 47, 48.
    {
        let mut inputs = vec![IDLE; 12];
        inputs[0][0].buttons = BUTTON_A;
        inputs[1][0].buttons = BUTTON_A;
        inputs[3][0].buttons = BUTTON_A;
        inputs[5][0].buttons = BUTTON_A;
        let recording = Recording::from_script(data.clone(), 47, inputs);
        for &(row, action, state, animation) in &[
            (0, Action::Jab, 44, 46),
            (3, Action::Attack12, 45, 47),
            (5, Action::Attack13, 46, 48),
        ] {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "row {row}");
            assert_eq!(fighter.action_frame, 1, "row {row}");
            assert_eq!(observation::action_state(fighter, Some(2)), Some(state));
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation)
            );
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        // Removing row 3's press: it is no longer fresh (row 2 was already
        // released), so the second jab never fires.
        let row = 3;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            pre.buttons.set(row, Some(0));
            pre.buttons_physical.set(row, Some(0));
        });
        assert!(matches!(
            recording.compare(&changed).outcome,
            Outcome::Mismatch {
                frame,
                checked_frames,
                ..
            } if frame == FIRST + row as i32 && checked_frames == row as u64
        ));
    }

    // press, release, press, release: the entry pose's rapid flag is live
    // immediately, so three counted presses/releases (1, 2, 3) reach the
    // rapid window on row 3, before the ordinary follow-up (pose 3) is ever
    // checked: Attack100Start, Attack100Loop and Attack100End on their
    // first poses, states 47, 48, 49 with animations 49, 50, 51.
    {
        let mut inputs = vec![IDLE; 12];
        inputs[0][0].buttons = BUTTON_A;
        inputs[2][0].buttons = BUTTON_A;
        let recording = Recording::from_script(data, 47, inputs);
        for &(row, action, state, animation, frame) in &[
            (3, Action::Attack100Start, 47, 49, 1),
            (6, Action::Attack100Loop, 48, 50, 1),
            (9, Action::Attack100End, 49, 51, 1),
        ] {
            let fighter = &recording.states[row].fighters[0];
            assert_eq!(fighter.action, action, "row {row}");
            assert_eq!(fighter.action_frame, frame, "row {row}");
            assert_eq!(observation::action_state(fighter, Some(2)), Some(state));
            assert_eq!(
                observation::animation_index(fighter, Some(2)),
                Some(animation)
            );
        }

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        // Removing row 2's press leaves the count at 2 (rows 1 and 3 are
        // both releases, and row 3 is not fresh once row 2 is already
        // idle), so Attack100Start no longer fires on row 3.
        let edited_row: usize = 2;
        let expected_row: i32 = 3;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            pre.buttons.set(edited_row, Some(0));
            pre.buttons_physical.set(edited_row, Some(0));
        });
        assert!(matches!(
            recording.compare(&changed).outcome,
            Outcome::Mismatch {
                frame,
                checked_frames,
                ..
            } if frame == FIRST + expected_row && checked_frames == expected_row as u64
        ));
    }
}

fn dash_replay_data() -> MatchData {
    dash_support::profile(grab_support::profile(shield_drop_data()))
}

#[test]
fn file_backed_dash_attacks_and_late_redash_match_and_detect_their_first_changed_input_frame() {
    // Hold forward through Dash (dash_run_frame 8) into Run, then press A:
    // ftCo_AttackDash_CheckInput/SetMv0 fire from Run, entering AttackDash
    // (Slippi state 50, animation 52).
    {
        let mut inputs = vec![IDLE; 16];
        for input in &mut inputs[0..=9] {
            input[0].stick = [1.0, 0.0];
        }
        inputs[9][0].buttons = BUTTON_A;
        let recording = Recording::from_script(dash_replay_data(), 47, inputs);
        assert_eq!(recording.states[8].fighters[0].action, Action::Run);
        let attack = &recording.states[9].fighters[0];
        assert_eq!(attack.action, Action::AttackDash);
        assert_eq!(attack.action_frame, 1);
        assert_eq!(observation::action_state(attack, Some(2)), Some(50));
        assert_eq!(observation::animation_index(attack, Some(2)), Some(52));

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        // Removing row 9's press: Run continues instead of entering
        // AttackDash, so the first Peppi field diverges on that same row.
        let edited_row: usize = 9;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            pre.buttons.set(edited_row, Some(0));
            pre.buttons_physical.set(edited_row, Some(0));
        });
        assert!(matches!(
            recording.compare(&changed).outcome,
            Outcome::Mismatch { frame, checked_frames, .. }
                if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
        ));
    }

    // Dash, release to neutral through the early and middle phases (limit
    // 6.0), then a fresh forward press in the late phase (frame 8: `game::
    // locomotion::start_dash`'s entry-time `action_frame = 1` keeps every
    // Dash gate aligned with decomp's own frame count, one higher than
    // before this alignment) restarts Dash via ftCo_Dash_CheckInput
    // (dash_from_input = true, action_frame resets to 1, read back as 2
    // once this same frame's ordinary end-of-frame tail also runs), rather
    // than merely continuing the same Dash instance.
    {
        let mut inputs = vec![IDLE; 14];
        inputs[0][0].stick = [1.0, 0.0];
        inputs[7][0].stick = [1.0, 0.0];
        let recording = Recording::from_script(dash_replay_data(), 47, inputs);
        for row in 1..=6 {
            assert_eq!(
                recording.states[row].fighters[0].action,
                Action::Dash,
                "row {row}"
            );
        }
        let redashed = &recording.states[7].fighters[0];
        assert_eq!(redashed.action, Action::Dash);
        assert_eq!(redashed.action_frame, 2);

        let bytes = recording.bytes(support::Fixture::default(), |_| {});
        matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
        // Removing row 7's press: the stick stays neutral, so the dash
        // never restarts and the first Peppi field diverges on that row.
        let edited_row: usize = 7;
        let changed = recording.bytes(support::Fixture::default(), move |frames| {
            let pre = &mut frames.ports[0].leader.pre;
            pre.joystick.x.set(edited_row, Some(0.0));
        });
        assert!(matches!(
            recording.compare(&changed).outcome,
            Outcome::Mismatch { frame, checked_frames, .. }
                if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
        ));
    }
}

/// The fixture's `landing_frames` is 2, too short for an interrupt window;
/// this widens it to 6 and opens `normal_landing_lag` at 3.0, matching
/// `tests/game_landing.rs`.
fn landing_replay_data() -> MatchData {
    let mut data = smash_support::profile(shield_drop_data());
    for fighter in &mut data.fighters {
        fighter.movement.landing_frames = 6;
        fighter.movement.normal_landing_lag = Some(3.0);
    }
    data
}

#[test]
fn file_backed_short_hop_landing_and_first_interruptible_smash_match() {
    // X pressed then released selects a short hop; the fighter lands
    // fifteen steps later (Slippi state 42 while grounded, `ftCo_Landing.c`
    // ends into Wait at `landing_frames`). Two more neutral steps put
    // `cur_anim_frame` at `normal_landing_lag` (3.0); the next step's fresh
    // C-stick opens the Wait chain's smash check on that first interruptible
    // frame.
    let mut inputs = vec![IDLE; 22];
    inputs[0][0].buttons = BUTTON_X;
    inputs[18][0].cstick = [1.0, 0.0];
    let recording = Recording::from_script(landing_replay_data(), 47, inputs);
    for row in 15..=17 {
        assert_eq!(
            recording.states[row].fighters[0].action,
            Action::Landing,
            "row {row}"
        );
        assert_eq!(
            observation::action_state(&recording.states[row].fighters[0], Some(2)),
            Some(42)
        );
    }
    let smash = &recording.states[18].fighters[0];
    assert_eq!(smash.action, Action::AttackS4S);
    assert_eq!(smash.action_frame, 1);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Removing row 18's C-stick sample: the smash never opens, so Landing
    // continues (and later ends into Wait) instead, diverging at that row.
    let edited_row: usize = 18;
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        let pre = &mut frames.ports[0].leader.pre;
        pre.cstick.x.set(edited_row, Some(0.0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
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
fn file_backed_powershield_covers_reflector_immunity_and_guard_reflect_state() {
    let mut inputs = vec![IDLE; 8];
    for input in &mut inputs {
        input[1].buttons = BUTTON_L;
    }
    inputs[0][0].buttons = BUTTON_A;
    let recording = Recording::from_script(powershield_data(), 23, inputs);
    let first = &recording.states[0].fighters[1];
    assert_eq!(first.action, Action::GuardReflect);
    assert_eq!(observation::action_state(first, Some(2)), Some(182));
    assert_eq!(observation::animation_index(first, Some(2)), Some(37));
    assert_eq!(observation::state_flags(first)[0] & 0x10, 0x10);
    assert_eq!(observation::state_flags(first)[3] & 0x20, 0x20);
    assert!(recording.states.iter().any(|state| {
        state.fighters[1].shield.reflecting && !state.fighters[1].shield.powershield
    }));
    assert!(recording.states.iter().any(|state| {
        !state.fighters[1].shield.reflecting && !state.fighters[1].shield.powershield
    }));
    let touch_row = recording
        .states
        .iter()
        .position(|state| state.fighters[1].shield.touched)
        .expect("inert jab must overlap the powershield");
    assert_eq!(
        observation::state_flags(&recording.states[touch_row].fighters[1])[3] & 0x04,
        0x04
    );
    assert!(
        recording.states[touch_row]
            .events
            .iter()
            .all(|event| !matches!(event, Event::Hit { .. } | Event::ShieldHit { .. }))
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    for (byte, mask, field, row) in [
        (0, 0x10, "state_flags.reflect", 0),
        (3, 0x20, "state_flags.powershield", 0),
        (3, 0x04, "state_flags.shield_touch", touch_row),
    ] {
        let corrupted = recording.bytes(support::Fixture::default(), |frames| {
            let flags = frames.ports[1].leader.post.state_flags.as_mut().unwrap();
            let value = observation::state_flags(&recording.states[row].fighters[1])[byte] ^ mask;
            match byte {
                0 => flags.0.set(row, Some(value)),
                3 => flags.3.set(row, Some(value)),
                _ => unreachable!(),
            }
        });
        assert!(matches!(
            recording.compare(&corrupted).outcome,
            Outcome::Mismatch {
                frame,
                checked_frames,
                ref difference,
            } if frame == FIRST + row as i32
                && checked_frames == row as u64
                && difference.port == PORTS[1]
                && difference.field == field
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
                "state_flags.reflect" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .0
                    .set(row, Some(observation::state_flags(fighter)[0] ^ 0x10)),
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
                "state_flags.shield_touch" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .3
                    .set(row, Some(observation::state_flags(fighter)[3] ^ 0x04)),
                "state_flags.powershield" => post
                    .state_flags
                    .as_mut()
                    .unwrap()
                    .3
                    .set(row, Some(observation::state_flags(fighter)[3] ^ 0x20)),
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
    let report_path = directory.path().join("report.json");
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
            .arg("--report")
            .arg(&report_path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            !mismatch,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        // `--report` writes exactly the same JSON also printed to stdout, so
        // CI can read the outcome without capturing process output.
        let written: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
        assert_eq!(report, written);
        assert_eq!(report["policy"], "fighter-post-v11");
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

#[test]
fn cli_chains_make_initialization_into_validate_replay_for_a_self_recorded_fox_battlefield_replay()
{
    // `support::Fixture`'s defaults already record P1/P3 as human Fox players
    // with 4 stocks each on stage 31 (Battlefield); naming the match data to
    // match lets `make-initialization` accept it without inventing a real
    // export. This is a self-recorded regression, not independent Melee
    // evidence; see docs/parity.md.
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    data.stage.name = "Battlefield".to_string();
    data.rules.stocks = 4;
    for fighter in &mut data.fighters {
        fighter.name = "Fox".to_string();
    }
    let recording = Recording::from_script(data, 7, vec![IDLE; 4]);
    let bytes = recording.bytes(support::Fixture::default(), |_| {});

    let directory = tempfile::tempdir().unwrap();
    let replay_path = directory.path().join("self-recorded.slp");
    let match_data_path = directory.path().join("match-data.json");
    let initialization_path = directory.path().join("initialization.json");
    let report_path = directory.path().join("report.json");
    fs::write(&replay_path, &bytes).unwrap();
    fs::write(
        &match_data_path,
        serde_json::to_vec(&recording.initialization.data).unwrap(),
    )
    .unwrap();

    let make = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("make-initialization")
        .arg("--match-data")
        .arg(&match_data_path)
        .arg("--replay")
        .arg(&replay_path)
        .arg("--output")
        .arg(&initialization_path)
        .output()
        .unwrap();
    assert!(
        make.status.success(),
        "{}",
        String::from_utf8_lossy(&make.stderr)
    );

    let validate = Command::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg("validate-replay")
        .arg(&replay_path)
        .arg("--initialization")
        .arg(&initialization_path)
        .arg("--report")
        .arg(&report_path)
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(report["outcome"]["status"], "matched");
    assert_eq!(report["outcome"]["checked_frames"], 4);
}

fn edge_replay_data() -> MatchData {
    let mut data = edge_support::profile(shield_drop_data());
    data.stage.geometry = None;
    data.stage.floor.left = -40.0;
    data.stage.floor.right = -0.5;
    data.stage.spawns = [[-2.0, 0.0], [-39.0, 0.0]];
    data
}

#[test]
fn file_backed_teeter_walk_then_jump_matches_and_detects_the_first_outward_stick_frame() {
    // Walking into the right floor end (mode 1, `mpColl_8004A678_Floor`)
    // clamps and teeters: Ottotto (245, animation 210) at row 3, then
    // OttottoWait (246, animation 211) at row 6, held with no physics of
    // its own through row 9. Releasing the stick before jumping (row 10)
    // avoids the stale `interruptible_tilt` reuse in `locomotion::
    // update_actions`'s shared Wait-chain arm -- a fresh jump press
    // combined with an admissible walk stick on the very same frame lets
    // that arm's walk branch re-fire against the just-entered JumpSquat
    // action and immediately overwrite it with Walk, which this profile's
    // own Teeter mode then re-clamps back into Ottotto within the same
    // frame; this looks like a pre-existing quirk of the shared dispatch,
    // not something this batch introduced, and is avoided here rather than
    // fixed (out of scope: it is not specific to the edge/teeter work).
    let mut inputs = vec![IDLE; 20];
    for input in &mut inputs[0..9] {
        input[0].stick = [0.5, 0.0];
    }
    inputs[10][0].buttons = BUTTON_X;
    let recording = Recording::from_script(edge_replay_data(), 47, inputs);
    assert_eq!(recording.states[2].fighters[0].action, Action::Walk);
    let teeter = &recording.states[3];
    assert_eq!(teeter.fighters[0].action, Action::Ottotto);
    assert_eq!(
        observation::action_state(&teeter.fighters[0], Some(2)),
        Some(245)
    );
    assert_eq!(
        observation::animation_index(&teeter.fighters[0], Some(2)),
        Some(210)
    );
    assert_eq!(teeter.fighters[0].velocity, [0.0, 0.0]);
    let waiting = &recording.states[6];
    assert_eq!(waiting.fighters[0].action, Action::OttottoWait);
    assert_eq!(
        observation::action_state(&waiting.fighters[0], Some(2)),
        Some(246)
    );
    assert_eq!(
        observation::animation_index(&waiting.fighters[0], Some(2)),
        Some(211)
    );
    for row in 6..=9 {
        assert_eq!(
            recording.states[row].fighters[0].position[0],
            teeter.fighters[0].position[0]
        );
    }
    assert_eq!(recording.states[11].fighters[0].action, Action::JumpSquat);
    assert_eq!(recording.states[12].fighters[0].action, Action::Jump);
    assert!(!recording.states[12].fighters[0].grounded);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Row 3 is the exact frame the walk crosses the floor's right end
    // (mpcoll.c's mode-1 literal is exclusive: `lstick_x < 0.75`). Pushing
    // that one row's stick out to precisely 0.75 instead of 0.5 fails the
    // teeter gate, so the fighter falls off the edge instead of entering
    // Ottotto, diverging every observation from that row on.
    let edited_row: usize = 3;
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        let pre = &mut frames.ports[0].leader.pre;
        pre.joystick.x.set(edited_row, Some(0.75));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
    ));
}

fn run_replay_data() -> MatchData {
    let mut data = run_support::profile(shield_drop_data());
    data.stage.floor.left = -200.0;
    data.stage.floor.right = 200.0;
    data.stage.blast = [-500.0, 500.0, -100.0, 200.0];
    if let Some(geometry) = &mut data.stage.geometry {
        geometry.lines[0].start[0] = -200.0;
        geometry.lines[0].end[0] = 200.0;
        geometry.joints[0].bounds_min[0] = -200.0;
        geometry.joints[0].bounds_max[0] = 200.0;
    }
    data
}

#[test]
fn file_backed_dash_into_a_run_reports_state_21_with_float_ages_and_a_reduced_stick_mismatch() {
    let mut inputs = vec![IDLE; 40];
    for input in &mut inputs {
        input[0].stick = [1.0, 0.0];
    }
    let recording = Recording::from_script(run_replay_data(), 47, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::Dash);

    let run_row = recording
        .states
        .iter()
        .position(|state| state.fighters[0].action == Action::Run)
        .expect("expected the dash ramp to reach Run");
    assert_eq!(
        recording.states[run_row].fighters[0].locomotion.run.frame,
        0.0
    );
    for row in run_row..recording.states.len() {
        let fighter = &recording.states[row].fighters[0];
        if fighter.action != Action::Run {
            break;
        }
        assert_eq!(observation::action_state(fighter, Some(2)), Some(21));
        assert_eq!(observation::animation_index(fighter, Some(2)), Some(13));
    }

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // Unlike Walk's mid-walk retype (which recomputes the reported kind/
    // frame from the *current* frame's velocity within the same frame,
    // `game::locomotion::retype_walk`), Run's reported age
    // (`locomotion.run.frame`) is always the *previous* frame's stored
    // rate, applied before this frame's own `ground_velocity` is read
    // (`advance_run_animation`); Anim precedes the ground-movement physics
    // that reacts to the stick, so a stick edited at row `r` first changes
    // `ground_velocity` at the *end* of row `r`. This codebase's own
    // comparison already reports a mismatch starting at row `r`
    // (ground_velocity/position are compared fields too, and already
    // differ there), but the reported *age* itself is one frame further
    // removed still: row `r + 1`'s Anim call is the first to read the
    // changed velocity into a new `last_rate`, so `run.frame` itself only
    // starts to differ from row `r + 2` onward. Edit well into the ramp
    // (row `run_row + 5`, past the one-frame entry transient) and confirm
    // both: the mismatch is reported starting at that row, and the
    // recorded age's own first divergence is exactly two rows later.
    let edited_row = run_row + 5;
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        let pre = &mut frames.ports[0].leader.pre;
        pre.joystick.x.set(edited_row, Some(0.75));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
    ));

    let edited_recording = Recording::from_script(run_replay_data(), 47, {
        let mut edited_inputs = recording.inputs.clone();
        edited_inputs[edited_row][0].stick[0] = 0.75;
        edited_inputs
    });
    let first_age_divergence = (0..recording.states.len())
        .find(|&row| {
            recording.states[row].fighters[0].locomotion.run.frame
                != edited_recording.states[row].fighters[0]
                    .locomotion
                    .run
                    .frame
        })
        .expect("expected the reduced stick to eventually change the reported age");
    assert_eq!(first_age_divergence, edited_row + 2);
}

fn idle_replay_data() -> MatchData {
    let mut data = idle_support::profile(shield_drop_data());
    data.stage.floor.left = -200.0;
    data.stage.floor.right = 200.0;
    data.stage.blast = [-500.0, 500.0, -100.0, 200.0];
    if let Some(geometry) = &mut data.stage.geometry {
        geometry.lines[0].start[0] = -200.0;
        geometry.lines[0].end[0] = 200.0;
        geometry.joints[0].bounds_min[0] = -200.0;
        geometry.joints[0].bounds_max[0] = 200.0;
    }
    data
}

#[test]
fn file_backed_idle_fighter_reports_the_picked_animation_index_with_a_restarting_age() {
    // Seed 12345's first HSD_Randi(100) draw (max 62) exceeds the two-entry
    // table's first weight (60), so this seed's first pick lands on the
    // second entry (animation 3) -- a visible transition away from Wait1_0,
    // unlike the default 42 used elsewhere (whose first draw stays on
    // animation 2, see `tests/game_idle.rs`).
    let seed = 12345;
    let recording = Recording::from_script(idle_replay_data(), seed, vec![IDLE; 12]);
    assert_eq!(recording.states[0].fighters[0].action, Action::Wait);
    for row in 0..2 {
        let fighter = &recording.states[row].fighters[0];
        assert_eq!(observation::animation_index(fighter, Some(2)), Some(2));
    }

    // Find the restart/pick row from a trace of the tracked idle frame,
    // rather than assuming which row it lands on.
    let pick_row = (1..recording.states.len())
        .find(|&row| {
            recording.states[row].fighters[0].idle.frame == 0.0
                && recording.states[row - 1].fighters[0].idle.frame > 0.0
        })
        .expect("expected a restart/pick within the recorded window");
    let picked = recording.states[pick_row].fighters[0].idle.animation;
    assert_eq!(picked, 3, "seed 12345's first draw picks the second entry");
    assert_eq!(
        observation::animation_index(&recording.states[pick_row].fighters[0], Some(2)),
        Some(picked)
    );
    // action_age is the restarted action_frame (unchanged by this batch),
    // observed here as 1: `restart` sets it to 0 within the animation
    // phase, then simulation::advance's own generic end-of-frame
    // `action_frame += 1` applies afterward (docs/idle.md).
    assert_eq!(recording.states[pick_row].fighters[0].action_frame, 1);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // Corrupt the picked row's own animation-index field and confirm the
    // comparison reports a mismatch starting exactly there.
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        let post = &mut frames.ports[0].leader.post;
        if let Some(animation) = &mut post.animation_index {
            animation.set(pick_row, Some(picked + 1));
        }
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + pick_row as i32 && checked_frames == pick_row as u64
    ));
}

fn walk_replay_data() -> MatchData {
    let mut data = walk_support::profile(shield_drop_data());
    data.stage.floor.left = -200.0;
    data.stage.floor.right = 200.0;
    data.stage.blast = [-500.0, 500.0, -100.0, 200.0];
    if let Some(geometry) = &mut data.stage.geometry {
        geometry.lines[0].start[0] = -200.0;
        geometry.lines[0].end[0] = 200.0;
        geometry.joints[0].bounds_min[0] = -200.0;
        geometry.joints[0].bounds_max[0] = 200.0;
    }
    data
}

#[test]
fn file_backed_walk_ramp_reports_15_16_and_17_with_float_ages_and_a_reduced_stick_mismatch() {
    let mut inputs = vec![IDLE; 40];
    for input in &mut inputs {
        input[0].stick = [0.75, 0.0];
    }
    let recording = Recording::from_script(walk_replay_data(), 47, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::Walk);

    let mut kind_changes = vec![];
    let mut previous = skirmish::game::locomotion::WalkKind::Slow;
    for (row, state) in recording.states.iter().enumerate() {
        let fighter = &state.fighters[0];
        assert_eq!(fighter.action, Action::Walk, "row {row}");
        let kind = fighter.locomotion.walk.kind;
        if kind != previous {
            kind_changes.push((row, kind));
            previous = kind;
        }
        let expected_state = 15
            + match kind {
                skirmish::game::locomotion::WalkKind::Slow => 0,
                skirmish::game::locomotion::WalkKind::Middle => 1,
                skirmish::game::locomotion::WalkKind::Fast => 2,
            };
        assert_eq!(
            observation::action_state(fighter, Some(2)),
            Some(expected_state)
        );
        assert_eq!(
            observation::animation_index(fighter, Some(2)),
            Some(expected_state as u32 - 8)
        );
    }
    assert_eq!(
        kind_changes
            .iter()
            .map(|(_, kind)| *kind)
            .collect::<Vec<_>>(),
        [
            skirmish::game::locomotion::WalkKind::Middle,
            skirmish::game::locomotion::WalkKind::Fast,
        ],
        "expected the ramp to reach Middle then Fast: {kind_changes:?}"
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // The first retype row (Slow -> Middle): pushing that row's stick down
    // to a magnitude that still walks but can no longer sustain the
    // velocity driving the retype changes the reported kind starting
    // exactly there, diverging from the recorded (unedited) expectation.
    let edited_row = kind_changes[0].0;
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        let pre = &mut frames.ports[0].leader.pre;
        pre.joystick.x.set(edited_row, Some(0.21));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + edited_row as i32 && checked_frames == edited_row as u64
    ));
}

fn taunt_replay_data() -> MatchData {
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    taunt_support::profile(data)
}

#[test]
fn file_backed_dpad_up_taunt_from_wait_reports_264_239_and_detects_its_removal() {
    let mut inputs = vec![IDLE; 6];
    inputs[0][0].buttons = skirmish::game::BUTTON_DPAD_UP;
    let recording = Recording::from_script(taunt_replay_data(), 42, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::AppealSR);
    assert_eq!(
        observation::action_state(&recording.states[0].fighters[0], Some(2)),
        Some(264)
    );
    assert_eq!(
        observation::animation_index(&recording.states[0].fighters[0], Some(2)),
        Some(239)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // Removing the D-pad-up press leaves fighter 0 in Wait: the comparison
    // diverges starting at the very frame the press's effect first appears.
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
fn file_backed_dpad_up_taunt_facing_left_reports_265_240_and_detects_its_removal() {
    // Fighter 1 faces -X by default, and the shared taunt fixture supplies
    // a left motion.
    let mut inputs = vec![IDLE; 6];
    inputs[0][1].buttons = skirmish::game::BUTTON_DPAD_UP;
    let recording = Recording::from_script(taunt_replay_data(), 42, inputs);
    assert_eq!(recording.states[0].fighters[1].action, Action::AppealSL);
    assert_eq!(
        observation::action_state(&recording.states[0].fighters[1], Some(2)),
        Some(265)
    );
    assert_eq!(
        observation::animation_index(&recording.states[0].fighters[1], Some(2)),
        Some(240)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[1].leader.pre.buttons.set(0, Some(0));
        frames.ports[1].leader.pre.buttons_physical.set(0, Some(0));
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
fn file_backed_wait_chain_spot_dodge_reports_235_on_the_l_press_frame() {
    // ftCo_Wait.c:58 precedes the ordinary shield check: a held shoulder
    // with the stick already down dodges on the very press frame, without a
    // GuardOn frame in between (`docs/shield.md`).
    let mut data = escape_support::profile(taunt_replay_data());
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(
            serde_json::from_str(include_str!("../../../tests/fixtures/game/locomotion.json"))
                .unwrap(),
        );
    }
    let mut inputs = vec![IDLE; 5];
    inputs[0][0].buttons = BUTTON_L;
    inputs[0][0].stick = [0.0, -1.0];
    let recording = Recording::from_script(data, 42, inputs);
    assert_eq!(recording.states[0].fighters[0].action, Action::EscapeN);
    assert_eq!(
        observation::action_state(&recording.states[0].fighters[0], Some(2)),
        Some(235)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // Removing the L press leaves fighter 0 in Wait (the stick alone never
    // dodges), diverging starting at the press frame.
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
fn a_replay_containing_an_inert_dpad_down_press_still_imports() {
    let mut inputs = vec![IDLE; 4];
    inputs[0][0].buttons = skirmish::game::BUTTON_DPAD_DOWN;
    let recording = Recording::from_script(taunt_replay_data(), 42, inputs);
    // Every D-pad bit except up is inert: fighter 0 stays in Wait.
    assert_eq!(recording.states[0].fighters[0].action, Action::Wait);
    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
}

#[test]
fn physical_b_drives_file_backed_fox_ground_illusion_and_detects_its_removal() {
    // Slippi 347 (SpecialSStart)/348 (SpecialS)/349 (SpecialSEnd); their
    // animation indices are an unverified extrapolation from the neutral
    // shell's own two confirmed data points (`docs/fox-side-special.md`),
    // not a confirmed fact -- the replay round trip below only requires
    // this recording's own `action_state`/`animation_index` calls to agree
    // with themselves, which they do regardless of that open question.
    let mut data = fox_side_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 10];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[0] = 0.6;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialSStart
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialS)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialSEnd)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press entirely: the fighter stays in Wait instead of
    // entering SpecialSStart, so the removed press's effect is visible on
    // the very first recorded frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.x.set(0, Some(0.0));
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
fn physical_b_drives_file_backed_fox_air_illusion_into_landing_fall_special() {
    // Slippi 43 (LandingFallSpecial, an already-verified mapping, unlike
    // the side-special ids in the ground scenario above).
    let mut data = fox_side_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 6.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 60];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[0] = 0.6;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialAirSStart
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialAirS)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialAirSEnd)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::FallSpecial)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::LandingFallSpecial)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press: the fighter free-falls (Fall) instead of
    // entering SpecialAirSStart, diverging from the very first frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.x.set(0, Some(0.0));
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
fn physical_b_drives_file_backed_fox_ground_reflector_through_start_loop_and_end() {
    // Slippi 360 (SpecialLwStart)/361 (SpecialLwLoop)/363 (SpecialLwEnd);
    // 362 (SpecialLwHit) is unreachable in play (no projectiles to reflect,
    // see docs/fox-down-special.md) and is not exercised here. Animation
    // indices are the same kind of unverified extrapolation the side
    // special's own ids are; this replay only requires this recording's
    // own action_state/animation_index calls to agree with themselves,
    // which they do regardless of that open question.
    let mut data = fox_down_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 60];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[1] = -0.8;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialLwStart
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialLw)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialLwEnd)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press entirely: the fighter stays in Wait instead of
    // entering SpecialLwStart, so the removed press's effect is visible on
    // the very first recorded frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.y.set(0, Some(0.0));
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
fn physical_b_drives_file_backed_fox_air_reflector_through_start_loop_and_end() {
    // Unlike the side special's own air End (which exits into FallSpecial),
    // the down special's End -- ground or air -- exits through the shared
    // Wait/Fall dispatch (`ftCommon_8007D92C`); Fall (Slippi's own
    // established mapping) is the expected terminal action here.
    let mut data = fox_down_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 6.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 60];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[1] = -0.6;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialAirLwStart
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialAirLw)
    );
    // The fixture's slow fall (gravity_delay/fall_accel) may land the
    // fighter before releaseLag elapses, converting to the grounded End
    // instead (ground<->air conversions preserve the frame and release
    // state, exercised separately by the native test suite); either End
    // variant confirms the release-lag exit fired.
    assert!(recording.states.iter().any(|s| matches!(
        s.fighters[0].action,
        Action::SpecialAirLwEnd | Action::SpecialLwEnd
    )));
    assert!(
        recording
            .states
            .iter()
            .any(|s| matches!(s.fighters[0].action, Action::Fall | Action::Wait))
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.y.set(0, Some(0.0));
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

/// Self-recorded harness regressions for the up special, like the side
/// special's own pair above: this proves the recorded trajectory reproduces
/// bit-exactly through this crate's own replay pipeline, not Melee parity
/// (`docs/parity.md`). Unlike the side special's one-shot entry press,
/// this move's grounded-vs-aerial launch decision reads the stick at the
/// moment Hold's own animation ends (`docs/fox-up-special.md`), so the
/// directional press is held for several frames here, not just the entry
/// one.
#[test]
fn physical_b_drives_file_backed_fox_ground_firefox_into_travel_and_landing() {
    // Slippi 353 (SpecialHiHold)/355 (SpecialHi)/357 (SpecialHiLanding);
    // their animation indices are an unverified extrapolation, exactly
    // like the side special's own ids (`docs/fox-up-special.md`) -- the
    // replay round trip below only requires this recording's own
    // `action_state`/`animation_index` calls to agree with themselves.
    let mut data = fox_up_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 20];
    // Frame 0's own entry press must stay clear of the side special's own
    // (checked-first) horizontal threshold, so it is purely vertical; the
    // held direction from frame 1 on drives the eventual launch decision
    // and no longer risks the side special's entry, since by then the
    // fighter is already in one of this move's own actions.
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick = [0.0, 0.9];
    for input in inputs.iter_mut().take(8).skip(1) {
        input[0].buttons = BUTTON_B;
        input[0].stick = [0.9, -0.5];
    }
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialHiHold
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialHi)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialHiLanding)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press entirely: the fighter stays in Wait instead of
    // entering SpecialHiHold, so the removed press's effect is visible on
    // the very first recorded frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.x.set(0, Some(0.0));
        frames.ports[0].leader.pre.joystick.y.set(0, Some(0.0));
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
fn physical_b_drives_file_backed_fox_air_firefox_into_fall_special() {
    // Slippi 354 (SpecialHiHoldAir)/356 (SpecialAirHi)/358 (SpecialHiFall).
    let mut data = fox_up_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 6.0], [2.0, 0.0]];
    let mut inputs = vec![IDLE; 70];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[1] = 0.9;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialHiHoldAir
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialAirHi)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::SpecialHiFall)
    );
    assert!(
        recording
            .states
            .iter()
            .any(|s| s.fighters[0].action == Action::FallSpecial)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press: the fighter free-falls (Fall) instead of
    // entering SpecialHiHoldAir, diverging from the very first frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.y.set(0, Some(0.0));
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

/// Self-recorded regression for the pack-derived Hold charge hitbox
/// (`docs/fox-up-special.md`'s "Hitboxes" section): a connecting hit against
/// a nearby second fighter during Hold's own charge, replayed byte-for-byte
/// through this crate's own Peppi-backed pipeline like the pair above --
/// again self-consistency evidence, not Melee parity (`docs/parity.md`).
#[test]
fn firefox_hold_charge_hitbox_self_recorded_replay_matches() {
    let mut data = fox_up_special_support::profile(aerial_support::conformance::data());
    data.stage.spawns = [[0.0, 0.0], [1.0, 0.0]];
    data.rules.knockback_speed = 0.0;
    let bones = data.fighters[0].bones.clone();
    let hold = fox_up_special_support::hold_attack_with_pack_hitboxes(&bones);
    match data.fighters[0].specials.as_mut() {
        Some(skirmish::game::characters::Specials::Fox { up: Some(up), .. }) => {
            up.hold.ground = hold.clone();
            up.hold.air = hold;
        }
        _ => panic!("fixture is missing its up-special resource"),
    }
    let mut inputs = vec![IDLE; 25];
    inputs[0][0].buttons = BUTTON_B;
    inputs[0][0].stick[1] = 0.9;
    let recording = Recording::from_script(data, 7, inputs);
    assert_eq!(
        recording.states[0].fighters[0].action,
        Action::SpecialHiHold
    );
    // The pack's own pulse (frame 20 of Hold's 44-pose set) connects well
    // within this recording's own 25 frames.
    assert!(recording.states.iter().any(|s| matches!(
        s.events.as_slice(),
        [Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }]
    )));
    assert!(recording.states.last().unwrap().fighters[1].percent > 0.0);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
    // Remove the entry press entirely: the fighter stays in Wait instead of
    // ever charging (and thus never connects the hit), diverging from the
    // very first recorded frame.
    let changed = recording.bytes(support::Fixture::default(), |frames| {
        frames.ports[0].leader.pre.buttons.set(0, Some(0));
        frames.ports[0].leader.pre.buttons_physical.set(0, Some(0));
        frames.ports[0].leader.pre.joystick.y.set(0, Some(0.0));
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

fn entry_replay_data() -> MatchData {
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -200.0;
    data.stage.floor.right = 200.0;
    data.stage.blast = [-300.0, 300.0, -300.0, 300.0];
    data.stage.spawns = [[-60.0, 10.0], [20.0, 10.0]];
    data.rules.entry = Some(skirmish::game::entry::EntryRules {
        start_frames: 30,
        end_frames: 30,
        scale_y: 0.0,
        invincibility_frames: 0,
        // `countdown_frames` is 0 above (validated `input_lock_frames <=
        // countdown_frames`); this fixture predates the input lock
        // (`docs/input-lock.md`) and is unaffected by it.
        input_lock_frames: 0,
    });
    for fighter in &mut data.fighters {
        fighter.trophy_scale = Some(0.9);
    }
    data
}

#[test]
fn file_backed_match_start_reports_322_323_324_then_fall_and_detects_a_state_id_change() {
    // Port P1 is Skirmish player 0 (slot 0, delay 5 frames); P3 is player 1
    // (slot 2, delay 15). `docs/match-start.md`'s replay table is verified
    // for slots 0 and 3 by `tests/game_entry.rs`'s native test; this file-
    // backed regression only needs the state-id/age sequence to reach the
    // file-format round trip, not the exact port slots.
    let recording = Recording::from_script(entry_replay_data(), 0, vec![IDLE; 70]);
    assert_eq!(recording.states[0].fighters[0].action, Action::Entry);
    let start_row = (1..recording.states.len())
        .find(|&row| recording.states[row].fighters[0].action == Action::EntryStart)
        .expect("slot 0 must reach EntryStart within the recorded window");
    let end_row = (start_row..recording.states.len())
        .find(|&row| recording.states[row].fighters[0].action == Action::EntryEnd)
        .expect("slot 0 must reach EntryEnd within the recorded window");
    let fall_row = (end_row..recording.states.len())
        .find(|&row| recording.states[row].fighters[0].action == Action::Fall)
        .expect("slot 0 must exit into Fall within the recorded window");
    assert_eq!(
        observation::action_state(&recording.states[0].fighters[0], Some(2)),
        Some(322)
    );
    assert_eq!(
        observation::action_state(&recording.states[start_row].fighters[0], Some(2)),
        Some(323)
    );
    assert_eq!(
        observation::action_state(&recording.states[end_row].fighters[0], Some(2)),
        Some(324)
    );

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());

    // Corrupt the EntryStart row's own recorded state id and confirm the
    // comparison reports a mismatch starting exactly there.
    let changed = recording.bytes(support::Fixture::default(), move |frames| {
        frames.ports[0].leader.post.state.set(start_row, Some(0));
    });
    assert!(matches!(
        recording.compare(&changed).outcome,
        Outcome::Mismatch { frame, checked_frames, .. }
            if frame == FIRST + start_row as i32 && checked_frames == start_row as u64
    ));
    // Fall is reached well before this fixture's synthetic landing would
    // matter for this test; confirming the row exists documents that the
    // exit transition itself was recorded, not asserted further here.
    let _ = fall_row;
}

#[test]
fn physical_z_drives_file_backed_grab_capture_at_differing_stocks_with_the_real_formula() {
    use skirmish::game::grab::EscapeFormula;

    // Fox's real `ftCommonData` constants (`x354..x368`), extracted to
    // `/mnt/archive/datasets/melee/skirmish-gameplay/v2/rules.json`'s
    // `grab.escape_formula`: base 30.0, handicap_scale 8.0, handicap_max
    // 9.0, rank_scale 15.0, rank_max 4.0, percent_scale 1.6. At handicap 9
    // (handicap rule off) and standing 0 (`slot = 1`), `ftCo_800DA824`
    // reduces to `8.0 * (9.0 - 9.0) + 30.0 + 15.0 * (4.0 - 1.0) = 75.0`,
    // matching the flattened `timer_base` this same exporter wrote
    // (`docs/grab-escape-timer.md`). This test exercises standing != 0.
    let formula = EscapeFormula {
        base: 30.0,
        handicap_scale: 8.0,
        handicap_max: 9.0,
        rank_scale: 15.0,
        rank_max: 4.0,
        percent_scale: 1.6,
    };

    let mut data =
        grab_support::profile(death_support::profile(aerial_support::conformance::data()));
    data.rules.stocks = 3;
    data.rules.countdown_frames = 0;
    data.rules.respawn_frames = 2;
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    data.stage.floor.left = -50.0;
    data.stage.floor.right = 50.0;
    data.stage.blast = [-80.0, 80.0, -40.0, 5.0];
    data.rules.knockback_decay = 0.0;
    data.rules.knockback_speed = 1.0;
    data.rules.hitlag.base = 0.0;
    data.rules.hitlag.damage_scale = 0.0;
    data.rules.grab.as_mut().unwrap().escape.formula = Some(formula);
    let death = data.rules.death.as_mut().unwrap();
    death.force_normal_top[1] = true;
    death.screen_chance_percent = 0;
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

    // Fighter 0 jabs fighter 1 straight up into the top blast zone (frame
    // 0), costing fighter 1 a stock while fighter 0 keeps all three; once
    // fighter 1 returns to active play, fighter 0 grabs it (frame 60), so
    // the two fighters now hold different live stock counts at the grab.
    let mut inputs = vec![IDLE; 70];
    inputs[0][0].buttons = BUTTON_A;
    inputs[60][0].buttons = BUTTON_Z;
    let recording = Recording::from_script(data, 23, inputs);

    let knocked_out = recording
        .states
        .iter()
        .find_map(|state| {
            state.events.iter().find_map(|event| match event {
                Event::Knockout { player: 1, stocks } => Some(*stocks),
                _ => None,
            })
        })
        .expect("script must knock out fighter 1 once");
    assert_eq!(knocked_out, 2, "fighter 1 must lose exactly one stock");

    let grabbed = recording
        .states
        .iter()
        .position(|state| {
            state.events.iter().any(|event| {
                matches!(
                    event,
                    Event::Grabbed {
                        holder: 0,
                        victim: 1
                    }
                )
            })
        })
        .expect("fighter 0 must grab fighter 1 after the respawn cycle");
    let victim = &recording.states[grabbed].fighters[1];
    assert_eq!(
        recording.states[grabbed].fighters[0].stocks, 3,
        "the holder must still hold every stock"
    );
    assert_eq!(
        victim.stocks, 2,
        "the victim must be down exactly one stock"
    );
    // Fighter 1's score (2 stocks) is strictly less than fighter 0's (3),
    // so fighter 1's standing is 1 (one opponent scores strictly higher),
    // not the tied 0 both fighters would hold at equal stocks.
    let expected = skirmish::fighter::grab::escape_timer(
        formula.base,
        formula.handicap_scale,
        formula.handicap_max,
        formula.rank_scale,
        formula.rank_max,
        formula.percent_scale,
        victim.percent,
        1,
        9,
    );
    assert_eq!(victim.grab.escape_timer, expected);
    // The tied-standing value both fighters would have held at equal
    // stocks is different, confirming the timer really tracks standing.
    let equal_standing = skirmish::fighter::grab::escape_timer(
        formula.base,
        formula.handicap_scale,
        formula.handicap_max,
        formula.rank_scale,
        formula.rank_max,
        formula.percent_scale,
        victim.percent,
        0,
        9,
    );
    assert_ne!(victim.grab.escape_timer, equal_standing);

    let bytes = recording.bytes(support::Fixture::default(), |_| {});
    matched(&recording.compare(&bytes), FIRST, recording.inputs.len());
}
