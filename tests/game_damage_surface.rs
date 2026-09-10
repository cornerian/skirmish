//! Match-level wall and ceiling damage reflection through ordinary inputs.
//! Exact mirror arithmetic is covered separately against the original C body.
#[path = "support/aerial.rs"]
mod aerial_resources;
#[path = "support/special.rs"]
mod special_resources;

use skirmish::{
    collision::{ecb, stage},
    game::{
        Action, BUTTON_A, BUTTON_B, BUTTON_L, BUTTON_R, BUTTON_X, Controller, Event, Match, State,
        damage::{
            FloorResponseRules, SurfaceResponseRules, SurfaceTechAttributes, SurfaceTechRules,
        },
        data::{Bone, CollisionBox, MatchData, StageGeometry},
        wall_jump::{Attributes as WallJumpAttributes, Rules as WallJumpRules},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn line(start: [f32; 2], end: [f32; 2], kind: u32) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: stage::ENABLED | kind,
        ..stage::Line::default()
    }
}

fn profile() -> SurfaceResponseRules {
    SurfaceResponseRules {
        knockback_threshold: 0.1,
        velocity_multiplier: 0.8,
        lockout_frames: 2,
        wall_frames: 3,
        ceiling_frames: 4,
    }
}

fn tech_profile() -> SurfaceTechRules {
    SurfaceTechRules {
        wall_freeze_frames: 3,
        wall_frames: 6,
        wall_jump_frames: 7,
        ceiling_frames: 6,
        ceiling_horizontal_frame: 2,
        jump_stick_threshold: 0.8,
    }
}

fn pose_track(bones: &[Bone], frames: u32, translation: [f32; 2]) -> Vec<Vec<Bone>> {
    (0..frames)
        .map(|frame| {
            let mut pose = bones.to_vec();
            pose[1].translation[0] += translation[0] + frame as f32 * 0.25;
            pose[1].translation[1] += translation[1] + frame as f32 * 0.25;
            pose
        })
        .collect()
}

fn tech_attributes(bones: &[Bone], profile: &SurfaceTechRules) -> SurfaceTechAttributes {
    SurfaceTechAttributes {
        passive_wall_velocity: 2.0,
        wall_jump_horizontal_velocity: 3.0,
        wall_jump_vertical_velocity: 4.0,
        passive_ceiling_velocity: 3.0,
        passive_wall_poses: pose_track(bones, profile.wall_frames, [5.0, 0.0]),
        passive_wall_jump_poses: pose_track(bones, profile.wall_jump_frames, [6.0, 0.0]),
        passive_ceiling_poses: pose_track(bones, profile.ceiling_frames, [0.0, 8.0]),
    }
}

fn data(angle: f32) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.damage.floor_response = Some(FloorResponseRules {
        tumble_knockback_threshold: 1.0,
        tech_window: 20.0,
        tech_repeat_lockout: 40,
        tech_roll: None,
        knockdown_options: None,
        recovery_invincibility: None,
        down_damage: None,
        passive_frames: 3,
        down_bound_frames: 4,
        down_wait_frames: 5,
        down_stand_frames: 3,
    });
    data.rules.damage.surface_response = Some(profile());
    data.stage.spawns = [[-2.0, 0.0], [2.0, 2.0]];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            line([-100.0, 0.0], [100.0, 0.0], stage::FLOOR),
            line([100.0, 60.0], [-100.0, 60.0], stage::CEILING),
            line([5.0, -20.0], [5.0, 20.0], stage::LEFT_WALL),
        ],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-110.0, -30.0],
            bounds_max: [110.0, 80.0],
            floor: 0..1,
            ceiling: 1..2,
            left_wall: 2..3,
            ..stage::Joint::default()
        }],
    });
    for fighter in &mut data.fighters {
        fighter.collision_box = CollisionBox::Fixed {
            source: ecb::FixedSource {
                up: 3.0,
                down: 0.0,
                front: 1.5,
                back: 1.5,
                angle: 0.0,
            },
        };
    }
    data.fighters[1].movement.gravity = 0.0;
    for hit in data.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.radius = 8.0;
        hit.damage = 40;
        hit.angle_degrees = angle;
    }
    data
}

