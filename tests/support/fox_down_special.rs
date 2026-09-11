#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::game::{
    characters::{
        Specials,
        fox::{down::DownSpecial, side::Rules},
    },
    data::MatchData,
};

#[derive(serde::Deserialize)]
struct Fixture {
    rules: Rules,
    parameters: DownSpecial,
}

/// Install the invented `tests/fixtures/game/fox-down-special.json` motion
/// on both fighters. `rules` here is `characters::fox::side::Rules`, shared
/// common data (`x218`/`x220`/`x21C`) the down special's own aerial entry
/// reads too (`vertical_threshold`); `data.locomotion` (already installed by
/// `conformance::data()`) supplies the mid-move turn/jump-cancel/platform-
/// drop fields instead.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-down-special.json")).unwrap();
    data.rules.specials = Some(fixture.rules);
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials::Fox {
            neutral: None,
            side: None,
            down: Some(fixture.parameters.clone()),
        });
    }
    data
}
