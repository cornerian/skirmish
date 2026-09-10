//! Static ledge discovery, bone attachment and full ledge-option lifecycle.

#[path = "support/ledge.rs"]
mod ledge_resources;

use skirmish::collision::stage::{self, Joint, Line};
use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, Controller, Event, Match, State,
    data::{AttackFrame, Hitbox, MatchData, StageGeometry},
    ledge::{self, Options, Side, SlowRules},
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -2.0;
    data.stage.floor.right = 2.0;
    data.stage.spawns = [[-1.9, 1.0], [0.0, 0.0]];
    data.stage.blast = [-20.0, 20.0, -30.0, 30.0];
    ledge_resources::profile(data)
}

fn variant_data(threshold: f32) -> MatchData {
    let mut data = data();
    let rules = data.rules.ledge.as_mut().unwrap();
    rules.wait_frames = 3;
    rules.slow = Some(SlowRules {
        percent_threshold: threshold,
        wait_frames: 7,
    });
    for fighter in &mut data.fighters {
        let parameters = fighter.ledge.as_mut().unwrap();
        let mut slow = Options {
            climb: parameters.climb.clone(),
            jump: parameters.jump.clone(),
            attack: parameters.attack.clone(),
            escape: parameters.escape.clone(),
        };
        slow.climb
            .frames
            .push(slow.climb.frames.last().unwrap().clone());
        slow.jump.release_frame = 3;
        slow.jump.launch_velocity = [1.2, 2.5];
        slow.attack
            .attack
            .frames
            .iter_mut()
            .flat_map(|frame| &mut frame.hitboxes)
            .for_each(|hitbox| hitbox.damage = 13);
        slow.escape.frames.last_mut().unwrap().anchor_offset[0] = 2.5;
        parameters.slow = Some(slow);
    }
    data
}

fn input(player: usize, buttons: u16, stick: [f32; 2], cstick: [f32; 2]) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = Controller {
        buttons,
        stick,
        cstick,
        trigger: 0.0,
    };
    inputs
}

fn step(game: &mut Match, inputs: [Controller; 2]) -> State {
    game.step(inputs).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..120 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game, IDLE);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn hanging(mut resource: MatchData) -> Match {
    resource.stage.spawns[0] = [-1.9, 1.0];
    let mut game = Match::new(resource, 7).unwrap();
    let caught = step(&mut game, input(0, 0, [-1.0, 0.0], [0.0; 2]));
    assert!(caught.events.contains(&Event::LedgeCaught {
        player: 0,
        line: 0,
        side: Side::Left,
    }));
    until(&mut game, |state| {
        state.fighters[0].action == Action::CliffWait
    });
    // A neutral sample arms the source stick-region gate.
    step(&mut game, IDLE);
    game
}

#[test]
fn descending_fighter_catches_each_free_endpoint_and_bone_anchor_stays_attached() {
    for (spawn, stick, side, facing, expected_x) in [
        ([-1.9, 1.0], [-1.0, 0.0], Side::Left, 1.0, -2.35),
        ([1.9, 1.0], [1.0, 0.0], Side::Right, -1.0, 2.35),
    ] {
        let mut resource = data();
        resource.stage.spawns[0] = spawn;
        let mut game = Match::new(resource, 0).unwrap();
        let caught = step(&mut game, input(0, 0, stick, [0.0; 2]));
        assert!(caught.events.contains(&Event::LedgeCaught {
            player: 0,
            line: 0,
            side,
        }));
        let wait = until(&mut game, |state| {
            state.fighters[0].action == Action::CliffWait
        });
        let fighter = &wait.fighters[0];
        assert_eq!(fighter.ledge.line, Some(0));
        assert_eq!(fighter.ledge.side, Some(side));
        assert_eq!(fighter.facing, facing);
        assert_eq!(fighter.velocity, [0.0; 2]);
        assert_eq!(fighter.knockback, [0.0; 2]);
        assert!((fighter.position[0] - expected_x).abs() < 0.000_01);
        assert!((fighter.position[1] + 0.7).abs() < 0.000_01);
    }
}