fn tech_data(angle: f32) -> MatchData {
    let mut data = data(angle);
    let profile = tech_profile();
    data.rules.damage.surface_tech = Some(profile.clone());
    for fighter in &mut data.fighters {
        fighter.surface_tech = Some(tech_attributes(&fighter.bones, &profile));
    }
    data
}

fn use_bone_ecb(data: &mut MatchData) {
    data.fighters[1].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
}

fn add_ordinary_wall_jump(data: &mut MatchData) {
    data.rules.wall_jump = Some(WallJumpRules {
        tilt_deadzone: 0.3,
        input_window: 5.0,
        stick_threshold: 0.7,
        tilt_window: 3.0,
        startup_frames: 2,
        vertical_velocity_base: 0.5,
    });
    for fighter in &mut data.fighters {
        let mut frames = vec![fighter.bones.clone(); 8];
        for pose in &mut frames {
            pose[1].translation[0] += 20.0;
        }
        fighter.wall_jump = Some(WallJumpAttributes {
            can_walljump: true,
            minimum_approach_speed: 0.2,
            horizontal_velocity: 3.0,
            vertical_velocity: 4.0,
            frames,
        });
    }
}

fn interrupt_data(angle: f32) -> MatchData {
    let mut data = tech_data(angle);
    let mut aerial = aerial_resources::data();
    for player in 0..2 {
        data.fighters[player].locomotion = aerial.fighters[player].locomotion.take();
        data.fighters[player].aerials = aerial.fighters[player].aerials.take();
    }
    special_resources::profile(data)
}

fn released_neutral_wall_tech(data: MatchData) -> Match {
    let mut game = hit(data);
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    until(&mut game, |state| {
        state.fighters[1].action == Action::PassiveWall
    });
    while game.state().fighters[1].surface_tech.timer != 0 {
        step(&mut game);
    }
    assert_eq!(game.state().fighters[1].action, Action::PassiveWall);
    game
}

fn attack() -> [Controller; 2] {
    let mut input = IDLE;
    input[0].buttons = BUTTON_A;
    input
}

fn step(game: &mut Match) -> State {
    game.step(IDLE).unwrap().clone()
}

fn step_with(game: &mut Match, input: [Controller; 2]) -> State {
    game.step(input).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    until_with(game, IDLE, condition)
}

