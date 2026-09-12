#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::game::{
    characters::{Specials, fox::side::SideSpecial, fox::up::UpSpecial},
    data::{Attack, AttackFrame, Bone, Hitbox, MatchData},
    escape_air::{Parameters as EscapeAirParameters, Rules as EscapeAirRules},
};

#[derive(serde::Deserialize)]
struct Fixture {
    // Shared with the side special (`up::validate` takes the same `Rules`);
    // only `vertical_threshold` is actually read by this move's own dispatch.
    rules: skirmish::game::characters::fox::side::Rules,
    parameters: UpSpecial,
}

#[derive(serde::Deserialize)]
struct SideFixture {
    parameters: SideSpecial,
}

#[derive(serde::Deserialize)]
struct EscapeAirFixture {
    rules: EscapeAirRules,
    parameters: EscapeAirParameters,
}

/// Install the invented `tests/fixtures/game/fox-up-special.json` motion on
/// both fighters. `rules.specials` is shared with the side special
/// (`Rules { side_stick_threshold, turn_threshold, vertical_threshold }`),
/// and its own validation requires a side-special motion for every fighter
/// whenever it is present at all (`validation.rs`'s "side-special rules
/// require a motion for every fighter"), so this also installs the side
/// special's own fixture and its escape-air dependency, even though no
/// test in this file drives the side special itself.
pub fn profile(mut data: MatchData) -> MatchData {
    let escape_air: EscapeAirFixture =
        serde_json::from_str(include_str!("../fixtures/game/escape-air.json")).unwrap();
    data.rules.escape_air = Some(escape_air.rules);
    let side: SideFixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-side-special.json")).unwrap();
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-up-special.json")).unwrap();
    data.rules.specials = Some(fixture.rules);
    for fighter in &mut data.fighters {
        fighter.escape_air = Some(escape_air.parameters.clone());
        fighter.specials = Some(Specials::Fox {
            neutral: None,
            side: Some(side.parameters.clone()),
            up: Some(fixture.parameters.clone()),
            down: None,
        });
    }
    data
}

/// Fox/Falco's Hold-phase charge hit: the gameplay export pack
/// (`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911/
/// fox-fd/match-data.json`, `fighters[0].specials.up.hold.ground`) shows a
/// real script-embedded hitbox on frames 20/22/24/26/28/30/32 of the 44-pose
/// Hold set (a periodic "charge spark" pulse, absent on every frame in
/// between). Every numeric field below is the pack's own value verbatim;
/// only `bone` is adapted (the pack's own bone 0 already exists in every
/// synthetic skeleton in this test suite, so no adaptation was even needed
/// here) and `clank`/`rebound` are forced off (the pack reports both
/// `false` for this hitbox already, so no deviation there either). See
/// `docs/fox-up-special.md`'s own "Hitboxes" section.
pub fn hold_charge_hitbox() -> Hitbox {
    Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        group: 0,
        bone: 0,
        center: [0.0, 7.812, 0.0],
        radius: 8.2026,
        damage: 2,
        shield_damage: 0,
        angle_degrees: 70.0,
        growth: 40,
        fixed: 0,
        base: 40,
    }
}

/// Travel's own continuous hit (same pack,
/// `fighters[0].specials.up.travel.ground`: every one of the pack's own 31
/// sampled frames carries this same hitbox, i.e. it never clears while
/// Travel runs). Every field is the pack's own value verbatim except `bone`
/// (the pack's own bone 58 has no equivalent in this suite's shared
/// two-bone synthetic skeleton -- see `tests/game_swept_hitboxes.rs`'s own
/// precedent -- adapted to bone 1) and `clank`/`rebound` (the pack reports
/// both `true`; forced off here since `rules.clank` is not configured by
/// this move's own test profile and wiring that separate subsystem's rules
/// is out of this batch's scope -- clank/rebound already work generically
/// once `rules.clank` exists, per `src/game/clank.rs`, and are unaffected
/// by anything in `up.rs`/`down.rs`).
pub fn travel_hitbox() -> Hitbox {
    Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        group: 0,
        bone: 1,
        center: [0.0, 0.0, 0.0],
        radius: 3.999_744,
        damage: 14,
        shield_damage: 5,
        angle_degrees: 80.0,
        growth: 60,
        fixed: 0,
        base: 60,
    }
}

/// A 44-pose Hold `Attack` (matching the pack's own frame count) with
/// [`hold_charge_hitbox`] active on exactly the pack's own frames, sharing
/// the caller's own static bones (no root motion in either the pinned
/// source or the existing synthetic fixture). Built by callers rather than
/// baked into the shared `fixtures/game/fox-up-special.json` (whose own
/// short, hitbox-free 6-pose stand-in backs every state-machine-transition
/// test's own step counts), so this data cannot perturb any of those.
pub fn hold_attack_with_pack_hitboxes(bones: &[Bone]) -> Attack {
    Attack {
        move_id: Some(20),
        frames: (0..44)
            .map(|index| AttackFrame {
                bones: bones.to_vec(),
                hitboxes: if matches!(index, 20 | 22 | 24 | 26 | 28 | 30 | 32) {
                    vec![hold_charge_hitbox()]
                } else {
                    vec![]
                },
                hurtbox_states: vec![],
            })
            .collect(),
    }
}
