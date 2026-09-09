//! Synthetic orchestration regressions, not Melee fidelity certification.
use serde_json::Value;
use skirmish_match::{
    Action, BUTTON_A, BUTTON_X, Controller, Error, Event, FinishReason, Match, Phase, State,
    data::{MatchData, Profile},
};
use skirmish_replay::{Checkpoint, Transition, ValidationError, branch_from, validate};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    serde_json::from_str(include_str!("fixtures/integration-match.json")).unwrap()
}

fn press(player: usize, button: u16) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player].buttons = button;
    inputs
}

fn playing(mut resource: MatchData) -> Match {
    resource.rules.countdown_frames = 0;
    Match::new(resource, 42).unwrap()
}

fn until(game: &mut Match, limit: usize, condition: impl Fn(&State) -> bool) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..limit {
        if condition(game.state()) {
            return events;
        }
        events.extend(game.step(IDLE).unwrap().events.clone());
    }
    assert!(
        condition(game.state()),
        "condition not reached: {:?}",
        game.state()
    );
    events
}

// serde_json::to_value widens each f32 exactly to f64. Converting these numbers
// to bit strings compares every float field, including signed zero, recursively.
fn state_bits(state: &State) -> Value {
    fn normalize(value: &mut Value) {
        match value {
            Value::Number(number) if number.is_f64() => {
                *value = Value::String(format!("f64:{:016x}", number.as_f64().unwrap().to_bits()));
            }
            Value::Array(values) => values.iter_mut().for_each(normalize),
            Value::Object(values) => values.values_mut().for_each(normalize),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(state).unwrap();
    normalize(&mut value);
    value
}

fn compare(expected: &State, actual: &State) -> Option<String> {
    (state_bits(expected) != state_bits(actual)).then(|| "native state bits differ".to_owned())
}

#[test]
fn countdown_walk_jump_land_hitlag_respawn_and_second_stock_finish() {
    let resource = data();
    assert_eq!(resource.profile, Profile::IntegrationFixture);
    assert!(resource.provenance.contains("Wholly synthetic"));
    let mut game = Match::new(resource.clone(), 7).unwrap();
    let initial_positions = game
        .state()
        .fighters
        .each_ref()
        .map(|fighter| fighter.position);
    game.step(IDLE).unwrap();
    assert!(matches!(
        game.state().phase,
        Phase::Countdown { remaining: 1 }
    ));
    game.step(IDLE).unwrap();
    assert_eq!(game.state().phase, Phase::Playing);
    assert_eq!(game.state().events, [Event::Started]);
    assert_eq!(
        game.state().remaining_frames,
        resource.rules.time_limit_frames
    );
    assert_eq!(
        game.state().fighters.each_ref().map(|f| f.position),
        initial_positions
    );

    let mut walking = IDLE;
    walking[0].stick[0] = 1.0;
    game.step(walking).unwrap();
    game.step(IDLE).unwrap();
    game.step(IDLE).unwrap();
    assert!(game.state().fighters[0].position[0] > initial_positions[0][0]);
    game.step(press(0, BUTTON_X)).unwrap();
    assert_eq!(game.state().fighters[0].action, Action::JumpSquat);
    for _ in 0..4 {
        if game.state().fighters[0].action == Action::Jump {
            break;
        }
        game.step(press(0, BUTTON_X)).unwrap();
    }
    let launched = &game.state().fighters[0];
    assert_eq!(launched.action, Action::Jump);
    assert!(!launched.grounded);
    assert_eq!(
        launched.velocity[1].to_bits(),
        resource.fighters[0]
            .movement
            .jump_vertical_velocity
            .to_bits()
    );
    game.step(IDLE).unwrap();
    assert_eq!(
        game.state().fighters[0].velocity[1],
        resource.fighters[0].movement.jump_vertical_velocity
            - resource.fighters[0].movement.gravity
    );
    let landing_events = until(&mut game, 40, |state| state.fighters[0].grounded);
    assert!(landing_events.contains(&Event::Landed { player: 0 }));
    assert_eq!(game.state().fighters[0].position[1], resource.stage.floor.y);
    until(&mut game, 5, |state| {
        state.fighters[0].action == Action::Wait
    });

    game.step(press(0, BUTTON_A)).unwrap();
    game.step(IDLE).unwrap();
    assert!(matches!(
        game.state().events.as_slice(),
        [Event::Hit {
            attacker: 0,
            victim: 1,
            damage: 10.0,
            ..
        }]
    ));
    assert_eq!(game.state().fighters[1].percent, 10.0);
    assert_eq!(game.state().fighters[1].action, Action::Damage);
    let contact = game.state().clone();
    assert!(contact.fighters.iter().all(|f| f.hitlag > 0.0));
    for _ in 0..contact.fighters[0].hitlag as usize {
        game.step(IDLE).unwrap();
        for player in 0..2 {
            assert_eq!(
                game.state().fighters[player].position,
                contact.fighters[player].position
            );
            assert_eq!(
                game.state().fighters[player].action_frame,
                contact.fighters[player].action_frame
            );
        }
        assert!(game.state().events.is_empty());
    }
    let ko = until(&mut game, 12, |state| {
        state.fighters[1].action == Action::Respawn
    });
    assert!(ko.contains(&Event::Knockout {
        player: 1,
        stocks: 1
    }));
    let respawn = until(&mut game, 10, |state| {
        state.fighters[1].action != Action::Respawn
    });
    assert!(respawn.contains(&Event::Respawned { player: 1 }));
    assert_eq!(game.state().fighters[1].percent, 0.0);
    assert_eq!(game.state().fighters[1].position, resource.stage.spawns[1]);
    until(&mut game, 10, |state| {
        state.fighters[1].invincibility == 0 && state.fighters[0].action == Action::Wait
    });
    game.step(press(0, BUTTON_A)).unwrap();
    let events = until(&mut game, 20, |state| {
        matches!(state.phase, Phase::Finished { .. })
    });
    assert!(events.contains(&Event::Knockout {
        player: 1,
        stocks: 0
    }));
    assert_eq!(
        game.state().phase,
        Phase::Finished {
            winner: Some(0),
            reason: FinishReason::Stocks
        }
    );
    assert!(game.state().next_frame < 100);
    let finished = state_bits(game.state());
    assert!(matches!(game.step(IDLE), Err(Error::Finished)));
    assert_eq!(state_bits(game.state()), finished);
}

#[test]
fn rotating_the_child_bone_changes_contact_and_facing_mirrors_the_attack() {
    let mut rotated = playing(data());
    rotated.step(press(0, BUTTON_A)).unwrap();
    rotated.step(IDLE).unwrap();
    assert_eq!(rotated.state().fighters[1].percent, 10.0);

    let mut unrotated_data = data();
    for frame in &mut unrotated_data.fighters[0].jab.frames {
        frame.bones[1].rotation = [0.0; 3];
    }
    let mut unrotated = playing(unrotated_data);
    unrotated.step(press(0, BUTTON_A)).unwrap();
    for _ in 0..8 {
        unrotated.step(IDLE).unwrap();
    }
    assert_eq!(unrotated.state().fighters[1].percent, 0.0);

    let mut mirrored = playing(data());
    mirrored.step(press(1, BUTTON_A)).unwrap();
    mirrored.step(IDLE).unwrap();
    assert_eq!(mirrored.state().fighters[0].percent, 10.0);
    assert!(rotated.state().fighters[1].knockback[0] > 0.0);
    assert!(mirrored.state().fighters[0].knockback[0] < 0.0);
}

#[test]
fn held_attack_and_repeated_active_frames_hit_once_until_a_new_press() {
    let mut resource = data();
    resource.rules.knockback_speed = 0.0;
    let mut game = playing(resource);
    let mut hits = 0;
    for _ in 0..30 {
        hits += game
            .step(press(0, BUTTON_A))
            .unwrap()
            .events
            .iter()
            .filter(|event| matches!(event, Event::Hit { .. }))
            .count();
    }
    assert_eq!(hits, 1);
    assert_eq!(game.state().fighters[1].percent, 10.0);
    assert_eq!(game.state().fighters[0].action, Action::Wait);
    game.step(IDLE).unwrap();
    game.step(press(0, BUTTON_A)).unwrap();
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].percent, 20.0);
}