fn until_with(
    game: &mut Match,
    input: [Controller; 2],
    condition: impl Fn(&State) -> bool,
) -> State {
    for _ in 0..240 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step_with(game, input);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn hit(data: MatchData) -> Match {
    let mut game = Match::new(data, 71).unwrap();
    game.step(attack()).unwrap();
    until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(game.state().fighters[1].tumbling);
    game
}

fn buffer_tech(game: &mut Match, buttons: u16, stick: [f32; 2]) {
    while game.state().fighters[1].hitlag > 1.0 {
        step(game);
    }
    let mut input = IDLE;
    input[1].buttons = buttons;
    input[1].stick = stick;
    step_with(game, input);
}

#[test]
fn horizontal_launch_reflects_from_wall_and_replays_from_checkpoint() {
    let mut game = hit(data(0.0));
    let checkpoint = game.checkpoint();
    let mut suffix = Vec::new();
    let reflected = loop {
        let state = step(&mut game);
        suffix.push(state.clone());
        if state.events.iter().any(|event| {
            matches!(
                event,
                Event::SurfaceReflected {
                    player: 1,
                    surface: stage::Surface::LeftWall,
                    line: 2
                }
            )
        }) {
            break state;
        }
        assert!(suffix.len() < 240);
    };
    let fighter = &reflected.fighters[1];
    assert_eq!(fighter.action, Action::FlyReflectWall);
    assert_eq!(fighter.contacts[2], Some(2));
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert!(fighter.knockback[0] < 0.0);
    assert_eq!(fighter.facing, -1.0);
    assert_eq!(fighter.last_damage_surface, Some(stage::Surface::LeftWall));
    assert!(fighter.reflect_lockout <= profile().lockout_frames);

    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in suffix {
        assert_eq!(step(&mut game), expected);
    }

    let mut samples = 1;
    while game.state().fighters[1].action == Action::FlyReflectWall {
        step(&mut game);
        samples += usize::from(game.state().fighters[1].action == Action::FlyReflectWall);
    }
    assert_eq!(samples, profile().wall_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::DamageFall);
}

#[test]
fn vertical_launch_reflects_from_ceiling_and_uses_its_duration() {
    let mut game = hit(data(90.0));
    let reflected = until(&mut game, |state| {
        state.events.iter().any(|event| {
            matches!(
                event,
                Event::SurfaceReflected {
                    player: 1,
                    surface: stage::Surface::Ceiling,
                    line: 1
                }
            )
        })
    });
    let fighter = &reflected.fighters[1];
    assert_eq!(fighter.action, Action::FlyReflectCeiling);
    assert_eq!(fighter.contacts[1], Some(1));
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert!(fighter.knockback[1] < 0.0);
    assert_eq!(fighter.last_damage_surface, Some(stage::Surface::Ceiling));

    let mut samples = 1;
    while game.state().fighters[1].action == Action::FlyReflectCeiling {
        step(&mut game);
        samples += usize::from(game.state().fighters[1].action == Action::FlyReflectCeiling);
    }
    assert_eq!(samples, profile().ceiling_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::DamageFall);
}

#[test]
fn simultaneous_floor_contact_wins_over_wall_reflection() {
    let mut game = hit(data(315.0));
    let landed = until(&mut game, |state| {
        state.events.contains(&Event::Landed { player: 1 })
    });
    let fighter = &landed.fighters[1];
    assert!(fighter.grounded);
    assert_eq!(fighter.ground_line, Some(0));
    assert_eq!(fighter.action, Action::DownBound);
    assert_eq!(fighter.knockback, [0.0; 2]);
    assert_eq!(fighter.last_damage_surface, None);
    assert!(
        !landed
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceReflected { .. }))
    );
}

#[test]
fn absent_or_unmet_profiles_stop_at_the_surface_without_reflecting() {
    for profile in [
        None,
        Some(SurfaceResponseRules {
            knockback_threshold: 1_000_000.0,
            ..profile()
        }),
    ] {
        let mut resource = data(0.0);
        resource.rules.damage.surface_response = profile;
        let mut game = hit(resource);
        let mut touched = false;
        for _ in 0..120 {
            let state = step(&mut game);
            touched |= state.fighters[1].contacts[2] == Some(2);
            assert!(
                !state
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::SurfaceReflected { .. }))
            );
            assert!(!matches!(
                state.fighters[1].action,
                Action::FlyReflectWall | Action::FlyReflectCeiling
            ));
        }
        assert!(touched);
    }
}

#[test]
fn malformed_surface_profiles_are_rejected_and_valid_profiles_roundtrip() {
    let encoded = serde_json::to_string(&data(0.0)).unwrap();
    let decoded: MatchData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.rules.damage.surface_response, Some(profile()));

    for mutate in [
        |rules: &mut SurfaceResponseRules| rules.knockback_threshold = f32::NAN,
        |rules: &mut SurfaceResponseRules| rules.velocity_multiplier = 1.01,
        |rules: &mut SurfaceResponseRules| rules.wall_frames = 0,
        |rules: &mut SurfaceResponseRules| rules.ceiling_frames = 0,
    ] {
        let mut resource = data(0.0);
        mutate(resource.rules.damage.surface_response.as_mut().unwrap());
        assert!(Match::new(resource, 0).is_err());
    }
    let mut missing_tumble_rules = data(0.0);
    missing_tumble_rules.rules.damage.floor_response = None;
    assert!(Match::new(missing_tumble_rules, 0).is_err());
}

