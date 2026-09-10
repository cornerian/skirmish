//! End-to-end air dodges (EscapeAir), their FallSpecial continuation and the
//! LandingFallSpecial landing in an explicitly synthetic native world.
#[path = "support/aerial.rs"]
mod aerial_support;
#[path = "support/escape_air.rs"]
mod escape_air_support;

use skirmish::collision::stage;
use skirmish::fighter::escape_air::launch_velocity;
use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_R, BUTTON_X, Controller, Event, Match, State,
    data::{BodyState, MatchData, StageGeometry},
};

/// Fighter 0 (facing +X at -2) dodges; fighter 1 (facing -X at +2) attacks.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
    }
    escape_air_support::profile(data)
}

fn buttons(buttons: u16) -> Controller {
    Controller {
        buttons,
        ..Default::default()
    }
}

fn stick(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn step(game: &mut Match, dodger: Controller, attacker: Controller) -> State {
    game.step([dodger, attacker]).unwrap().clone()
}

fn idle(game: &mut Match, dodger: Controller) -> State {
    step(game, dodger, Controller::default())
}

/// Press X for the two-frame JumpSquat (held for a full hop) and return the
/// launch-frame state: Jump with the vertical launch already integrated.
fn jump(game: &mut Match, full: bool) -> State {
    idle(game, buttons(BUTTON_X));
    idle(game, buttons(if full { BUTTON_X } else { 0 }));
    let launched = idle(game, buttons(0));
    assert_eq!(launched.fighters[0].action, Action::Jump);
    assert!(!launched.fighters[0].grounded);
    launched
}

fn hits(state: &State) -> usize {
    state
        .events
        .iter()
        .filter(|event| matches!(event, Event::Hit { victim: 0, .. }))
        .count()
}

#[test]
fn fresh_physical_l_or_r_dodges_from_jump_fall_and_double_jump_only() {
    let mut from_jump = Match::new(data(), 42).unwrap();
    let launched = jump(&mut from_jump, true);
    let dodged = idle(&mut from_jump, buttons(BUTTON_L));
    assert_eq!(dodged.fighters[0].action, Action::EscapeAir);
    assert_eq!(dodged.fighters[0].action_frame, 1);
    assert!(!dodged.fighters[0].fast_fall);
    assert_eq!(dodged.fighters[0].velocity, [0.0, 0.0]);
    assert_eq!(dodged.fighters[0].position, launched.fighters[0].position);
    assert_eq!(dodged.fighters[0].body_state, BodyState::Normal);

    let mut from_fall = Match::new(data(), 42).unwrap();
    jump(&mut from_fall, true);
    let mut frames = 0;
    while from_fall.state().fighters[0].action != Action::Fall {
        idle(&mut from_fall, buttons(0));
        frames += 1;
        assert!(frames < 40);
    }
    assert_eq!(
        idle(&mut from_fall, buttons(BUTTON_R)).fighters[0].action,
        Action::EscapeAir
    );

    let mut from_double = Match::new(data(), 42).unwrap();
    jump(&mut from_double, true);
    assert_eq!(
        idle(&mut from_double, buttons(BUTTON_X)).fighters[0].action,
        Action::JumpAerial
    );
    assert_eq!(
        idle(&mut from_double, buttons(BUTTON_L)).fighters[0].action,
        Action::EscapeAir
    );

    // Grounded presses, held shoulders and analog pressure never dodge.
    let mut grounded = Match::new(data(), 42).unwrap();
    assert_eq!(
        idle(&mut grounded, buttons(BUTTON_L)).fighters[0].action,
        Action::Wait
    );
    let mut held = Match::new(data(), 42).unwrap();
    idle(&mut held, buttons(BUTTON_X | BUTTON_L));
    idle(&mut held, buttons(BUTTON_X | BUTTON_L));
    let launched = idle(&mut held, buttons(BUTTON_L));
    assert_eq!(launched.fighters[0].action, Action::Jump);
    assert_eq!(
        idle(&mut held, buttons(BUTTON_L)).fighters[0].action,
        Action::Jump
    );
    let mut analog = Match::new(data(), 42).unwrap();
    jump(&mut analog, true);
    let state = idle(
        &mut analog,
        Controller {
            trigger: 1.0,
            ..Default::default()
        },
    );
    assert_eq!(state.fighters[0].action, Action::Jump);
}

#[test]
fn launch_velocity_decays_without_gravity_until_the_script_resumes_falling() {
    let mut game = Match::new(data(), 42).unwrap();
    jump(&mut game, true);
    // Rise one more frame so the later fast fall cannot reach the floor.
    let risen = idle(&mut game, buttons(0));
    let [x0, y0] = risen.fighters[0].position;
    let entry = idle(&mut game, stick(BUTTON_L, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::EscapeAir);
    let mut velocity = launch_velocity([1.0, 0.0], [0.3, 0.3], 6.0);
    assert_eq!(velocity, [6.0, 0.0]);
    let mut x = x0;
    let mut state = entry;
    for sample in 0..5 {
        velocity = [velocity[0] * 0.5, velocity[1] * 0.5];
        x += velocity[0];
        assert_eq!(
            state.fighters[0].action,
            Action::EscapeAir,
            "sample {sample}"
        );
        assert_eq!(state.fighters[0].velocity, velocity, "sample {sample}");
        assert_eq!(state.fighters[0].position, [x, y0], "sample {sample}");
        state = idle(&mut game, buttons(0));
    }
    // Sample 5 raises the skip-decay flag: gravity returns and the decayed
    // horizontal speed now follows ordinary aerial friction.
    assert_eq!(state.fighters[0].action, Action::EscapeAir);
    assert!(state.fighters[0].position[1] < y0);
    assert_eq!(state.fighters[0].velocity[1], -0.2);
    assert!(state.fighters[0].velocity[0] < velocity[0]);
    // A fresh downward tilt can fast fall again once falling resumed.
    let fast = idle(&mut game, stick(0, [0.0, -1.0]));
    assert_eq!(fast.fighters[0].action, Action::EscapeAir);
    assert!(fast.fighters[0].fast_fall);
    assert_eq!(fast.fighters[0].velocity[1], -3.0);

    let mut neutral = Match::new(data(), 42).unwrap();
    let launched = jump(&mut neutral, true);
    let position = launched.fighters[0].position;
    idle(&mut neutral, buttons(BUTTON_R));
    for _ in 0..4 {
        let state = idle(&mut neutral, buttons(0));
        assert_eq!(state.fighters[0].position, position);
        assert_eq!(state.fighters[0].velocity, [0.0, 0.0]);
    }

    // Inside the strict deadzone the stick is ignored; on its edge it is not.
    let mut inside = Match::new(data(), 42).unwrap();
    jump(&mut inside, true);
    assert_eq!(
        idle(&mut inside, stick(BUTTON_L, [0.29, -0.29])).fighters[0].velocity,
        [0.0, 0.0]
    );
    let mut edge = Match::new(data(), 42).unwrap();
    jump(&mut edge, true);
    let edge_state = idle(&mut edge, stick(BUTTON_L, [0.3, 0.0]));
    assert_eq!(edge_state.fighters[0].velocity, [3.0, 0.0]);
}

#[test]
fn intangible_samples_block_hits_only_on_their_frames() {
    // A short hop keeps the hovering hurtbox inside the attacker's jab reach.
    fn run(attack_frame: usize) -> (usize, f32) {
        let mut game = Match::new(data(), 42).unwrap();
        let mut total = 0;
        for frame in 0..12 {
            let dodger = match frame {
                0 => buttons(BUTTON_X),
                3 => buttons(BUTTON_L),
                _ => buttons(0),
            };
            let attacker = buttons(if frame == attack_frame { BUTTON_A } else { 0 });
            let state = step(&mut game, dodger, attacker);
            if frame == 3 && attack_frame != 2 {
                assert_eq!(state.fighters[0].action, Action::EscapeAir);
                assert!((state.fighters[0].position[1] - 1.2).abs() < 0.001);
            }
            if (5..=8).contains(&frame) && attack_frame != 2 {
                assert_eq!(state.fighters[0].action, Action::EscapeAir);
                assert_eq!(state.fighters[0].body_state, BodyState::Intangible);
            }
            total += hits(&state);
        }
        (total, game.state().fighters[0].percent)
    }
    // Sample 0 is vulnerable on the dodge's entry frame.
    assert_eq!(run(2), (1, 10.0));
    // Samples 2 and 3 are intangible while the jab is active.
    assert_eq!(run(4), (0, 0.0));
}

#[test]
fn air_dodge_ends_in_fall_special_with_all_jumps_used_then_lands_specially() {
    let mut game = Match::new(data(), 42).unwrap();
    jump(&mut game, true);
    idle(&mut game, buttons(0));
    let entry = idle(&mut game, buttons(BUTTON_L));
    assert_eq!(entry.fighters[0].action, Action::EscapeAir);
    for _ in 0..7 {
        let state = idle(&mut game, buttons(BUTTON_X));
        assert_eq!(state.fighters[0].action, Action::EscapeAir);
    }
    let special = idle(&mut game, buttons(BUTTON_X));
    assert_eq!(special.fighters[0].action, Action::FallSpecial);
    assert_eq!(special.fighters[0].locomotion.jumps_used, 2);
    assert!(!special.fighters[0].grounded);
    assert_eq!(special.fighters[0].body_state, BodyState::Normal);
    let mut frames = 0;
    while game.state().fighters[0].action == Action::FallSpecial {
        let state = idle(&mut game, buttons(BUTTON_X | BUTTON_A));
        assert!(matches!(
            state.fighters[0].action,
            Action::FallSpecial | Action::LandingFallSpecial
        ));
        frames += 1;
        assert!(frames < 40);
    }
    let landed = game.state().clone();
    assert_eq!(landed.fighters[0].action, Action::LandingFallSpecial);
    assert!(landed.fighters[0].grounded);
    assert!(landed.events.contains(&Event::Landed { player: 0 }));
    assert_eq!(landed.fighters[0].aerial.landing_rate, 6.1 / 10.0);
    assert!(!landed.fighters[0].aerial.allow_interrupt);
    // The special landing ignores input and lasts until its rate crosses x2EC.
    for callback in 1..10 {
        let state = idle(&mut game, buttons(BUTTON_X | BUTTON_A | BUTTON_L));
        assert_eq!(
            state.fighters[0].action,
            Action::LandingFallSpecial,
            "callback {callback}"
        );
        let mut expected = 0.0_f32;
        for _ in 0..callback {
            expected += 6.1 / 10.0;
        }
        assert_eq!(state.fighters[0].aerial.landing_elapsed, expected);
    }
    assert_eq!(idle(&mut game, buttons(0)).fighters[0].action, Action::Wait);
}

#[test]
fn a_downward_dodge_lands_directly_and_slides_with_ground_friction() {
    let mut game = Match::new(data(), 42).unwrap();
    let launched = jump(&mut game, true);
    assert!((launched.fighters[0].position[1] - 2.4).abs() < 0.001);
    let entry = idle(&mut game, stick(BUTTON_L, [1.0, -1.0]));
    assert_eq!(entry.fighters[0].action, Action::EscapeAir);
    let launch = launch_velocity([1.0, -1.0], [0.3, 0.3], 6.0);
    assert_eq!(
        entry.fighters[0].velocity,
        [launch[0] * 0.5, launch[1] * 0.5]
    );
    assert!(!entry.fighters[0].grounded);
    let landed = idle(&mut game, buttons(0));
    assert_eq!(landed.fighters[0].action, Action::LandingFallSpecial);
    assert!(landed.fighters[0].grounded);
    assert_eq!(landed.fighters[0].ground_velocity, launch[0] * 0.25);
    assert_eq!(landed.fighters[0].velocity[1], 0.0);
    let sliding = idle(&mut game, buttons(BUTTON_X));
    assert_eq!(sliding.fighters[0].action, Action::LandingFallSpecial);
    assert_eq!(sliding.fighters[0].ground_velocity, launch[0] * 0.25 - 0.2);
    assert!(sliding.fighters[0].position[0] > landed.fighters[0].position[0]);
}

#[test]
fn fall_special_passes_one_way_platforms_only_while_holding_down() {
    let mut resource = data();
    resource.stage.floor.left = -40.0;
    resource.stage.floor.right = 40.0;
    resource.stage.spawns = [[0.0, 6.0], [20.0, 6.0]];
    resource.stage.blast = [-100.0, 100.0, -100.0, 100.0];
    resource.stage.geometry = Some(StageGeometry {
        lines: vec![
            stage::Line {
                start: [-40.0, 6.0],
                end: [40.0, 6.0],
                flags: stage::FLOOR | stage::ENABLED,
                material_flags: stage::PLATFORM as u16,
                ..Default::default()
            },
            stage::Line {
                start: [-40.0, 0.0],
                end: [40.0, 0.0],
                flags: stage::FLOOR | stage::ENABLED,
                ..Default::default()
            },
        ],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-40.0, -100.0],
            bounds_max: [40.0, 100.0],
            floor: 0..2,
            ..Default::default()
        }],
    });
    let mut game = Match::new(resource, 42).unwrap();
    jump(&mut game, true);
    idle(&mut game, buttons(0));
    idle(&mut game, buttons(BUTTON_L));
    let mut frames = 0;
    while game.state().fighters[0].action != Action::FallSpecial {
        idle(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 20);
    }
    assert!(game.state().fighters[0].position[1] > 6.0);
    let checkpoint = game.checkpoint();

    let mut frames = 0;
    while game.state().fighters[0].action == Action::FallSpecial {
        idle(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 40);
    }
    let on_platform = game.state().clone();
    assert_eq!(on_platform.fighters[0].action, Action::LandingFallSpecial);
    assert_eq!(on_platform.fighters[0].ground_line, Some(0));
    assert!((on_platform.fighters[0].position[1] - 6.0).abs() < 0.001);

    game.restore_checkpoint(&checkpoint).unwrap();
    let mut frames = 0;
    while game.state().fighters[0].action == Action::FallSpecial {
        let state = idle(&mut game, stick(0, [0.0, -1.0]));
        assert!(state.fighters[0].position[1] > 0.0 || state.fighters[0].grounded);
        frames += 1;
        assert!(frames < 60);
    }
    let through = game.state().clone();
    assert_eq!(through.fighters[0].action, Action::LandingFallSpecial);
    assert_eq!(through.fighters[0].ground_line, Some(1));
    assert!(through.fighters[0].position[1] < 6.0);
}