#[test]
fn simultaneous_contacts_trade_and_timer_uses_damage_to_break_stock_ties() {
    let mut trade = playing(data());
    trade
        .step(
            [Controller {
                buttons: BUTTON_A,
                stick: [0.0; 2],
            }; 2],
        )
        .unwrap();
    trade.step(IDLE).unwrap();
    assert_eq!(
        trade
            .state()
            .events
            .iter()
            .filter(|e| matches!(e, Event::Hit { .. }))
            .count(),
        2
    );
    assert!(
        trade
            .state()
            .fighters
            .iter()
            .all(|fighter| fighter.percent == 10.0 && fighter.action == Action::Damage)
    );

    let mut tied_data = data();
    tied_data.rules.time_limit_frames = 1;
    let mut tied = playing(tied_data);
    tied.step(IDLE).unwrap();
    assert_eq!(
        tied.state().phase,
        Phase::Finished {
            winner: None,
            reason: FinishReason::Time
        }
    );

    let mut timed_data = data();
    timed_data.rules.time_limit_frames = 3;
    timed_data.rules.knockback_speed = 0.0;
    let mut timed = playing(timed_data);
    timed.step(press(0, BUTTON_A)).unwrap();
    timed.step(IDLE).unwrap();
    timed.step(IDLE).unwrap();
    assert_eq!(
        timed.state().phase,
        Phase::Finished {
            winner: Some(0),
            reason: FinishReason::Time
        }
    );
}