#[test]
fn buffered_shoulder_enters_neutral_wall_tech_then_launches_away() {
    let mut game = hit(tech_data(0.0));
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let checkpoint = game.checkpoint();
    let teched = until(&mut game, |state| {
        state.events.contains(&Event::SurfaceTeched {
            player: 1,
            surface: stage::Surface::LeftWall,
            line: 2,
            jump: false,
        })
    });
    let fighter = &teched.fighters[1];
    assert_eq!(fighter.action, Action::PassiveWall);
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert_eq!(fighter.knockback, [0.0; 2]);
    assert_eq!(fighter.facing, -1.0);
    assert_eq!(
        fighter.surface_tech.timer,
        tech_profile().wall_freeze_frames
    );
    assert_eq!(fighter.locomotion.tilt_x_age, 254);
    assert_eq!(fighter.locomotion.tilt_y_age, 254);
    assert!(
        !teched
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceReflected { .. }))
    );

    let mut expected = (0..3).map(|_| step(&mut game)).collect::<Vec<_>>();
    assert_eq!(expected[0].fighters[1].surface_tech.timer, 2);
    assert_eq!(expected[0].fighters[1].position, fighter.position);
    assert_eq!(expected[1].fighters[1].surface_tech.timer, 1);
    assert_eq!(expected[1].fighters[1].position, fighter.position);
    assert_eq!(expected[2].fighters[1].surface_tech.timer, 0);
    assert!(expected[2].fighters[1].velocity[0] < 0.0);
    assert!(expected[2].fighters[1].position[0] < fighter.position[0]);
    while game.state().fighters[1].action == Action::PassiveWall {
        expected.push(step(&mut game));
    }
    assert_eq!(
        1 + expected
            .iter()
            .filter(|state| state.fighters[1].action == Action::PassiveWall)
            .count(),
        tech_profile().wall_frames as usize
    );
    assert_eq!(game.state().fighters[1].action, Action::Fall);

    game.restore_checkpoint(&checkpoint).unwrap();
    until(&mut game, |state| {
        matches!(state.fighters[1].action, Action::PassiveWall)
    });
    for expected in expected {
        assert_eq!(step(&mut game), expected);
    }
}

#[test]
fn buffered_jump_and_upward_stick_select_wall_jump_tech() {
    for (buttons, stick) in [
        (BUTTON_L | BUTTON_X, [0.0; 2]),
        (BUTTON_L, [0.0, tech_profile().jump_stick_threshold]),
    ] {
        let mut game = hit(tech_data(0.0));
        buffer_tech(&mut game, buttons, stick);
        let mut held = IDLE;
        held[1].stick = stick;
        let teched = until_with(&mut game, held, |state| {
            state.events.contains(&Event::SurfaceTeched {
                player: 1,
                surface: stage::Surface::LeftWall,
                line: 2,
                jump: true,
            })
        });
        assert_eq!(teched.fighters[1].action, Action::PassiveWallJump);
        let mut samples = 1;
        while game.state().fighters[1].surface_tech.timer != 0 {
            step(&mut game);
            samples += usize::from(game.state().fighters[1].action == Action::PassiveWallJump);
        }
        let fighter = &game.state().fighters[1];
        assert_eq!(fighter.velocity, [-2.9, 4.0]);
        assert!(fighter.position[0] < teched.fighters[1].position[0]);
        assert!(fighter.position[1] > teched.fighters[1].position[1]);
        while game.state().fighters[1].action == Action::PassiveWallJump {
            step(&mut game);
            samples += usize::from(game.state().fighters[1].action == Action::PassiveWallJump);
        }
        assert_eq!(samples, tech_profile().wall_jump_frames as usize);
        assert_eq!(game.state().fighters[1].action, Action::Fall);
    }
}

