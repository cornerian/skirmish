//! The match-start warp-in (Entry/EntryStart/EntryEnd, `docs/match-start.md`).
//! Y values below are cross-checked against the replay-verified frame table
//! in the design note (trophy_scale 0.9, start/end_frames 30); this is a
//! synthetic-fixture regression, not parity evidence (see `docs/parity.md`).

use skirmish::game::{
    Action, Controller, Match,
    data::MatchData,
    entry::{EntryAnimation, EntryRules},
};
use skirmish_replay::{observation, slippi::Port};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.floor.left = -200.0;
    data.stage.floor.right = 200.0;
    data.stage.blast = [-300.0, 300.0, -300.0, 300.0];
    data.stage.spawns = [[-60.0, 10.0], [20.0, 10.0]];
    data.rules.entry = Some(EntryRules {
        start_frames: 30,
        end_frames: 30,
        scale_y: 0.0,
        invincibility_frames: 0,
    });
    for fighter in &mut data.fighters {
        fighter.trophy_scale = Some(0.9);
    }
    data
}

fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 0.0005, "{a} != {b}");
}

/// Steps `game` `count` times with `IDLE` input and returns the resulting
/// state clone after the last step.
fn run(game: &mut Match, count: usize) -> skirmish::game::State {
    let mut state = game.state().clone();
    for _ in 0..count {
        state = game.step(IDLE).unwrap().clone();
    }
    state
}

#[test]
fn spawn_enters_entry_with_the_per_slot_delay_and_no_position_change() {
    let resource = data();
    // Port slots: fighter 0 = P1 (slot 0, delay 5), fighter 1 = P4 (slot 3,
    // delay 20), matching `docs/match-start.md`'s replay table columns.
    let game = Match::new_with_slots(resource.clone(), 0, [0, 3]).unwrap();
    for (player, delay) in [(0, 5), (1, 20)] {
        let fighter = &game.state().fighters[player];
        assert_eq!(fighter.action, Action::Entry);
        assert_eq!(fighter.entry.timer, delay);
        assert_eq!(fighter.entry.y0, 10.0);
        assert_eq!(fighter.position[1], 10.0);
    }
    // gmvs.c:1780-1830: each fighter faces the other.
    assert_eq!(game.state().fighters[0].facing, 1.0);
    assert_eq!(game.state().fighters[1].facing, -1.0);
}

#[test]
fn the_replay_verified_frame_table_is_reproduced_for_slots_zero_and_three() {
    let resource = data();
    let mut game = Match::new_with_slots(resource, 0, [0, 3]).unwrap();

    // Step 1 == Melee frame -123 (both start in Entry, delay 5 and 20).
    let state = run(&mut game, 1);
    for player in 0..2 {
        assert_eq!(state.fighters[player].action, Action::Entry);
        assert_eq!(state.fighters[player].position[1], 10.0);
    }

    // Step 6 == Melee frame -118: slot 0 (delay 5) has just entered
    // EntryStart; slot 3 (delay 20) is still in Entry.
    let state = run(&mut game, 5);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    near(state.fighters[0].position[1], 10.045);
    assert_eq!(state.fighters[1].action, Action::Entry);

    // Step 21 == Melee frame -103: slot 0 is 15 frames into EntryStart;
    // slot 3 has just entered it.
    let state = run(&mut game, 15);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    near(state.fighters[0].position[1], 10.719);
    assert_eq!(state.fighters[1].action, Action::EntryStart);
    near(state.fighters[1].position[1], 10.045);

    // Step 35 == Melee frame -89: slot 0 has just entered EntryEnd at full
    // amplitude; slot 3 is still in EntryStart.
    let state = run(&mut game, 14);
    assert_eq!(state.fighters[0].action, Action::EntryEnd);
    near(state.fighters[0].position[1], 11.348);
    assert_eq!(state.fighters[1].action, Action::EntryStart);

    // Step 50 == Melee frame -74: slot 0 is 15 frames into EntryEnd; slot 3
    // has just entered EntryEnd at full amplitude.
    let state = run(&mut game, 15);
    assert_eq!(state.fighters[0].action, Action::EntryEnd);
    near(state.fighters[0].position[1], 10.674);
    assert_eq!(state.fighters[1].action, Action::EntryEnd);
    near(state.fighters[1].position[1], 11.348);

    // Step 65 == Melee frame -59: slot 0 has exited into ordinary Fall.
    let state = run(&mut game, 15);
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert!(!state.fighters[0].grounded);
}

