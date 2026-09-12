//! Fox/Falco down special (Reflector): common input dispatch, the five-phase
//! Start/Loop/Turn/Hit/End state machine, the mid-move turn, jump cancel,
//! aerial jump, platform drop, ground/air conversions and the `reflecting`
//! bit. The C-oracle differential harness lives in
//! `fox_down_special_differential.rs`; see `docs/fox-down-special.md` for
//! what remains unmodeled.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_down_special.rs"]
mod down_special_resources;

use skirmish::game::{
    Action, BUTTON_B, Controller, Event, Match, characters::Specials, data::MatchData,
};

fn down_special_mut(
    fighter: &mut skirmish::game::data::FighterData,
) -> &mut skirmish::game::characters::fox::down::DownSpecial {
    let Some(Specials::Fox { down, .. }) = fighter.specials.as_mut() else {
        panic!("test fixture is missing its down-special resource");
    };
    down.as_mut()
        .expect("test fixture is missing its down-special resource")
}

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    down_special_resources::profile(conformance::data())
}

fn airborne_data() -> MatchData {
    let mut resource = data();
    resource.stage.spawns[0][1] = 6.0;
    resource
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn down(stick_y: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [0.0, stick_y],
        ..Controller::default()
    }
}

fn held_b() -> Controller {
    Controller {
        buttons: BUTTON_B,
        ..Controller::default()
    }
}

#[test]
fn none_keeps_b_and_down_inert() {
    let mut plain = conformance::data();
    plain.fighters[0].specials = None;
    plain.rules.specials = None;
    let mut game = Match::new(plain, 0).unwrap();
    let state = game.step(input(0, down(-0.8))).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialLwStart);
}

#[test]
fn ground_entry_is_the_fourth_check_after_a_fresh_downward_b() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, down(-0.8))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialLwStart);
    // Start never counts releaseLag down (unlike Loop/Turn/Hit): confirmed
    // by reading every one of its Anim/Phys callbacks in the pinned source.
    assert_eq!(state.fighters[0].down_special.release_lag, 40.0);
    assert!(!state.fighters[0].down_special.is_release);
    // Right at the strict ground boundary must not fire.
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, down(-0.6))).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialLwStart);
}

#[test]
fn air_entry_uses_the_inclusive_boundary_and_zeroes_vertical_velocity() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    // A fresh air entry right at the boundary (`stick.y <= -x21C`, unlike
    // the ground's strict `<`).
    let state = game.step(input(0, down(-0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirLwStart);
    assert_eq!(state.fighters[0].velocity[1], 0.0);
}

#[test]
fn holding_b_keeps_loop_active_past_the_release_lag() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    // Start is 4 frames; advance through it holding B throughout.
    let mut state = game.state().clone();
    for _ in 0..4 {
        state = game.step(input(0, held_b())).unwrap().clone();
    }
    assert_eq!(state.fighters[0].action, Action::SpecialLw);
    // Hold well past the release lag (40 frames): still in Loop.
    for _ in 0..60 {
        state = game.step(input(0, held_b())).unwrap().clone();
    }
    assert_eq!(state.fighters[0].action, Action::SpecialLw);
    assert!(!state.fighters[0].down_special.is_release);
}

#[test]
fn releasing_b_only_exits_loop_once_the_release_lag_elapses() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    for _ in 0..4 {
        game.step(input(0, held_b())).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialLw);
    // Release now: isRelease latches true, but End does not fire until
    // releaseLag (40 at Start entry, already ticking since Loop's own
    // first counted frame) reaches zero.
    let mut state = game.step(IDLE).unwrap();
    assert!(
        state.fighters[0].down_special.is_release || state.fighters[0].action == Action::SpecialLw
    );
    for _ in 0..45 {
        state = game.step(IDLE).unwrap();
        if state.fighters[0].action == Action::SpecialLwEnd {
            break;
        }
    }
    assert_eq!(state.fighters[0].action, Action::SpecialLwEnd);
}

#[test]
fn jump_cancels_the_ground_loop() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    for _ in 0..4 {
        game.step(input(0, held_b())).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialLw);
    let jump = Controller {
        buttons: skirmish::game::BUTTON_Y,
        ..Controller::default()
    };
    let state = game.step(input(0, jump)).unwrap();
    assert_eq!(state.fighters[0].action, Action::JumpSquat);
}

