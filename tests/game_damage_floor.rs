//! Native damage-floor scheduling with explicit synthetic state durations.
use skirmish::{
    collision::ecb,
    game::{
        Action, BUTTON_A, BUTTON_B, BUTTON_L, BUTTON_R, Controller, Event, Match, State,
        damage::{
            FloorTechAttributes, FloorTechFrame, FloorTechMotion, FloorTechRules,
            KnockdownAttributes, KnockdownRules,
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
        passive_frames: 3,
        down_bound_frames: 4,
        down_wait_frames: 5,
        down_stand_frames: 3,
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
                })
                .collect(),
        };
        let mut stand_poses = vec![fighter.bones.clone(); stand_frames];
        stand_poses[0][1].translation[0] = 7.0;
        fighter.knockdown = Some(KnockdownAttributes {
            forward: roll_motion(&fighter.bones, &[0.0, 0.6, 0.9, 0.3], 6.0),
            backward: roll_motion(&fighter.bones, &[0.0, -0.4, -0.7, -0.2, -0.1], -6.0),
            stand_poses,
            attack,
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
    bad.fighters[0].knockdown = None;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
        .stand_poses
        .pop();
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0].knockdown.as_mut().unwrap().forward.frames[0].root_translation = f32::INFINITY;
    cases.push(bad);
    let mut bad = knockdown_data();
    bad.fighters[0]
        .knockdown
        .as_mut()
        .unwrap()
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
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
