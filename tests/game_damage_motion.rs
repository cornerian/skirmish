//! End-to-end ordinary Damage motion selection, physics poses and timing.
use skirmish::{
    collision::ecb,
    fighter::damage::{DamageMotion, HurtHeight},
    game::{
        Action, BUTTON_A, Controller, Error, Event, Match,
        damage::{DamageMotionRules, DamagePoseAttributes},
        data::{Bone, CollisionBox, MatchData},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn base_data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data
}

fn repeated_pose(base: &[Bone], x: f32, frames: usize) -> Vec<Vec<Bone>> {
    (0..frames)
        .map(|frame| {
            let mut pose = base.to_vec();
            pose[1].translation[0] = x + frame as f32;
            pose
        })
        .collect()
}

fn poses(base: &[Bone], height: HurtHeight, frames: usize, x: f32) -> DamagePoseAttributes {
    let motion = repeated_pose(base, x, frames);
    DamagePoseAttributes {
        hurtbox_heights: vec![height],
        ground: core::array::from_fn(|_| core::array::from_fn(|_| motion.clone())),
        air: core::array::from_fn(|_| motion.clone()),
        fly: core::array::from_fn(|_| motion.clone()),
    }
}

fn profile(thresholds: [f32; 3], height: HurtHeight, frames: usize, x: f32) -> MatchData {
    let mut data = base_data();
    data.rules.damage.damage_motion = Some(DamageMotionRules { thresholds });
    for fighter in &mut data.fighters {
        fighter.damage_poses = Some(poses(&fighter.bones, height, frames, x));
    }
    data
}

fn hit(mut data: MatchData, airborne: bool) -> Match {
    if airborne {
        data.stage.spawns[1][1] = 1.5;
    }
    let mut game = Match::new(data, 42).unwrap();
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    game.step(IDLE).unwrap();
    assert!(game.state().events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }
    )));
    game
}

fn finish_hitlag(game: &mut Match) {
    while game.state().fighters[1].hitlag > 0.0 {
        game.step(IDLE).unwrap();
    }
}

fn event_knockback(game: &Match) -> f32 {
    game.state()
        .events
        .iter()
        .find_map(|event| match event {
            Event::Hit { knockback, .. } => Some(*knockback),
            _ => None,
        })
        .unwrap()
}

fn repeat_jab(game: &mut Match) -> f32 {
    for _ in 0..100 {
        if game.state().fighters[0].action == Action::Wait && game.state().fighters[0].hitlag == 0.0
        {
            break;
        }
        game.step(IDLE).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::Wait);
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..10 {
        if game
            .state()
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
        {
            return event_knockback(game);
        }
        game.step(IDLE).unwrap();
    }
    panic!("repeated jab did not connect")
}

fn repeated_jab_connects(game: &mut Match) -> bool {
    for _ in 0..100 {
        if game.state().fighters[0].action == Action::Wait && game.state().fighters[0].hitlag == 0.0
        {
            break;
        }
        game.step(IDLE).unwrap();
    }
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    for _ in 0..5 {
        if game
            .state()
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
        {
            return true;
        }
        game.step(IDLE).unwrap();
    }
    false
}

#[test]
fn contact_height_pre_hit_ground_state_and_all_knockback_levels_select_the_source_motions() {
    let levels = [
        ([50.0, 60.0, 70.0], 0),
        ([0.0, 50.0, 60.0], 1),
        ([0.0, 1.0, 50.0], 2),
        ([0.0, 1.0, 2.0], 3),
    ];
    for (thresholds, level) in levels {
        for height in [HurtHeight::Low, HurtHeight::Middle, HurtHeight::High] {
            for airborne in [false, true] {
                let game = hit(profile(thresholds, height, 1, 0.0), airborne);
                let expected = match (airborne, level) {
                    (_, 3) => DamageMotion::Fly { height },
                    (true, level) => DamageMotion::Air { level },
                    (false, level) => DamageMotion::Ground { level, height },
                };
                assert_eq!(game.state().fighters[1].damage_motion, Some(expected));
            }
        }
    }
}

#[test]
fn exact_threshold_equality_advances_to_the_next_level() {
    let baseline = hit(base_data(), false);
    let knockback = baseline
        .state()
        .events
        .iter()
        .find_map(|event| match event {
            Event::Hit { knockback, .. } => Some(*knockback),
            _ => None,
        })
        .unwrap();
    let scaled = knockback * baseline.data().rules.hitstun_scale;
    let game = hit(
        profile(
            [scaled * 0.5, scaled, scaled * 2.0],
            HurtHeight::Middle,
            1,
            0.0,
        ),
        false,
    );
    assert_eq!(
        game.state().fighters[1].damage_motion,
        Some(DamageMotion::Ground {
            level: 2,
            height: HurtHeight::Middle,
        })
    );
}

#[test]
fn a_second_hit_reselects_the_motion_and_checkpoint_replay_is_exact() {
    let mut baseline_data = base_data();
    baseline_data.rules.knockback_speed = 0.0;
    let mut baseline = hit(baseline_data, false);
    let first = event_knockback(&baseline) * baseline.data().rules.hitstun_scale;
    let second = repeat_jab(&mut baseline) * baseline.data().rules.hitstun_scale;
    assert!(second > first);

    let mut data = profile(
        [(first + second) * 0.5, second + 1.0, second + 2.0],
        HurtHeight::Middle,
        1,
        0.0,
    );
    data.rules.knockback_speed = 0.0;
    let mut game = hit(data, false);
    assert_eq!(
        game.state().fighters[1].damage_motion,
        Some(DamageMotion::Ground {
            level: 0,
            height: HurtHeight::Middle,
        })
    );
    let checkpoint = game.checkpoint();
    repeat_jab(&mut game);
    assert_eq!(
        game.state().fighters[1].damage_motion,
        Some(DamageMotion::Ground {
            level: 1,
            height: HurtHeight::Middle,
        })
    );
    let expected = serde_json::to_vec(game.state()).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    repeat_jab(&mut game);
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), expected);
}