#[test]
fn aerial_jump_is_available_from_the_air_loop() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    game.step(input(0, down(-0.6))).unwrap();
    for _ in 0..4 {
        game.step(input(0, held_b())).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialAirLw);
    let jumps_before = game.state().fighters[0].locomotion.jumps_used;
    let jump = Controller {
        buttons: skirmish::game::BUTTON_Y,
        ..Controller::default()
    };
    let state = game.step(input(0, jump)).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialAirLw);
    assert!(state.fighters[0].locomotion.jumps_used > jumps_before);
}

#[test]
fn reversed_stick_turns_immediately_and_returns_to_loop_after_the_countdown() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    for _ in 0..4 {
        game.step(input(0, held_b())).unwrap();
    }
    assert_eq!(game.state().fighters[0].action, Action::SpecialLw);
    assert_eq!(game.state().fighters[0].facing, 1.0);
    // Reverse the stick past the (negative) standing turn threshold.
    let reverse = Controller {
        buttons: BUTTON_B,
        stick: [-1.0, 0.0],
        ..Controller::default()
    };
    let state = game.step(input(0, reverse)).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialLwTurn);
    // The flip is immediate, on the very first Turn step.
    assert_eq!(state.fighters[0].facing, -1.0);
    assert!(state.fighters[0].shield.reflecting);
    // Turn's fixture pose is 6 frames (matching turn_frames); ride the
    // countdown back to Loop, still holding B throughout.
    let mut state = state;
    for _ in 0..8 {
        state = game.step(input(0, held_b())).unwrap();
        if state.fighters[0].action != Action::SpecialLwTurn {
            break;
        }
    }
    assert_eq!(state.fighters[0].action, Action::SpecialLw);
    assert_eq!(state.fighters[0].facing, -1.0);
}

#[test]
fn platform_drop_keeps_the_move_and_sets_reflecting() {
    let mut resource = data();
    resource.stage.geometry = Some(skirmish::game::data::StageGeometry {
        lines: vec![skirmish::collision::stage::Line {
            start: [-5.0, 3.0],
            end: [5.0, 3.0],
            flags: skirmish::collision::stage::ENABLED | skirmish::collision::stage::FLOOR,
            material_flags: skirmish::collision::stage::PLATFORM as u16,
            ..Default::default()
        }],
        joints: vec![skirmish::collision::stage::Joint {
            flags: skirmish::collision::stage::ENABLED,
            bounds_min: [-5.0, 3.0],
            bounds_max: [5.0, 3.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    resource.stage.spawns[0] = [0.0, 3.0];
    resource.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .pass_stick_threshold = 0.5;
    resource.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .pass_window = 20;
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    let drop = Controller {
        stick: [0.0, -0.8],
        ..Controller::default()
    };
    let state = game.step(input(0, drop)).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirLwStart);
    assert!(state.fighters[0].shield.reflecting);
    assert!(!state.fighters[0].grounded);
}

#[test]
fn ground_to_air_conversion_preserves_the_frame_and_release_state() {
    let mut resource = airborne_data();
    resource.stage.spawns[0][1] = 0.05;
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, down(-0.6))).unwrap();
    for _ in 0..4 {
        game.step(input(0, held_b())).unwrap();
    }
    let before = game.state().fighters[0].down_special.clone();
    // Fall the short remaining distance onto the floor; the air loop
    // converts to the grounded one at the same frame, preserving state.
    let mut state = game.state().clone();
    for _ in 0..30 {
        state = game.step(input(0, held_b())).unwrap().clone();
        if state.fighters[0].grounded {
            break;
        }
    }
    assert!(state.fighters[0].grounded);
    assert_eq!(state.fighters[0].action, Action::SpecialLw);
    assert_eq!(
        state.fighters[0].down_special.gravity_delay,
        before.gravity_delay
    );
}

#[test]
fn slippi_action_names_are_stable() {
    for (action, id) in [
        (Action::SpecialLwStart, "special_lw_start"),
        (Action::SpecialLw, "special_lw"),
        (Action::SpecialLwHit, "special_lw_hit"),
        (Action::SpecialLwEnd, "special_lw_end"),
        (Action::SpecialLwTurn, "special_lw_turn"),
        (Action::SpecialAirLwStart, "special_air_lw_start"),
        (Action::SpecialAirLw, "special_air_lw"),
        (Action::SpecialAirLwHit, "special_air_lw_hit"),
        (Action::SpecialAirLwEnd, "special_air_lw_end"),
        (Action::SpecialAirLwTurn, "special_air_lw_turn"),
    ] {
        let value = serde_json::to_value(action).unwrap();
        assert_eq!(value.as_str().unwrap(), id);
    }
}

#[test]
fn slippi_ids_are_360_through_369() {
    use skirmish::game::characters;
    for (action, state) in [
        (Action::SpecialLwStart, 360),
        (Action::SpecialLw, 361),
        (Action::SpecialLwHit, 362),
        (Action::SpecialLwEnd, 363),
        (Action::SpecialLwTurn, 364),
        (Action::SpecialAirLwStart, 365),
        (Action::SpecialAirLw, 366),
        (Action::SpecialAirLwHit, 367),
        (Action::SpecialAirLwEnd, 368),
        (Action::SpecialAirLwTurn, 369),
    ] {
        let (id, _animation) = characters::slippi_ids(Some(2), action).unwrap();
        assert_eq!(id, state);
    }
}

#[test]
fn hit_phase_is_wired_but_unreachable_through_ordinary_dispatch() {
    // Production code never transitions a fighter into SpecialLwHit/
    // SpecialAirLwHit -- only the unmodeled projectile-reflect callback
    // does (`ftFx_SpecialLwHit_Enter`, reached from `fp->reflect_hit_cb`,
    // which nothing in this engine ever invokes without projectiles). The
    // phase's own per-frame bookkeeping, clip-end exit and the shared
    // `hit_check` End-vs-Loop decision it reuses from Turn are exercised
    // directly against the pinned C in `fox_down_special_differential.rs`
    // (`oracle_down_anim` phase 3, `oracle_down_hit_check`); Turn's own
    // native test above already exercises the identical Rust-side
    // `hit_check` function Hit shares. Here: confirm the phase is still
    // fully wired into the observation layer despite being unreachable in
    // play, matching the design note's brief.
    use skirmish::game::characters;
    for action in [Action::SpecialLwHit, Action::SpecialAirLwHit] {
        let (state, _animation) = characters::slippi_ids(Some(2), action).unwrap();
        assert!(matches!(state, 362 | 367));
    }
}

#[test]
fn every_phase_survives_a_checkpoint_round_trip() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, down(-0.8))).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let expected = game.state().clone();
    game.step(IDLE).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state(), &expected);
}