#[test]
fn entrystart_age_counts_from_zero_and_entry_and_entryend_stay_at_the_spawn_action_frame_convention()
 {
    let resource = data();
    let mut game = Match::new_with_slots(resource, 0, [0, 1]).unwrap();
    // `action_frame` is Skirmish's internal per-frame counter; observation
    // publishes `action_frame - 1` for EntryStart specifically
    // (`crates/skirmish-replay/src/observation.rs`) to recover the
    // replay-verified 0-based state_age -- covered directly at that layer's
    // own unit tests. Here we only check the underlying counter resets to 1
    // (one frame already elapsed) on EntryStart's own first externally
    // observed frame, and keeps incrementing by exactly one every frame
    // after.
    let state = run(&mut game, 6);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    assert_eq!(state.fighters[0].action_frame, 1);
    let state = run(&mut game, 14);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    assert_eq!(state.fighters[0].action_frame, 15);
}

#[test]
fn entry_lands_on_a_floor_within_reach() {
    let mut resource = data();
    // Spawn just above a floor so the entry box lands mid-EntryStart.
    resource.stage.spawns = [[-60.0, 0.05], [20.0, 10.0]];
    resource.stage.floor.y = 0.0;
    let mut game = Match::new_with_slots(resource, 0, [0, 1]).unwrap();
    let mut landed = false;
    for _ in 0..90 {
        let state = game.step(IDLE).unwrap();
        if state.fighters[0].grounded {
            landed = true;
            assert_eq!(state.fighters[0].action, Action::Landing);
            break;
        }
    }
    assert!(landed, "entry never landed on a floor within reach");
}

#[test]
fn post_entryend_invincibility_applies_only_when_nonzero() {
    let mut resource = data();
    resource.rules.entry.as_mut().unwrap().invincibility_frames = 12;
    let mut game = Match::new_with_slots(resource, 0, [0, 1]).unwrap();
    // Slot 0's delay 5 + start_frames 30 + end_frames 30 == 65 frames in
    // Entry. The generic per-frame invincibility countdown (shared by every
    // other action) decrements once more within this same transition frame
    // before it is externally observed, matching `action_frame`'s identical
    // same-frame-tail behavior covered above.
    let state = run(&mut game, 65);
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert_eq!(state.fighters[0].invincibility, 11);
}

#[test]
fn no_input_or_side_effects_are_processed_while_in_an_entry_state() {
    let resource = data();
    let mut game = Match::new_with_slots(resource, 0, [0, 1]).unwrap();
    let mut input = IDLE;
    input[0] = Controller {
        buttons: skirmish::game::BUTTON_A | skirmish::game::BUTTON_X,
        stick: [1.0, 0.0],
        ..Controller::default()
    };
    let state = game.step(input).unwrap();
    // No jab, no jump-squat, no walk: Entry's IASA is empty.
    assert_eq!(state.fighters[0].action, Action::Entry);
    assert_eq!(state.fighters[0].position[0], -60.0);
}

#[test]
fn none_keeps_the_pre_batch_fall_start_and_the_player_zero_facing_hardcode() {
    let mut resource = data();
    resource.rules.entry = None;
    // An asymmetric spawn layout that the general gmvs.c facing rule would
    // read differently from the hardcode, confirming the hardcode is
    // exactly what still runs when `rules.entry` is absent.
    resource.stage.spawns = [[60.0, 10.0], [-60.0, 10.0]];
    let game = Match::new(resource, 0).unwrap();
    for fighter in &game.state().fighters {
        assert_eq!(fighter.action, Action::Fall);
    }
    assert_eq!(game.state().fighters[0].facing, 1.0);
    assert_eq!(game.state().fighters[1].facing, -1.0);
}

