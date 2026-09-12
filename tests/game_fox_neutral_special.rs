//! Fox/Falco neutral special (Blaster): the Start/Loop/End state machine
//! and the minimal generic projectile system it fires through. See
//! `docs/fox-neutral-special.md` for the full citation list and what
//! remains unmodeled.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_neutral_special.rs"]
mod neutral_special_resources;

use skirmish::game::{Action, BUTTON_B, Controller, Event, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    neutral_special_resources::profile(conformance::data())
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn press_b() -> Controller {
    Controller {
        buttons: BUTTON_B,
        ..Controller::default()
    }
}

fn spawned_this_frame(state: &skirmish::game::State, owner: usize) -> bool {
    state
        .events
        .iter()
        .any(|event| matches!(event, Event::ProjectileSpawned { owner: o, .. } if *o == owner))
}

fn hit_this_frame(state: &skirmish::game::State, owner: usize, victim: usize) -> bool {
    state.events.iter().any(
        |event| matches!(event, Event::ProjectileHit { owner: o, victim: v } if *o == owner && *v == victim),
    )
}

#[test]
fn none_keeps_b_inert() {
    let mut plain = conformance::data();
    plain.fighters[0].specials = None;
    let mut game = Match::new(plain, 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialNStart);
}

#[test]
fn strict_thresholds_gate_the_grounded_entry() {
    let mut game = Match::new(data(), 0).unwrap();
    // The fixture's own thresholds are [0.5, 0.5]; 0.5 itself must fail
    // (strict), matching the retained `neutral_input`/`ftCo_800D67C4`
    // boundary.
    let boundary = Controller {
        buttons: BUTTON_B,
        stick: [0.5, 0.0],
        ..Controller::default()
    };
    let state = game.step(input(0, boundary)).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialNStart);

    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
}

#[test]
fn aerial_entry_selects_the_air_variant() {
    let mut resource = data();
    resource.stage.spawns[0][1] = 6.0;
    let mut game = Match::new(resource, 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirNStart);
}

#[test]
fn a_fresh_b_press_repeats_the_loop_while_no_press_ends_it() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // Two-frame Start pose (per the fixture): `ftFx_SpecialN_Enter`'s own
    // extra `ftAnim_8006EBA4` advance (`docs/validation.md`'s entry-advance
    // table) starts the entry frame's `action_frame` at 1 (2 after this
    // same step's generic increment), so the clip (indices 0..=1) already
    // runs out one idle frame later.
    let entered_loop = game.step(IDLE).unwrap();
    assert_eq!(entered_loop.fighters[0].action, Action::SpecialNLoop);
    assert!(spawned_this_frame(entered_loop, 0));

    // A fresh B press during Loop arms the repeat.
    game.step(input(0, press_b())).unwrap();
    let repeated = game.step(IDLE).unwrap();
    assert_eq!(repeated.fighters[0].action, Action::SpecialNLoop);
    assert!(
        spawned_this_frame(repeated, 0),
        "a repeated Loop cycle fires another shot"
    );
}

#[test]
fn no_repeat_press_ends_the_move_and_returns_to_wait() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut saw_loop = false;
    let mut saw_end = false;
    let mut reached_wait = false;
    for _ in 0..20 {
        let state = game.step(IDLE).unwrap();
        match state.fighters[0].action {
            Action::SpecialNLoop => saw_loop = true,
            Action::SpecialNEnd => saw_end = true,
            Action::Wait if saw_loop && saw_end => {
                reached_wait = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_loop, "the move must enter Loop at least once");
    assert!(
        saw_end,
        "no repeat press must end the move through SpecialNEnd"
    );
    assert!(reached_wait, "SpecialNEnd must return to Wait");
}

#[test]
fn the_laser_travels_before_hitting_and_despawns_on_contact() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // The Start clip already runs out one idle frame later than a naive
    // frame count would suggest -- see
    // `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s own
    // comment.
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));
    assert_eq!(spawn_state.projectiles.len(), 1);
    let spawn_position = spawn_state.projectiles[0].position;

    let mut hit_frame = None;
    for _ in 0..10 {
        let state = game.step(IDLE).unwrap().clone();
        if hit_this_frame(&state, 0, 1) {
            hit_frame = Some(state);
            break;
        }
    }
    let hit_frame = hit_frame.expect("the laser must eventually hit fighter 1");
    assert!(hit_frame.fighters[1].percent > 0.0);
    assert!(hit_frame.projectiles.is_empty(), "no piercing");
    assert_ne!(
        spawn_position[0], hit_frame.fighters[1].position[0],
        "the hit happens after travel, not at the spawn position"
    );
}

#[test]
fn staling_reduces_repeated_laser_damage() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    resource.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.5, 0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
        debug_bypass: false,
    });
    for fighter in &mut resource.fighters {
        fighter.jab.move_id = Some(10);
    }
    let mut game = Match::new(resource, 0).unwrap();
    let mut percents = Vec::new();
    for _ in 0..3 {
        game.step(input(0, press_b())).unwrap();
        game.step(IDLE).unwrap();
        game.step(IDLE).unwrap();
        for _ in 0..10 {
            let state = game.step(IDLE).unwrap().clone();
            if hit_this_frame(&state, 0, 1) {
                percents.push(state.fighters[1].percent);
                break;
            }
        }
        // Wait out End before firing again.
        for _ in 0..6 {
            game.step(IDLE).unwrap();
        }
    }
    assert_eq!(percents.len(), 3);
    let first_hit = percents[0];
    let second_hit = percents[1] - percents[0];
    let third_hit = percents[2] - percents[1];
    assert!(
        second_hit < first_hit || third_hit < first_hit,
        "a repeated laser hit must stale toward less damage: {percents:?}"
    );
}

#[test]
fn leaving_the_ground_mid_move_falls_through_to_ordinary_fall() {
    let mut resource = data();
    resource.stage.spawns[0] = [95.0, 0.0];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut walked_off = None;
    for _ in 0..40 {
        let state = game
            .step(input(
                0,
                Controller {
                    stick: [1.0, 0.0],
                    ..Controller::default()
                },
            ))
            .unwrap();
        if !state.fighters[0].grounded {
            walked_off = Some(state.fighters[0].action);
            break;
        }
    }
    // Blaster's own grounded phases have no ground->air conversion of
    // their own (`ft_80083F88`): leaving the ground always falls through
    // to the generic `Action::Fall` fallback, never `SpecialAirN*`.
    assert_eq!(walked_off, Some(Action::Fall));
}

#[test]
fn slippi_ids_cover_all_six_phases() {
    use skirmish::game::characters;
    for (action, state, animation) in [
        (Action::SpecialNStart, 341, 295),
        (Action::SpecialNLoop, 342, 296),
        (Action::SpecialNEnd, 343, 297),
        (Action::SpecialAirNStart, 344, 298),
        (Action::SpecialAirNLoop, 345, 299),
        (Action::SpecialAirNEnd, 346, 300),
    ] {
        assert_eq!(
            characters::slippi_ids(Some(2), action),
            Some((state, animation))
        );
    }
}

#[test]
fn checkpoint_round_trip_preserves_the_move_and_in_flight_projectiles() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    game.step(IDLE).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let expected = game.step(IDLE).unwrap().clone();
    game.restore_checkpoint(&checkpoint).unwrap();
    let actual = game.step(IDLE).unwrap().clone();
    assert_eq!(actual, expected);
}
