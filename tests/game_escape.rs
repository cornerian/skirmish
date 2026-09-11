//! End-to-end grounded shield evasions (EscapeF/EscapeB/EscapeN) in an
//! explicitly synthetic native world with invented escape motions.
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/grab.rs"]
mod grab_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_Z, Controller, Event, Match, State,
    data::{BodyState, MatchData},
    shield,
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
    }
    escape_support::profile(data)
}

fn held() -> Controller {
    Controller {
        buttons: BUTTON_L,
        ..Default::default()
    }
}

fn guard(stick: [f32; 2], cstick: [f32; 2]) -> Controller {
    Controller {
        buttons: BUTTON_L,
        stick,
        cstick,
        ..Default::default()
    }
}

fn attacker(buttons: u16) -> Controller {
    Controller {
        buttons,
        ..Default::default()
    }
}

fn step(game: &mut Match, first: Controller, second: Controller) -> State {
    game.step([first, second]).unwrap().clone()
}

/// Shield on the first frame, then apply the evasion input on the first Guard
/// callback. Returns the fighter-1 state after that second frame.
fn escape_from_guard(game: &mut Match, input: Controller) -> State {
    assert_eq!(
        step(game, attacker(0), held()).fighters[1].action,
        Action::GuardOn
    );
    step(game, attacker(0), input)
}

fn hit_events(state: &State, victim: usize) -> usize {
    state
        .events
        .iter()
        .filter(|event| matches!(event, Event::Hit { victim: v, .. } if *v == victim))
        .count()
}

#[test]
fn fresh_main_stick_rolls_forward_and_backward_relative_to_facing() {
    // Fighter 1 faces -X, so a negative stick is forward for it.
    for (stick, expected) in [(-1.0, Action::EscapeF), (1.0, Action::EscapeB)] {
        let mut game = Match::new(data(), 42).unwrap();
        let state = escape_from_guard(&mut game, guard([stick, 0.0], [0.0; 2]));
        assert_eq!(state.fighters[1].action, expected, "stick {stick}");
        // The scheduler advances the counter after the entry frame's callbacks.
        assert_eq!(state.fighters[1].action_frame, 1);
        assert_eq!(state.fighters[1].body_state, BodyState::Normal);
        assert_eq!(state.fighters[1].facing, -1.0);
    }
    for (stick, expected) in [(1.0, Action::EscapeF), (-1.0, Action::EscapeB)] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, held(), attacker(0));
        let state = step(&mut game, guard([stick, 0.0], [0.0; 2]), attacker(0));
        assert_eq!(state.fighters[0].action, expected, "stick {stick}");
    }
    // Below the inclusive magnitude threshold nothing happens.
    let mut game = Match::new(data(), 42).unwrap();
    let state = escape_from_guard(&mut game, guard([-0.69, 0.0], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::GuardOn);
}

#[test]
fn roll_applies_exact_sampled_root_motion_then_returns_to_wait_with_zero_ground_speed() {
    let mut game = Match::new(data(), 42).unwrap();
    let entry = escape_from_guard(&mut game, guard([-1.0, 0.0], [0.0; 2]));
    assert_eq!(entry.fighters[1].action, Action::EscapeF);
    assert_eq!(entry.fighters[1].position[0], 2.0);
    // Floor snapping keeps the resolved height a hair above the line; only
    // the sampled horizontal motion is asserted exactly.
    let expected = [1.5, 0.5, -0.5, -1.5, -2.0, -2.25, -2.5];
    let roots = [0.5, 1.0, 1.0, 1.0, 0.5, 0.25, 0.25];
    for (sample, (x, root)) in expected.into_iter().zip(roots).enumerate() {
        let state = step(&mut game, attacker(0), Controller::default());
        let fighter = &state.fighters[1];
        assert_eq!(fighter.action, Action::EscapeF, "sample {sample}");
        assert_eq!(fighter.action_frame, sample as u32 + 2);
        assert_eq!(fighter.position[0], x, "sample {sample}");
        assert!(fighter.position[1].abs() < 0.001);
        assert_eq!(fighter.ground_velocity, -root, "sample {sample}");
        assert!(fighter.grounded);
    }
    let waiting = step(&mut game, attacker(0), Controller::default());
    assert_eq!(waiting.fighters[1].action, Action::Wait);
    assert_eq!(waiting.fighters[1].ground_velocity, 0.0);
    assert_eq!(waiting.fighters[1].position[0], -2.5);
    assert_eq!(waiting.fighters[1].body_state, BodyState::Normal);
}

