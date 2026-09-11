#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::game::{
    characters::{Specials, fox::side::SideSpecial, fox::up::UpSpecial},
    data::MatchData,
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
