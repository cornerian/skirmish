use skirmish::game::{
    data::MatchData,
    escape_air::{Parameters, Rules},
};

#[derive(serde::Deserialize)]
struct Fixture {
    rules: Rules,
    parameters: Parameters,
}

/// Install the invented `tests/fixtures/game/escape-air.json` air-dodge motion
/// and special-landing poses on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/escape-air.json")).unwrap();
    data.rules.escape_air = Some(fixture.rules);
    for fighter in &mut data.fighters {
        fighter.escape_air = Some(fixture.parameters.clone());
    }
    data
}