#[test]
fn mid_hitlag_checkpoint_replays_exactly_and_branches_without_shared_state() {
    let mut reference = playing(data());
    reference.step(press(0, BUTTON_A)).unwrap();
    reference.step(IDLE).unwrap();
    assert!(reference.state().fighters[0].hitlag > 0.0);
    let checkpoint = Checkpoint {
        next_frame: -123,
        state: reference.checkpoint(),
    };
    let checkpoint_bits = state_bits(reference.state());
    let transitions: Vec<_> = (0..24)
        .map(|index| Transition {
            frame: -123 + index,
            input: IDLE,
            expected: reference.step(IDLE).unwrap().clone(),
        })
        .collect();
    let original_bits = state_bits(reference.state());
    let mut candidate = branch_from(&reference, &checkpoint).unwrap();
    assert_eq!(state_bits(candidate.state()), checkpoint_bits);
    let report = validate(&mut candidate, &checkpoint, transitions.clone(), compare).unwrap();
    assert_eq!(report.checked_frames, 24);
    assert_eq!(state_bits(candidate.state()), original_bits);

    let mut altered = transitions;
    altered[7].expected.fighters[0].previous_input.stick[1] = -0.0;
    assert!(matches!(
        validate(&mut candidate, &checkpoint, altered, compare),
        Err(ValidationError::Mismatch {
            frame: -116,
            checked_frames: 7,
            ..
        })
    ));
    let mut branch = branch_from(&reference, &checkpoint).unwrap();
    let mut left = IDLE;
    left[0].stick[0] = -1.0;
    for _ in 0..12 {
        branch.step(left).unwrap();
    }
    assert_ne!(state_bits(branch.state()), original_bits);
    assert_eq!(state_bits(reference.state()), original_bits);
    reference.restore_checkpoint(&checkpoint.state).unwrap();
    assert_eq!(state_bits(reference.state()), checkpoint_bits);
}

#[test]
fn simultaneous_last_stock_knockouts_are_a_draw() {
    let mut resource = data();
    resource.rules.stocks = 1;
    let mut game = playing(resource);
    game.step(
        [Controller {
            buttons: BUTTON_A,
            stick: [0.0; 2],
        }; 2],
    )
    .unwrap();
    let events = until(&mut game, 20, |state| {
        matches!(state.phase, Phase::Finished { .. })
    });
    assert!(events.contains(&Event::Knockout {
        player: 0,
        stocks: 0
    }));
    assert!(events.contains(&Event::Knockout {
        player: 1,
        stocks: 0
    }));
    assert_eq!(
        game.state().phase,
        Phase::Finished {
            winner: None,
            reason: FinishReason::Stocks
        }
    );
}

#[test]
fn final_invincible_frame_blocks_contact_and_zero_hitlag_preserves_damage_entry() {
    let mut game = playing(data());
    game.step(press(0, BUTTON_A)).unwrap();
    until(&mut game, 20, |state| {
        state.fighters[1].action == Action::Respawn
    });
    until(&mut game, 10, |state| {
        state.fighters[1].action != Action::Respawn
    });
    assert_eq!(game.state().fighters[1].invincibility, 3);
    game.step(IDLE).unwrap();
    game.step(press(0, BUTTON_A)).unwrap();
    assert_eq!(game.state().fighters[1].invincibility, 1);
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].invincibility, 0);
    assert_eq!(game.state().fighters[1].percent, 0.0);
    assert!(game.state().events.is_empty());
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].percent, 10.0);

    let mut no_lag_data = data();
    no_lag_data.rules.hitlag.base = 0.0;
    no_lag_data.rules.hitlag.damage_scale = 0.0;
    no_lag_data.rules.hitstun_scale = 0.0;
    let mut no_lag = playing(no_lag_data);
    no_lag.step(press(0, BUTTON_A)).unwrap();
    no_lag.step(IDLE).unwrap();
    assert_eq!(no_lag.state().fighters[1].hitlag, 0.0);
    assert_eq!(no_lag.state().fighters[1].hitstun, 1);
    assert_eq!(no_lag.state().fighters[1].action_frame, 0);
    no_lag.step(IDLE).unwrap();
    assert_eq!(no_lag.state().fighters[1].hitstun, 0);
    assert_eq!(no_lag.state().fighters[1].action_frame, 1);
}

