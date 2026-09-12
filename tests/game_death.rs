//! Resource-driven normal, star and screen blast-death lifecycles.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/death.rs"]
mod death_resources;

use skirmish::{
    compat::math::random::HsdRng,
    game::{
        Action, BUTTON_A, Controller, Event, Match, Phase, State, data::MatchData, death::Kind,
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let mut data = death_resources::profile(conformance::data());
    data.rules.stocks = 3;
    data.rules.respawn_frames = 2;
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    data.stage.floor.left = -50.0;
    data.stage.floor.right = 50.0;
    data.stage.blast = [-80.0, 80.0, -40.0, 5.0];
    data.rules.knockback_decay = 0.0;
    data.rules.knockback_speed = 1.0;
    data.rules.hitlag.base = 0.0;
    data.rules.hitlag.damage_scale = 0.0;
    for fighter in &mut data.fighters {
        fighter.movement.gravity = 0.0;
        for frame in &mut fighter.jab.frames {
            for hit in &mut frame.hitboxes {
                hit.angle_degrees = 90.0;
                hit.growth = 0;
                hit.base = 2;
            }
        }
    }
    data
}

fn top_death(game: &mut Match, attacker: usize) -> State {
    let mut inputs = IDLE;
    inputs[attacker].buttons = BUTTON_A;
    game.step(inputs).unwrap();
    for _ in 0..12 {
        let state = game.step(IDLE).unwrap().clone();
        if state
            .events
            .iter()
            .any(|event| matches!(event, Event::DeathStarted { .. }))
        {
            return state;
        }
    }
    panic!("top damage never reached the death selector");
}

#[test]
fn star_death_delays_stock_loss_moves_exactly_and_replays_from_checkpoint() {
    let seed = 17;
    let mut game = Match::new(data(), seed).unwrap();
    let started = top_death(&mut game, 0);
    let fighter = &started.fighters[1];
    assert_eq!(fighter.action, Action::DeadUpStar);
    assert_eq!(fighter.death.kind, Some(Kind::UpStar));
    assert_eq!(fighter.stocks, 3);
    assert!(!fighter.death.stock_lost);
    assert_eq!(
        started.events,
        vec![Event::DeathStarted {
            player: 1,
            death: Kind::UpStar,
        }]
    );
    let mut expected_rng = HsdRng::new(seed);
    expected_rng.randi(100);
    assert_eq!(started.rng_seed, expected_rng.seed());

    let checkpoint = game.checkpoint();
    let start_y = fighter.position[1];
    let start_depth = fighter.depth;
    let rules = game.data().rules.death.unwrap();
    let velocity = (rules.star.height_scale * rules.star.camera_top - start_y)
        / rules.star.ascent_frames as f32;
    let mut expected = Vec::new();
    for _ in 0..rules.star.startup_frames {
        expected.push(game.step(IDLE).unwrap().clone());
    }
    assert_eq!(game.state().fighters[1].velocity[1], velocity);
    for _ in 1..rules.star.ascent_frames {
        expected.push(game.step(IDLE).unwrap().clone());
    }
    let fighter = &game.state().fighters[1];
    assert_eq!(
        fighter.position[1],
        start_y + velocity * rules.star.ascent_frames as f32
    );
    assert_eq!(fighter.depth, start_depth + rules.star.depth_distance);
    expected.push(game.step(IDLE).unwrap().clone());
    let lost = &game.state().fighters[1];
    assert_eq!(lost.stocks, 2);
    assert!(lost.death.hidden && lost.death.stock_lost);
    assert!(game.state().events.contains(&Event::Knockout {
        player: 1,
        stocks: 2,
    }));
    for _ in 0..rules.star.finish_frames {
        expected.push(game.step(IDLE).unwrap().clone());
    }
    assert_eq!(game.state().fighters[1].action, Action::Respawn);

    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(game.step(IDLE).unwrap(), &state);
    }
}

#[test]
fn screen_death_runs_approach_camera_fall_and_delayed_stock_phases() {
    let mut resource = data();
    resource.rules.death.as_mut().unwrap().screen_chance_percent = 100;
    let mut game = Match::new(resource, 23).unwrap();
    let started = top_death(&mut game, 0);
    assert_eq!(started.fighters[1].action, Action::DeadUpFall);
    assert_eq!(started.fighters[1].death.kind, Some(Kind::UpScreen));
    assert_eq!(started.fighters[1].stocks, 3);
    let rules = game.data().rules.death.unwrap();
    assert_eq!(
        started.fighters[1].death.camera_offset,
        rules.screen.approach_start
    );

    for _ in 0..rules.screen.startup_frames {
        game.step(IDLE).unwrap();
    }
    let first = &game.state().fighters[1].death.camera_offset;
    assert_eq!(first[0], -2.0);
    assert_eq!(first[1], 3.0);
    for _ in 1..rules.screen.approach_frames {
        game.step(IDLE).unwrap();
    }
    assert_eq!(
        game.state().fighters[1].death.camera_offset,
        rules.screen.approach_end
    );
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].action, Action::DeadUpFallHitCamera);

    for _ in 0..rules.screen.camera_hold_frames {
        game.step(IDLE).unwrap();
    }
    let falling = &game.state().fighters[1];
    assert_eq!(falling.death.phase, 3);
    assert_eq!(falling.velocity[1], -1.0);
    assert_eq!(falling.death.camera_offset[1], 0.0);
    for _ in 1..rules.screen.fall_frames {
        game.step(IDLE).unwrap();
    }
    assert_eq!(game.state().fighters[1].velocity[1], -2.0);
    game.step(IDLE).unwrap();
    let lost = &game.state().fighters[1];
    assert_eq!(lost.stocks, 2);
    assert!(lost.death.hidden && lost.death.stock_lost);
    assert!(game.state().events.contains(&Event::Knockout {
        player: 1,
        stocks: 2,
    }));
}

