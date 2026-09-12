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
fn a_terrain_line_despawns_the_laser_before_it_reaches_the_far_fighter() {
    // A real stage-line raycast (`it_8026E9A4`), not merely the stage's own
    // outer bounding box: place a wall between the two fighters, well
    // inside the blast zone, and confirm the laser despawns against it
    // instead of reaching fighter 1 (see `game::projectile::step`'s own
    // terrain-despawn citation).
    use skirmish::collision::stage;
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    resource.stage.geometry = Some(skirmish::game::data::StageGeometry {
        lines: vec![
            stage::Line {
                start: [-100.0, 0.0],
                end: [100.0, 0.0],
                flags: stage::ENABLED | stage::FLOOR,
                material_flags: stage::LEDGE as u16,
                ..Default::default()
            },
            // A vertical wall at x=7, between the two fighters, spanning
            // well past the laser's own flight height.
            stage::Line {
                start: [7.0, -5.0],
                end: [7.0, 20.0],
                flags: stage::ENABLED | stage::LEFT_WALL,
                ..Default::default()
            },
        ],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-100.0, -5.0],
            bounds_max: [100.0, 20.0],
            floor: 0..1,
            left_wall: 1..2,
            ..Default::default()
        }],
    });
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // The Start clip already runs out one idle frame later than a naive
    // frame count would suggest -- see `docs/validation.md`'s entry-advance
    // table and `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s
    // own comment above.
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));

    let mut despawned_at_wall = false;
    for _ in 0..10 {
        let state = game.step(IDLE).unwrap().clone();
        assert!(
            !hit_this_frame(&state, 0, 1),
            "the wall must stop the laser before it ever reaches fighter 1"
        );
        if state.projectiles.is_empty() {
            despawned_at_wall = true;
            break;
        }
        assert!(
            state.projectiles[0].position[0] < 7.5,
            "the laser must not pass through the wall: {:?}",
            state.projectiles[0].position
        );
    }
    assert!(
        despawned_at_wall,
        "the laser must despawn against the wall, not merely at the blast zone"
    );
}

/// Extends the fixture's own `script`-driven coverage: a synthetic
/// `NeutralSpecial::script` shaped like this batch's own exported
/// `fighters/fox.json` real timings (Start 7 frames/`cmd_vars[0]` at
/// frame 4, Loop 10 frames/`cmd_vars[2]` at frame 5, ground and air alike),
/// grafted onto the shared fixture by repeating its own last pose to reach
/// those lengths (the fixture's bone/hitbox data is otherwise irrelevant to
/// these tests). Exercises the full engine end to end, distinct from
/// `tests/fox_neutral_special_differential.rs`'s own oracle-only trace.
mod script_resources {
    use skirmish::game::{
        characters::{
            Specials,
            fox::{
                neutral::NeutralScript,
                side::{ScriptFrames, ScriptPhase},
            },
        },
        data::{Attack, MatchData},
    };

    fn extend(attack: &mut Attack, len: usize) {
        let last = attack
            .frames
            .last()
            .expect("fixture always has a pose")
            .clone();
        while attack.frames.len() < len {
            attack.frames.push(last.clone());
        }
    }

    fn cmd_var_table(len: usize, slot: usize, set_frame: usize, value: u32) -> ScriptFrames {
        let mut cmd_vars = vec![[None; 4]; len];
        for row in &mut cmd_vars[set_frame..] {
            row[slot] = Some(value);
        }
        ScriptFrames {
            cmd_vars,
            allow_interrupt: vec![false; len],
        }
    }

    pub fn profile(mut data: MatchData) -> MatchData {
        data = super::neutral_special_resources::profile(data);
        for fighter in &mut data.fighters {
            let Some(Specials::Fox {
                neutral: Some(neutral),
                ..
            }) = &mut fighter.specials
            else {
                panic!("neutral_special_resources::profile always installs Fox's neutral");
            };
            extend(&mut neutral.start.ground, 7);
            extend(&mut neutral.start.air, 7);
            extend(&mut neutral.loop_phase.ground, 10);
            extend(&mut neutral.loop_phase.air, 10);
            neutral.script = Some(Box::new(NeutralScript {
                start: ScriptPhase {
                    ground: cmd_var_table(7, 0, 4, 1),
                    air: cmd_var_table(7, 0, 4, 1),
                },
                loop_phase: ScriptPhase {
                    ground: cmd_var_table(10, 2, 5, 1),
                    air: cmd_var_table(10, 2, 5, 1),
                },
                end: ScriptPhase {
                    ground: cmd_var_table(neutral.end.ground.frames.len(), 1, 1, 2),
                    air: cmd_var_table(neutral.end.air.frames.len(), 1, 1, 2),
                },
            }));
        }
        data
    }
}

