//! Native damage-floor scheduling with explicit synthetic state durations.
use skirmish::{
    collision::ecb,
    fighter::damage::HurtHeight,
    game::{
        Action, BUTTON_A, BUTTON_B, BUTTON_L, BUTTON_R, Controller, Event, Match, State,
        damage::{
            DamageMotionRules, DamagePoseAttributes, DownDamageRules, FloorTechAttributes,
            FloorTechFrame, FloorTechMotion, FloorTechRules, GroundLaunchRules,
            KnockdownAttributes, KnockdownRules, ProneOrientation, ProneOrientationRules,
            ProneRecoveryAttributes, RecoveryInvincibilityRules,
        },
        data::{Attack, AttackFrame, Bone, CollisionBox},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn profile() -> skirmish::game::damage::FloorResponseRules {
    skirmish::game::damage::FloorResponseRules {
        tumble_knockback_threshold: 20.0,
        tech_window: 20.0,
        tech_repeat_lockout: 40,
        tech_roll: None,
        knockdown_options: None,
        recovery_invincibility: None,
        down_damage: None,
        passive_frames: 3,
        down_bound_frames: 4,
        down_bound_frames_face_up: None,
        down_bound_frames_face_down: None,
        down_wait_frames: 5,
        down_wait_frames_face_up: None,
        down_wait_frames_face_down: None,
        down_stand_frames: 3,
        down_stand_frames_face_up: None,
        down_stand_frames_face_down: None,
    }
}

fn data() -> skirmish::game::data::MatchData {
    let mut data: skirmish::game::data::MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.damage.floor_response = Some(profile());
    data.stage.spawns = [[-2.0, 0.0], [2.0, 2.0]];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    for frame in &mut data.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.damage = 40;
            hit.angle_degrees = 270.0;
        }
    }
    data
}

fn roll_motion(bones: &[Bone], roots: &[f32], extension: f32) -> FloorTechMotion {
    FloorTechMotion {
        frames: roots
            .iter()
            .enumerate()
            .map(|(index, &root_translation)| {
                let mut bones = bones.to_vec();
                if index != 0 {
                    bones[1].translation[0] = extension;
                }
                FloorTechFrame {
                    bones,
                    root_translation,
                }
            })
            .collect(),
    }
}

fn roll_data() -> skirmish::game::data::MatchData {
    let mut resource = data();
    let profile = resource.rules.damage.floor_response.as_mut().unwrap();
    profile.tech_roll = Some(FloorTechRules {
        stick_threshold: 0.7,
    });
    for fighter in &mut resource.fighters {
        fighter.floor_tech = Some(FloorTechAttributes {
            forward: roll_motion(&fighter.bones, &[0.0, 0.75, 1.0, 0.5], 6.0),
            backward: roll_motion(&fighter.bones, &[0.0, -0.5, -0.75, -0.25, -0.1], -6.0),
        });
    }
    resource.fighters[1].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    resource
}

fn knockdown_data() -> skirmish::game::data::MatchData {
    let mut resource = data();
    resource
        .rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .knockdown_options = Some(KnockdownRules {
        horizontal_stick_threshold: 0.7,
        stand_stick_threshold: 0.7,
        vertical_angle_radians: 0.8,
        attack_cstick_threshold: 0.8,
        bound_attack_window: 4.0,
    });
    let stand_frames = profile().down_stand_frames as usize;
    for fighter in &mut resource.fighters {
        let mut hit = fighter
            .jab
            .frames
            .iter()
            .flat_map(|frame| &frame.hitboxes)
            .next()
            .unwrap()
            .clone();
        hit.group = 9;
        hit.bone = 0;
        hit.center = [0.0; 3];
        hit.radius = 30.0;
        hit.damage = 5;
        let attack = Attack {
            move_id: fighter.jab.move_id,
            frames: (0..4)
                .map(|frame| AttackFrame {
                    bones: fighter.bones.clone(),
                    hitboxes: (frame == 0).then_some(hit.clone()).into_iter().collect(),
                    hurtbox_states: vec![],
                })
                .collect(),
        };
        let mut passive_poses = vec![fighter.bones.clone(); profile().passive_frames as usize];
        passive_poses[1][1].translation[0] = 8.0;
        let mut bound_poses = vec![fighter.bones.clone(); profile().down_bound_frames as usize];
        bound_poses[1][1].translation[0] = 9.0;
        let mut wait_poses = vec![fighter.bones.clone(); profile().down_wait_frames as usize];
        wait_poses[0][1].translation[0] = 10.0;
        let mut stand_poses = vec![fighter.bones.clone(); stand_frames];
        stand_poses[0][1].translation[0] = 7.0;
        let face_up = ProneRecoveryAttributes {
            bound_poses,
            wait_poses,
            damage_poses: None,
            forward: roll_motion(&fighter.bones, &[0.0, 0.6, 0.9, 0.3], 6.0),
            backward: roll_motion(&fighter.bones, &[0.0, -0.4, -0.7, -0.2, -0.1], -6.0),
            stand_poses,
            attack,
        };
        let mut face_down = face_up.clone();
        face_down.bound_poses[1][1].translation[0] = 12.0;
        face_down.wait_poses[0][1].translation[0] = 13.0;
        face_down.stand_poses[0][1].translation[0] = 14.0;
        face_down.forward = roll_motion(&fighter.bones, &[0.0, 1.2, 1.8, 0.6], 15.0);
        face_down.backward = roll_motion(&fighter.bones, &[0.0, -0.8, -1.4, -0.4, -0.2], -15.0);
        for hit in face_down
            .attack
            .frames
            .iter_mut()
            .flat_map(|frame| &mut frame.hitboxes)
        {
            hit.damage = 6;
        }
        fighter.knockdown = Some(KnockdownAttributes {
            passive_poses,
            orientation: ProneOrientationRules {
                hip_bone: 1,
                use_z_axis: false,
                invert: false,
            },
            face_up,
            face_down,
        });
    }
    resource.fighters[1].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    resource
}

