//! Headless neutral-special dispatch, animation, combat and terrain transitions.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/special.rs"]
mod special_resources;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_B, BUTTON_L, BUTTON_Z, Controller, Event, Match, data::MatchData,
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    special_resources::profile(conformance::data())
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn special(stick: [f32; 2]) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick,
        ..Controller::default()
    }
}

#[test]
fn both_slots_use_fresh_strict_neutral_b_with_special_priority_and_rearming() {
    for player in 0..2 {
        for competing in [BUTTON_A, BUTTON_Z, BUTTON_L] {
            let mut game = Match::new(data(), 0).unwrap();
            let state = game
                .step(input(
                    player,
                    Controller {
                        buttons: BUTTON_B | competing,
                        stick: [0.599, -0.599],
                        ..Controller::default()
                    },
                ))
                .unwrap();
            assert_eq!(state.fighters[player].action, Action::SpecialN);
        }

        let mut game = Match::new(data(), 0).unwrap();
        assert_ne!(
            game.step(input(player, special([0.6, 0.0])))
                .unwrap()
                .fighters[player]
                .action,
            Action::SpecialN
        );

        let mut game = Match::new(data(), 0).unwrap();
        assert_eq!(
            game.step(input(player, special([0.0; 2])))
                .unwrap()
                .fighters[player]
                .action,
            Action::SpecialN
        );
        for _ in 0..12 {
            game.step(input(player, special([0.0; 2]))).unwrap();
        }
        assert_eq!(game.state().fighters[player].action, Action::Wait);
        game.step(IDLE).unwrap();
        assert_eq!(
            game.step(input(player, special([0.0; 2])))
                .unwrap()
                .fighters[player]
                .action,
            Action::SpecialN
        );
    }
}

#[test]
fn grounded_special_uses_sampled_bones_and_the_shared_damage_pipeline() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, special([0.0; 2]))).unwrap();
    let hit = game.step(IDLE).unwrap();
    assert_eq!(hit.fighters[0].action, Action::SpecialN);
    assert_eq!(hit.fighters[1].percent, 10.0);
    assert!(hit.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            damage: 10.0,
            ..
        }
    )));
}

#[test]
fn aerial_special_keeps_air_physics_and_landing_preserves_the_animation_frame() {
    let mut resource = data();
    for fighter in &mut resource.fighters {
        let parameters = fighter.special.as_mut().unwrap();
        while parameters.air.frames.len() < 24 {
            parameters
                .air
                .frames
                .push(parameters.air.frames.last().unwrap().clone());
            parameters
                .ground
                .frames
                .push(parameters.ground.frames.last().unwrap().clone());
        }
    }
    resource.stage.spawns[0][1] = 2.0;
    let mut special_game = Match::new(resource.clone(), 0).unwrap();
    let mut falling = Match::new(resource, 0).unwrap();
    let started = special_game.step(input(0, special([0.0; 2]))).unwrap();
    assert_eq!(started.fighters[0].action, Action::SpecialAirN);
    falling.step(IDLE).unwrap();
    assert_eq!(
        started.fighters[0].position,
        falling.state().fighters[0].position
    );
    assert_eq!(
        started.fighters[0].velocity,
        falling.state().fighters[0].velocity
    );

    for _ in 0..20 {
        let before = special_game.state().fighters[0].action_frame;
        let state = special_game.step(IDLE).unwrap();
        if state.fighters[0].grounded {
            assert_eq!(state.fighters[0].action, Action::SpecialN);
            assert_eq!(state.fighters[0].action_frame, before + 1);
            assert!(state.events.contains(&Event::Landed { player: 0 }));
            return;
        }
    }
    panic!("aerial special did not reach the floor");
}

#[test]
fn checkpoint_replays_special_contact_and_completion_exactly() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [2.0, 0.0]];
    let mut game = Match::new(resource, 17).unwrap();
    game.step(input(0, special([0.0; 2]))).unwrap();
    let checkpoint = game.checkpoint();
    let suffix = [IDLE, IDLE, IDLE, IDLE, IDLE];
    let expected = suffix
        .into_iter()
        .map(|inputs| game.step(inputs).unwrap().clone())
        .collect::<Vec<_>>();
    game.restore_checkpoint(&checkpoint).unwrap();
    for (inputs, state) in suffix.into_iter().zip(expected) {
        assert_eq!(game.step(inputs).unwrap(), &state);
    }
}

#[test]
fn malformed_special_resources_are_rejected() {
    let mut bad = data();
    bad.fighters[0].special.as_mut().unwrap().neutral_thresholds[0] = 0.0;
    assert!(Match::new(bad, 0).is_err());

    let mut bad = data();
    bad.fighters[0].special.as_mut().unwrap().air.frames.pop();
    assert!(Match::new(bad, 0).is_err());

    let mut bad = data();
    bad.fighters[0]
        .special
        .as_mut()
        .unwrap()
        .ground
        .frames
        .clear();
    bad.fighters[0].special.as_mut().unwrap().air.frames.clear();
    assert!(Match::new(bad, 0).is_err());
}
