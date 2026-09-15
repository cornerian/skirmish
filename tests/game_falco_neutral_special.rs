//! Falco's own neutral special (Laser), wired through `Specials` with the
//! Falco character key rather than Fox's for the first time (`docs/falco.md`): the
//! exact same `scripts/fighters/fox.luau` `neutral` state machine and
//! `game::projectile` system Fox's own Blaster already uses (see that
//! module's doc and `tests/falco_laser_table_differential.rs`'s own
//! confirmation that the pinned decomp gives "Falco laser" the
//! byte-identical logic-table stanza "Fox laser" uses), now exercised with
//! Falco's own exporter-confirmed numbers: slower speed (`5.0` vs. Fox's
//! `7.0`), a much longer lifetime (`100` vs. `35` frames), and -- the one
//! genuine gameplay difference this batch found -- **nonzero knockback**
//! (`growth: 100, fixed: 5` vs. Fox's all-zero, which always produces
//! exactly `[0.0, 0.0]`). Whether that nonzero knockback pushes Falco's own
//! laser past the universal one-frame hitstun minimum every confirmed hit
//! gets regardless of knockback (`fighter::combat::initial_hitstun`)
//! depends on a real match's own weight/defense/`hitstun_scale` rules, which
//! this suite's own invented `tests/fixtures/game/integration-match.json`
//! rules do not reproduce -- with those rules, it does not (still exactly
//! `1`); the difference this test proves directly is the knockback vector
//! itself, real Melee's actual rules aside.
//!
//! `tests/game_neutral_special.rs` already covers the shared state
//! machine end to end (thresholds, Start/Loop/End cycling, staling, ground-
//! leaves-to-Fall, terrain despawn, script-driven timing); this file does
//! not re-derive that coverage, only what is different when `Specials::
//! Falco` is the one firing.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/falco_neutral_special.rs"]
mod falco_neutral_special_resources;

use skirmish::game::{
    Action, BUTTON_B, BUTTON_L, Controller, Event, Match, data::MatchData,
    projectile::ProjectileKind,
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    falco_neutral_special_resources::profile(conformance::data())
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

fn hold_shield() -> Controller {
    Controller {
        buttons: BUTTON_L,
        ..Controller::default()
    }
}

fn spawned_kind(state: &skirmish::game::State, owner: usize) -> Option<ProjectileKind> {
    state.events.iter().find_map(|event| match event {
        Event::ProjectileSpawned {
            owner: o,
            projectile_kind,
        } if *o == owner => Some(*projectile_kind),
        _ => None,
    })
}

fn spawned_count(state: &skirmish::game::State, owner: usize) -> usize {
    state
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ProjectileSpawned { owner: o, .. } if *o == owner
            )
        })
        .count()
}

fn hit_this_frame(state: &skirmish::game::State, owner: usize, victim: usize) -> bool {
    state.events.iter().any(
        |event| matches!(event, Event::ProjectileHit { owner: o, victim: v } if *o == owner && *v == victim),
    )
}

/// Runs fighter 0 through the Start-to-Loop entry transition and returns the
/// frame whose Loop row 0 fires the laser. The entry callback's own extra
/// animation advance makes the two-frame Start clip transition on this second
/// step; the command row is sampled before the Loop frame advances.
fn fire(game: &mut Match) -> skirmish::game::State {
    game.step(input(0, press_b())).unwrap();
    game.step(IDLE).unwrap().clone()
}

#[test]
fn falco_plays_the_shared_neutral_special_state_machine() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
}

#[test]
fn falcos_shot_is_tagged_as_his_own_laser_kind() {
    let mut game = Match::new(data(), 0).unwrap();
    let spawn_state = fire(&mut game);
    assert_eq!(
        spawned_kind(&spawn_state, 0),
        Some(ProjectileKind::FalcoLaser),
        "a laser fired by a Specials::Falco fighter must carry ProjectileKind::FalcoLaser, \
         not ::FoxLaser, even though both run through the identical generic code"
    );
}

#[test]
fn falcos_entry_command_row_fires_once_and_does_not_repeat_next_frame() {
    let mut game = Match::new(data(), 0).unwrap();
    let first = game.step(input(0, press_b())).unwrap();
    assert_eq!(first.fighters[0].action, Action::SpecialNStart);
    let entry_fire = game.step(IDLE).unwrap().clone();
    assert_eq!(entry_fire.fighters[0].action, Action::SpecialNLoop);
    assert_eq!(entry_fire.fighters[0].action_frame, 1);
    assert_eq!(spawned_count(&entry_fire, 0), 1);
    assert_eq!(
        spawned_kind(&entry_fire, 0),
        Some(ProjectileKind::FalcoLaser)
    );

    let next_frame = game.step(IDLE).unwrap().clone();
    assert_eq!(spawned_count(&next_frame, 0), 0);
    assert_eq!(next_frame.projectiles.len(), 1);
}