fn recovery_data() -> skirmish::game::data::MatchData {
    let mut resource = knockdown_data();
    let floor = resource.rules.damage.floor_response.as_mut().unwrap();
    floor.tech_roll = Some(FloorTechRules {
        stick_threshold: 0.7,
    });
    floor.recovery_invincibility = Some(RecoveryInvincibilityRules {
        passive_frames: 2,
        tech_roll_frames: 2,
        missed_roll_frames: 2,
        stand_frames: 2,
        attack_frames: 2,
    });
    for fighter in &mut resource.fighters {
        fighter.floor_tech = Some(FloorTechAttributes {
            forward: roll_motion(&fighter.bones, &[0.0, 0.75, 1.0, 0.5], 6.0),
            backward: roll_motion(&fighter.bones, &[0.0, -0.5, -0.75, -0.25, -0.1], -6.0),
        });
    }
    resource
}

fn face_down_data() -> skirmish::game::data::MatchData {
    let mut resource = knockdown_data();
    resource.fighters[1]
        .knockdown
        .as_mut()
        .unwrap()
        .orientation
        .invert = true;
    resource
}

/// Distinct, deliberately-mismatched-from-the-shared-fallback per-orientation
/// DownBound/DownWait/DownStand durations, with every pose vector resized to
/// match. Face-up and face-down get different lengths from each other too,
/// mirroring Fox's real DownWaitU/D (70 vs 90 frames, `ftmotionstates.c`)
/// asymmetry that motivated the override fields.
fn oriented_frame_data(face_down: bool) -> skirmish::game::data::MatchData {
    let mut resource = if face_down {
        face_down_data()
    } else {
        knockdown_data()
    };
    let floor = resource.rules.damage.floor_response.as_mut().unwrap();
    floor.down_bound_frames_face_up = Some(2);
    floor.down_bound_frames_face_down = Some(6);
    floor.down_wait_frames_face_up = Some(3);
    floor.down_wait_frames_face_down = Some(7);
    floor.down_stand_frames_face_up = Some(2);
    floor.down_stand_frames_face_down = Some(5);
    for fighter in &mut resource.fighters {
        let bones = fighter.bones.clone();
        let knockdown = fighter.knockdown.as_mut().unwrap();
        knockdown.face_up.bound_poses.resize(2, bones.clone());
        knockdown.face_up.wait_poses.resize(3, bones.clone());
        knockdown.face_up.stand_poses.resize(2, bones.clone());
        knockdown.face_down.bound_poses.resize(6, bones.clone());
        knockdown.face_down.wait_poses.resize(7, bones.clone());
        knockdown.face_down.stand_poses.resize(5, bones);
    }
    resource
}

fn down_damage_data(face_down: bool) -> skirmish::game::data::MatchData {
    let mut resource = if face_down {
        face_down_data()
    } else {
        knockdown_data()
    };
    let floor = resource.rules.damage.floor_response.as_mut().unwrap();
    floor.down_damage = Some(DownDamageRules {
        pending_damage_threshold: 41,
        frames: 3,
    });
    floor.down_bound_frames = 120;
    floor.down_wait_frames = 30;
    for fighter in &mut resource.fighters {
        let bones = fighter.bones.clone();
        let knockdown = fighter.knockdown.as_mut().unwrap();
        for (variant, extension) in [
            (&mut knockdown.face_up, 16.0),
            (&mut knockdown.face_down, 17.0),
        ] {
            variant.bound_poses.resize(120, bones.clone());
            variant.wait_poses.resize(30, bones.clone());
            let mut poses = vec![bones.clone(); 3];
            for pose in &mut poses {
                pose[1].translation[0] = extension;
            }
            variant.damage_poses = Some(poses);
        }
    }
    for hit in resource.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.radius = 30.0;
    }
    resource
}

fn grounded_launch_down_damage_data() -> skirmish::game::data::MatchData {
    let mut resource = down_damage_data(false);
    resource.rules.damage.damage_motion = Some(DamageMotionRules {
        thresholds: [100.0, 200.0, 300.0],
    });
    resource.rules.damage.ground_launch = Some(GroundLaunchRules {
        fly_bounce_angle_radians: 0.2,
        fly_bounce_vertical_multiplier: 0.5,
        ground_knockback_friction_multiplier: 2.0,
    });
    for fighter in &mut resource.fighters {
        let motion = vec![fighter.bones.clone()];
        fighter.damage_poses = Some(DamagePoseAttributes {
            hurtbox_heights: vec![HurtHeight::Middle; fighter.hurtboxes.len()],
            ground: core::array::from_fn(|_| core::array::from_fn(|_| motion.clone())),
            air: core::array::from_fn(|_| motion.clone()),
            fly: core::array::from_fn(|_| motion.clone()),
        });
    }
    resource
}

fn recovery_timer_data() -> skirmish::game::data::MatchData {
    let mut resource = recovery_data();
    for fighter in &mut resource.fighters {
        let knockdown = fighter.knockdown.as_mut().unwrap();
        for variant in [&mut knockdown.face_up, &mut knockdown.face_down] {
            for frame in &mut variant.attack.frames {
                frame.hitboxes.clear();
            }
        }
    }
    resource
}

fn input(player: usize, buttons: u16) -> [Controller; 2] {
    let mut input = IDLE;
    input[player].buttons = buttons;
    input
}

fn directional_input(player: usize, buttons: u16, stick_x: f32) -> [Controller; 2] {
    let mut input = input(player, buttons);
    input[player].stick[0] = stick_x;
    input
}

fn recovery_input(buttons: u16, stick: [f32; 2], cstick: [f32; 2]) -> [Controller; 2] {
    let mut input = IDLE;
    input[1] = Controller {
        buttons,
        stick,
        cstick,
        trigger: 0.0,
    };
    input
}

fn step(game: &mut Match, input: [Controller; 2]) -> State {
    game.step(input).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..240 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game, IDLE);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn downward_hit(data: skirmish::game::data::MatchData) -> Match {
    let mut game = Match::new(data, 7).unwrap();
    step(&mut game, input(0, BUTTON_A));
    let hit = until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(hit.fighters[1].tumbling);
    assert!(!hit.fighters[1].grounded);
    game
}

