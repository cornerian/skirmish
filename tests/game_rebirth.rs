//! Headless stock-loss, rebirth-platform travel, wait and release scenarios.

#[path = "support/rebirth.rs"]
mod rebirth_resources;

use skirmish::game::{
    Action, BUTTON_A, Controller, Event, Match, State,
    data::{AttackFrame, Hitbox, MatchData},
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
    data.stage.floor.left = -5.0;
    data.stage.floor.right = 5.0;
    data.stage.spawns = [[4.0, 0.0], [-4.0, 0.0]];
    data.stage.blast = [-8.0, 6.0, -20.0, 20.0];
    rebirth_resources::profile(data)
}

fn input(player: usize, buttons: u16, stick: [f32; 2]) -> [Controller; 2] {
    let mut input = IDLE;
    input[player] = Controller {
        buttons,
        stick,
        ..Controller::default()
    };
    input
}

fn step(game: &mut Match, input: [Controller; 2]) -> State {
    game.step(input).unwrap().clone()
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

fn reborn(resource: MatchData, player: usize) -> Match {
    let mut game = Match::new(resource, 0).unwrap();
    let direction = if player == 0 { 0.5 } else { -0.5 };
    for _ in 0..80 {
        let state = step(&mut game, input(player, 0, [direction, 0.0]));
        if state.events.contains(&Event::Respawned { player }) {
            assert_eq!(state.fighters[player].action, Action::Rebirth);
            return game;
        }
    }
    panic!("stock loss never reached rebirth: {:?}", game.state());
}

#[test]
fn stock_loss_enters_airborne_rebirth_with_reset_state_and_inward_facing() {
    let resource = data();
    let rules = resource.rules.rebirth.unwrap();
    for player in 0..2 {
        let mut game = reborn(resource.clone(), player);
        let fighter = &game.state().fighters[player];
        assert_eq!(fighter.position, rules.entry_positions[player]);
        assert_eq!(fighter.action, Action::Rebirth);
        assert!(!fighter.grounded);
        assert_eq!(fighter.ground_line, None);
        assert_eq!(fighter.percent, 0.0);
        assert_eq!(fighter.stocks, 1);
        assert_eq!(fighter.facing, if player == 0 { 1.0 } else { -1.0 });
        assert!(fighter.invincibility > 0);
        assert_eq!(fighter.locomotion.jumps_used, 0);
        assert!(game.state().events.contains(&Event::Respawned { player }));

        // The event is one frame only.
        assert!(
            !step(&mut game, IDLE)
                .events
                .contains(&Event::Respawned { player })
        );
    }
}

#[test]
fn travel_uses_remaining_frame_velocity_and_arrives_exactly_at_the_platform() {
    let resource = data();
    let rules = resource.rules.rebirth.unwrap();
    let mut game = reborn(resource, 0);
    let mut current = rules.entry_positions[0];
    for remaining in (1..=rules.travel_frames).rev() {
        let expected_velocity = skirmish::fighter::rebirth::approach_velocity(
            current,
            rules.platform_positions[0],
            remaining,
        );
        let state = step(&mut game, IDLE);
        assert_eq!(state.fighters[0].velocity, expected_velocity);
        current = [
            current[0] + expected_velocity[0],
            current[1] + expected_velocity[1],
        ];
        assert_eq!(state.fighters[0].position, current);
        assert_eq!(state.fighters[0].action, Action::Rebirth);
    }
    assert_eq!(current, rules.platform_positions[0]);
    let waiting = step(&mut game, IDLE);
    assert_eq!(waiting.fighters[0].action, Action::RebirthWait);
    assert_eq!(waiting.fighters[0].position, rules.platform_positions[0]);
    assert_eq!(waiting.fighters[0].velocity, [0.0; 2]);
    assert!(!waiting.fighters[0].grounded);
}

#[test]
fn timeout_and_input_release_to_fall_with_post_platform_invincibility() {
    let resource = data();
    let wait_frames = resource.rules.rebirth.unwrap().wait_frames;
    let post_frames = resource.rules.respawn_invincibility_frames;
    let mut timeout = reborn(resource.clone(), 0);
    until(&mut timeout, |state| {
        state.fighters[0].action == Action::RebirthWait
    });
    for _ in 0..wait_frames {
        if timeout.state().fighters[0].action != Action::RebirthWait {
            break;
        }
        step(&mut timeout, IDLE);
    }
    let falling = until(&mut timeout, |state| {
        state.fighters[0].action == Action::Fall
    });
    assert!(!falling.fighters[0].grounded);
    assert!(falling.fighters[0].position[1] < 6.0);
    assert_eq!(falling.fighters[0].invincibility, post_frames - 1);

    let mut early = reborn(resource, 0);
    until(&mut early, |state| {
        state.fighters[0].action == Action::RebirthWait
    });
    let falling = step(&mut early, input(0, BUTTON_A, [0.0; 2]));
    assert_eq!(falling.fighters[0].action, Action::Fall);
    assert_eq!(falling.fighters[0].invincibility, post_frames - 1);

    let mut stick = reborn(data(), 0);
    until(&mut stick, |state| {
        state.fighters[0].action == Action::RebirthWait
    });
    let falling = step(&mut stick, input(0, 0, [0.3, 0.0]));
    assert_eq!(falling.fighters[0].action, Action::Fall);

    for release in [
        Controller {
            cstick: [-0.3, 0.0],
            ..Controller::default()
        },
        Controller {
            trigger: 0.1,
            ..Controller::default()
        },
    ] {
        let mut game = reborn(data(), 0);
        until(&mut game, |state| {
            state.fighters[0].action == Action::RebirthWait
        });
        let mut inputs = IDLE;
        inputs[0] = release;
        assert_eq!(step(&mut game, inputs).fighters[0].action, Action::Fall);
    }

    let mut below = reborn(data(), 0);
    until(&mut below, |state| {
        state.fighters[0].action == Action::RebirthWait
    });
    assert_eq!(
        step(&mut below, input(0, 0, [0.299, 0.0])).fighters[0].action,
        Action::RebirthWait
    );
}

#[test]
fn rebirth_actions_cannot_be_hit_or_leave_the_platform_during_wait() {
    let mut resource = data();
    resource.stage.spawns[1] = [0.0, 0.0];
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
                group: 0,
                bone: 0,
                center: [2.0, 6.0, 0.0],
                radius: 2.0,
                damage: 9,
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
    let mut game = reborn(resource, 0);
    until(&mut game, |state| {
        state.fighters[0].action == Action::RebirthWait
    });
    let platform = game.state().fighters[0].position;
    step(&mut game, input(1, BUTTON_A, [0.0; 2]));
    let attacked = step(&mut game, IDLE);
    assert_eq!(attacked.fighters[0].percent, 0.0);
    assert_eq!(attacked.fighters[0].position, platform);
    assert_eq!(attacked.fighters[0].action, Action::RebirthWait);
}

#[test]
fn travel_and_release_inputs_replay_identically_from_a_checkpoint() {
    let mut game = reborn(data(), 0);
    step(&mut game, IDLE);
    let checkpoint = game.checkpoint();
    let suffix = [IDLE, IDLE, IDLE, input(0, BUTTON_A, [0.0; 2]), IDLE];
    let expected = suffix.map(|input| step(&mut game, input));
    game.restore_checkpoint(&checkpoint).unwrap();
    for (input, expected) in suffix.into_iter().zip(expected) {
        assert_eq!(step(&mut game, input), expected);
    }
}

#[test]
fn malformed_rebirth_resources_are_rejected() {
    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules.rebirth.as_mut().unwrap().travel_frames = 0;
    cases.push(bad);
    let mut bad = data();
    bad.rules.rebirth.as_mut().unwrap().wait_frames = 0;
    cases.push(bad);
    let mut bad = data();
    bad.rules.rebirth.as_mut().unwrap().release_stick_threshold = 0.0;
    cases.push(bad);
    let mut bad = data();
    bad.rules.rebirth.as_mut().unwrap().entry_positions[0][1] = f32::NAN;
    cases.push(bad);
    let mut bad = data();
    bad.rules.rebirth.as_mut().unwrap().platform_positions[1][0] = f32::INFINITY;
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
