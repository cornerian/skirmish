//! Composition of ordinary aerial and shield callbacks using explicit synthetic
//! resources. No character numerics or full-game equivalence are asserted.
#[path = "support/aerial.rs"]
mod support;

use skirmish::{
    fighter::stale,
    game::{Action, BUTTON_A, BUTTON_L, BUTTON_X, Controller, Event, Match},
};

fn shoulder() -> Controller {
    Controller {
        buttons: BUTTON_L,
        ..Default::default()
    }
}

// ftColl_80076CBC shield contact consumes the aerial's cached damage, freezes
// both actors, and does not call the hurt-contact stale queue insertion path.
#[test]
fn an_airborne_aerial_hits_grounded_shield_without_percent_or_stale_queue_changes() {
    let mut data = support::data();
    data.stage.spawns = [[-2.0, 2.0], [2.0, 0.0]];
    data.rules.hitlag.base = 3.0;
    data.rules.hitlag.damage_scale = 0.0;
    data.rules.staling = Some(stale::Rules {
        penalties: [0.1; 9],
        debug_bypass: false,
    });
    for fighter in &mut data.fighters {
        fighter.jab.move_id = Some(10);
    }
    let mut hit = data.fighters[0].jab.frames[1].hitboxes[0].clone();
    hit.center = [0.0; 3];
    hit.radius = 0.25;
    let movement = &mut data.fighters[0].aerials.as_mut().unwrap().moves[0];
    for frame in &mut movement.attack.frames[..2] {
        frame.bones[1].translation = [4.0, -0.4, 0.0];
        frame.hitboxes = vec![hit.clone()];
    }
    let mut game = Match::new(data, 7).unwrap();
    let held = [Controller::default(), shoulder()];
    assert_eq!(game.step(held).unwrap().fighters[1].action, Action::GuardOn);
    let checkpoint = game.checkpoint();
    let queues = game
        .state()
        .fighters
        .each_ref()
        .map(|f| f.staling.queue.clone());
    let attack = [
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
        shoulder(),
    ];
    let contact = game.step(attack).unwrap().clone();
    assert_eq!(contact.fighters[0].action, Action::AttackAirN);
    assert!(!contact.fighters[0].grounded);
    assert!(contact.fighters[1].grounded);
    assert_eq!(contact.fighters[1].action, Action::GuardSetOff);
    assert_eq!(contact.fighters[1].percent, 0.0);
    assert!(contact.fighters[1].shield.health < 50.0);
    assert!(contact.fighters.iter().all(|f| f.hitlag > 0.0));
    assert_eq!(contact.fighters[0].staling.identity.move_id, 20);
    assert!(contact.events.iter().any(|event| matches!(
        event,
        Event::ShieldHit {
            attacker: 0,
            victim: 1,
            damage: 10.0,
            broken: false,
            ..
        }
    )));
    assert!(
        !contact
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
    );
    let positions = contact.fighters.each_ref().map(|f| f.position);
    let mut expected = vec![(attack, contact)];
    for _ in 0..3 {
        let frozen = game.step(held).unwrap().clone();
        assert_eq!(frozen.fighters.each_ref().map(|f| f.position), positions);
        assert!(frozen.events.is_empty());
        expected.push((held, frozen));
    }
    let resumed = game.step(held).unwrap().clone();
    // ftColl_80076CBC creates attacker recoil only for a grounded attacker.
    assert_eq!(resumed.fighters[0].shield.attacker_push, [0.0; 2]);
    assert_eq!(resumed.fighters[0].position[0], positions[0][0]);
    assert!(resumed.fighters[0].position[1] < positions[0][1]);
    assert!(resumed.fighters[1].position[0] > positions[1][0]);
    expected.push((held, resumed));
    for (_, state) in &expected {
        assert_eq!(state.fighters[1].percent, 0.0);
        assert_eq!(
            state.fighters.each_ref().map(|f| f.staling.queue.clone()),
            queues
        );
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    for (input, state) in expected {
        assert_eq!(game.step(input).unwrap(), &state);
    }
}

// Guard IASA installs KneeBend; its launch Anim installs Jump before input
// dispatch. The fresh C-stick must select an aerial without spending a double jump.
#[test]
fn shielding_then_jumping_allows_a_cstick_aerial_on_the_exact_launch_frame() {
    let mut data = support::data();
    data.stage.spawns[0][1] = 0.0;
    let mut game = Match::new(data, 8).unwrap();
    for _ in 0..3 {
        support::step(&mut game, shoulder());
    }
    assert_eq!(game.state().fighters[0].action, Action::Guard);
    let squat = support::step(
        &mut game,
        Controller {
            buttons: BUTTON_L | BUTTON_X,
            ..Default::default()
        },
    );
    assert_eq!(squat.fighters[0].action, Action::JumpSquat);
    let ready = support::step(&mut game, Controller::default());
    assert_eq!(ready.fighters[0].action, Action::JumpSquat);
    assert_eq!(ready.fighters[0].action_frame, 2);
    let checkpoint = game.checkpoint();
    let cstick = Controller {
        cstick: [1.0, 0.0],
        ..Default::default()
    };
    let mut expected = vec![];
    for _ in 0..4 {
        let state = support::step(&mut game, cstick).clone();
        assert_eq!(state.fighters[0].action, Action::AttackAirF);
        assert_eq!(state.fighters[0].locomotion.jumps_used, 1);
        assert!(!state.fighters[0].grounded);
        expected.push(state);
    }
    assert!(expected[0].fighters[0].velocity[1] > 0.0);
    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(support::step(&mut game, cstick), &state);
    }
}

// LandingAir_Anim installs Wait before the same frame's Wait IASA runs. The
// explicit fixture has (9.9+0.1)/8 = 1.25 animation rate and eight recovery steps.
#[test]
fn held_shoulder_enters_guard_on_the_exact_aerial_landing_recovery_frame() {
    let mut data = support::data();
    data.stage.spawns[0][1] = 4.0;
    let mut game = Match::new(data, 9).unwrap();
    support::step(
        &mut game,
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
    );
    for _ in 0..5 {
        support::step(&mut game, Controller::default());
    }
    assert_eq!(game.state().fighters[0].action, Action::LandingAirN);
    assert_eq!(game.state().fighters[0].aerial.landing_rate, 1.25);
    let checkpoint = game.checkpoint();
    let mut expected = vec![];
    for frame in 1..=8 {
        let state = support::step(&mut game, shoulder()).clone();
        assert_eq!(
            state.fighters[0].action,
            if frame == 8 {
                Action::GuardOn
            } else {
                Action::LandingAirN
            }
        );
        expected.push(state);
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(support::step(&mut game, shoulder()), &state);
    }
}
