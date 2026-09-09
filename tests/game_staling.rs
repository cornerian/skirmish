//! Synthetic resource integration, distinct from the original-C kernel tests.
use skirmish::{
    fighter::stale::Rules,
    game::{Action, BUTTON_A, Controller, Event, Match, State, data::MatchData},
};

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.knockback_speed = 0.0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -20.0;
    data.stage.floor.right = 20.0;
    data.stage.blast = [-25.0, 25.0, -10.0, 30.0];
    data.rules.staling = Some(Rules {
        penalties: [0.1, 0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
        debug_bypass: false,
    });
    for fighter in &mut data.fighters {
        fighter.jab.move_id = Some(10);
    }
    data
}
fn idle() -> [Controller; 2] {
    [Controller::default(); 2]
}
fn wait(game: &mut Match, condition: impl Fn(&State) -> bool) {
    for _ in 0..250 {
        if condition(game.state()) {
            return;
        }
        game.step(idle()).unwrap();
    }
    panic!("condition not reached: {:?}", game.state());
}
fn hit(game: &mut Match) -> (f32, f32) {
    let mut input = idle();
    input[0].buttons = BUTTON_A;
    game.step(input).unwrap();
    for _ in 0..10 {
        let state = game.step(idle()).unwrap();
        if let Some(result) = state.events.iter().find_map(|event| match event {
            Event::Hit {
                attacker: 0,
                damage,
                knockback,
                ..
            } => Some((*damage, *knockback)),
            _ => None,
        }) {
            return result;
        }
    }
    panic!("jab did not hit");
}
fn recover(game: &mut Match) {
    wait(game, |state| {
        state
            .fighters
            .iter()
            .all(|f| f.action == Action::Wait && f.hitlag == 0.0)
    });
}

#[test]
fn repeat_uses_staled_percent_but_unstaled_damage_term_in_knockback() {
    let mut game = Match::new(data(), 1).unwrap();
    assert_eq!(hit(&mut game), (10.0, 32.5));
    recover(&mut game);
    let (damage, kb) = hit(&mut game);
    assert_eq!(damage, 9.0);
    assert_eq!(game.state().fighters[1].percent, 19.0);
    // Explicit fixture formula at 10 prior percent: 0.5 *
    // (0.1*(10+9) + 0.02*10*(10+9) + 2) + 30 = 33.85.
    assert_eq!(kb.to_bits(), 33.85_f32.to_bits());
    assert_eq!(game.state().fighters[0].staling.queue.next(), 2);
    assert_eq!(
        game.state().fighters[0].staling.queue.entries()[0].move_id,
        10
    );
    assert_ne!(
        game.state().fighters[0].staling.queue.entries()[0].attack_instance,
        game.state().fighters[0].staling.queue.entries()[1].attack_instance
    );
}

#[test]
fn active_slot_retains_created_damage_new_slot_recomputes_and_instance_is_queued_once() {
    let mut resource = data();
    // Two consecutive active slots after one attack creation: frame 2 retains
    // slot0 but adds slot1 with a distinct hit group after the first contact.
    let second = resource.fighters[0].jab.frames[2].hitboxes[0].clone();
    let mut second = second;
    second.group = 1;
    resource.fighters[0].jab.frames[2].hitboxes.push(second);
    let mut game = Match::new(resource, 1).unwrap();
    hit(&mut game);
    wait(&mut game, |state| state.fighters[0].hitlag == 0.0);
    assert_eq!(
        game.state().fighters[0].staling.hits[0].unwrap().damage,
        10.0
    );
    wait(&mut game, |state| {
        state.fighters[0].staling.hits[1].is_some()
    });
    let state = game.state();
    assert_eq!(state.fighters[0].staling.hits[0].unwrap().damage, 10.0);
    assert_eq!(state.fighters[0].staling.hits[1].unwrap().damage, 9.0);
    assert_eq!(state.fighters[1].percent, 19.0);
    assert_eq!(state.fighters[0].staling.queue.next(), 1);
}

#[test]
fn checkpoint_restores_queue_instances_and_damage_cache_and_reset_is_fresh() {
    let resource = data();
    let mut game = Match::new(resource.clone(), 3).unwrap();
    hit(&mut game);
    let checkpoint = game.checkpoint();
    recover(&mut game);
    let expected = hit(&mut game);
    let expected_state = serde_json::to_string(game.state()).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    recover(&mut game);
    assert_eq!(hit(&mut game), expected);
    assert_eq!(serde_json::to_string(game.state()).unwrap(), expected_state);
    let mut independent = Match::new(resource, 3).unwrap();
    assert_eq!(hit(&mut independent).0, 10.0);
    game.reset(3);
    assert_eq!(game.state().attack_instances.next_value(), 1);
    assert_eq!(hit(&mut game).0, 10.0);
}

#[test]
fn dying_players_queue_resets_at_ko_while_global_instance_sequence_survives() {
    let mut game = Match::new(data(), 1).unwrap();
    let mut trade = idle();
    for input in &mut trade {
        input.buttons = BUTTON_A;
    }
    game.step(trade).unwrap();
    game.step(idle()).unwrap();
    for fighter in &game.state().fighters {
        assert_eq!(fighter.staling.queue.next(), 1);
        assert_eq!(fighter.staling.queue.entries()[0].move_id, 10);
        assert_eq!(fighter.staling.identity.move_id, 1);
    }
    assert_ne!(
        game.state().fighters[0].staling.queue.entries()[0].attack_instance,
        game.state().fighters[1].staling.queue.entries()[0].attack_instance
    );
    let opponent_queue = game.state().fighters[1].staling.queue.clone();
    recover(&mut game);
    let counter = game.state().attack_instances.next_value();
    let mut run = idle();
    run[0].stick = [-1.0, 0.0];
    for _ in 0..100 {
        game.step(run).unwrap();
        if game.state().fighters[0].action == Action::Respawn {
            break;
        }
    }
    assert_eq!(game.state().fighters[0].action, Action::Respawn);
    assert_eq!(game.state().fighters[0].staling.queue.next(), 0);
    assert!(
        game.state().fighters[0]
            .staling
            .queue
            .entries()
            .iter()
            .all(|entry| entry.move_id == 0)
    );
    assert!(game.state().attack_instances.next_value() > counter);
    assert_eq!(game.state().fighters[1].staling.queue, opponent_queue);
    wait(&mut game, |state| {
        state.fighters[0].action == Action::Wait && state.fighters[0].invincibility == 0
    });
    assert_eq!(hit(&mut game).0, 10.0);
}

#[test]
fn exempt_identity_debug_mode_and_invalid_resources_have_explicit_behavior() {
    for bypass in [false, true] {
        let mut resource = data();
        resource.rules.staling.as_mut().unwrap().debug_bypass = bypass;
        if !bypass {
            resource.fighters[0].jab.move_id = Some(1);
        }
        let mut game = Match::new(resource, 1).unwrap();
        assert_eq!(hit(&mut game).0, 10.0);
        recover(&mut game);
        assert_eq!(hit(&mut game).0, 10.0);
        assert_eq!(
            game.state().fighters[0].staling.queue.next(),
            if bypass { 2 } else { 0 }
        );
    }
    let mut missing = data();
    missing.fighters[0].jab.move_id = None;
    assert!(Match::new(missing, 1).is_err());
    let mut negative = data();
    negative.rules.staling.as_mut().unwrap().penalties = [0.5; 9];
    assert!(Match::new(negative, 1).is_err());
}
