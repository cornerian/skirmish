//! Idle animation cycling (`docs/idle.md`): restart-at-length with no
//! table, weighted picks deterministic from the match seed, the non-Wait1
//! re-draw gate, the shared RNG hand-off into the same frame's blast-zone
//! death draw, checkpoints and invalid/absent resources.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/death.rs"]
mod death_resources;
#[path = "support/idle.rs"]
mod idle_support;

use skirmish::{
    fighter::idle::pick,
    game::{
        Action, Controller, Match, State,
        data::MatchData,
        death::Kind,
        idle::{IdleAnimations, IdleEntry},
    },
    random::HsdRng,
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn step(game: &mut Match) -> State {
    game.step(IDLE).unwrap().clone()
}

#[test]
fn no_entries_restarts_at_wait1_length_without_drawing() {
    let mut data = conformance::data();
    data.fighters[0].idle = Some(IdleAnimations {
        wait1_length: idle_support::WAIT1_LENGTH,
        entries: vec![],
    });
    let mut game = Match::new(data, 42).unwrap();
    let seed_before = game.state().rng_seed;

    // wait1_length is 3.0: frames 0 and 1 only advance the tracked frame.
    let mut state = step(&mut game);
    assert_eq!(state.fighters[0].action, Action::Wait);
    assert_eq!(state.fighters[0].idle.animation, 2);
    assert_eq!(state.fighters[0].idle.frame, 1.0);
    assert_eq!(state.fighters[0].action_frame, 1);
    state = step(&mut game);
    assert_eq!(state.fighters[0].idle.frame, 2.0);
    assert_eq!(state.fighters[0].action_frame, 2);

    // Frame 2: 2.0 + 1.0 = 3.0 >= wait1_length, restart with no draw.
    // `restart` sets `action_frame` to 0 within this same frame's animation
    // phase, but the generic end-of-frame `action_frame += 1`
    // (`simulation::advance`, unconditional while not frozen/held) still
    // applies afterward, so the *observed* value is 1, matching every
    // other fresh action entry (`docs/idle.md`).
    state = step(&mut game);
    assert_eq!(state.fighters[0].idle.animation, 2);
    assert_eq!(state.fighters[0].idle.frame, 0.0);
    assert_eq!(state.fighters[0].action_frame, 1);
    assert_eq!(
        state.rng_seed, seed_before,
        "a table-less restart draws no RNG"
    );
}

#[test]
fn two_entry_table_pick_matches_the_pure_helper_from_the_same_seed() {
    let seed = 42;
    let mut data = conformance::data();
    data.fighters[0].idle = Some(idle_support::animations(&idle_support::TWO_ENTRY));
    let mut game = Match::new(data, seed).unwrap();

    // wait1_length 3.0: the restart/pick fires on the third step (index 2).
    step(&mut game);
    step(&mut game);
    let state = step(&mut game);

    let mut expected_rng = HsdRng::new(seed);
    let (expected_animation, expected_draws) = pick(
        idle_support::TWO_ENTRY
            .iter()
            .map(|entry| (entry.animation, entry.weight)),
        || expected_rng.randi(100),
        2,
    );
    assert_eq!(expected_draws, 1, "Wait1_0 is exempt from the redraw gate");
    assert_eq!(state.fighters[0].idle.animation, expected_animation);
    assert_eq!(state.fighters[0].idle.frame, 0.0);
    // See the restart-with-no-draw test above for why 1, not 0.
    assert_eq!(state.fighters[0].action_frame, 1);
    assert_eq!(state.rng_seed, expected_rng.seed());
}

/// A repeated pick from a non-Wait1 current animation re-draws
/// (`inlineA0`): construct a table where the seed's first draw at the
/// second restart repeats the (non-Wait1) current animation, and confirm
/// the seed advances by two draws for that event, not one.
#[test]
fn a_repeat_pick_from_a_non_wait1_animation_redraws() {
    let seed = 12345;
    // seed 12345's first four HSD_Randi(100) draws are 61, 29, 89, 33
    // (`max` 62, 30, 90, 34). Entry weights are chosen so the first draw of
    // each restart always lands on animation 3 (weight 62 covers max <=
    // 62), forcing a redraw whenever the current animation is already 3;
    // the second draw (max 30) then always lands on animation 2 (the
    // remaining weight, 100 - 62 = 38 > 30).
    let entries = vec![
        IdleEntry {
            animation: 3,
            weight: 62,
            length: 1.0,
        },
        IdleEntry {
            animation: 2,
            weight: 38,
            length: idle_support::WAIT1_LENGTH,
        },
    ];
    let mut data = conformance::data();
    data.fighters[0].idle = Some(IdleAnimations {
        wait1_length: 1.0,
        entries: entries.clone(),
    });
    let mut game = Match::new(data, seed).unwrap();

    // Step 0: current is Wait1_0 (2, exempt from the redraw gate). The
    // first draw (max 62) picks animation 3 and is accepted immediately,
    // despite repeating nothing -- exactly one draw.
    let after_first = step(&mut game);
    assert_eq!(after_first.fighters[0].idle.animation, 3);
    let mut after_one_draw = HsdRng::new(seed);
    after_one_draw.randi(100);
    assert_eq!(after_first.rng_seed, after_one_draw.seed());

    // Step 1: current is 3 (not exempt). The next draw (max 30, from the
    // same entries) again picks animation 3 -- equal to current -- so
    // inlineA0's gate forces a redraw; the following draw (max 90) picks
    // animation 2, which differs and is accepted. Two draws this event.
    let after_second = step(&mut game);
    assert_eq!(after_second.fighters[0].idle.animation, 2);
    assert_eq!(after_second.fighters[0].idle.frame, 0.0);
    assert_eq!(after_second.fighters[0].action_frame, 1);
    let mut after_three_draws = HsdRng::new(seed);
    after_three_draws.randi(100);
    after_three_draws.randi(100);
    after_three_draws.randi(100);
    assert_eq!(after_second.rng_seed, after_three_draws.seed());

    // Cross-check directly against the pure helper: from current = 3, the
    // same two draws (30 then 90's underlying value) must report exactly
    // two draws consumed.
    let mut redraw_rng = HsdRng::new(seed);
    redraw_rng.randi(100); // discard step 0's own draw
    let (animation, draws) = pick(
        entries.iter().map(|entry| (entry.animation, entry.weight)),
        || redraw_rng.randi(100),
        3,
    );
    assert_eq!((animation, draws), (2, 2));
}

/// The idle animation phase draws from the match's shared RNG before the
/// same frame's blast-zone death draw (`simulation::advance`: the
/// animation-phase loop reassigns `state.rng_seed` before the death block
/// reconstructs its own `HsdRng` from it), so an idle table's draw shifts a
/// later death roll's numeric value exactly as the seed hand-off intends.
/// A real hit (not a synthetic position/`force_normal_top` shortcut -- a
/// grounded fighter can never exceed a valid `blast.top`, since stage
/// validation requires `floor.y < blast.top` strictly) launches fighter 0
/// into a top blast-death; fighter 0 also holds an idle resource that
/// fires on every Wait frame it is still standing (before the hit lands),
/// so the death roll's position in the shared `HsdRng` sequence differs
/// between "resource present" and "resource absent" by exactly that many
/// draws, and (with a threshold between the two resulting rolls) selects a
/// different `Kind`.
#[test]
fn the_idle_draw_shifts_the_same_frames_death_draw() {
    let seed = 17u32;
    // seed 17's HSD_Randi(100) sequence is 0, 40, 45 (`max` 1, 41, 46).
    // Empirically (see docs/idle.md), fighter 0 (the eventual victim) is
    // still in Wait for exactly the first two frames of this scenario
    // (the attacker's jab startup frame, then the frame its hitbox lands),
    // each drawing once from the always-current single-entry table below;
    // the death roll itself fires two frames later, once fighter 0's
    // knockback carries it past the (still ordinarily valid) blast top.
    // With the idle resource present, those 2 draws shift the death roll
    // from the sequence's 1st value (0, roll 1) to its 3rd (45, roll 46).
    // screen_chance_percent 1 makes the two rolls select different Kinds:
    // 1 >= 1 (UpScreen, resource absent) vs. 1 >= 46 is false (UpStar,
    // resource present).
    let without_idle = death_hit_data(seed);
    let with_idle = {
        let mut data = death_hit_data(seed);
        data.fighters[0].idle = Some(IdleAnimations {
            wait1_length: 1.0,
            entries: vec![IdleEntry {
                animation: 2,
                weight: 100,
                length: 1.0,
            }],
        });
        data
    };

    let without_state = drive_to_death(without_idle, seed);
    assert_eq!(without_state.fighters[0].death.kind, Some(Kind::UpScreen));
    let mut expected_without = HsdRng::new(seed);
    expected_without.randi(100); // the death roll, the sequence's 1st draw.
    assert_eq!(without_state.rng_seed, expected_without.seed());

    let with_state = drive_to_death(with_idle, seed);
    assert_eq!(with_state.fighters[0].death.kind, Some(Kind::UpStar));
    let mut expected_with = HsdRng::new(seed);
    expected_with.randi(100); // fighter 0's idle draw, attack-startup frame.
    expected_with.randi(100); // fighter 0's idle draw, the hit-landing frame.
    expected_with.randi(100); // the death roll, now the sequence's 3rd draw.
    assert_eq!(with_state.rng_seed, expected_with.seed());
}

/// `tests/game_death.rs`'s own proven star/screen scenario shape (a
/// zero-hitlag, zero-gravity, straight-up jab launching the target off the
/// (lowered) top blast line), reused here rather than re-derived.
fn death_hit_data(_seed: u32) -> MatchData {
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
    data.rules.death.as_mut().unwrap().screen_chance_percent = 1;
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

/// Player 1 presses A (jab) from frame 0; player 0 is the target. Neither
/// the idle resource nor its RNG draws affect this physics/hit timing (no
/// system besides `fighter::death`/`game::idle` consumes the match RNG at
/// all), so `DeathStarted` reliably fires on the same frame regardless.
fn drive_to_death(data: MatchData, seed: u32) -> State {
    let mut game = Match::new(data, seed).unwrap();
    let mut inputs = IDLE;
    inputs[1].buttons = skirmish::game::BUTTON_A;
    game.step(inputs).unwrap();
    for _ in 0..12 {
        let state = step(&mut game);
        if state
            .events
            .iter()
            .any(|event| matches!(event, skirmish::game::Event::DeathStarted { .. }))
        {
            return state;
        }
    }
    panic!("expected the hit to eventually reach a top blast-death");
}

#[test]
fn checkpoints_restore_the_idle_state_exactly_across_a_pick() {
    let mut data = conformance::data();
    data.fighters[0].idle = Some(idle_support::animations(&idle_support::TWO_ENTRY));
    let mut game = Match::new(data, 42).unwrap();
    step(&mut game);

    let checkpoint = game.checkpoint();
    let saved = game.state().fighters[0].idle;
    let saved_seed = game.state().rng_seed;

    // Advance across the pick (the second remaining step to the boundary),
    // plus a few more frames into the next cycle.
    let mut expected = vec![];
    for _ in 0..6 {
        expected.push(step(&mut game));
    }

    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state().fighters[0].idle, saved);
    assert_eq!(game.state().rng_seed, saved_seed);
    for expected in expected {
        assert_eq!(step(&mut game), expected);
    }
}

#[test]
fn invalid_idle_resources_are_rejected_without_constructing_a_match() {
    let mut zero_wait1 = conformance::data();
    zero_wait1.fighters[0].idle = Some(IdleAnimations {
        wait1_length: 0.0,
        entries: vec![],
    });
    assert!(Match::new(zero_wait1, 42).is_err());

    let mut non_finite_wait1 = conformance::data();
    non_finite_wait1.fighters[0].idle = Some(IdleAnimations {
        wait1_length: f32::NAN,
        entries: vec![],
    });
    assert!(Match::new(non_finite_wait1, 42).is_err());

    let mut zero_weight = conformance::data();
    zero_weight.fighters[0].idle = Some(IdleAnimations {
        wait1_length: 3.0,
        entries: vec![IdleEntry {
            animation: 3,
            weight: 0,
            length: 5.0,
        }],
    });
    assert!(Match::new(zero_weight, 42).is_err());

    let mut zero_length = conformance::data();
    zero_length.fighters[0].idle = Some(IdleAnimations {
        wait1_length: 3.0,
        entries: vec![IdleEntry {
            animation: 3,
            weight: 100,
            length: 0.0,
        }],
    });
    assert!(Match::new(zero_length, 42).is_err());

    // getAnimID's walk asserts once the accumulated weight can fall short
    // of `max` (ftwaitanim.c:50-59); weights below 100 are rejected.
    let mut short_weights = conformance::data();
    short_weights.fighters[0].idle = Some(IdleAnimations {
        wait1_length: 3.0,
        entries: vec![
            IdleEntry {
                animation: 2,
                weight: 40,
                length: 3.0,
            },
            IdleEntry {
                animation: 3,
                weight: 59,
                length: 5.0,
            },
        ],
    });
    assert!(Match::new(short_weights, 42).is_err());
}

#[test]
fn without_the_resource_wait_keeps_the_unbounded_integer_age() {
    let mut game = Match::new(conformance::data(), 42).unwrap();
    let mut state = step(&mut game);
    assert_eq!(state.fighters[0].action, Action::Wait);
    assert_eq!(state.fighters[0].idle.animation, 2);
    for expected in 2..30u32 {
        state = step(&mut game);
        assert_eq!(state.fighters[0].action, Action::Wait);
        assert_eq!(state.fighters[0].action_frame, expected);
        assert_eq!(state.fighters[0].idle.animation, 2);
        assert_eq!(state.fighters[0].idle.frame, 0.0);
    }
}