#[test]
fn falcos_laser_travels_at_his_own_slower_speed() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [30.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    let spawn_state = fire(&mut game);
    let position_0 = spawn_state.projectiles[0].position[0];
    let next = game.step(IDLE).unwrap();
    let position_1 = next.projectiles[0].position[0];
    // `fighters/falco.json`'s own `specials.neutral.attributes.speed`
    // (`5.0`), not Fox's `7.0` -- see the `neutral.attributes.speed`
    // resource in `scripts/fighters/fox.luau`.
    assert!(
        (position_1 - position_0 - 5.0).abs() < 1e-4,
        "{position_1} - {position_0}"
    );
}

#[test]
fn falcos_laser_hits_with_real_knockback_unlike_foxs_all_zero_laser() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    fire(&mut game);

    let mut hit_frame = None;
    for _ in 0..15 {
        let state = game.step(IDLE).unwrap().clone();
        if hit_this_frame(&state, 0, 1) {
            hit_frame = Some(state);
            break;
        }
    }
    let hit_frame = hit_frame.expect("Falco's laser must eventually hit fighter 1");
    assert_eq!(
        hit_frame.fighters[1].percent, 3.0,
        "the same 3% damage as Fox's own laser"
    );
    assert!(
        hit_frame.projectiles.is_empty(),
        "no piercing, matching Fox's own laser"
    );
    // The one real, exporter-confirmed gameplay difference this batch
    // found: Falco's `growth: 100, fixed: 5` (vs. Fox's all-zero) feeds a
    // genuinely nonzero `KnockbackHit` into the ordinary `fighter::combat::
    // knockback` formula, so `target.knockback` comes out nonzero -- unlike
    // Fox's own laser (`tests/game_neutral_special.rs`'s identical hit
    // path, all-zero attributes), which always produces exactly `[0.0,
    // 0.0]` there. Every confirmed hit still gets at least one frame of
    // hitstun regardless of knockback magnitude
    // (`fighter::combat::initial_hitstun`'s own minimum), and this suite's
    // own invented rules (`tests/fixtures/game/integration-match.json`)
    // happen not to push Falco's own small `fixed: 5` past that minimum --
    // real Melee's own weight/defense/`hitstun_scale` constants are not
    // reproduced here, so this test does not claim a specific frame count,
    // only the knockback vector itself.
    assert_ne!(
        hit_frame.fighters[1].knockback,
        [0.0, 0.0],
        "Falco's laser must impart real, nonzero knockback"
    );
    assert!(hit_frame.fighters[1].hitstun >= 1);
}

#[test]
fn falcos_laser_still_bounces_off_a_shield_like_foxs_does() {
    // The shield-bounce/Reflector paths are entirely generic over
    // `Projectile` (`game::projectile::step` never matches on `kind`), and
    // `tests/falco_laser_table_differential.rs` confirms the pinned source
    // gives "Falco laser" the byte-identical dispatch stanza "Fox laser"
    // uses -- so this is a light smoke test proving the wiring, not a
    // second full derivation of `tests/game_neutral_special_reflect.rs`'s
    // own bounce-vector/Reflector-handoff coverage, which already exercises
    // that same generic code through Fox.
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [10.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    fire(&mut game);
    game.step(input(1, hold_shield())).unwrap();

    let mut bounced = false;
    for _ in 0..15 {
        let state = game.step(input(1, hold_shield())).unwrap().clone();
        if hit_this_frame(&state, 0, 1) {
            bounced = true;
            assert_eq!(
                state.fighters[1].percent, 0.0,
                "a shield bounce must not damage the shielding fighter"
            );
            assert!(
                !state.projectiles.is_empty(),
                "a shield bounce must not despawn the laser"
            );
            break;
        }
    }
    assert!(
        bounced,
        "the laser must contact the shield before the loop ends"
    );
}

/// Falco and Fox `Specials` both resolve `SpecialNStart`/
/// `SpecialAirNStart` (and the other four neutral-special phases) to the
/// same Slippi ids -- already covered end to end by `tests::
/// game_falco_specials::falco_and_fox_resolve_the_same_slippi_special_ids`;
/// repeated here narrowly as a same-file sanity check that this fixture's
/// own Falco `Specials` wiring reaches the real dispatcher, not just the
/// bare `game::script::definition::builtin_slippi_ids` function.
#[test]
fn falco_neutral_actions_resolve_through_the_shared_slippi_table() {
    use skirmish::game::script::definition::builtin_slippi_ids;
    assert_eq!(
        builtin_slippi_ids(Some(20), Action::SpecialNStart),
        Some((341, 295))
    );
}