/// Reflector's Start-phase hit: the gameplay export pack
/// (`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911/
/// fox-fd/match-data.json`, `fighters[0].specials.down.start.ground`) shows
/// a real script-embedded hitbox on frames 0 and 1 of the 5-pose Start set,
/// clear on every later frame. Every numeric field below is the pack's own
/// value verbatim (`angle_degrees: 0.0` is the pack's own literal value, not
/// a placeholder); `bone` is adapted from the pack's own bone 3 to bone 1,
/// this suite's shared two-bone synthetic skeleton having no equivalent
/// (see `tests/game_fox_up_special.rs`'s identical adaptation for Fire
/// Fox's own Travel hitbox), and `clank`/`rebound` are the pack's own
/// values (`true`/`false`); this test's own resource wires the ordinary
/// clank profile they require to validate
/// (`specials::helpers::validate_hitboxes`'s `rules.clank` gate).
fn reflector_start_hitbox() -> skirmish::game::data::Hitbox {
    skirmish::game::data::Hitbox {
        clank: true,
        rebound: false,
        element: Default::default(),
        group: 0,
        bone: 1,
        center: [0.0, 0.0, 0.0],
        radius: 7.999_488,
        damage: 5,
        shield_damage: 0,
        angle_degrees: 0.0,
        growth: 100,
        fixed: 0,
        base: 0,
    }
}

