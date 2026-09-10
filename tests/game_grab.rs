//! Native bone-driven grab, capture, and four-direction throw scenarios.

#[path = "support/grab.rs"]
mod grab_resources;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_X, BUTTON_Z, Controller, Event, Match, State, data::MatchData, grab,
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
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    grab_resources::profile(data)
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

fn held(mut data: MatchData) -> Match {
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    let mut game = Match::new(data, 7).unwrap();
    let caught = step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert!(caught.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    }));
    until(&mut game, |state| {
        state.fighters[0].action == Action::CatchWait
    });
    game
}

#[test]
fn catch_contact_uses_the_sampled_bone_pose_and_miss_recovers() {
    let mut animated = data();
    animated.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let parameters = animated.fighters[0].grab.as_mut().unwrap();
    for frame in &mut parameters.catch.frames {
        frame.bones[1].translation[0] = 2.0;
        if let Some(grabbox) = frame.grabboxes.first_mut() {
            grabbox.start = [0.0; 3];
            grabbox.end = [0.0; 3];
            grabbox.radius = 0.1;
        }
    }
    let mut reaches = Match::new(animated.clone(), 0).unwrap();
    let caught = step(&mut reaches, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert_eq!(caught.fighters[0].grab.victim, Some(1));

    for frame in &mut animated.fighters[0].grab.as_mut().unwrap().catch.frames {
        frame.bones[1].translation[0] = 0.0;
    }
    let mut misses = Match::new(animated, 0).unwrap();
    step(&mut misses, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    for _ in 0..8 {
        step(&mut misses, IDLE);
    }
    assert_eq!(misses.state().fighters[0].action, Action::Wait);
    assert!(
        misses
            .state()
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
}

#[test]
fn all_four_throw_directions_release_into_the_shared_damage_pipeline() {
    let cases = [
        ([1.0, 0.0], Action::ThrowF, Action::ThrownF, [1, 0]),
        ([-1.0, 0.0], Action::ThrowB, Action::ThrownB, [-1, 0]),
        ([0.0, 1.0], Action::ThrowHi, Action::ThrownHi, [0, 1]),
        ([0.0, -1.0], Action::ThrowLw, Action::ThrownLw, [0, -1]),
    ];
    for (stick, holder_action, victim_action, direction) in cases {
        let mut game = held(data());
        let entered = step(&mut game, input(0, 0, stick, [0.0; 2]));
        assert_eq!(entered.fighters[0].action, holder_action);
        assert_eq!(entered.fighters[1].action, victim_action);
        let released = until(&mut game, |state| state.fighters[1].percent > 0.0);
        assert_eq!(released.fighters[1].percent, 8.0);
        assert_eq!(released.fighters[1].facing, -1.0);
        assert!(
            released
                .fighters
                .iter()
                .all(|fighter| fighter.grab == grab::State::default())
        );
        if direction[0] != 0 {
            assert_eq!(
                released.fighters[1].knockback[0].signum(),
                direction[0] as f32
            );
        }
        if direction[1] != 0 {
            assert_eq!(
                released.fighters[1].knockback[1].signum(),
                direction[1] as f32
            );
        }
        assert!(released.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                damage: 8.0,
                ..
            }
        )));
    }
}

#[test]
fn cstick_throws_and_source_priority_select_main_horizontal_first() {
    let mut cstick = held(data());
    let entered = step(&mut cstick, input(0, 0, [0.0; 2], [0.0, 1.0]));
    assert_eq!(entered.fighters[0].action, Action::ThrowHi);

    let mut priority = held(data());
    let entered = step(&mut priority, input(0, 0, [-1.0, -1.0], [1.0, 1.0]));
    assert_eq!(entered.fighters[0].action, Action::ThrowB);
}

#[test]
fn held_stick_does_not_become_a_fresh_throw_when_pull_enters_wait() {
    let mut game = Match::new(data(), 0).unwrap();
    step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    let waiting = step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(waiting.fighters[0].action, Action::CatchWait);
    step(&mut game, input(0, 0, [0.0; 2], [0.0; 2]));
    let fresh = step(&mut game, input(0, 0, [1.0, 0.0], [0.0; 2]));
    assert_eq!(fresh.fighters[0].action, Action::ThrowF);
}

#[test]
fn capture_ignores_victim_actions_and_checkpoint_replays_the_pair_exactly() {
    let mut game = held(data());
    let held_position = game.state().fighters[1].position;
    for buttons in [BUTTON_A, BUTTON_X, BUTTON_A | BUTTON_X] {
        let state = step(&mut game, input(1, buttons, [1.0, 1.0], [1.0, 1.0]));
        assert_eq!(state.fighters[1].action, Action::CaptureWait);
        assert_eq!(state.fighters[1].position, held_position);
        assert_eq!(state.fighters[1].grab.captor, Some(0));
    }

    let checkpoint = game.checkpoint();
    let suffix = [input(0, 0, [1.0, 0.0], [0.0; 2]), IDLE, IDLE, IDLE, IDLE];
    let expected = suffix.map(|inputs| step(&mut game, inputs));
    game.restore_checkpoint(&checkpoint).unwrap();
    for (inputs, expected) in suffix.into_iter().zip(expected) {
        assert_eq!(step(&mut game, inputs), expected);
    }
}

#[test]
fn simultaneous_catches_resolve_in_stable_player_order() {
    let mut game = Match::new(data(), 0).unwrap();
    let inputs = [Controller {
        buttons: BUTTON_Z,
        ..Controller::default()
    }; 2];
    let state = step(&mut game, inputs);
    assert_eq!(state.fighters[0].grab.victim, Some(1));
    assert_eq!(state.fighters[1].grab.captor, Some(0));
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| matches!(event, Event::Grabbed { .. }))
            .count(),
        1
    );
}

#[test]
fn grounded_only_catch_rejects_airborne_targets() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [1.0, 4.0]];
    let mut game = Match::new(resource, 0).unwrap();
    step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    for _ in 0..5 {
        step(&mut game, IDLE);
    }
    assert!(
        game.state()
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
}

#[test]
fn blast_exit_breaks_the_pair_before_stock_loss_is_published() {
    let mut resource = data();
    resource.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .attachment
        .holder_point[0] = 100.0;
    let mut game = Match::new(resource, 0).unwrap();
    let state = step(&mut game, input(0, BUTTON_Z, [0.0; 2], [0.0; 2]));
    assert!(state.events.contains(&Event::Knockout {
        player: 1,
        stocks: 1,
    }));
    assert!(
        state
            .fighters
            .iter()
            .all(|fighter| fighter.grab == grab::State::default())
    );
    assert_eq!(state.fighters[0].action, Action::Wait);
    assert_eq!(state.fighters[1].action, Action::Respawn);
}

#[test]
fn malformed_grab_resources_are_rejected() {
    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules.grab.as_mut().unwrap().down_threshold = 0.0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0].grab.as_mut().unwrap().catch.frames[0]
        .grabboxes
        .clear();
    for frame in &mut bad.fighters[0].grab.as_mut().unwrap().catch.frames {
        frame.grabboxes.clear();
    }
    cases.push(bad);
    let mut bad = data();
    bad.fighters[0]
        .grab
        .as_mut()
        .unwrap()
        .throws
        .forward
        .release_frame = 0;
    cases.push(bad);
    let mut bad = data();
    bad.fighters[1].grab = None;
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
