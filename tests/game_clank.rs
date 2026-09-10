//! Native match contracts for ordinary non-Slash clashes, with synthetic resources.
//! Source: ftColl_80078C70/8007699C, Fighter_ProcessHit and ftCo_Rebound callbacks.
#[path = "support/aerial.rs"]
mod aerial_fixture;

use skirmish::{
    fighter::{clank as math, stale},
    game::{
        Action, BUTTON_A, Controller, Event, Match, State, clank,
        data::{AttackFrame, MatchData},
    },
};

fn data(damage: [u32; 2]) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.rules.knockback_speed = 0.0;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.rules.clank = Some(clank::Rules {
        profile: clank::Profile::OrdinaryGroundedNonSlash,
        response: math::Rules {
            damage_gap: 9,
            duration_scale: 0.5,
            duration_base: 2.0,
        },
        push_scale: 0.2,
        push_base: 0.6,
        hitlag_maximum: 20.0,
        surface_friction_multiplier: 0.5,
    });
    for (player, fighter) in data.fighters.iter_mut().enumerate() {
        fighter.hurtboxes[0].bone = 0;
        fighter.hurtboxes[0].start = [0.0, 0.0, 0.0];
        fighter.hurtboxes[0].end = [0.0, 2.0, 0.0];
        let mut hit = fighter.jab.frames[1].hitboxes[0].clone();
        hit.bone = 0;
        hit.center = [2.5, 1.0, 0.0];
        hit.damage = damage[player];
        hit.clank = true;
        hit.rebound = true;
        let mut active_pose = fighter.bones.clone();
        active_pose[1].translation[1] = 3.0;
        let active = AttackFrame {
            bones: active_pose,
            hitboxes: vec![hit],
            hurtbox_states: vec![],
        };
        let idle = AttackFrame {
            bones: fighter.bones.clone(),
            hitboxes: vec![],
            hurtbox_states: vec![],
        };
        fighter.jab.frames = vec![idle.clone(), active.clone(), active.clone(), active, idle];
        fighter.rebound = Some(clank::Animation {
            animation_length: 13.9,
            poses: (0..15)
                .map(|frame| {
                    let mut pose = fighter.bones.clone();
                    pose[1].translation[1] = 1.0 + frame as f32 * 0.1;
                    pose
                })
                .collect(),
        });
    }
    data
}

fn input(attack: [bool; 2]) -> [Controller; 2] {
    attack.map(|attack| Controller {
        buttons: if attack { BUTTON_A } else { 0 },
        ..Default::default()
    })
}

fn step(game: &mut Match) -> State {
    game.step(input([false; 2])).unwrap().clone()
}

fn attack(game: &mut Match, players: [bool; 2]) -> State {
    game.step(input(players)).unwrap();
    step(game)
}

fn wait(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..250 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game);
    }
    panic!("condition not reached: {:?}", game.state());
}

fn recovered(game: &mut Match) {
    wait(game, |s| {
        s.fighters
            .iter()
            .all(|f| f.action == Action::Wait && f.hitlag == 0.0)
    });
}