fn down_wait(data: skirmish::game::data::MatchData) -> Match {
    let mut game = downward_hit(data);
    until(&mut game, |state| {
        state.fighters[1].action == Action::DownWait
    });
    game
}

fn down_bound(data: skirmish::game::data::MatchData) -> Match {
    let mut game = downward_hit(data);
    until(&mut game, |state| {
        state.fighters[1].action == Action::DownBound
    });
    game
}

fn hit_prone(game: &mut Match) -> State {
    let before = game.state().fighters[1].percent;
    step(game, input(0, BUTTON_A));
    until(game, |state| state.fighters[1].percent > before)
}

fn advance_to_bound_expiry(game: &mut Match) {
    let duration = profile().down_bound_frames;
    while game.state().fighters[1].action_frame < duration {
        step(game, IDLE);
    }
    assert_eq!(game.state().fighters[1].action, Action::DownBound);
}

fn arm_tech(game: &mut Match) {
    while game.state().fighters[1].hitlag > 1.0 {
        step(game, IDLE);
    }
    step(game, input(1, BUTTON_L));
}

fn arm_directional_tech(game: &mut Match, stick_x: f32) -> State {
    while game.state().fighters[1].hitlag > 1.0 {
        step(game, IDLE);
    }
    step(game, directional_input(1, BUTTON_L, stick_x));
    for _ in 0..240 {
        if game.state().fighters[1].grounded {
            return game.state().clone();
        }
        step(game, directional_input(1, 0, stick_x));
    }
    panic!("directional floor tech was not reached: {:?}", game.state());
}

fn lingering_tumble_data() -> skirmish::game::data::MatchData {
    let mut resource = data();
    resource.stage.spawns[1][1] = 6.0;
    resource.rules.knockback_speed = 0.01;
    resource.rules.knockback_decay = 0.01;
    resource.rules.hitstun_scale = 0.01;
    for hit in resource.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.radius = 8.0;
    }
    resource
}

#[test]
fn buffered_neutral_tech_stops_launch_and_recovers_for_input() {
    let mut game = downward_hit(data());
    arm_tech(&mut game);
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    let fighter = &landed.fighters[1];
    assert_eq!(fighter.action, Action::Passive);
    assert_eq!(fighter.action_frame, 1);
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert_eq!(fighter.knockback, [0.0; 2]);
    assert!(landed.events.contains(&Event::Landed { player: 1 }));

    let mut passive_samples = 1;
    while game.state().fighters[1].action == Action::Passive {
        step(&mut game, IDLE);
        passive_samples += usize::from(game.state().fighters[1].action == Action::Passive);
    }
    assert_eq!(passive_samples, profile().passive_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::Wait);
}

#[test]
fn directional_floor_tech_rolls_use_sampled_root_motion_and_bone_ecbs() {
    for (stick_x, action, roots, ecb_side) in [
        (-0.7, Action::PassiveStandF, vec![0.0, 0.75, 1.0, 0.5], -6.0),
        (
            0.7,
            Action::PassiveStandB,
            vec![0.0, -0.5, -0.75, -0.25, -0.1],
            6.0,
        ),
    ] {
        let mut game = downward_hit(roll_data());
        let landed = arm_directional_tech(&mut game, stick_x);
        let fighter = &landed.fighters[1];
        assert_eq!(fighter.action, action);
        assert_eq!(fighter.action_frame, 1);
        assert_eq!(fighter.facing, -1.0);
        assert_eq!(fighter.velocity, [0.0; 2]);
        assert!(landed.events.contains(&Event::Landed { player: 1 }));
        let checkpoint = game.checkpoint();
        let mut expected = Vec::new();
        let mut previous_x = fighter.position[0];
        let mut previous_ground_velocity = fighter.ground_velocity;
        for &local_delta in &roots[1..] {
            let controls = directional_input(1, 0, stick_x);
            let state = step(&mut game, controls);
            let fighter = &state.fighters[1];
            assert_eq!(fighter.action, action);
            let target = local_delta * fighter.facing;
            let velocity = previous_ground_velocity + (target - previous_ground_velocity);
            assert_eq!(
                fighter.position[0].to_bits(),
                (previous_x + velocity).to_bits()
            );
            assert_eq!(fighter.ground_velocity.to_bits(), velocity.to_bits());
            assert_eq!(fighter.velocity[0].to_bits(), velocity.to_bits());
            if ecb_side > 0.0 {
                assert!((fighter.ecb.current.right[0] - ecb_side).abs() < 1e-5);
            } else {
                assert!((fighter.ecb.current.left[0] - ecb_side).abs() < 1e-5);
            }
            previous_x = fighter.position[0];
            previous_ground_velocity = fighter.ground_velocity;
            expected.push((controls, state));
        }
        assert_eq!(1 + expected.len(), roots.len());
        expected.push((IDLE, step(&mut game, IDLE)));
        assert_eq!(game.state().fighters[1].action, Action::Wait);

        game.restore_checkpoint(&checkpoint).unwrap();
        for (controls, state) in expected {
            assert_eq!(step(&mut game, controls), state);
        }
    }
}

#[test]
fn below_threshold_floor_tech_remains_neutral() {
    let mut game = downward_hit(roll_data());
    let landed = arm_directional_tech(&mut game, 0.699);
    assert_eq!(landed.fighters[1].action, Action::Passive);
}

#[test]
fn unteched_tumble_runs_bound_wait_stand_and_complete_recovery() {
    let mut game = downward_hit(data());
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    let mut transitions = vec![Action::DownBound];
    let mut counts = vec![1_usize];
    while game.state().fighters[1].action != Action::Wait {
        let action = step(&mut game, IDLE).fighters[1].action;
        if transitions.last() == Some(&action) {
            *counts.last_mut().unwrap() += 1;
        } else {
            transitions.push(action);
            counts.push(1);
        }
    }
    assert_eq!(
        transitions,
        [
            Action::DownBound,
            Action::DownWait,
            Action::DownStand,
            Action::Wait,
        ]
    );
    assert_eq!(
        &counts[..3],
        &[
            profile().down_bound_frames as usize,
            profile().down_wait_frames as usize,
            profile().down_stand_frames as usize,
        ]
    );
}

