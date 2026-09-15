#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::{
    game::clank as clank_math,
    game::{
        clank,
        data::MatchData,
        script::resources::{Resources, Specials},
    },
};

#[derive(serde::Deserialize)]
struct Fixture {
    rules: skirmish::fighter::specials::Rules,
    parameters: serde_json::Value,
}

/// Install the invented `tests/fixtures/game/fox-down-special.json` motion
/// on both fighters. `rules` here is `fighter::specials::Rules`, shared
/// common data (`x218`/`x220`/`x21C`) the down special's own aerial entry
/// reads too (`vertical_threshold`); `data.locomotion` (already installed by
/// `conformance::data()`) supplies the mid-move turn/jump-cancel/platform-
/// drop fields instead.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-down-special.json")).unwrap();
    data.rules.specials = Some(fixture.rules);
    for fighter in &mut data.fighters {
        let mut values = std::collections::BTreeMap::new();
        values.insert("down".into(), fixture.parameters.clone());
        fighter.specials = Some(Specials {
            character: "fox".into(),
            resources: Resources::new(values).unwrap(),
        });
    }
    data
}

/// Wire the same ordinary clank profile `tests/game_clank.rs` (and
/// `tests/support/up_special.rs::with_ordinary_clank`) use onto `data`,
/// plus the rebound animation `rules.clank` requires for every fighter
/// (`validation.rs`'s "rebound animation requires a clank profile").
/// Reflector's Start hitbox carries the pack's own `clank: true` value; this
/// is what lets it validate.
pub fn with_ordinary_clank(mut data: MatchData) -> MatchData {
    data.rules.clank = Some(clank::Rules {
        profile: clank::Profile::OrdinaryGroundedNonSlash,
            response: clank_math::ResponseRules {
            damage_gap: 9,
            duration_scale: 0.5,
            duration_base: 2.0,
        },
        push_scale: 0.2,
        push_base: 0.6,
        hitlag_maximum: 20.0,
        surface_friction_multiplier: 0.5,
    });
    for fighter in &mut data.fighters {
        fighter.rebound = Some(clank::Animation {
            animation_length: 13.9,
            poses: (0..15)
                .map(|frame| {
                    let mut pose = fighter.bones.clone();
                    pose[1].translation[1] = 1.0 + frame as f32 * 0.1;
                    pose
                })
                .collect(),
        });
    }
    data
}