#[test]
fn air_dodge_precedes_aerial_attacks_and_double_jumps_and_fall_special_offers_neither() {
    let mut resource = escape_air_support::profile(aerial_support::data());
    resource.stage.spawns = [[-10.0, 100.0], [10.0, 100.0]];
    let mut sanity = Match::new(resource.clone(), 0).unwrap();
    assert_eq!(
        idle(&mut sanity, buttons(BUTTON_A)).fighters[0].action,
        Action::AttackAirN
    );
    for combined in [BUTTON_L | BUTTON_A, BUTTON_R | BUTTON_X] {
        let mut game = Match::new(resource.clone(), 0).unwrap();
        assert_eq!(
            idle(&mut game, buttons(combined)).fighters[0].action,
            Action::EscapeAir
        );
    }
    let mut game = Match::new(resource, 0).unwrap();
    idle(&mut game, buttons(BUTTON_L));
    for _ in 0..8 {
        idle(&mut game, buttons(0));
    }
    assert_eq!(game.state().fighters[0].action, Action::FallSpecial);
    assert_eq!(
        idle(&mut game, buttons(BUTTON_A)).fighters[0].action,
        Action::FallSpecial
    );
    assert_eq!(
        idle(&mut game, buttons(BUTTON_X)).fighters[0].action,
        Action::FallSpecial
    );
    assert_eq!(
        idle(&mut game, buttons(BUTTON_L)).fighters[0].action,
        Action::FallSpecial
    );
}