/// The source drives DownBound/DownWait/DownStand's exit from
/// `ftAnim_IsFramesRemaining` against whichever of the U/D motion pair
/// (`ftCo_DownBound.c`, `ftCo_Down.c`, `ftCo_DownStand.c`) the hip-orientation
/// test selected, so face-up and face-down can run different lengths (Fox's
/// DownWaitU/D are 70/90 frames). `down_*_frames_face_up`/`_face_down`
/// let a pack express that split; this exercises both orientations landing
/// with the shared field untouched and only the per-orientation overrides
/// governing the transition timing.
#[test]
fn per_orientation_overrides_use_the_matching_orientation_duration() {
    let mut face_up = downward_hit(oriented_frame_data(false));
    let landed = until(&mut face_up, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    assert_eq!(landed.fighters[1].prone, Some(ProneOrientation::FaceUp));
    let mut transitions = vec![Action::DownBound];
    let mut counts = vec![1_usize];
    while face_up.state().fighters[1].action != Action::Wait {
        let action = step(&mut face_up, IDLE).fighters[1].action;
        if transitions.last() == Some(&action) {
            *counts.last_mut().unwrap() += 1;
        } else {
            transitions.push(action);
            counts.push(1);
        }
    }
    assert_eq!(
        transitions,
        [
            Action::DownBound,
            Action::DownWait,
            Action::DownStand,
            Action::Wait,
        ]
    );
    assert_eq!(&counts[..3], &[2, 3, 2]);

    let mut face_down = downward_hit(oriented_frame_data(true));
    let landed = until(&mut face_down, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    assert_eq!(landed.fighters[1].prone, Some(ProneOrientation::FaceDown));
    let mut transitions = vec![Action::DownBound];
    let mut counts = vec![1_usize];
    while face_down.state().fighters[1].action != Action::Wait {
        let action = step(&mut face_down, IDLE).fighters[1].action;
        if transitions.last() == Some(&action) {
            *counts.last_mut().unwrap() += 1;
        } else {
            transitions.push(action);
            counts.push(1);
        }
    }
    assert_eq!(
        transitions,
        [
            Action::DownBound,
            Action::DownWait,
            Action::DownStand,
            Action::Wait,
        ]
    );
    assert_eq!(&counts[..3], &[6, 7, 5]);
}

#[test]
fn neutral_tech_bound_and_wait_sample_their_supplied_physics_poses() {
    let mut passive = downward_hit(knockdown_data());
    arm_tech(&mut passive);
    until(&mut passive, |state| state.fighters[1].grounded);
    let state = step(&mut passive, IDLE);
    assert_eq!(state.fighters[1].action, Action::Passive);
    assert!((state.fighters[1].ecb.current.left[0] + 8.0).abs() < 1e-5);

    let mut missed = down_bound(knockdown_data());
    let state = step(&mut missed, IDLE);
    assert_eq!(state.fighters[1].action, Action::DownBound);
    assert!((state.fighters[1].ecb.current.left[0] + 9.0).abs() < 1e-5);
    advance_to_bound_expiry(&mut missed);
    let state = step(&mut missed, IDLE);
    assert_eq!(state.fighters[1].action, Action::DownWait);
    assert!((state.fighters[1].ecb.current.left[0] + 10.0).abs() < 1e-5);
}

#[test]
fn evaluated_hip_orientation_selects_and_preserves_each_prone_physics_family() {
    let mut face_up = down_bound(knockdown_data());
    assert_eq!(
        face_up.state().fighters[1].prone,
        Some(ProneOrientation::FaceUp)
    );
    let state = step(&mut face_up, IDLE);
    assert!((state.fighters[1].ecb.current.left[0] + 9.0).abs() < 1e-5);

    let mut face_down = down_bound(face_down_data());
    assert_eq!(
        face_down.state().fighters[1].prone,
        Some(ProneOrientation::FaceDown)
    );
    let checkpoint = face_down.checkpoint();
    let state = step(&mut face_down, IDLE);
    assert!((state.fighters[1].ecb.current.left[0] + 12.0).abs() < 1e-5);
    advance_to_bound_expiry(&mut face_down);
    let expected = step(&mut face_down, IDLE);
    assert_eq!(expected.fighters[1].action, Action::DownWait);
    assert_eq!(expected.fighters[1].prone, Some(ProneOrientation::FaceDown));
    assert!((expected.fighters[1].ecb.current.left[0] + 13.0).abs() < 1e-5);

    face_down.restore_checkpoint(&checkpoint).unwrap();
    step(&mut face_down, IDLE);
    advance_to_bound_expiry(&mut face_down);
    assert_eq!(step(&mut face_down, IDLE), expected);

    for (stick_x, action, delta, left) in [
        (-0.7, Action::DownForward, -1.2, true),
        (0.7, Action::DownBack, 0.8, false),
    ] {
        let mut roll = down_wait(face_down_data());
        let entered = step(&mut roll, recovery_input(0, [stick_x, 0.0], [0.0; 2]));
        assert_eq!(entered.fighters[1].action, action);
        let before_x = entered.fighters[1].position[0];
        let moved = step(&mut roll, IDLE);
        assert_eq!(moved.fighters[1].prone, Some(ProneOrientation::FaceDown));
        assert!((moved.fighters[1].position[0] - (before_x + delta)).abs() < 1e-5);
        let side = if left {
            moved.fighters[1].ecb.current.left[0]
        } else {
            moved.fighters[1].ecb.current.right[0]
        };
        assert!((side - if left { -15.0 } else { 15.0 }).abs() < 1e-5);
    }

    let mut stand = down_wait(face_down_data());
    let state = step(&mut stand, recovery_input(0, [0.0, 0.7], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::DownStand);
    assert!((state.fighters[1].ecb.current.left[0] + 14.0).abs() < 1e-5);
    until(&mut stand, |state| state.fighters[1].action == Action::Wait);
    assert_eq!(stand.state().fighters[1].prone, None);

    let mut attack = down_wait(face_down_data());
    let state = step(&mut attack, recovery_input(BUTTON_A, [0.0; 2], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::DownAttack);
    assert!(state.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 1,
            victim: 0,
            damage,
            ..
        } if *damage == 6.0
    )));
}

#[test]
fn prone_low_damage_uses_oriented_poses_and_preserves_checkpointed_recovery() {
    for (face_down, orientation, ecb_left) in [
        (false, ProneOrientation::FaceUp, -16.0),
        (true, ProneOrientation::FaceDown, -17.0),
    ] {
        let mut game = down_wait(down_damage_data(face_down));
        assert_eq!(game.state().fighters[0].action, Action::Wait);
        let entered = hit_prone(&mut game);
        assert_eq!(entered.fighters[1].action, Action::DownDamage);
        assert_eq!(entered.fighters[1].action_frame, 0);
        assert_eq!(entered.fighters[1].prone, Some(orientation));
        assert_eq!(entered.fighters[1].down_timer, entered.fighters[1].hitstun);
        assert!(!entered.fighters[1].grounded);

        let checkpoint = game.checkpoint();
        let expected = (0..120)
            .map(|_| step(&mut game, IDLE))
            .take_while(|state| state.fighters[1].action == Action::DownDamage)
            .collect::<Vec<_>>();
        assert!(!expected.is_empty());
        assert!(
            expected
                .iter()
                .any(|state| (state.fighters[1].ecb.current.left[0] - ecb_left).abs() < 1e-5)
        );
        assert_eq!(game.state().fighters[1].action, Action::DownWait);
        assert!(game.state().fighters[1].down_timer > 0);

        game.restore_checkpoint(&checkpoint).unwrap();
        for state in expected {
            assert_eq!(step(&mut game, IDLE), state);
        }
        assert_eq!(step(&mut game, IDLE).fighters[1].action, Action::DownWait);
        until(&mut game, |state| {
            state.fighters[1].action == Action::DownStand
        });
        assert_eq!(game.state().fighters[1].down_timer, 0);
    }
}

#[test]
fn prone_damage_launches_away_but_keeps_the_pretransition_facing_override() {
    let mut game = down_wait(down_damage_data(false));
    assert_eq!(game.state().fighters[1].facing, -1.0);
    for _ in 0..12 {
        if game.state().fighters[0].position[0] > game.state().fighters[1].position[0] {
            break;
        }
        step(&mut game, directional_input(0, 0, 1.0));
    }
    assert_eq!(game.state().fighters[1].action, Action::DownWait);
    assert!(game.state().fighters[0].position[0] > game.state().fighters[1].position[0]);

    let entered = hit_prone(&mut game);
    assert_eq!(entered.fighters[1].action, Action::DownDamage);
    assert_eq!(entered.fighters[1].facing, -1.0);
    assert!(entered.fighters[1].knockback[0] < 0.0);
}

#[test]
fn prone_down_damage_forces_the_source_fly_launch_branch() {
    let mut game = down_wait(grounded_launch_down_damage_data());
    assert!(game.state().fighters[1].grounded);
    assert_eq!(game.state().fighters[1].action, Action::DownWait);
    let entered = hit_prone(&mut game);
    let fighter = &entered.fighters[1];
    assert_eq!(fighter.action, Action::DownDamage);
    assert!(!fighter.grounded);
    assert_eq!(fighter.ground_line, None);
    assert_eq!(fighter.ground_knockback, 0.0);
    assert!(fighter.knockback[1] > 0.0);
}

#[test]
fn downbound_hit_uses_the_original_face_down_selector_quirk() {
    let mut game = down_bound(down_damage_data(false));
    assert_eq!(
        game.state().fighters[1].prone,
        Some(ProneOrientation::FaceUp)
    );
    until(&mut game, |state| state.fighters[0].action == Action::Wait);
    assert_eq!(game.state().fighters[1].action, Action::DownBound);
    let entered = hit_prone(&mut game);
    assert_eq!(entered.fighters[1].action, Action::DownDamage);
    assert_eq!(entered.fighters[1].prone, Some(ProneOrientation::FaceDown));
}

#[test]
fn threshold_equality_and_non_prone_recovery_use_ordinary_damage() {
    let mut equal = down_damage_data(false);
    equal
        .rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_damage
        .as_mut()
        .unwrap()
        .pending_damage_threshold = 40;
    let state = hit_prone(&mut down_wait(equal));
    assert_eq!(state.fighters[1].action, Action::Damage);
    assert_eq!(state.fighters[1].prone, None);

    let mut stand = down_wait(down_damage_data(false));
    step(&mut stand, recovery_input(0, [0.0, 0.7], [0.0; 2]));
    assert_eq!(stand.state().fighters[1].action, Action::DownStand);
    let state = hit_prone(&mut stand);
    assert_eq!(state.fighters[1].action, Action::Damage);
    assert_eq!(state.fighters[1].prone, None);
}

#[test]
fn downbound_expiry_uses_buffered_attacks_then_fresh_rolls_without_standing() {
    for button in [BUTTON_A, BUTTON_B] {
        let mut game = down_bound(knockdown_data());
        assert_eq!(game.state().fighters[1].locomotion.attack_a_age, 255);
        assert_eq!(game.state().fighters[1].locomotion.attack_b_age, 255);
        step(&mut game, recovery_input(button, [0.0; 2], [0.0; 2]));
        let checkpoint = game.checkpoint();
        advance_to_bound_expiry(&mut game);
        let expected = step(&mut game, IDLE);
        assert_eq!(expected.fighters[1].action, Action::DownAttack);
        let age = if button == BUTTON_A {
            expected.fighters[1].locomotion.attack_a_age
        } else {
            expected.fighters[1].locomotion.attack_b_age
        };
        assert_eq!(age, 3);
        game.restore_checkpoint(&checkpoint).unwrap();
        advance_to_bound_expiry(&mut game);
        assert_eq!(step(&mut game, IDLE), expected);
    }

    for (controls, action) in [
        (recovery_input(0, [0.0; 2], [0.0, 0.8]), Action::DownAttack),
        (
            recovery_input(0, [0.0; 2], [-0.7, 0.0]),
            Action::DownForward,
        ),
        (recovery_input(0, [0.7, 0.0], [0.0; 2]), Action::DownBack),
    ] {
        let mut game = down_bound(knockdown_data());
        advance_to_bound_expiry(&mut game);
        assert_eq!(step(&mut game, controls).fighters[1].action, action);
    }

    for controls in [
        recovery_input(0, [0.0, 0.7], [0.0; 2]),
        recovery_input(BUTTON_L, [0.0; 2], [0.0; 2]),
    ] {
        let mut game = down_bound(knockdown_data());
        advance_to_bound_expiry(&mut game);
        let state = step(&mut game, controls);
        assert_eq!(state.fighters[1].action, Action::DownWait);
        assert_eq!(state.fighters[1].action_frame, 1);
    }
}

#[test]
fn attack_input_before_downbound_is_reset_instead_of_becoming_a_buffer() {
    let mut game = downward_hit(knockdown_data());
    let held = recovery_input(BUTTON_A, [0.0; 2], [0.0; 2]);
    while !game.state().fighters[1].grounded {
        step(&mut game, held);
    }
    assert_eq!(game.state().fighters[1].action, Action::DownBound);
    assert_eq!(game.state().fighters[1].locomotion.attack_a_age, 255);
    advance_to_bound_expiry(&mut game);
    assert_eq!(step(&mut game, IDLE).fighters[1].action, Action::DownWait);
}

#[test]
fn downwait_inputs_use_attack_roll_stand_priority_and_inclusive_boundaries() {
    let cases = [
        (
            recovery_input(BUTTON_A, [0.7, 0.0], [0.0; 2]),
            Action::DownAttack,
        ),
        (
            recovery_input(BUTTON_B, [0.0; 2], [0.0; 2]),
            Action::DownAttack,
        ),
        (recovery_input(0, [0.0; 2], [0.0, 0.8]), Action::DownAttack),
        (
            recovery_input(0, [0.7, 0.0], [-0.7, 0.0]),
            Action::DownForward,
        ),
        (recovery_input(0, [0.7, 0.0], [0.0; 2]), Action::DownBack),
        (recovery_input(0, [0.0, 0.7], [0.0; 2]), Action::DownStand),
        (
            recovery_input(BUTTON_L, [0.0; 2], [0.0; 2]),
            Action::DownStand,
        ),
        (
            recovery_input(BUTTON_R, [0.0; 2], [0.0; 2]),
            Action::DownStand,
        ),
    ];
    for (controls, action) in cases {
        let mut game = down_wait(knockdown_data());
        let state = step(&mut game, controls);
        assert_eq!(state.fighters[1].action, action);
        assert!(state.fighters[1].action_frame <= 1);
    }

    let mut game = down_wait(knockdown_data());
    let state = step(&mut game, recovery_input(0, [0.699, 0.0], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::DownWait);
}

#[test]
fn recovery_invincibility_covers_each_floor_option_for_its_exact_contact_ticks() {
    let resource = recovery_timer_data();

    let mut passive = downward_hit(resource.clone());
    arm_tech(&mut passive);
    let entered = until(&mut passive, |state| state.fighters[1].grounded);
    assert_eq!(entered.fighters[1].action, Action::Passive);
    assert_eq!(entered.fighters[1].invincibility, 1);
    assert_eq!(step(&mut passive, IDLE).fighters[1].invincibility, 0);

    let mut tech_roll = downward_hit(resource.clone());
    let entered = arm_directional_tech(&mut tech_roll, -0.7);
    assert_eq!(entered.fighters[1].action, Action::PassiveStandF);
    assert_eq!(entered.fighters[1].invincibility, 1);
    assert_eq!(step(&mut tech_roll, IDLE).fighters[1].invincibility, 0);

    for (controls, action) in [
        (
            recovery_input(0, [-0.7, 0.0], [0.0; 2]),
            Action::DownForward,
        ),
        (recovery_input(0, [0.0, 0.7], [0.0; 2]), Action::DownStand),
        (
            recovery_input(BUTTON_A, [0.0; 2], [0.0; 2]),
            Action::DownAttack,
        ),
    ] {
        let mut game = down_wait(resource.clone());
        let entered = step(&mut game, controls);
        assert_eq!(entered.fighters[1].action, action);
        assert_eq!(entered.fighters[1].invincibility, 1);
        assert_eq!(step(&mut game, IDLE).fighters[1].invincibility, 0);
    }
}

#[test]
fn recovery_invincibility_blocks_combat_until_the_first_vulnerable_frame() {
    let mut resource = recovery_timer_data();
    for frame in &mut resource.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.radius = 30.0;
        }
    }
    let mut game = down_wait(resource);
    let before = game.state().fighters[1].percent;

    let mut controls = recovery_input(0, [0.0, 0.7], [0.0; 2]);
    controls[0].buttons = BUTTON_A;
    let entered = step(&mut game, controls);
    assert_eq!(entered.fighters[1].action, Action::DownStand);
    assert_eq!(entered.fighters[1].invincibility, 1);

    let protected = step(&mut game, IDLE);
    assert_eq!(protected.fighters[1].invincibility, 0);
    assert_eq!(protected.fighters[1].percent, before);
    assert!(!protected.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }
    )));

    let vulnerable = step(&mut game, IDLE);
    assert!(vulnerable.fighters[1].percent > before);
    assert!(vulnerable.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }
    )));
}