#[test]
fn invalid_resources_inputs_and_foreign_checkpoints_fail_atomically() {
    let changes: [fn(&mut MatchData); 8] = [
        |data| data.provenance.clear(),
        |data| data.rules.stocks = 0,
        |data| data.stage.floor.right = data.stage.blast[1],
        |data| data.fighters[0].bones[0].parent = Some(1),
        |data| data.fighters[0].jab.frames.clear(),
        |data| data.fighters[0].jab.frames[1].hitboxes[0].angle_degrees = 361.0,
        |data| data.fighters[0].jab.frames[1].bones[1].classical_scale = true,
        |data| data.fighters[0].bones[0].scale = [1_000_000.0; 3],
    ];
    for change in changes {
        let mut resource = data();
        change(&mut resource);
        assert!(matches!(Match::new(resource, 42), Err(Error::Data(_))));
    }
    let mut game = playing(data());
    let before = state_bits(game.state());
    for invalid in [
        Controller {
            buttons: 1,
            stick: [0.0; 2],
        },
        Controller {
            buttons: 0,
            stick: [1.01, 0.0],
        },
        Controller {
            buttons: 0,
            stick: [f32::NAN, 0.0],
        },
        Controller {
            buttons: 0,
            stick: [0.0, f32::INFINITY],
        },
    ] {
        let mut input = IDLE;
        input[1] = invalid;
        assert!(matches!(game.step(input), Err(Error::Input(1))));
        assert_eq!(state_bits(game.state()), before);
    }
    let mut other_data = data();
    other_data.rules.walk_accel_taper_gain = 0.5;
    let other = playing(other_data);
    assert!(matches!(
        game.restore_checkpoint(&other.checkpoint()),
        Err(Error::Resources)
    ));
    assert_eq!(state_bits(game.state()), before);
}

#[test]
fn finite_inputs_that_produce_invalid_combat_arithmetic_leave_state_unchanged() {
    // This binary32 witness makes the weight term round slightly negative.
    let mut resource = data();
    resource.fighters[1].weight = f32::from_bits(0x46e5_0e42);
    resource.rules.knockback.weight_scale = f32::from_bits(0x494c_1595);
    resource.rules.knockback.weight_base = f32::from_bits(0x48d3_4ff4);
    resource.rules.knockback.growth_base = 0.0;
    for frame in &mut resource.fighters[0].jab.frames {
        for hitbox in &mut frame.hitboxes {
            hitbox.base = 0;
        }
    }
    let mut game = playing(resource);
    game.step(press(0, BUTTON_A)).unwrap();
    let before = state_bits(game.state());
    assert!(matches!(game.step(IDLE), Err(Error::Physics(_))));
    assert_eq!(state_bits(game.state()), before);
}

#[test]
fn independent_parallel_instances_and_reset_reproduce_the_same_native_state() {
    let workers: Vec<_> = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                let mut game = playing(data());
                for frame in 0..24 {
                    game.step(if frame == 0 { press(0, BUTTON_A) } else { IDLE })
                        .unwrap();
                }
                game
            })
        })
        .collect();
    let mut results = workers.into_iter().map(|worker| worker.join().unwrap());
    let mut first = results.next().unwrap();
    for game in results {
        assert_eq!(state_bits(game.state()), state_bits(first.state()));
    }
    let finished_rollout = state_bits(first.state());
    let reset_state = state_bits(first.reset(42));
    assert_eq!(reset_state, state_bits(playing(data()).state()));
    for frame in 0..24 {
        first
            .step(if frame == 0 { press(0, BUTTON_A) } else { IDLE })
            .unwrap();
    }
    assert_eq!(state_bits(first.state()), finished_rollout);
}
