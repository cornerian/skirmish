//! Native integration coverage for the Blaster laser's shield-bounce and
//! Reflector hand-off paths, and the Reflector eligibility gate. These were
//! flagged in `docs/fox-neutral-special.md` as implemented (per the exact
//! decomp call chain cited in `src/game/projectile.rs`) but not covered by
//! an automated test; this file closes that gap.
//!
//! Fighter 0 carries the Blaster (`characters::fox::neutral`); fighter 1
//! carries the Reflector (`characters::fox::down`), so a laser fired by 0
//! can be shield-bounced or Reflector-caught by 1. The two fixtures are
//! merged by hand (each `support` profile otherwise installs its own
//! `Specials::Fox` wholesale on both fighters, clobbering the other move).

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_down_special.rs"]
mod down_special_resources;
#[path = "support/fox_neutral_special.rs"]
mod neutral_special_resources;

use skirmish::characters::Specials;
use skirmish::game::{Action, BUTTON_B, BUTTON_L, Controller, Event, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

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

fn down_input(stick_y: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [0.0, stick_y],
        ..Controller::default()
    }
}

/// `damage_mul` is overridden per test (the fixture's own default is `1.0`,
/// which cannot distinguish the correct `damage * mul + 0.99` truncation
/// from the buggy plain product this batch fixed -- both round to the same
/// integer when `mul == 1.0`). `reflect.offset`/`size` are also overridden:
/// the down-special fixture's own values (`y = 8`, far above the Blaster's
/// own ECB-midpoint spawn height) were never exercised against a projectile
/// before this batch, since nothing fired one at a Reflector-locked fighter
/// previously; this file's own geometry is chosen to actually overlap the
/// laser's flight line (fighter mid-height, `bone 0` = root per the shared
/// integration fixture's own bone table), not to assert a real game value.
fn data(damage_mul: f32, max_damage: i32) -> MatchData {
    let neutral_data = neutral_special_resources::profile(conformance::data());
    let Some(Specials::Fox { neutral, .. }) = neutral_data.fighters[0].specials.clone() else {
        panic!("test fixture is missing its neutral-special resource");
    };
    let mut resource = down_special_resources::profile(conformance::data());
    let Some(Specials::Fox { down, .. }) = resource.fighters[1].specials.clone() else {
        panic!("test fixture is missing its down-special resource");
    };
    let mut down = down.expect("down-special fixture must be populated");
    down.reflect.offset = [0.0, 1.5, 0.0];
    down.reflect.size = 2.0;
    down.reflect.bone = 0;
    down.reflect.damage_mul = damage_mul;
    down.reflect.max_damage = max_damage;
    // `rules.specials` (shared side/down common data) requires every
    // fighter to carry a side- or down-special resource once it is set
    // (`validation.rs`); fighter 0 gets an unused `down` too so it still
    // validates -- dispatch priority (`side, up, neutral, down`) means its
    // own centered-stick B press is always claimed by `neutral` first.
    resource.fighters[0].specials = Some(Specials::Fox {
        neutral,
        side: None,
        up: None,
        down: Some(down.clone()),
    });
    resource.fighters[1].specials = Some(Specials::Fox {
        neutral: None,
        side: None,
        up: None,
        down: Some(down),
    });
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    resource.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    resource
}

/// `other`: fighter 1's own input to repeat while fighter 0 fires --
/// dropping to `Controller::default()` for these two frames (as a plain
/// `input(0, ..)` would) matters when fighter 1 is mid-shield: `shield::
/// update`'s own `release_latched |= !input.shield_held()` sticks the
/// first frame the button reads as not held, even briefly.
fn fire_laser(game: &mut Match, other: Controller) {
    game.step([press_b(), other]).unwrap();
    // The fixture's Start pose is two frames, but the entry frame's own
    // extra `ftAnim_8006EBA4` advance (`docs/validation.md`'s entry-advance
    // table) makes the clip run out one idle frame sooner than a naive
    // frame count would suggest -- see `game_neutral_special.rs`'s own
    // comment on the same fixture.
    game.step([Controller::default(), other]).unwrap();
}