#[test]
fn held_cstick_before_downwait_is_not_a_fresh_attack_or_roll() {
    let mut game = downward_hit(knockdown_data());
    until(&mut game, |state| {
        state.fighters[1].action == Action::DownBound
    });
    let held = recovery_input(0, [0.0; 2], [-0.7, 0.8]);
    while game.state().fighters[1].action == Action::DownBound {
        step(&mut game, held);
    }
    assert_eq!(game.state().fighters[1].action, Action::DownWait);
    assert_eq!(step(&mut game, held).fighters[1].action, Action::DownWait);
}

#[test]
fn missed_tech_rolls_use_sampled_root_motion_bones_and_checkpoint_suffixes() {
    for (stick_x, action, roots, ecb_side) in [
        (-0.7, Action::DownForward, vec![0.0, 0.6, 0.9, 0.3], -6.0),
        (
            0.7,
            Action::DownBack,
            vec![0.0, -0.4, -0.7, -0.2, -0.1],
            6.0,
        ),
    ] {
        let mut game = down_wait(knockdown_data());
        let entered = step(&mut game, recovery_input(0, [stick_x, 0.0], [0.0; 2]));
        assert_eq!(entered.fighters[1].action, action);
        let checkpoint = game.checkpoint();
        let mut expected = Vec::new();
        let mut previous_x = entered.fighters[1].position[0];
        let mut previous_ground_velocity = entered.fighters[1].ground_velocity;
        for &local_delta in &roots[1..] {
            let controls = recovery_input(0, [stick_x, 0.0], [0.0; 2]);
            let state = step(&mut game, controls);
            let fighter = &state.fighters[1];
            assert_eq!(fighter.action, action);
            let target = local_delta * fighter.facing;
            let velocity = previous_ground_velocity + (target - previous_ground_velocity);
            assert_eq!(
                fighter.position[0].to_bits(),
                (previous_x + velocity).to_bits()
            );
            assert_eq!(fighter.ground_velocity.to_bits(), velocity.to_bits());
            if ecb_side > 0.0 {
                assert!((fighter.ecb.current.right[0] - ecb_side).abs() < 1e-5);
            } else {
                assert!((fighter.ecb.current.left[0] - ecb_side).abs() < 1e-5);
            }
            previous_x = fighter.position[0];
            previous_ground_velocity = fighter.ground_velocity;
            expected.push((controls, state));
        }
        expected.push((IDLE, step(&mut game, IDLE)));
        assert_eq!(game.state().fighters[1].action, Action::Wait);
        game.restore_checkpoint(&checkpoint).unwrap();
        for (controls, state) in expected {
            assert_eq!(step(&mut game, controls), state);
        }
    }
}