#[test]
fn sampled_roll_bones_move_the_hurtbox_only_on_their_frame() {
    // Only the forward roll's seventh sample displaces bone 1 by three units;
    // an attacker placed far behind the roll can reach only that hurtbox.
    fn run(displaced: bool) -> (usize, f32) {
        let mut resource = data();
        resource.stage.spawns = [[-7.0, 0.0], [2.0, 0.0]];
        if !displaced {
            resource.fighters[1].escape.as_mut().unwrap().forward.frames[6].bones[1].translation
                [0] = 0.0;
        }
        let mut game = Match::new(resource, 42).unwrap();
        escape_from_guard(&mut game, guard([-1.0, 0.0], [0.0; 2]));
        for _ in 0..4 {
            step(&mut game, attacker(0), Controller::default());
        }
        step(&mut game, attacker(BUTTON_A), Controller::default());
        let contact = step(&mut game, attacker(0), Controller::default());
        assert_eq!(contact.fighters[0].action, Action::Jab);
        let hits = hit_events(&contact, 1);
        let mut total = hits;
        for _ in 0..3 {
            total += hit_events(&step(&mut game, attacker(0), Controller::default()), 1);
        }
        assert_eq!(total, hits);
        (hits, contact.fighters[1].percent)
    }
    assert_eq!(run(true), (1, 10.0));
    assert_eq!(run(false), (0, 0.0));
}

#[test]
fn held_cstick_rolls_without_freshness_and_main_stick_keeps_priority() {
    for (cstick, expected) in [(-1.0, Action::EscapeF), (1.0, Action::EscapeB)] {
        let mut game = Match::new(data(), 42).unwrap();
        let hold = Controller {
            cstick: [cstick, 0.0],
            ..Default::default()
        };
        for _ in 0..6 {
            assert_eq!(
                step(&mut game, attacker(0), hold).fighters[1].action,
                Action::Wait
            );
        }
        let state = escape_from_guard(&mut game, guard([0.0; 2], [cstick, 0.0]));
        assert_eq!(state.fighters[1].action, expected, "cstick {cstick}");
    }

    // A fresh main stick outranks a contradicting held C-stick.
    let mut game = Match::new(data(), 42).unwrap();
    let state = escape_from_guard(&mut game, guard([1.0, 0.0], [-1.0, 0.0]));
    assert_eq!(state.fighters[1].action, Action::EscapeB);

    // A stale main stick fails its window, so the held C-stick decides.
    for (cstick, expected) in [(-1.0, Action::EscapeF), (0.0, Action::Guard)] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, attacker(0), held());
        for _ in 0..5 {
            let state = step(&mut game, attacker(0), guard([0.5, 0.0], [0.0; 2]));
            assert!(matches!(
                state.fighters[1].action,
                Action::GuardOn | Action::Guard
            ));
        }
        let state = step(&mut game, attacker(0), guard([1.0, 0.0], [cstick, 0.0]));
        assert!(state.fighters[1].locomotion.tilt_x_age >= 4);
        assert_eq!(state.fighters[1].action, expected, "cstick {cstick}");
    }
}