#[test]
fn shield_bounce_deflects_the_laser_without_damage() {
    let mut game = Match::new(data(1.0, 20), 0).unwrap();
    // Hold shield on fighter 1 for the whole test, including the two
    // frames `fire_laser` itself steps through (see its own comment on
    // why that input must keep being supplied explicitly).
    for _ in 0..3 {
        game.step(input(1, hold_shield())).unwrap();
    }
    fire_laser(&mut game, hold_shield());
    assert_eq!(game.state().projectiles.len(), 1);

    let mut bounced = false;
    let mut last_x = game.state().projectiles[0].position[0];
    for _ in 0..40 {
        let state = game.step(input(1, hold_shield())).unwrap().clone();
        if state.projectiles.is_empty() {
            break;
        }
        let x = state.projectiles[0].position[0];
        if x < last_x {
            // The projectile is now moving back toward fighter 0: it must
            // have bounced (it never overshoots fighter 1 without either
            // bouncing or connecting, since fighter 1's hurtbox sits well
            // before the shield-check position along the same line).
            bounced = true;
            break;
        }
        last_x = x;
    }
    assert!(bounced, "the laser must reverse direction off the shield");
    assert_eq!(
        game.state().fighters[1].percent,
        0.0,
        "a shield bounce must not damage the shielding fighter"
    );
    assert!(
        !game.state().projectiles.is_empty(),
        "a shield bounce keeps the laser alive (no piercing, but not destroyed either)"
    );
}

#[test]
fn reflector_reverses_owner_and_damages_the_original_shooter() {
    // damage_mul = 1.5: real formula `floor(3.0 * 1.5 + 0.99) = 5`; the
    // buggy plain-product formula this batch fixed would instead give
    // `floor(3.0 * 1.5) = 4`. Asserting the exact value of 5 below pins the
    // fix, not just the fact that some damage occurred.
    let mut game = Match::new(data(1.5, 20), 0).unwrap();
    // Enter Reflector and reach Loop (4-frame Start, per
    // `game_fox_down_special.rs`'s own fixture).
    game.step(input(1, down_input(-0.8))).unwrap();
    for _ in 0..4 {
        game.step(input(1, down_input(-0.8))).unwrap();
    }
    assert_eq!(game.state().fighters[1].action, Action::SpecialLw);
    assert!(game.state().fighters[1].shield.reflecting);

    fire_laser(&mut game, down_input(-0.8));
    assert_eq!(game.state().projectiles.len(), 1);
    assert_eq!(game.state().projectiles[0].owner, 0);

    let mut reflected = false;
    for _ in 0..40 {
        let state = game.step(input(1, down_input(-0.8))).unwrap().clone();
        if state
            .events
            .iter()
            .any(|e| matches!(e, Event::ProjectileReflected { owner: 0 }))
        {
            reflected = true;
            assert_eq!(state.projectiles[0].owner, 1, "reflection swaps the owner");
            break;
        }
    }
    assert!(
        reflected,
        "the laser must reach and reflect off the Reflector"
    );

    let mut hit = false;
    for _ in 0..40 {
        let state = game.step(input(1, down_input(-0.8))).unwrap().clone();
        if state.events.iter().any(|e| {
            matches!(
                e,
                Event::ProjectileHit {
                    owner: 1,
                    victim: 0
                }
            )
        }) {
            hit = true;
            assert_eq!(state.fighters[0].percent, 5.0);
            break;
        }
    }
    assert!(
        hit,
        "the reflected laser must fly back and damage the original shooter"
    );
}

#[test]
fn reflection_is_gated_on_the_laser_not_exceeding_max_damage() {
    // The laser's own hitboxes each deal 3 damage (`fox-neutral-special.json`);
    // a `max_damage` of 1 makes every one of them ineligible to reflect
    // (`ftColl_80077464`, `ftcoll.c:764`: `damage > x1A30_maxDamage`).
    let mut game = Match::new(data(1.0, 1), 0).unwrap();
    game.step(input(1, down_input(-0.8))).unwrap();
    for _ in 0..4 {
        game.step(input(1, down_input(-0.8))).unwrap();
    }
    assert_eq!(game.state().fighters[1].action, Action::SpecialLw);

    fire_laser(&mut game, down_input(-0.8));
    let mut hit_normally = false;
    for _ in 0..40 {
        let state = game.step(input(1, down_input(-0.8))).unwrap().clone();
        assert!(
            !state
                .events
                .iter()
                .any(|e| matches!(e, Event::ProjectileReflected { .. })),
            "a laser above max_damage must never reflect"
        );
        if state.events.iter().any(|e| {
            matches!(
                e,
                Event::ProjectileHit {
                    owner: 0,
                    victim: 1
                }
            )
        }) {
            hit_normally = true;
            assert!(state.fighters[1].percent > 0.0);
            break;
        }
    }
    assert!(
        hit_normally,
        "an ineligible laser must hit the Reflector-locked fighter normally instead"
    );
}