#[test]
fn missed_tech_stand_uses_bone_poses_and_getup_attack_hits_through_combat() {
    let mut stand = down_wait(knockdown_data());
    let state = step(&mut stand, recovery_input(0, [0.0, 0.7], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::DownStand);
    assert!((state.fighters[1].ecb.current.left[0] + 7.0).abs() < 1e-5);
    until(&mut stand, |state| state.fighters[1].action == Action::Wait);

    let mut attack = down_wait(knockdown_data());
    let state = step(&mut attack, recovery_input(BUTTON_A, [0.0; 2], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::DownAttack);
    assert!(state.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 1,
            victim: 0,
            damage,
            ..
        } if *damage == 5.0
    )));
    while attack.state().fighters[1].action == Action::DownAttack {
        step(&mut attack, IDLE);
    }
    assert_eq!(attack.state().fighters[1].action, Action::Wait);
}

#[test]
fn a_second_recent_shoulder_press_fails_the_repeat_lockout() {
    let mut game = downward_hit(data());
    step(&mut game, input(1, BUTTON_L));
    step(&mut game, IDLE);
    while game.state().fighters[1].hitlag > 1.0 {
        step(&mut game, IDLE);
    }
    step(&mut game, input(1, BUTTON_R));
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    assert!(
        landed.fighters[1].locomotion.previous_tech_press_age < profile().tech_repeat_lockout as u8
    );
}

