#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::characters::{
    Specials,
    fox::side::{Rules, SideSpecial},
};
use skirmish::game::{
    data::MatchData,
    escape_air::{Parameters as EscapeAirParameters, Rules as EscapeAirRules},
};

#[derive(serde::Deserialize)]
struct Fixture {
    rules: Rules,
    parameters: SideSpecial,
}

#[derive(serde::Deserialize)]
struct EscapeAirFixture {
    rules: EscapeAirRules,
    parameters: EscapeAirParameters,
}

/// Install the invented `tests/fixtures/game/fox-side-special.json` motion on
/// both fighters, plus the escape-air fixture its landing shares (`x2EC`).
/// Inlines `tests/support/escape_air.rs`'s own fixture load (rather than
/// including that file as a module) so callers that already declare their
/// own `escape_air` support module do not hit a duplicate-module error.
pub fn profile(mut data: MatchData) -> MatchData {
    let escape_air: EscapeAirFixture =
        serde_json::from_str(include_str!("../fixtures/game/escape-air.json")).unwrap();
    data.rules.escape_air = Some(escape_air.rules);
    for fighter in &mut data.fighters {
        fighter.escape_air = Some(escape_air.parameters.clone());
    }
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-side-special.json")).unwrap();
    data.rules.specials = Some(fixture.rules);
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials::Fox {
            neutral: None,
            side: Some(fixture.parameters.clone()),
            up: None,
            down: None,
        });
    }
    data
}