#[test]
fn spot_dodge_accepts_both_channels_precedes_rolls_and_keeps_ordinary_friction() {
    let mut fresh = Match::new(data(), 42).unwrap();
    let entry = escape_from_guard(&mut fresh, guard([0.0, -1.0], [0.0; 2]));
    assert_eq!(entry.fighters[1].action, Action::EscapeN);
    for frame in 1..8 {
        let state = step(&mut fresh, attacker(0), Controller::default());
        assert_eq!(state.fighters[1].action, Action::EscapeN);
        assert_eq!(state.fighters[1].action_frame, frame + 1);
        assert_eq!(state.fighters[1].position[0], 2.0);
        assert_eq!(state.fighters[1].ground_velocity, 0.0);
        assert_eq!(
            state.fighters[1].body_state,
            if (1..=3).contains(&frame) {
                BodyState::Intangible
            } else {
                BodyState::Normal
            }
        );
    }
    let waiting = step(&mut fresh, attacker(0), Controller::default());
    assert_eq!(waiting.fighters[1].action, Action::Wait);

    let mut held_cstick = Match::new(data(), 42).unwrap();
    let hold = Controller {
        cstick: [0.0, -1.0],
        ..Default::default()
    };
    for _ in 0..6 {
        step(&mut held_cstick, attacker(0), hold);
    }
    let state = escape_from_guard(&mut held_cstick, guard([0.0; 2], [0.0, -1.0]));
    assert_eq!(state.fighters[1].action, Action::EscapeN);

    let mut diagonal = Match::new(data(), 42).unwrap();
    let state = escape_from_guard(&mut diagonal, guard([1.0, -1.0], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::EscapeN);

    // A stale downward main stick cannot dodge without the C-stick.
    let mut stale = Match::new(data(), 42).unwrap();
    step(&mut stale, attacker(0), held());
    for _ in 0..5 {
        step(&mut stale, attacker(0), guard([0.0, -0.5], [0.0; 2]));
    }
    let state = step(&mut stale, attacker(0), guard([0.0, -1.0], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::Guard);
}

#[test]
fn guard_off_allows_spot_dodge_but_not_directional_rolls() {
    fn release_to_guard_off(game: &mut Match) {
        step(game, attacker(0), held());
        step(game, attacker(0), Controller::default());
        step(game, attacker(0), held());
        assert_eq!(
            step(game, attacker(0), held()).fighters[1].action,
            Action::GuardOff
        );
    }
    for stick in [-1.0, 1.0] {
        let mut game = Match::new(data(), 42).unwrap();
        release_to_guard_off(&mut game);
        let state = step(
            &mut game,
            attacker(0),
            Controller {
                stick: [stick, 0.0],
                ..Default::default()
            },
        );
        assert_eq!(state.fighters[1].action, Action::GuardOff, "stick {stick}");
        assert_eq!(state.fighters[1].locomotion.tilt_x_age, 0);
    }
    let mut game = Match::new(data(), 42).unwrap();
    release_to_guard_off(&mut game);
    let state = step(
        &mut game,
        attacker(0),
        Controller {
            stick: [0.0, -1.0],
            ..Default::default()
        },
    );
    assert_eq!(state.fighters[1].action, Action::EscapeN);
}

#[test]
fn guard_reflect_offers_the_same_evasions() {
    let mut resource = data();
    let rules = resource.rules.shield.as_mut().unwrap();
    rules.powershield_input_window = 3;
    rules.powershield_reflect_frames = 3.0;
    rules.powershield_frames = 2.0;
    let mut game = Match::new(resource, 42).unwrap();
    assert_eq!(
        step(&mut game, attacker(0), held()).fighters[1].action,
        Action::GuardReflect
    );
    let state = step(&mut game, attacker(0), guard([-1.0, 0.0], [0.0; 2]));
    assert_eq!(state.fighters[1].action, Action::EscapeF);
    // The motion change drops the reflect bit and entry latch; the immunity
    // window and its timer stay frozen outside the guard callbacks.
    let shield = &state.fighters[1].shield;
    assert!(!shield.reflecting);
    assert!(!shield.powershield_just_started);
    assert!(shield.powershield);
    assert_eq!(shield.powershield_timer, 1.0);
    let later = step(&mut game, attacker(0), Controller::default());
    assert!(later.fighters[1].shield.powershield);
    assert_eq!(later.fighters[1].shield.powershield_timer, 1.0);
}

#[test]
fn escape_clears_the_shield_and_lets_health_regenerate() {
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, attacker(0), held());
    let guarding = step(&mut game, attacker(0), held());
    let rolling = step(&mut game, attacker(0), guard([-1.0, 0.0], [0.0; 2]));
    assert_eq!(rolling.fighters[1].action, Action::EscapeF);
    // The entry frame still ran the Guard drain before the transition; every
    // later escape frame regenerates instead of draining.
    assert!(rolling.fighters[1].shield.health < guarding.fighters[1].shield.health);
    let mut previous = rolling.fighters[1].shield.health;
    for _ in 0..3 {
        let state = step(&mut game, attacker(0), held());
        assert_eq!(state.fighters[1].action, Action::EscapeF);
        assert!(state.fighters[1].shield.health > previous);
        previous = state.fighters[1].shield.health;
    }
    let full = step(&mut game, attacker(0), held());
    assert_eq!(full.fighters[1].shield.health, 50.0);
}

#[test]
fn intangible_samples_block_hits_only_on_their_frames() {
    // Spot dodge: samples 1..=3 are intangible, later samples are ordinary.
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, attacker(0), held());
    let entry = step(&mut game, attacker(BUTTON_A), guard([0.0, -1.0], [0.0; 2]));
    assert_eq!(entry.fighters[1].action, Action::EscapeN);
    assert_eq!(entry.fighters[0].action, Action::Jab);
    for _ in 0..2 {
        let state = step(&mut game, attacker(0), Controller::default());
        assert_eq!(state.fighters[1].body_state, BodyState::Intangible);
        assert_eq!(hit_events(&state, 1), 0);
        assert_eq!(state.fighters[1].percent, 0.0);
    }
    for _ in 0..2 {
        step(&mut game, attacker(0), Controller::default());
    }
    let restart = step(&mut game, attacker(BUTTON_A), Controller::default());
    assert_eq!(restart.fighters[0].action, Action::Jab);
    assert_eq!(restart.fighters[1].action, Action::EscapeN);
    let landed = step(&mut game, attacker(0), Controller::default());
    assert_eq!(landed.fighters[1].body_state, BodyState::Normal);
    assert_eq!(hit_events(&landed, 1), 1);
    assert_eq!(landed.fighters[1].percent, 10.0);

    // Forward roll: the second sample is still vulnerable, samples 2..=5 are
    // not, and the roll ends behind the attacker.
    let mut early = Match::new(data(), 42).unwrap();
    step(&mut early, attacker(0), held());
    step(&mut early, attacker(BUTTON_A), guard([-1.0, 0.0], [0.0; 2]));
    let hit = step(&mut early, attacker(0), Controller::default());
    assert_eq!(hit_events(&hit, 1), 1);
    assert_eq!(hit.fighters[1].percent, 10.0);

    let mut late = Match::new(data(), 42).unwrap();
    step(&mut late, attacker(0), held());
    step(&mut late, attacker(0), guard([-1.0, 0.0], [0.0; 2]));
    step(&mut late, attacker(BUTTON_A), Controller::default());
    let mut hits = 0;
    for _ in 0..10 {
        hits += hit_events(&step(&mut late, attacker(0), Controller::default()), 1);
    }
    assert_eq!(hits, 0);
    assert_eq!(late.state().fighters[1].percent, 0.0);
    assert_eq!(late.state().fighters[1].action, Action::Wait);
}