#[test]
fn checkpoints_restore_every_air_dodge_phase() {
    let mut game = Match::new(data(), 42).unwrap();
    jump(&mut game, true);
    idle(&mut game, buttons(0));
    idle(&mut game, stick(BUTTON_L, [1.0, 0.0]));
    let mut phases = Vec::new();
    for _ in 0..40 {
        if [
            Action::EscapeAir,
            Action::FallSpecial,
            Action::LandingFallSpecial,
        ]
        .into_iter()
        .any(|action| action == game.state().fighters[0].action)
            && !phases
                .iter()
                .any(|(action, _)| *action == game.state().fighters[0].action)
        {
            phases.push((game.state().fighters[0].action, game.checkpoint()));
        }
        idle(&mut game, buttons(0));
    }
    assert_eq!(phases.len(), 3);
    for (action, checkpoint) in phases {
        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(game.state().fighters[0].action, action);
        let inputs: Vec<_> = (0..12)
            .map(|frame| {
                [
                    if frame % 4 == 1 {
                        stick(BUTTON_X | BUTTON_A, [0.0, -1.0])
                    } else {
                        buttons(0)
                    },
                    buttons(if frame % 5 == 0 { BUTTON_A } else { 0 }),
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
                "{action:?}"
            );
        }
    }
}

#[test]
fn invalid_air_dodge_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    assert!(Match::new(data(), 0).is_ok());
    rejected(|d| d.rules.escape_air.as_mut().unwrap().deadzone = [1.5, 0.3]);
    rejected(|d| d.rules.escape_air.as_mut().unwrap().deadzone = [0.3, -0.1]);
    for force in [-1.0, f32::NAN, f32::INFINITY] {
        rejected(|d| d.rules.escape_air.as_mut().unwrap().force = force);
    }
    for decay in [-0.5, f32::NAN] {
        rejected(|d| d.rules.escape_air.as_mut().unwrap().decay = decay);
    }
    for lag in [0.0, -10.0, f32::NAN] {
        rejected(|d| d.rules.escape_air.as_mut().unwrap().landing_lag = lag);
    }
    rejected(|d| {
        d.rules
            .escape_air
            .as_mut()
            .unwrap()
            .platform_stick_threshold = 2.0
    });
    rejected(|d| d.fighters[0].escape_air.as_mut().unwrap().frames.clear());
    rejected(|d| {
        d.fighters[1]
            .escape_air
            .as_mut()
            .unwrap()
            .landing_poses
            .pop();
    });
    rejected(|d| {
        d.fighters[1]
            .escape_air
            .as_mut()
            .unwrap()
            .landing_animation_end = -1.0
    });
    rejected(|d| {
        d.fighters[1]
            .escape_air
            .as_mut()
            .unwrap()
            .landing_animation_end = 4096.0
    });
    rejected(|d| {
        d.fighters[0].escape_air.as_mut().unwrap().frames[3]
            .bones
            .pop();
    });
    rejected(|d| d.fighters[0].escape_air = None);
    rejected(|d| d.rules.escape_air = None);
    rejected(|d| d.fighters[1].locomotion = None);
}
