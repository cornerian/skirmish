#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::characters::{Specials, fox::neutral::NeutralSpecial};
use skirmish::game::{Controller, Match, data::MatchData};

#[derive(serde::Deserialize)]
struct Fixture {
    parameters: NeutralSpecial,
}

/// Install the invented `tests/fixtures/game/fox-neutral-special.json`
/// motion on both fighters. Numeric attributes (`angle`/`speed`/
/// `landing_lag`/`laser` damage/knockback/lifetime) mirror this batch's own
/// exporter- and real-recording-confirmed values (see
/// `docs/fox-neutral-special.md`); the bone poses themselves are the same
/// invented two-bone pose every other synthetic fixture in this suite uses.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/fox-neutral-special.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials::Fox {
            neutral: Some(fixture.parameters.clone()),
            side: None,
            up: None,
            down: None,
        });
    }
    data
}

pub fn airborne_game(mut data: MatchData) -> Match {
    data.stage.spawns[0][1] = 6.0;
    let mut game = Match::new(profile(data), 0).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    game
}