#[test]
fn intangible_samples_block_grabs_only_on_their_frames() {
    fn run(delay: usize) -> Vec<Event> {
        let mut resource = grab_support::profile(data());
        resource.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
        let mut game = Match::new(resource, 42).unwrap();
        step(&mut game, attacker(0), held());
        let mut events = Vec::new();
        for frame in 0..8 {
            let holder = attacker(if frame == delay { BUTTON_Z } else { 0 });
            let victim = if frame == 0 {
                guard([0.0, -1.0], [0.0; 2])
            } else {
                Controller::default()
            };
            let state = step(&mut game, holder, victim);
            if frame == 0 && delay != 0 {
                assert_eq!(state.fighters[1].action, Action::EscapeN);
            }
            if frame == delay {
                // A same-frame capture already advances the holder to CatchPull.
                assert!(matches!(
                    state.fighters[0].action,
                    Action::Catch | Action::CatchPull
                ));
            }
            events.extend(
                state
                    .events
                    .iter()
                    .filter(|event| matches!(event, Event::Grabbed { .. }))
                    .cloned(),
            );
        }
        events
    }
    assert_eq!(
        run(0),
        vec![Event::Grabbed {
            holder: 0,
            victim: 1
        }]
    );
    assert!(run(1).is_empty());
}