#[test]
fn neutral_wall_tech_queues_a_jump_during_freeze_and_replays_from_checkpoint() {
    let mut resource = tech_data(0.0);
    use_bone_ecb(&mut resource);
    let mut game = hit(resource);
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let teched = until(&mut game, |state| {
        state.events.contains(&Event::SurfaceTeched {
            player: 1,
            surface: stage::Surface::LeftWall,
            line: 2,
            jump: false,
        })
    });
    assert_eq!(teched.fighters[1].action, Action::PassiveWall);

    let mut jump = IDLE;
    jump[1].buttons = BUTTON_X;
    let queued = step_with(&mut game, jump);
    assert_eq!(queued.fighters[1].action, Action::PassiveWall);
    assert_eq!(queued.fighters[1].surface_tech.timer, 2);
    assert!(queued.fighters[1].surface_tech.jump_queued);
    let checkpoint = game.checkpoint();

    let mut suffix = Vec::new();
    while game.state().fighters[1].action == Action::PassiveWall {
        suffix.push(step(&mut game));
    }
    let converted = &game.state().fighters[1];
    assert_eq!(converted.action, Action::PassiveWallJump);
    assert_eq!(converted.action_frame, 4);
    assert_eq!(converted.velocity, [-2.9, 4.0]);
    assert!(!converted.surface_tech.jump_queued);
    assert!((converted.ecb.desired.left[0] + 6.75).abs() < 0.0001);
    while game.state().fighters[1].action == Action::PassiveWallJump {
        suffix.push(step(&mut game));
    }
    assert_eq!(game.state().fighters[1].action, Action::Fall);

    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in suffix {
        assert_eq!(step(&mut game), expected);
    }
}

#[test]
fn jump_input_on_the_freeze_release_frame_does_not_convert_the_wall_tech() {
    let mut game = hit(tech_data(0.0));
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    until(&mut game, |state| {
        state.fighters[1].action == Action::PassiveWall
    });
    while game.state().fighters[1].surface_tech.timer > 1 {
        step(&mut game);
    }
    let mut jump = IDLE;
    jump[1].buttons = BUTTON_X;
    let released = step_with(&mut game, jump);
    assert_eq!(released.fighters[1].action, Action::PassiveWall);
    assert_eq!(released.fighters[1].surface_tech.timer, 0);
    assert!(!released.fighters[1].surface_tech.jump_queued);
    assert_eq!(released.fighters[1].velocity, [-1.9, 0.0]);
}

#[test]
fn released_wall_tech_dispatches_supported_air_actions_in_source_priority() {
    for (buttons, stick, expected) in [
        (BUTTON_B, [0.0; 2], Action::SpecialAirN),
        (BUTTON_A, [0.0, 1.0], Action::AttackAirHi),
        (BUTTON_X, [0.0; 2], Action::JumpAerial),
        (BUTTON_A | BUTTON_X, [0.0, 1.0], Action::AttackAirHi),
        (BUTTON_A | BUTTON_B, [0.0; 2], Action::SpecialAirN),
    ] {
        let mut game = released_neutral_wall_tech(interrupt_data(0.0));
        let mut input = IDLE;
        input[1].buttons = buttons;
        input[1].stick = stick;
        assert_eq!(step_with(&mut game, input).fighters[1].action, expected);
    }
}

#[test]
fn frozen_wall_tech_blocks_air_actions_and_requires_a_fresh_edge_after_release() {
    let mut game = hit(interrupt_data(0.0));
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    until(&mut game, |state| {
        state.fighters[1].action == Action::PassiveWall
    });
    let mut held = IDLE;
    held[1].buttons = BUTTON_B;
    while game.state().fighters[1].surface_tech.timer != 0 {
        assert_eq!(
            step_with(&mut game, held).fighters[1].action,
            Action::PassiveWall
        );
    }
    assert_eq!(
        step_with(&mut game, held).fighters[1].action,
        Action::PassiveWall
    );
    step(&mut game);
    assert_eq!(
        step_with(&mut game, held).fighters[1].action,
        Action::SpecialAirN
    );
}