#[test]
fn grounded_ascending_down_held_and_occupied_ledge_cases_are_rejected() {
    let mut grounded_data = data();
    grounded_data.stage.spawns[0] = [-1.9, 0.0];
    let mut grounded = Match::new(grounded_data.clone(), 0).unwrap();
    let state = step(&mut grounded, IDLE);
    assert!(state.fighters[0].ledge.line.is_none());

    let mut held_down = Match::new(data(), 0).unwrap();
    let state = step(&mut held_down, input(0, 0, [-1.0, -1.0], [0.0; 2]));
    assert!(state.fighters[0].ledge.line.is_none());

    let mut occupied_data = data();
    occupied_data.stage.spawns = [[-1.9, 1.0]; 2];
    let mut occupied = Match::new(occupied_data, 0).unwrap();
    let state = step(
        &mut occupied,
        [Controller {
            stick: [-1.0, 0.0],
            ..Controller::default()
        }; 2],
    );
    assert_eq!(state.fighters[0].ledge.line, Some(0));
    assert!(state.fighters[1].ledge.line.is_none());
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| matches!(event, Event::LedgeCaught { .. }))
            .count(),
        1
    );
}

#[test]
fn ascending_fighter_does_not_snap_to_a_nearby_ledge() {
    let mut resource = data();
    resource.stage.spawns[0] = [-1.9, 0.0];
    let mut game = Match::new(resource, 0).unwrap();
    for _ in 0..8 {
        let state = step(&mut game, input(0, BUTTON_X, [-1.0, 0.0], [0.0; 2]));
        if !state.fighters[0].grounded {
            assert!(state.fighters[0].velocity[1] > 0.0);
            assert!(state.fighters[0].ledge.line.is_none());
            assert_ne!(state.fighters[0].action, Action::CliffCatch);
            return;
        }
    }
    panic!("jump did not launch");
}

#[test]
fn only_marked_unconnected_visible_floor_endpoints_can_be_caught() {
    let seam = StageGeometry {
        lines: vec![
            Line {
                start: [-2.0, 0.0],
                end: [0.0, 0.0],
                flags: stage::FLOOR | stage::ENABLED,
                material_flags: stage::LEDGE as u16,
                next: [Some(1), None],
                ..Line::default()
            },
            Line {
                start: [0.0, 0.0],
                end: [2.0, 0.0],
                flags: stage::FLOOR | stage::ENABLED,
                material_flags: stage::LEDGE as u16,
                previous: [Some(0), None],
                ..Line::default()
            },
        ],
        joints: vec![Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-2.0, 0.0],
            bounds_max: [2.0, 0.0],
            floor: 0..2,
            ..Joint::default()
        }],
    };
    let mut connected = data();
    connected.stage.geometry = Some(seam);
    connected.stage.spawns[0] = [-0.1, 1.0];
    let mut game = Match::new(connected, 0).unwrap();
    let state = step(&mut game, IDLE);
    assert!(state.fighters[0].ledge.line.is_none());

    for (flags, material_flags) in [
        (
            stage::FLOOR | stage::ENABLED | stage::HIDDEN,
            stage::LEDGE as u16,
        ),
        (stage::FLOOR | stage::ENABLED, 0),
    ] {
        let mut resource = data();
        resource.stage.geometry = Some(StageGeometry {
            lines: vec![Line {
                start: [-2.0, 0.0],
                end: [2.0, 0.0],
                flags,
                material_flags,
                ..Line::default()
            }],
            joints: vec![Joint {
                id: 0,
                flags: stage::ENABLED,
                bounds_min: [-2.0, 0.0],
                bounds_max: [2.0, 0.0],
                floor: 0..1,
                ..Joint::default()
            }],
        });
        let mut game = Match::new(resource, 0).unwrap();
        let state = step(&mut game, input(0, 0, [-1.0, 0.0], [0.0; 2]));
        assert!(state.fighters[0].ledge.line.is_none());
    }
}