#[test]
fn rolling_into_a_floor_edge_clamps_and_stays_grounded() {
    // Escape's collision callback is `ft_80084104` (mode 2, always clamp,
    // `mpColl_8004A45C_Floor`, mpcoll.c:3584-3652): past the floor's end the
    // roll stops there instead of falling off (docs/edges.md, "Ground
    // collision modes"). This corrects the earlier "falls off a floor edge"
    // expectation from the shield-escape batch.
    let mut resource = data();
    resource.stage.floor.right = 3.0;
    let mut game = Match::new(resource, 42).unwrap();
    let entry = escape_from_guard(&mut game, guard([1.0, 0.0], [0.0; 2]));
    assert_eq!(entry.fighters[1].action, Action::EscapeB);
    let first = step(&mut game, attacker(0), Controller::default());
    assert_eq!(first.fighters[1].position[0], 2.5);
    assert!(first.fighters[1].grounded);
    let clamped = step(&mut game, attacker(0), Controller::default());
    assert_eq!(clamped.fighters[1].action, Action::EscapeB);
    assert!(clamped.fighters[1].grounded);
    assert_eq!(clamped.fighters[1].ground_line, Some(0));
    // edge.x - ecb.bottom.x with ecb.bottom.x == 0.0 for this fixture skeleton.
    assert_eq!(clamped.fighters[1].position, [3.0, 0.0]);
    assert_eq!(
        clamped.fighters[1].edge_contact,
        Some(skirmish::game::edge::EdgeSide::Right)
    );
    assert_eq!(clamped.fighters[1].body_state, BodyState::Intangible);
    // The roll keeps re-clamping at the end every remaining frame -- gr_vel is
    // untouched by the clamp and keeps following the sampled root motion, but
    // the position itself never slides past the edge -- then returns to Wait
    // exactly like an ordinary completed roll, still grounded at the edge.
    let mut current = clamped;
    let mut reached_wait = false;
    for _ in 0..8 {
        assert!(current.fighters[1].grounded);
        assert_eq!(current.fighters[1].position[0], 3.0);
        if current.fighters[1].action == Action::Wait {
            reached_wait = true;
            break;
        }
        current = step(&mut game, attacker(0), Controller::default());
    }
    assert!(reached_wait, "the roll should return to Wait");
    assert_eq!(current.fighters[1].ground_velocity, 0.0);
}