#[test]
fn ceiling_tech_does_not_dispatch_wall_tech_air_interrupts() {
    for buttons in [BUTTON_A, BUTTON_B, BUTTON_X, BUTTON_A | BUTTON_B | BUTTON_X] {
        let mut game = hit(interrupt_data(90.0));
        buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
        until(&mut game, |state| {
            state.fighters[1].action == Action::PassiveCeiling
        });
        let mut input = IDLE;
        input[1].buttons = buttons;
        assert_eq!(
            step_with(&mut game, input).fighters[1].action,
            Action::PassiveCeiling
        );
    }
}

#[test]
fn ceiling_tech_applies_scripted_horizontal_input_and_recovers() {
    let mut game = hit(tech_data(90.0));
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let teched = until(&mut game, |state| {
        state.events.contains(&Event::SurfaceTeched {
            player: 1,
            surface: stage::Surface::Ceiling,
            line: 1,
            jump: false,
        })
    });
    assert_eq!(teched.fighters[1].action, Action::PassiveCeiling);
    assert_eq!(teched.fighters[1].velocity, [0.0; 2]);
    let mut horizontal = IDLE;
    horizontal[1].stick[0] = 0.5;
    let before_command = step_with(&mut game, horizontal);
    assert_eq!(before_command.fighters[1].velocity[0], 0.0);
    let commanded = step_with(&mut game, horizontal);
    assert_eq!(commanded.fighters[1].velocity[0], 1.4);
    assert!(commanded.fighters[1].surface_tech.ceiling_velocity_applied);

    let mut samples = 3;
    while game.state().fighters[1].action == Action::PassiveCeiling {
        step(&mut game);
        samples += usize::from(game.state().fighters[1].action == Action::PassiveCeiling);
    }
    assert_eq!(samples, tech_profile().ceiling_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::Fall);
}

#[test]
fn each_surface_tech_pose_track_drives_the_headless_bone_ecb() {
    for (angle, buttons, action, expected) in [
        (0.0, BUTTON_L, Action::PassiveWall, [-5.25, 0.0]),
        (
            0.0,
            BUTTON_L | BUTTON_X,
            Action::PassiveWallJump,
            [-6.25, 0.0],
        ),
        (90.0, BUTTON_L, Action::PassiveCeiling, [0.0, 9.25]),
    ] {
        let mut resource = tech_data(angle);
        use_bone_ecb(&mut resource);
        let mut game = hit(resource);
        buffer_tech(&mut game, buttons, [0.0; 2]);
        let teched = until(&mut game, |state| state.fighters[1].action == action);
        assert_eq!(teched.fighters[1].action_frame, 1);
        let checkpoint = game.checkpoint();
        let sampled = step(&mut game);
        let ecb = sampled.fighters[1].ecb.desired;
        if expected[0] != 0.0 {
            assert!((ecb.left[0] - expected[0]).abs() < 0.0001);
        } else {
            assert!((ecb.top[1] - expected[1]).abs() < 0.0001);
        }
        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(step(&mut game), sampled);
    }
}

#[test]
fn damage_wall_jump_uses_its_pose_when_ordinary_wall_jump_is_also_configured() {
    let mut resource = tech_data(0.0);
    use_bone_ecb(&mut resource);
    add_ordinary_wall_jump(&mut resource);
    let mut game = hit(resource);
    buffer_tech(&mut game, BUTTON_L | BUTTON_X, [0.0; 2]);
    until(&mut game, |state| {
        state.fighters[1].action == Action::PassiveWallJump
    });
    assert!(!game.state().fighters[1].wall_jump.active);
    let sampled = step(&mut game);
    assert!((sampled.fighters[1].ecb.desired.left[0] + 6.25).abs() < 0.0001);
}