#[test]
fn forced_normal_top_death_loses_stock_immediately_and_ignores_input_until_respawn() {
    let mut resource = data();
    resource.rules.death.as_mut().unwrap().force_normal_top[1] = true;
    let normal_frames = resource.rules.death.unwrap().normal_frames;
    let mut game = Match::new(resource, 31).unwrap();
    let started = top_death(&mut game, 0);
    assert_eq!(started.fighters[1].action, Action::DeadUp);
    assert_eq!(started.fighters[1].stocks, 2);
    assert!(started.fighters[1].death.hidden);
    assert!(started.fighters[1].death.stock_lost);
    assert_eq!(started.rng_seed, 31);
    assert!(started.events.contains(&Event::DeathStarted {
        player: 1,
        death: Kind::Up,
    }));
    assert!(started.events.contains(&Event::Knockout {
        player: 1,
        stocks: 2,
    }));
    let noisy = [Controller {
        buttons: BUTTON_A,
        stick: [1.0; 2],
        cstick: [-1.0; 2],
        trigger: 1.0,
    }; 2];
    for _ in 1..normal_frames {
        assert_eq!(game.step(noisy).unwrap().fighters[1].action, Action::DeadUp);
    }
    assert_eq!(
        game.step(noisy).unwrap().fighters[1].action,
        Action::Respawn
    );
}

#[test]
fn left_right_and_bottom_deaths_publish_direction_and_complete_the_normal_timer() {
    for (player, kind, bottom) in [
        (0, Kind::Right, false),
        (1, Kind::Left, false),
        (0, Kind::Down, true),
    ] {
        let mut resource = death_resources::profile(conformance::data());
        resource.rules.stocks = 3;
        resource.stage.floor.left = -5.0;
        resource.stage.floor.right = 5.0;
        resource.stage.spawns = [[4.0, 0.0], [-4.0, 0.0]];
        resource.stage.blast = if bottom {
            [-200.0, 200.0, -3.0, 200.0]
        } else {
            [-6.0, 6.0, -200.0, 200.0]
        };
        let normal_frames = resource.rules.death.unwrap().normal_frames;
        let seed = 53;
        let mut game = Match::new(resource, seed).unwrap();
        let mut inputs = IDLE;
        inputs[player].stick[0] = if player == 0 { 1.0 } else { -1.0 };
        let started = (0..80)
            .find_map(|_| {
                let state = game.step(inputs).unwrap().clone();
                state
                    .events
                    .contains(&Event::DeathStarted {
                        player,
                        death: kind,
                    })
                    .then_some(state)
            })
            .expect("directional scenario must cross its blast line");
        assert_eq!(started.fighters[player].stocks, 2);
        assert_eq!(started.fighters[player].death.kind, Some(kind));
        assert!(started.fighters[player].death.hidden);
        assert!(started.fighters[player].death.stock_lost);
        assert_eq!(started.rng_seed, seed);
        assert!(
            started
                .events
                .contains(&Event::Knockout { player, stocks: 2 })
        );
        let action = match kind {
            Kind::Left => Action::DeadLeft,
            Kind::Right => Action::DeadRight,
            Kind::Down => Action::DeadDown,
            _ => unreachable!(),
        };
        for _ in 1..normal_frames {
            assert_eq!(game.step(IDLE).unwrap().fighters[player].action, action);
        }
        assert_eq!(
            game.step(IDLE).unwrap().fighters[player].action,
            Action::Respawn
        );
    }
}

#[test]
fn final_stock_match_finishes_only_when_the_star_animation_records_the_loss() {
    let mut resource = data();
    resource.rules.stocks = 1;
    let rules = resource.rules.death.unwrap();
    let mut game = Match::new(resource, 41).unwrap();
    let started = top_death(&mut game, 0);
    assert_eq!(started.fighters[1].stocks, 1);
    assert_eq!(started.phase, Phase::Playing);
    for _ in 1..rules.star.startup_frames + rules.star.ascent_frames {
        game.step(IDLE).unwrap();
    }
    assert_eq!(game.state().fighters[1].stocks, 1);
    let finished = game.step(IDLE).unwrap();
    assert_eq!(finished.fighters[1].action, Action::Eliminated);
    assert_eq!(
        finished.phase,
        Phase::Finished {
            winner: Some(0),
            reason: skirmish::game::FinishReason::Stocks,
        }
    );
}

#[test]
fn malformed_death_resources_are_rejected() {
    let mut bad = data();
    bad.rules.death.as_mut().unwrap().star.ascent_frames = 0;
    assert!(Match::new(bad, 0).is_err());

    let mut bad = data();
    bad.rules.death.as_mut().unwrap().screen_chance_percent = 101;
    assert!(Match::new(bad, 0).is_err());

    let mut bad = data();
    bad.rules.death.as_mut().unwrap().screen.gravity = f32::NAN;
    assert!(Match::new(bad, 0).is_err());

    let mut bad = data();
    bad.rules.top_ko_min_knockback = None;
    assert!(Match::new(bad, 0).is_err());
}