#[test]
fn overlap_nudges_skip_the_escaping_fighter() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [0.5, 0.0]];
    resource.rules.nudge = Some(skirmish::fighter::nudge::Rules {
        horizontal_step: 0.25,
        depth_step: 0.125,
        depth_limit: 0.5,
        follower_depth_step: 0.375,
        follower_depth_limit: 1.0,
    });
    for fighter in &mut resource.fighters {
        fighter.nudge = Some(skirmish::game::nudge::Attributes {
            center_offset: 0.0,
            half_width: 1.0,
            nudge_disabled: false,
            overlap_disabled: false,
        });
    }
    let mut game = Match::new(resource, 42).unwrap();
    let guarding = step(&mut game, attacker(0), held());
    assert_eq!(
        guarding.fighters.each_ref().map(|f| f.nudge[0]),
        [-0.25, 0.25]
    );
    let entry = step(&mut game, attacker(0), guard([0.0, -1.0], [0.0; 2]));
    assert_eq!(entry.fighters[1].action, Action::EscapeN);
    // Entry happens after this frame's push sampling; the next frame skips
    // the dodging subject while its opponent still moves away from it.
    let dodging = step(&mut game, attacker(0), Controller::default());
    assert_eq!(dodging.fighters[0].nudge[0], -0.25);
    assert_eq!(dodging.fighters[1].nudge[0], 0.0);
    assert_eq!(
        dodging.fighters[1].position[0],
        entry.fighters[1].position[0]
    );
}

#[test]
fn checkpoints_restore_every_escape_action() {
    for (input, action) in [
        (guard([-1.0, 0.0], [0.0; 2]), Action::EscapeF),
        (guard([1.0, 0.0], [0.0; 2]), Action::EscapeB),
        (guard([0.0, -1.0], [0.0; 2]), Action::EscapeN),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, attacker(0), held());
        assert_eq!(
            step(&mut game, attacker(0), input).fighters[1].action,
            action
        );
        step(&mut game, attacker(0), Controller::default());
        let checkpoint = game.checkpoint();
        let inputs: Vec<_> = (0..14)
            .map(|frame| {
                [
                    attacker(if frame % 5 == 0 { BUTTON_A } else { 0 }),
                    if frame % 3 == 0 {
                        guard([-1.0, -1.0], [1.0, 0.0])
                    } else {
                        Controller::default()
                    },
                ]
            })
            .collect();
        let expected: Vec<_> = inputs
            .iter()
            .map(|&input| serde_json::to_vec(game.step(input).unwrap()).unwrap())
            .collect();
        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(game.state().fighters[1].action, action);
        for (input, expected) in inputs.into_iter().zip(expected) {
            assert_eq!(
                serde_json::to_vec(game.step(input).unwrap()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn invalid_escape_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    assert!(Match::new(data(), 0).is_ok());
    for window in [0, 254, 255] {
        rejected(|d| d.rules.escape.as_mut().unwrap().roll_window = window);
        rejected(|d| d.rules.escape.as_mut().unwrap().spot_dodge_window = window);
    }
    for threshold in [0.0, 1.5, -0.5, f32::NAN] {
        rejected(|d| d.rules.escape.as_mut().unwrap().roll_stick_threshold = threshold);
    }
    for threshold in [0.0, -1.5, 0.5, f32::NAN] {
        rejected(|d| d.rules.escape.as_mut().unwrap().spot_dodge_stick_threshold = threshold);
    }
    for value in [f32::NAN, f32::INFINITY, 2_000_000.0] {
        rejected(|d| {
            d.fighters[1].escape.as_mut().unwrap().backward.frames[3].root_translation = value
        });
    }
    rejected(|d| {
        d.fighters[0]
            .escape
            .as_mut()
            .unwrap()
            .forward
            .frames
            .clear()
    });
    rejected(|d| {
        d.fighters[0]
            .escape
            .as_mut()
            .unwrap()
            .spot_dodge
            .frames
            .clear()
    });
    rejected(|d| {
        d.fighters[1].escape.as_mut().unwrap().spot_dodge.frames[2]
            .bones
            .pop();
    });
    rejected(|d| {
        d.fighters[1].escape.as_mut().unwrap().forward.frames[1].bones[1].scale = [0.0; 3];
    });
    rejected(|d| d.fighters[1].escape = None);
    rejected(|d| d.rules.escape = None);
    rejected(|d| d.fighters[0].locomotion = None);
}