/// A fresh B press during Start's own arm window (`cmd_vars[0]` sets at
/// frame 4) arms the repeat *before* Loop is ever entered -- behavior the
/// pre-script approximation could not produce at all (it hard-coded Start
/// as never-armed). The entry press itself is only "fresh" on entry, so
/// this releases B (`IDLE` steps) before pressing again on the exact tick
/// `action_frame` reaches 4.
#[test]
fn a_press_during_starts_own_arm_window_arms_before_loop_is_entered() {
    let mut game = Match::new(script_resources::profile(data()), 0).unwrap();
    // `action_frame` after each tick (the extra-advance hack starts it at 1,
    // then this port's generic per-tick increment runs once more the same
    // tick -- see `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s
    // own comment): entry -> 2, then +1 per further tick. A fresh press
    // lands on `action_frame == 4` (the script's own arm frame) when issued
    // on the third tick after entry.
    game.step(input(0, press_b())).unwrap(); // entry, action_frame -> 2
    game.step(IDLE).unwrap(); // action_frame 2 processed -> 3
    game.step(IDLE).unwrap(); // action_frame 3 processed -> 4
    let state = game.step(input(0, press_b())).unwrap(); // action_frame 4 processed: fresh press, arms
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);

    // Run out the rest of Start and confirm the move enters Loop and
    // repeats *without any further press*, proving the arm from Start's own
    // window survived the Start->Loop transition. `Action::SpecialNLoop`
    // reports identically for every pass, so a repeat is detected by its
    // own `action_frame` resetting back down mid-Loop (not by any action
    // change) -- watch for a low frame following an already-seen high one,
    // before `SpecialNEnd` (a genuine end, no repeat) is ever reached.
    // (A laser hit briefly freezes `action_frame` via ordinary hitlag,
    // which this loop tolerates by only requiring monotonic *eventual*
    // progress, not a fixed tick schedule.)
    let mut saw_high_frame_in_loop = false;
    let mut saw_repeat = false;
    for _ in 0..40 {
        let state = game.step(IDLE).unwrap();
        let action_frame = state.fighters[0].action_frame;
        match state.fighters[0].action {
            Action::SpecialNLoop if action_frame >= 8 => saw_high_frame_in_loop = true,
            Action::SpecialNLoop if action_frame <= 1 && saw_high_frame_in_loop => {
                saw_repeat = true;
                break;
            }
            Action::SpecialNEnd => break,
            _ => {}
        }
    }
    assert!(
        saw_repeat,
        "the Start-window arm must survive into Loop and repeat it without a fresh press"
    );
}

/// The laser fires on the script's own frame-5 of Loop, not Loop's entry
/// frame (the pre-script approximation's timing).
#[test]
fn the_shot_fires_on_the_scripts_own_frame_not_loop_entry() {
    let mut game = Match::new(script_resources::profile(data()), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut entered_loop_tick = None;
    let mut fired_tick = None;
    for tick in 0..20 {
        let state = game.step(IDLE).unwrap();
        if entered_loop_tick.is_none() && state.fighters[0].action == Action::SpecialNLoop {
            entered_loop_tick = Some(tick);
        }
        if fired_tick.is_none() && spawned_this_frame(state, 0) {
            fired_tick = Some(tick);
        }
        if fired_tick.is_some() {
            break;
        }
    }
    let entered_loop_tick = entered_loop_tick.expect("the move must enter Loop");
    let fired_tick = fired_tick.expect("the shot must fire");
    assert!(
        fired_tick > entered_loop_tick,
        "entered Loop at tick {entered_loop_tick}, fired at tick {fired_tick}: \
         the script-driven fire must lag Loop's own entry, not coincide with it"
    );
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