#[test]
fn selected_damage_bones_drive_ecb_sampling() {
    let mut data = profile([50.0, 60.0, 70.0], HurtHeight::Middle, 3, 4.0);
    data.rules.knockback_speed = 0.0;
    data.fighters[1].collision_box = CollisionBox::Bones {
        indices: [0, 1, 0, 1, 0, 1],
        parameters: ecb::JointParameters {
            side_y_offset: 0.0,
            height_threshold: 4.0,
            width_threshold: 4.0,
        },
        flags: 5,
    };
    let mut game = hit(data, false);
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].ecb.current.left[0], -4.0);
    finish_hitlag(&mut game);
    game.step(IDLE).unwrap();
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].ecb.current.left[0], -5.0);
}

#[test]
fn selected_damage_bones_also_drive_hurtbox_contact() {
    let mut near = profile([50.0, 60.0, 70.0], HurtHeight::Middle, 1, 0.0);
    near.rules.knockback_speed = 0.0;
    let mut far = profile([50.0, 60.0, 70.0], HurtHeight::Middle, 1, 40.0);
    far.rules.knockback_speed = 0.0;
    let mut near = hit(near, false);
    let mut far = hit(far, false);
    assert!(repeated_jab_connects(&mut near));
    assert!(!repeated_jab_connects(&mut far));
}

#[test]
fn damage_waits_for_both_animation_and_hitstun_and_holds_the_final_pose() {
    let mut short_hitstun = profile([1.0, 2.0, 3.0], HurtHeight::Middle, 4, 0.0);
    short_hitstun.rules.hitstun_scale = 0.01;
    short_hitstun.rules.knockback_speed = 0.0;
    let mut short_hitstun = hit(short_hitstun, false);
    finish_hitlag(&mut short_hitstun);
    assert_eq!(short_hitstun.state().fighters[1].hitstun, 1);
    for frame in 1..=4 {
        short_hitstun.step(IDLE).unwrap();
        let fighter = &short_hitstun.state().fighters[1];
        assert_eq!(fighter.action, Action::Damage, "frame {frame}");
        assert_eq!(fighter.hitstun, 0);
    }
    short_hitstun.step(IDLE).unwrap();
    assert_eq!(short_hitstun.state().fighters[1].action, Action::Wait);
    assert_eq!(short_hitstun.state().fighters[1].damage_motion, None);

    let mut long_data = profile([50.0, 60.0, 70.0], HurtHeight::Middle, 1, 0.0);
    long_data.rules.knockback_speed = 0.0;
    let mut long_hitstun = hit(long_data, false);
    let checkpoint = long_hitstun.checkpoint();
    let saved = serde_json::to_vec(long_hitstun.state()).unwrap();
    finish_hitlag(&mut long_hitstun);
    long_hitstun.step(IDLE).unwrap();
    long_hitstun.step(IDLE).unwrap();
    assert_eq!(long_hitstun.state().fighters[1].action, Action::Damage);
    assert!(long_hitstun.state().fighters[1].action_frame > 1);
    assert!(long_hitstun.state().fighters[1].hitstun > 0);
    long_hitstun.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(serde_json::to_vec(long_hitstun.state()).unwrap(), saved);
}

#[test]
fn damage_motion_resources_are_paired_bounded_and_roundtrip_without_defaults() {
    let data = profile([10.0, 20.0, 30.0], HurtHeight::High, 2, 0.0);
    let encoded = serde_json::to_string(&data).unwrap();
    let decoded: MatchData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, data);
    assert!(encoded.contains("\"damage_motion\""));
    assert!(encoded.contains("\"hurtbox_heights\":[\"high\"]"));

    let mut invalid = Vec::new();
    let mut missing = data.clone();
    missing.fighters[0].damage_poses = None;
    invalid.push(missing);
    let mut orphan = data.clone();
    orphan.rules.damage.damage_motion = None;
    invalid.push(orphan);
    let mut heights = data.clone();
    heights.fighters[0]
        .damage_poses
        .as_mut()
        .unwrap()
        .hurtbox_heights
        .clear();
    invalid.push(heights);
    let mut empty = data.clone();
    empty.fighters[0].damage_poses.as_mut().unwrap().air[1].clear();
    invalid.push(empty);
    let mut topology = data.clone();
    topology.fighters[0].damage_poses.as_mut().unwrap().fly[2][0][1].parent = None;
    invalid.push(topology);
    for thresholds in [
        [10.0, 10.0, 30.0],
        [20.0, 10.0, 30.0],
        [10.0, 20.0, f32::NAN],
    ] {
        let mut bad = data.clone();
        bad.rules.damage.damage_motion.as_mut().unwrap().thresholds = thresholds;
        invalid.push(bad);
    }
    for data in invalid {
        assert!(matches!(Match::new(data, 1), Err(Error::Data(_))));
    }
}