#[test]
fn floor_tech_wins_over_armed_wall_tech_in_a_diagonal_collision() {
    let mut game = hit(tech_data(315.0));
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let landed = until(&mut game, |state| {
        state.events.contains(&Event::Landed { player: 1 })
    });
    assert_eq!(landed.fighters[1].action, Action::Passive);
    assert!(
        !landed
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceTeched { .. }))
    );
}

#[test]
fn wall_tech_has_priority_over_ceiling_tech_at_a_corner() {
    let mut resource = tech_data(45.0);
    let ceiling = &mut resource.stage.geometry.as_mut().unwrap().lines[1];
    ceiling.start[1] = 6.0;
    ceiling.end[1] = 6.0;
    let mut game = hit(resource);
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let state = until(&mut game, |state| {
        state
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceTeched { .. }))
    });
    assert_eq!(state.fighters[1].action, Action::PassiveWall);
    assert!(state.events.contains(&Event::SurfaceTeched {
        player: 1,
        surface: stage::Surface::LeftWall,
        line: 2,
        jump: false,
    }));
}

#[test]
fn leftward_launch_techs_the_opposite_wall_and_faces_away() {
    let mut resource = tech_data(180.0);
    let geometry = resource.stage.geometry.as_mut().unwrap();
    geometry
        .lines
        .push(line([-5.0, 20.0], [-5.0, -20.0], stage::RIGHT_WALL));
    geometry.joints[0].right_wall = 3..4;
    let mut game = hit(resource);
    buffer_tech(&mut game, BUTTON_L, [0.0; 2]);
    let state = until(&mut game, |state| {
        state.events.contains(&Event::SurfaceTeched {
            player: 1,
            surface: stage::Surface::RightWall,
            line: 3,
            jump: false,
        })
    });
    assert_eq!(state.fighters[1].action, Action::PassiveWall);
    assert_eq!(state.fighters[1].facing, 1.0);
    assert_eq!(state.fighters[1].contacts[3], Some(3));
}

#[test]
fn a_second_recent_shoulder_press_fails_surface_tech_lockout() {
    let mut game = hit(tech_data(0.0));
    while game.state().fighters[1].hitlag > 3.0 {
        step(&mut game);
    }
    let mut shoulder = IDLE;
    shoulder[1].buttons = BUTTON_L;
    step_with(&mut game, shoulder);
    step(&mut game);
    shoulder[1].buttons = BUTTON_R;
    step_with(&mut game, shoulder);
    let state = until(&mut game, |state| {
        state
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceReflected { .. }))
    });
    assert!(state.fighters[1].locomotion.previous_tech_press_age < 40);
    assert!(
        !state
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceTeched { .. }))
    );
}

#[test]
fn malformed_surface_tech_resources_are_rejected() {
    let encoded = serde_json::to_string(&tech_data(0.0)).unwrap();
    let decoded: MatchData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        decoded.fighters[0].surface_tech,
        tech_data(0.0).fighters[0].surface_tech
    );

    let mut cases = Vec::new();
    let mut bad = tech_data(0.0);
    bad.rules
        .damage
        .surface_tech
        .as_mut()
        .unwrap()
        .wall_freeze_frames = 0;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.rules
        .damage
        .surface_tech
        .as_mut()
        .unwrap()
        .ceiling_horizontal_frame = 6;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.fighters[0].surface_tech = None;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.fighters[0]
        .surface_tech
        .as_mut()
        .unwrap()
        .wall_jump_vertical_velocity = f32::NAN;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.fighters[0]
        .surface_tech
        .as_mut()
        .unwrap()
        .passive_wall_poses
        .pop();
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.fighters[0]
        .surface_tech
        .as_mut()
        .unwrap()
        .passive_wall_jump_poses[0][1]
        .parent = None;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.fighters[0]
        .surface_tech
        .as_mut()
        .unwrap()
        .passive_ceiling_poses[0][1]
        .translation[1] = f32::NAN;
    cases.push(bad);
    let mut bad = tech_data(0.0);
    bad.rules.damage.floor_response = None;
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