#[test]
fn checkpoint_restores_mid_sequence_entry_state_exactly() {
    let resource = data();
    let mut game = Match::new_with_slots(resource, 0, [0, 1]).unwrap();
    run(&mut game, 20);
    let checkpoint = game.checkpoint();
    let before = game.state().clone();
    run(&mut game, 10);
    assert_ne!(game.state(), &before);
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state(), &before);
    // Replaying from the checkpoint reproduces the same continuation.
    let replayed = run(&mut game, 10);
    game.restore_checkpoint(&checkpoint).unwrap();
    let again = run(&mut game, 10);
    assert_eq!(replayed, again);
}

#[test]
fn invalid_entry_rules_are_rejected() {
    let mut resource = data();
    resource.rules.entry.as_mut().unwrap().start_frames = 0;
    assert!(Match::new(resource.clone(), 0).is_err());

    let mut resource = data();
    resource.rules.entry.as_mut().unwrap().end_frames = 0;
    assert!(Match::new(resource.clone(), 0).is_err());

    let mut resource = data();
    resource.fighters[0].trophy_scale = Some(f32::NAN);
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    resource.fighters[0].trophy_scale = Some(-1.0);
    assert!(Match::new(resource, 0).is_err());
}

#[test]
fn invalid_entry_animation_is_rejected() {
    let mut resource = data();
    resource.fighters[0].entry = Some(EntryAnimation { start_frames: 0 });
    assert!(Match::new(resource, 0).is_err());
}

/// Slippi's recorded `state_age` for EntryStart is the character's own
/// (much shorter) animation frame, not the 30-frame action duration: Fox's
/// real `fox-fd.slp` recording advances 0..10 then holds at 10 for the rest
/// of EntryStart (an 11-frame figatree), confirmed directly against the
/// replay while position keeps changing correctly under the unrelated
/// `x6BC` timer. This is `docs/match-start.md`'s frame table with the age
/// column filled in.
#[test]
fn entrystart_reported_age_holds_at_the_figatree_length_when_supplied() {
    let mut resource = data();
    resource.fighters[0].entry = Some(EntryAnimation { start_frames: 11 });
    let mut game = Match::new_with_slots(resource, 0, [0, 3]).unwrap();

    let observe = |game: &Match| observation::observe(game, [Port::P1, Port::P4], [2, 2]);

    // Step 6 == Melee frame -118: EntryStart's first frame, age 0.
    let state = run(&mut game, 6);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    assert_eq!(observe(&game).fighters[0].action_age, 0.0);

    // Step 16 == Melee frame -108: age reaches the figatree's last frame
    // (10) and holds there.
    let state = run(&mut game, 10);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    assert_eq!(observe(&game).fighters[0].action_age, 10.0);

    // Step 34 == Melee frame -90: still EntryStart, still held at 10.
    let state = run(&mut game, 18);
    assert_eq!(state.fighters[0].action, Action::EntryStart);
    assert_eq!(observe(&game).fighters[0].action_age, 10.0);

    // Step 35 == Melee frame -89: EntryEnd, animation-less, age -1.
    let state = run(&mut game, 1);
    assert_eq!(state.fighters[0].action, Action::EntryEnd);
    assert_eq!(observe(&game).fighters[0].action_age, -1.0);
}

#[test]
fn entrystart_reported_age_stays_uncapped_without_the_animation_resource() {
    // `data()` never sets `fighters[0].entry`: the pre-existing
    // approximation (uncapped `action_frame`-based age) still applies.
    let resource = data();
    let mut game = Match::new_with_slots(resource, 0, [0, 3]).unwrap();
    run(&mut game, 34);
    assert_eq!(game.state().fighters[0].action, Action::EntryStart);
    let observed = observation::observe(&game, [Port::P1, Port::P4], [2, 2]);
    assert_eq!(observed.fighters[0].action_age, 28.0);
}