#[test]
fn non_tumbling_damage_does_not_enter_the_tumble_floor_graph() {
    let mut resource = data();
    resource
        .rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tumble_knockback_threshold = 100.0;
    let mut game = Match::new(resource, 7).unwrap();
    step(&mut game, input(0, BUTTON_A));
    until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(!game.state().fighters[1].tumbling);
    let mut seen_floor_action = None;
    for _ in 0..120 {
        let state = step(&mut game, IDLE);
        if state.fighters[1].grounded {
            seen_floor_action = Some(state.fighters[1].action);
        }
        assert!(!matches!(
            state.fighters[1].action,
            Action::Passive
                | Action::PassiveStandF
                | Action::PassiveStandB
                | Action::DownBound
                | Action::DownWait
                | Action::DownForward
                | Action::DownBack
                | Action::DownAttack
                | Action::DownStand
        ));
        if state.fighters[1].action == Action::Wait {
            break;
        }
    }
    assert!(seen_floor_action.is_some());
    assert_eq!(game.state().fighters[1].action, Action::Wait);
}

#[test]
fn damagefall_preserves_tumble_until_a_later_floor_contact() {
    let mut game = downward_hit(lingering_tumble_data());
    let mut saw_damagefall = false;
    while !game.state().fighters[1].grounded {
        let state = step(&mut game, IDLE);
        saw_damagefall |= state.fighters[1].action == Action::DamageFall;
    }
    assert!(saw_damagefall);
    assert_eq!(game.state().fighters[1].action, Action::DownBound);
}

#[test]
fn damagefall_uses_ordinary_air_drift_and_fresh_fastfall_input() {
    let mut game = downward_hit(lingering_tumble_data());
    until(&mut game, |state| {
        state.fighters[1].action == Action::DamageFall
    });
    let before = game.state().fighters[1].clone();
    let mut controls = IDLE;
    controls[1].stick = [1.0, -1.0];
    let after = step(&mut game, controls).fighters[1].clone();
    assert!(after.fast_fall);
    assert!(after.velocity[0] > before.velocity[0]);
    assert!(after.velocity[1] < before.velocity[1]);
    assert!(!after.grounded);
}

#[test]
fn checkpoint_and_reset_preserve_tech_history_and_floor_suffixes() {
    let mut game = downward_hit(data());
    arm_tech(&mut game);
    let checkpoint = game.checkpoint();
    let expected = (0..20).map(|_| step(&mut game, IDLE)).collect::<Vec<_>>();
    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(step(&mut game, IDLE), state);
    }
    let reset = game.reset(99);
    assert_eq!(reset.fighters[1].locomotion.tech_press_age, 255);
    assert_eq!(reset.fighters[1].locomotion.previous_tech_press_age, 255);
    assert_eq!(reset.fighters[1].locomotion.attack_a_age, 255);
    assert_eq!(reset.fighters[1].locomotion.attack_b_age, 255);
    assert_eq!(reset.fighters[1].prone, None);
    assert_eq!(reset.fighters[1].down_timer, 0);
    assert!(!reset.fighters[1].tumbling);
}