#[test]
fn button_priority_and_main_vs_c_stick_gates_match_the_ledge_dispatch_graph() {
    let mut priority = hanging(data());
    let state = step(
        &mut priority,
        input(0, BUTTON_A | BUTTON_L | BUTTON_X, [0.0, -1.0], [0.0, -1.0]),
    );
    assert_eq!(state.fighters[0].action, Action::CliffAttack);

    let mut c_climb = hanging(data());
    let state = step(&mut c_climb, input(0, 0, [0.0; 2], [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::CliffWait);

    let mut c_drop = hanging(data());
    let state = step(&mut c_drop, input(0, 0, [0.0; 2], [0.0, -1.0]));
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert!(state.fighters[0].ledge.line.is_none());
}

#[test]
fn climb_attack_and_escape_apply_supplied_root_motion_then_finish_grounded() {
    for (buttons, stick, action, minimum_x) in [
        (0, [1.0, 0.0], Action::CliffClimb, -1.21),
        (BUTTON_A, [0.0; 2], Action::CliffAttack, -1.21),
        (BUTTON_L, [0.0; 2], Action::CliffEscape, -0.41),
    ] {
        let mut game = hanging(data());
        let entered = step(&mut game, input(0, buttons, stick, [0.0; 2]));
        assert_eq!(entered.fighters[0].action, action);
        let finished = until(&mut game, |state| state.fighters[0].action == Action::Wait);
        let fighter = &finished.fighters[0];
        assert!(fighter.grounded);
        assert_eq!(fighter.ground_line, Some(0));
        assert!(fighter.ledge.line.is_none());
        assert!(fighter.position[0] >= minimum_x);
        assert!((fighter.position[1] - 0.0001).abs() < 0.000_01);
    }
}

#[test]
fn ledge_jump_detaches_on_its_explicit_frame_and_launches_inward_and_up() {
    let mut game = hanging(data());
    let entered = step(&mut game, input(0, BUTTON_X, [0.0; 2], [0.0; 2]));
    assert_eq!(entered.fighters[0].action, Action::CliffJump);
    let released = until(&mut game, |state| state.fighters[0].ledge.line.is_none());
    let fighter = &released.fighters[0];
    assert_eq!(fighter.action, Action::CliffJump);
    assert_eq!(fighter.velocity, [0.8, 2.0]);
    assert!(fighter.position[0] > -2.0);
    assert!(fighter.position[1] > -0.7);
    assert_eq!(fighter.ledge.cooldown, 6);
    until(&mut game, |state| state.fighters[0].action == Action::Fall);
}

#[test]
fn ledge_attack_uses_its_bone_pose_and_shared_damage_pipeline() {
    let mut resource = data();
    resource.stage.spawns[1] = [-0.4, 0.0];
    let mut game = hanging(resource);
    step(&mut game, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let hit = until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert_eq!(hit.fighters[1].percent, 7.0);
    assert!(hit.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            damage: 7.0,
            ..
        }
    )));
}

#[test]
fn an_opponent_hit_releases_ledge_ownership_before_entering_damage() {
    let mut resource = data();
    resource.rules.ledge.as_mut().unwrap().intangibility_frames = 0;
    resource.stage.spawns[1] = [-1.0, 0.0];
    let bones = resource.fighters[1].bones.clone();
    resource.fighters[1].jab.frames = vec![
        AttackFrame {
            bones: bones.clone(),
            hitboxes: vec![],
            hurtbox_states: vec![],
        },
        AttackFrame {
            bones,
            hitboxes: vec![Hitbox {
                clank: false,
                rebound: false,
                element: Default::default(),
                group: 0,
                bone: 0,
                center: [1.3, 0.0, 0.0],
                radius: 0.8,
                damage: 5,
                shield_damage: 0,
                angle_degrees: 45.0,
                growth: 50,
                fixed: 0,
                base: 30,
            }],
            hurtbox_states: vec![],
        },
        AttackFrame {
            bones: resource.fighters[1].bones.clone(),
            hitboxes: vec![],
            hurtbox_states: vec![],
        },
    ];
    let mut game = hanging(resource);
    step(&mut game, input(1, BUTTON_A, [0.0; 2], [0.0; 2]));
    let damaged = until(&mut game, |state| state.fighters[0].percent > 0.0);
    assert_eq!(damaged.fighters[0].action, Action::Damage);
    assert_eq!(damaged.fighters[0].ledge.line, None);
    assert_eq!(damaged.fighters[0].ledge.side, None);
    assert_eq!(damaged.fighters[0].ledge.cooldown, 6);
}

#[test]
fn drop_cooldown_is_checkpointed_and_blocks_then_allows_regrab() {
    let mut game = hanging(data());
    let dropped = step(&mut game, input(0, 0, [0.0, -1.0], [0.0; 2]));
    assert_eq!(dropped.fighters[0].action, Action::Fall);
    assert_eq!(dropped.fighters[0].ledge.cooldown, 6);
    let checkpoint = game.checkpoint();
    let expected: Vec<_> = (0..6).map(|_| step(&mut game, IDLE)).collect();
    assert!(
        expected[..5]
            .iter()
            .all(|state| state.fighters[0].ledge.line.is_none())
    );
    assert_eq!(expected[5].fighters[0].ledge.line, Some(0));

    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut game, IDLE), expected);
    }
}