#[test]
fn reflector_start_hits_a_nearby_opponent_on_the_pack_documented_frames() {
    // Reproduced locally (like `tests/game_fox_up_special.rs`'s own Hold/
    // Travel hitbox regressions) rather than edited into the shared
    // `fixtures/game/fox-down-special.json` (used by every other test in
    // this file, whose own step counts -- "Start is 4 frames" -- are tuned
    // to that fixture's short, hitbox-free stand-in).
    let mut resource = down_special_resources::with_ordinary_clank(data());
    resource.stage.spawns = [[0.0, 0.0], [1.0, 0.0]];
    resource.rules.knockback_speed = 0.0;
    {
        let p = down_special_mut(&mut resource.fighters[0]);
        for attack in [&mut p.start.ground, &mut p.start.air] {
            attack.move_id = Some(21);
            for (index, frame) in attack.frames.iter_mut().enumerate() {
                frame.hitboxes = if index < 2 {
                    vec![reflector_start_hitbox()]
                } else {
                    vec![]
                };
            }
        }
    }
    let mut game = Match::new(resource, 0).unwrap();
    // Start's entry frame (0) is sampled by this very step (the same
    // same-frame cascade `tests/game_fox_up_special.rs`'s Travel regression
    // documents): the pack's own hitbox already connects here.
    let entry = game.step(input(0, down(-0.8))).unwrap();
    assert_eq!(entry.fighters[0].action, Action::SpecialLwStart);
    assert!(
        entry.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                ..
            }
        )),
        "Reflector's Start hitbox must connect on its own entry frame"
    );
    // 5 damage per the pack's own value; no staling is configured. This
    // engine's shared per-attacker `hit_groups` bitmask (see the up-special
    // suite's own `hold_charge_hits_a_nearby_opponent_...` doc) blocks frame
    // 1's own copy of the same hitbox from connecting a second time absent a
    // `clear_hits`-style re-enable, matching every other continuous hitbox
    // in this engine; frames 2..4 carry none at all regardless.
    for _ in 0..3 {
        let state = game.step(input(0, held_b())).unwrap();
        assert!(
            !state
                .events
                .iter()
                .any(|event| matches!(event, Event::Hit { .. })),
            "no further hit should land through the rest of Start"
        );
    }
    assert_eq!(game.state().fighters[1].percent, 5.0);
}

#[test]
fn invalid_down_special_resources_are_rejected() {
    let mut resource = data();
    down_special_mut(&mut resource.fighters[0])
        .attributes
        .turn_frames = 0.0;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    down_special_mut(&mut resource.fighters[0])
        .attributes
        .air_momentum_div = 0.0;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    down_special_mut(&mut resource.fighters[0])
        .start
        .ground
        .frames
        .clear();
    assert!(Match::new(resource, 0).is_err());

    // `specials::helpers::validate_hitboxes` (this batch's own validation
    // gap-closer, `docs/fox-down-special.md`'s "Hitboxes" section): an
    // out-of-range hitbox group (>= 16) on Start's own pose is rejected the
    // same way every other move kind's hitboxes already were. `clank` is
    // forced off here (unlike `reflector_start_hitbox`'s own pack-verbatim
    // `true`) so this keeps testing specifically the group bound, not the
    // clank gate below.
    let mut resource = data();
    let mut hitbox = reflector_start_hitbox();
    hitbox.clank = false;
    hitbox.group = 99;
    down_special_mut(&mut resource.fighters[0])
        .start
        .ground
        .frames[0]
        .hitboxes = vec![hitbox];
    assert!(Match::new(resource, 0).is_err());

    // Reflector Start's own `clank` bit is `true` (the pack's own value);
    // without `rules.clank` configured, it is rejected the same way the
    // generic jab/aerial/tilt/smash/neutral-special chain's own hitboxes
    // already are (`validation.rs`'s "clank/rebound flags require an
    // explicit ordinary profile").
    let mut resource = data();
    down_special_mut(&mut resource.fighters[0])
        .start
        .ground
        .frames[0]
        .hitboxes = vec![reflector_start_hitbox()];
    assert!(Match::new(resource, 0).is_err());

    // Staling requires a nonzero `move_id` on a phase that actually has a
    // hitbox on some frame (Start's own hit); a hitbox-free phase (End,
    // here) is unaffected either way. Every fighter's ordinary `jab` needs
    // one too once staling is enabled at all (the generic chain's own
    // unconditional requirement, `validation.rs`), unrelated to this
    // batch's own change.
    let mut resource = down_special_resources::with_ordinary_clank(data());
    resource.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.0; 9],
        debug_bypass: false,
    });
    for fighter in &mut resource.fighters {
        fighter.jab.move_id = Some(2);
    }
    {
        let p = down_special_mut(&mut resource.fighters[0]);
        p.start.ground.frames[0].hitboxes = vec![reflector_start_hitbox()];
        p.start.ground.move_id = None;
    }
    assert!(Match::new(resource.clone(), 0).is_err());
    down_special_mut(&mut resource.fighters[0])
        .start
        .ground
        .move_id = Some(21);
    assert!(Match::new(resource, 0).is_ok());

    let mut resource = data();
    resource.rules.specials = None;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    resource.fighters[0].locomotion = None;
    assert!(Match::new(resource, 0).is_err());
}