#[test]
fn malformed_floor_profiles_are_rejected_transactionally() {
    let encoded = serde_json::to_string(&roll_data()).unwrap();
    let decoded: skirmish::game::data::MatchData = serde_json::from_str(&encoded).unwrap();
    assert!(
        decoded
            .rules
            .damage
            .floor_response
            .unwrap()
            .tech_roll
            .is_some()
    );
    assert!(
        decoded
            .fighters
            .iter()
            .all(|fighter| fighter.floor_tech.is_some())
    );
    let encoded = serde_json::to_string(&knockdown_data()).unwrap();
    let decoded: skirmish::game::data::MatchData = serde_json::from_str(&encoded).unwrap();
    assert!(
        decoded
            .fighters
            .iter()
            .all(|fighter| fighter.knockdown.is_some())
    );
    let encoded = serde_json::to_string(&down_damage_data(false)).unwrap();
    let decoded: skirmish::game::data::MatchData = serde_json::from_str(&encoded).unwrap();
    assert!(
        decoded
            .rules
            .damage
            .floor_response
            .unwrap()
            .down_damage
            .is_some()
    );
    assert!(decoded.fighters.iter().all(|fighter| {
        fighter
            .knockdown
            .as_ref()
            .unwrap()
            .face_up
            .damage_poses
            .is_some()
    }));

    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tech_window = f32::NAN;
    cases.push(bad);
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tech_repeat_lockout = -1;
    cases.push(bad);
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .passive_frames = 0;
    cases.push(bad);
    let mut bad = roll_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tech_roll
        .as_mut()
        .unwrap()
        .stick_threshold = 0.0;
    cases.push(bad);
    let mut bad = roll_data();
    bad.fighters[0].floor_tech = None;
    cases.push(bad);
    let mut bad = roll_data();
    bad.fighters[0]
        .floor_tech
        .as_mut()
        .unwrap()
        .forward
        .frames
        .clear();
    cases.push(bad);
    let mut bad = roll_data();
    bad.fighters[0].floor_tech.as_mut().unwrap().backward.frames[0].root_translation = f32::NAN;
    cases.push(bad);
    let mut bad = roll_data();
    bad.fighters[0].floor_tech.as_mut().unwrap().forward.frames[0]
        .bones
        .pop();
    cases.push(bad);
    let mut bad = roll_data();
    bad.rules.damage.floor_response.as_mut().unwrap().tech_roll = None;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .knockdown_options
        .as_mut()
        .unwrap()
        .horizontal_stick_threshold = 0.0;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .knockdown_options
        .as_mut()
        .unwrap()
        .vertical_angle_radians = f32::NAN;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .knockdown_options
        .as_mut()
        .unwrap()
        .bound_attack_window = 256.0;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_bound_frames_face_up = Some(0);
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_wait_frames_face_down = Some(1_000_000);
    cases.push(bad);
    // An override with no matching pose-vector resize leaves the supplied
    // poses at the shared length, so the per-orientation duration and the
    // per-orientation pose count disagree.
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_stand_frames_face_up = Some(profile().down_stand_frames + 1);
    cases.push(bad);
    // Isolated from the pose-length check above: the override's own poses
    // are resized to match, so only the invincibility-vs-orientation-stand
    // comparison can reject this one.
    let mut bad = recovery_data();
    let short_stand = bad
        .rules
        .damage
        .floor_response
        .as_ref()
        .unwrap()
        .recovery_invincibility
        .as_ref()
        .unwrap()
        .stand_frames
        - 1;
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_stand_frames_face_down = Some(short_stand);
    for fighter in &mut bad.fighters {
        let bones = fighter.bones.clone();
        fighter
            .knockdown
            .as_mut()
            .unwrap()
            .face_down
            .stand_poses
            .resize(short_stand as usize, bones);
    }
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0].knockdown = None;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .orientation
        .hip_bone = 99;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_down
        .wait_poses
        .pop();
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .passive_poses
        .pop();
    cases.push(bad);
    for poses in ["bound", "wait"] {
        let mut bad = knockdown_data();
        let attributes = &mut bad.fighters[0].knockdown.as_mut().unwrap().face_up;
        match poses {
            "bound" => {
                attributes.bound_poses.pop();
            }
            "wait" => {
                attributes.wait_poses.pop();
            }
            _ => unreachable!(),
        }
        cases.push(bad);
    }
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_up
        .stand_poses
        .pop();
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_up
        .forward
        .frames[0]
        .root_translation = f32::INFINITY;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_down
        .attack
        .frames
        .clear();
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .knockdown_options = None;
    cases.push(bad);
    let mut bad = down_damage_data(false);
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_damage
        .as_mut()
        .unwrap()
        .pending_damage_threshold = -1;
    cases.push(bad);
    let mut bad = down_damage_data(false);
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_damage
        .as_mut()
        .unwrap()
        .frames = 0;
    cases.push(bad);
    let mut bad = down_damage_data(false);
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_down
        .damage_poses = None;
    cases.push(bad);
    let mut bad = down_damage_data(false);
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .face_up
        .damage_poses
        .as_mut()
        .unwrap()
        .pop();
    cases.push(bad);
    let mut bad = down_damage_data(false);
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .down_damage = None;
    cases.push(bad);
    let mut bad = recovery_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .recovery_invincibility
        .as_mut()
        .unwrap()
        .passive_frames = profile().passive_frames + 1;
    cases.push(bad);
    let mut bad = recovery_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .recovery_invincibility
        .as_mut()
        .unwrap()
        .tech_roll_frames = 6;
    cases.push(bad);
    let mut bad = recovery_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .recovery_invincibility
        .as_mut()
        .unwrap()
        .missed_roll_frames = 6;
    cases.push(bad);
    let mut bad = recovery_data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .recovery_invincibility
        .as_mut()
        .unwrap()
        .attack_frames = 5;
    cases.push(bad);
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .recovery_invincibility = Some(RecoveryInvincibilityRules {
        passive_frames: 1,
        tech_roll_frames: 0,
        missed_roll_frames: 1,
        stand_frames: 0,
        attack_frames: 0,
    });
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