#[test]
fn wait_timeout_drops_and_blast_exit_clears_ledge_ownership() {
    let mut timeout_data = data();
    timeout_data.rules.ledge.as_mut().unwrap().wait_frames = 2;
    let mut timeout = hanging(timeout_data);
    let dropped = until(&mut timeout, |state| {
        state.fighters[0].action == Action::Fall
    });
    assert!(dropped.fighters[0].ledge.line.is_none());
    assert_eq!(dropped.fighters[0].ledge.cooldown, 6);

    let mut ko_data = data();
    ko_data.stage.blast[0] = -2.2;
    let mut ko = Match::new(ko_data, 0).unwrap();
    let state = step(&mut ko, input(0, 0, [-1.0, 0.0], [0.0; 2]));
    assert!(state.events.contains(&Event::Knockout {
        player: 0,
        stocks: 1,
    }));
    assert_eq!(state.fighters[0].ledge, ledge::State::default());
    assert_eq!(state.fighters[0].action, Action::Respawn);
}

#[test]
fn percent_boundary_selects_and_checkpoints_the_slow_wait_timer() {
    let mut quick = hanging(variant_data(1.0));
    assert!(!quick.state().fighters[0].ledge.slow);
    let mut quick_steps = 0;
    while quick.state().fighters[0].action == Action::CliffWait {
        step(&mut quick, IDLE);
        quick_steps += 1;
    }

    let mut slow = hanging(variant_data(0.0));
    assert!(slow.state().fighters[0].ledge.slow);
    let checkpoint = slow.checkpoint();
    let mut expected = Vec::new();
    while slow.state().fighters[0].action == Action::CliffWait {
        expected.push(step(&mut slow, IDLE));
    }
    assert_eq!(expected.len() - quick_steps, 4);
    assert_eq!(expected.last().unwrap().fighters[0].action, Action::Fall);
    assert!(!expected.last().unwrap().fighters[0].ledge.slow);

    slow.restore_checkpoint(&checkpoint).unwrap();
    for expected in expected {
        assert_eq!(step(&mut slow, IDLE), expected);
    }
}

#[test]
fn slow_climb_jump_attack_and_escape_use_the_selected_physics_resources() {
    let mut climb = hanging(variant_data(0.0));
    let entered = step(&mut climb, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(entered.fighters[0].action, Action::CliffClimb);
    assert!(entered.fighters[0].ledge.slow);
    let finished = until(&mut climb, |state| state.fighters[0].action == Action::Wait);
    assert!(finished.fighters[0].grounded);
    assert!(!finished.fighters[0].ledge.slow);

    let mut jump = hanging(variant_data(0.0));
    step(&mut jump, input(0, BUTTON_X, [0.0; 2], [0.0; 2]));
    let released = until(&mut jump, |state| state.fighters[0].ledge.line.is_none());
    assert_eq!(released.fighters[0].action, Action::CliffJump);
    assert_eq!(released.fighters[0].velocity, [1.2, 2.5]);
    assert!(released.fighters[0].ledge.slow);
    let falling = until(&mut jump, |state| state.fighters[0].action == Action::Fall);
    assert!(!falling.fighters[0].ledge.slow);

    let mut attack_data = variant_data(0.0);
    attack_data.stage.spawns[1] = [-0.4, 0.0];
    let mut attack = hanging(attack_data);
    step(&mut attack, input(0, BUTTON_A, [0.0; 2], [0.0; 2]));
    let hit = until(&mut attack, |state| state.fighters[1].percent > 0.0);
    assert_eq!(hit.fighters[1].percent, 13.0);

    let mut escape = hanging(variant_data(0.0));
    step(&mut escape, input(0, BUTTON_L, [0.0; 2], [0.0; 2]));
    let escaped = until(&mut escape, |state| {
        state.fighters[0].action == Action::Wait
    });
    assert!(escaped.fighters[0].position[0] > 0.4);
}

#[test]
fn malformed_or_unpaired_ledge_resources_are_rejected() {
    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules.ledge.as_mut().unwrap().option_stick_threshold = 0.0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].ledge = None;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].ledge.as_mut().unwrap().jump.release_frame = 0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .ledge
        .as_mut()
        .unwrap()
        .attack
        .anchor_offsets
        .pop();
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].ledge.as_mut().unwrap().catch.frames[0].bones[1].parent = None;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].ledge.as_mut().unwrap().attachment.bone = usize::MAX;
    cases.push(bad);
    let mut bad = variant_data(0.0);
    bad.fighters[0].ledge.as_mut().unwrap().slow = None;
    cases.push(bad);
    let mut bad = variant_data(0.0);
    bad.rules
        .ledge
        .as_mut()
        .unwrap()
        .slow
        .as_mut()
        .unwrap()
        .percent_threshold = f32::NAN;
    cases.push(bad);
    let mut bad = variant_data(0.0);
    let slow = bad.fighters[0]
        .ledge
        .as_mut()
        .unwrap()
        .slow
        .as_mut()
        .unwrap();
    slow.jump.release_frame = slow.jump.motion.frames.len() as u32;
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
