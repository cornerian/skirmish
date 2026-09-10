use skirmish::game::{
    data::MatchData,
    escape::{Parameters, Rules},
};

#[derive(serde::Deserialize)]
struct Fixture {
    rules: Rules,
    parameters: Parameters,
}

/// Install the invented `tests/fixtures/game/escape.json` roll and spot-dodge
/// motions on both fighters. Its bones match the synthetic integration skeleton.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/escape.json")).unwrap();
    data.rules.escape = Some(fixture.rules);
    for fighter in &mut data.fighters {
        fighter.escape = Some(fixture.parameters.clone());
    }
    data
}