#[test]
fn equal_grounded_attacks_clash_without_damage_then_recover_with_opposing_recoil() {
    let mut game = Match::new(data([10; 2]), 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert!(
        contact
            .fighters
            .iter()
            .all(|f| { f.action == Action::ReboundStop && f.percent == 0.0 && f.hitlag == 3.0 })
    );
    assert!(
        !contact
            .events
            .iter()
            .any(|e| matches!(e, Event::Hit { .. }))
    );
    let positions = contact.fighters.each_ref().map(|f| f.position);
    recovered(&mut game);
    assert!(game.state().fighters[0].position[0] < positions[0][0]);
    assert!(game.state().fighters[1].position[0] > positions[1][0]);
    assert!(game.state().fighters.iter().all(|f| f.percent == 0.0));
    assert_eq!(game.state().rng_seed, 42);
}

#[test]
fn strict_damage_gap_leaves_the_stronger_attack_active_for_a_body_hit() {
    for strong in [18, 19, 20] {
        let mut game = Match::new(data([strong, 10]), 42).unwrap();
        let contact = attack(&mut game, [true; 2]);
        assert_eq!(contact.fighters[0].percent, 0.0);
        if strong < 19 {
            assert_eq!(contact.fighters[0].action, Action::ReboundStop);
            assert_eq!(contact.fighters[1].percent, 0.0);
        } else {
            assert_eq!(contact.fighters[0].action, Action::Jab);
            assert_eq!(contact.fighters[1].action, Action::Damage);
            assert_eq!(contact.fighters[1].percent, strong as f32);
            assert!(contact.events.iter().any(|e| matches!(
                e,
                Event::Hit {
                    attacker: 0,
                    victim: 1,
                    ..
                }
            )));
        }
    }
}

#[test]
fn non_rebounding_clashes_keep_victim_history_until_a_new_group_activates() {
    let mut resource = data([10; 2]);
    for fighter in &mut resource.fighters {
        for frame in &mut fighter.jab.frames {
            for hit in &mut frame.hitboxes {
                hit.rebound = false;
            }
        }
    }
    // Group0 survives a resumed active frame. A later group is independent.
    resource.fighters[0].jab.frames[3].hitboxes[0].group = 1;
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert!(contact.fighters.iter().all(|f| f.action == Action::Jab));
    assert!(
        contact
            .fighters
            .iter()
            .all(|f| f.percent == 0.0 && f.hitlag > 0.0)
    );
    assert!(contact.fighters[0].clank.slots[0].victims.contains(2));
    assert!(contact.fighters[1].clank.slots[0].victims.contains(1));
    wait(&mut game, |s| s.fighters[0].hitlag == 0.0);
    let still_suppressed = step(&mut game);
    assert_eq!(
        still_suppressed.fighters.each_ref().map(|f| f.percent),
        [0.0; 2]
    );
    assert!(
        still_suppressed.fighters[0].clank.slots[0]
            .victims
            .contains(2)
    );
    let next = wait(&mut game, |s| s.fighters[1].percent != 0.0);
    assert_eq!(next.fighters[0].percent, 0.0);
    assert_eq!(next.fighters[1].percent, 10.0);
}

#[test]
fn absent_clank_flags_airborne_fighters_and_separate_volumes_do_not_rebound() {
    for case in 0..3 {
        let mut resource = data([10; 2]);
        if case == 0 {
            for fighter in &mut resource.fighters {
                for frame in &mut fighter.jab.frames {
                    for hit in &mut frame.hitboxes {
                        hit.clank = false;
                    }
                }
            }
        } else if case == 1 {
            // Identical clank-enabled volumes trade damage when both attacks
            // actually execute in the air; the grounded clash path is excluded.
            resource.stage.spawns = [[-2.0, 10.0], [2.0, 10.0]];
            let aerials = aerial_fixture::data();
            for (player, fighter) in resource.fighters.iter_mut().enumerate() {
                let mut profile = aerials.fighters[player].aerials.clone().unwrap();
                for movement in &mut profile.moves {
                    movement.attack = fighter.jab.clone();
                    movement.flags.truncate(movement.attack.frames.len());
                }
                fighter.aerials = Some(profile);
            }
        } else {
            resource.stage.spawns = [[-20.0, 0.0], [20.0, 0.0]];
        }
        let mut game = Match::new(resource, 42).unwrap();
        let contact = attack(&mut game, [true; 2]);
        assert!(
            contact
                .fighters
                .iter()
                .all(|f| { !matches!(f.action, Action::ReboundStop | Action::Rebound) })
        );
        if case <= 1 {
            assert_eq!(contact.fighters.each_ref().map(|f| f.percent), [10.0; 2]);
        }
        if case == 2 {
            assert!(contact.fighters.iter().all(|f| f.hitlag == 0.0));
        }
    }
}

#[test]
fn cached_fractional_stale_damage_changes_priority_without_clashes_staling_again() {
    let mut resource = data([20, 10]);
    resource.rules.staling = Some(stale::Rules {
        penalties: [0.075, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        debug_bypass: false,
    });
    for fighter in &mut resource.fighters {
        fighter.jab.move_id = Some(10);
    }
    let mut game = Match::new(resource, 42).unwrap();
    assert_eq!(attack(&mut game, [true, false]).fighters[1].percent, 20.0);
    recovered(&mut game);
    let queue = game.state().fighters[0].staling.queue.clone();
    let contact = attack(&mut game, [true; 2]);
    assert!(
        contact
            .fighters
            .iter()
            .all(|f| f.action == Action::ReboundStop)
    );
    assert_eq!(contact.fighters.each_ref().map(|f| f.percent), [0.0, 20.0]);
    assert_eq!(contact.fighters[0].staling.queue, queue);
    assert_eq!(contact.fighters[1].staling.queue.next(), 0);
}

#[test]
fn hitlag_retains_the_contact_pose_then_rebound_applies_acceleration_after_projection() {
    let resource = data([10; 2]);
    let expected_pose = resource.fighters[0].jab.frames[1].bones.clone();
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    let positions = contact.fighters.each_ref().map(|f| f.position);
    for _ in 0..3 {
        let frozen = step(&mut game);
        assert_eq!(frozen.fighters[0].clank.frozen_pose, expected_pose);
        assert_eq!(frozen.fighters.each_ref().map(|f| f.position), positions);
        assert_eq!(frozen.fighters[0].clank.clock, 0.0);
    }
    let launch = step(&mut game);
    assert!(launch.fighters.iter().all(|f| f.action == Action::Rebound));
    assert_eq!(launch.fighters.each_ref().map(|f| f.position), positions);
    assert_eq!(launch.fighters[0].ground_velocity, -1.0);
    assert_eq!(launch.fighters[1].ground_velocity, 1.0);
    let moving = step(&mut game);
    assert!(moving.fighters[0].position[0] < positions[0][0]);
    assert!(moving.fighters[1].position[0] > positions[1][0]);
    assert_eq!(moving.fighters[0].ground_velocity, -0.9);
    assert_eq!(moving.fighters[1].ground_velocity, 0.9);
}

#[test]
fn rebound_surface_multiplier_applies_after_low_speed_friction_clamping() {
    let mut resource = data([10; 2]);
    let rules = resource.rules.clank.as_mut().unwrap();
    rules.push_scale = 0.0;
    rules.push_base = 0.1;
    let mut game = Match::new(resource, 42).unwrap();
    attack(&mut game, [true; 2]);
    wait(&mut game, |s| s.fighters[0].hitlag == 0.0);
    let launch = step(&mut game);
    assert_eq!(launch.fighters[0].ground_velocity, -0.05);
    assert_eq!(launch.fighters[1].ground_velocity, 0.05);
    let slowed = step(&mut game);
    // Clamp .2 friction to the .05 remaining speed, then scale by .5.
    // Scaling before clamping would incorrectly stop the fighter immediately.
    assert_eq!(slowed.fighters[0].ground_velocity, -0.025);
    assert_eq!(slowed.fighters[1].ground_velocity, 0.025);
}

#[test]
fn same_group_slots_share_suppression_and_fully_disabled_groups_can_hit_again() {
    let mut resource = data([10; 2]);
    for fighter in &mut resource.fighters {
        for frame in &mut fighter.jab.frames {
            for hit in &mut frame.hitboxes {
                hit.rebound = false;
            }
        }
        fighter.jab.frames[3].hitboxes.clear();
    }
    let fighter = &mut resource.fighters[0];
    let mut body = fighter.jab.frames[1].hitboxes[0].clone();
    body.center = [4.0, 1.0, 0.0];
    body.radius = 0.5;
    body.clank = false;
    fighter.jab.frames[1].hitboxes.push(body.clone());
    fighter.jab.frames[2].hitboxes = fighter.jab.frames[1].hitboxes.clone();
    fighter.jab.frames[2].hitboxes.push(body.clone());
    fighter.jab.frames[4].hitboxes = vec![body];
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert_eq!(contact.fighters.each_ref().map(|f| f.percent), [0.0; 2]);
    assert!(
        contact.fighters[0].clank.slots[..2]
            .iter()
            .all(|slot| slot.victims.contains(2))
    );
    wait(&mut game, |s| s.fighters[0].hitlag == 0.0);
    let continued = wait(&mut game, |s| s.fighters[0].action_frame >= 3);
    assert_eq!(continued.fighters[1].percent, 0.0);
    assert!(continued.fighters[0].clank.slots[2].victims.contains(2));
    let disabled = step(&mut game);
    assert!(
        disabled.fighters[0]
            .clank
            .slots
            .iter()
            .all(|s| s.group.is_none())
    );
    let reenabled = step(&mut game);
    assert_eq!(reenabled.fighters[1].percent, 10.0);
    assert!(reenabled.events.iter().any(|e| matches!(
        e,
        Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }
    )));
}

#[test]
fn zero_priority_gap_has_no_clash_effect_and_equal_attacks_trade_body_damage() {
    let mut resource = data([10; 2]);
    resource.rules.clank.as_mut().unwrap().response.damage_gap = 0;
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert!(
        !contact
            .events
            .iter()
            .any(|e| matches!(e, Event::Clank { .. }))
    );
    assert_eq!(contact.fighters.each_ref().map(|f| f.percent), [10.0; 2]);
    assert!(contact.fighters.iter().all(|f| f.action == Action::Damage));
}

#[test]
fn zero_rebound_duration_preserves_clash_hitlag_without_starting_rebound() {
    let mut resource = data([10; 2]);
    let response = &mut resource.rules.clank.as_mut().unwrap().response;
    response.duration_scale = 0.0;
    response.duration_base = 0.0;
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert!(
        contact
            .events
            .iter()
            .any(|e| matches!(e, Event::Clank { .. }))
    );
    assert!(
        contact
            .fighters
            .iter()
            .all(|f| { f.action == Action::Jab && f.hitlag == 3.0 && f.percent == 0.0 })
    );
    recovered(&mut game);
    assert!(game.state().fighters.iter().all(|f| f.percent == 0.0));
}

#[test]
fn malformed_clank_profiles_and_incomplete_animation_resources_are_rejected() {
    for case in 0..11 {
        let mut resource = data([10; 2]);
        match case {
            0 => resource.fighters[0].rebound = None,
            1 => resource.fighters[0].rebound.as_mut().unwrap().poses.clear(),
            2 => resource.fighters[0]
                .rebound
                .as_mut()
                .unwrap()
                .poses
                .truncate(13),
            3 => resource.rules.clank.as_mut().unwrap().push_scale = f32::NAN,
            4 => {
                resource
                    .rules
                    .clank
                    .as_mut()
                    .unwrap()
                    .surface_friction_multiplier = -0.1
            }
            5 => resource.rules.clank.as_mut().unwrap().response.damage_gap = -1,
            6 => {
                resource
                    .rules
                    .clank
                    .as_mut()
                    .unwrap()
                    .response
                    .duration_base = -1.0
            }
            7 => {
                let response = &mut resource.rules.clank.as_mut().unwrap().response;
                response.duration_scale = 0.0;
                response.duration_base = f32::from_bits(1);
            }
            8 => {
                resource.rules.clank = None;
                for fighter in &mut resource.fighters {
                    fighter.rebound = None;
                }
            }
            9 => resource.fighters[0].rebound.as_mut().unwrap().poses[0].clear(),
            _ => {
                resource.fighters[0]
                    .rebound
                    .as_mut()
                    .unwrap()
                    .animation_length = f32::NAN
            }
        }
        assert!(
            Match::new(resource, 42).is_err(),
            "invalid profile case {case} accepted"
        );
    }
}

#[test]
fn a_body_hit_overrides_pending_rebound_on_the_same_collision_pass() {
    let mut resource = data([10; 2]);
    for frame in &mut resource.fighters[0].jab.frames {
        if let Some(first) = frame.hitboxes.first().cloned() {
            let mut body = first;
            body.group = 1;
            body.center = [4.0, 1.0, 0.0];
            body.radius = 0.5;
            body.clank = false;
            frame.hitboxes.push(body);
        }
    }
    let mut game = Match::new(resource, 42).unwrap();
    let contact = attack(&mut game, [true; 2]);
    assert_eq!(contact.fighters[1].action, Action::Damage);
    assert_eq!(contact.fighters[1].percent, 10.0);
    assert_eq!(contact.fighters[0].percent, 0.0);
}

#[test]
fn checkpoint_and_reset_preserve_clash_history_pose_timers_and_recovery() {
    let resource = data([10; 2]);
    let mut game = Match::new(resource.clone(), 42).unwrap();
    attack(&mut game, [true; 2]);
    let checkpoint = game.checkpoint();
    let expected: Vec<_> = (0..30)
        .map(|_| serde_json::to_string(&step(&mut game)).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    let actual: Vec<_> = (0..30)
        .map(|_| serde_json::to_string(&step(&mut game)).unwrap())
        .collect();
    assert_eq!(actual, expected);
    game.reset(42);
    let independent = Match::new(resource, 42).unwrap();
    assert_eq!(
        serde_json::to_string(game.state()).unwrap(),
        serde_json::to_string(independent.state()).unwrap()
    );
    assert!(
        attack(&mut game, [true; 2])
            .fighters
            .iter()
            .all(|f| { f.action == Action::ReboundStop && f.percent == 0.0 })
    );
}
