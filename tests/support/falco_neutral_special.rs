#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::characters::{Specials, fox::neutral::NeutralSpecial};
use skirmish::game::{Controller, Match, data::MatchData};

#[derive(serde::Deserialize)]
struct Fixture {
    parameters: NeutralSpecial,
}

/// Install the invented `tests/fixtures/game/falco-neutral-special.json`
/// motion on both fighters, wired through `Specials::Falco` (not `::Fox`).
/// Numeric attributes (`speed`/`laser.lifetime`/every hitbox's own
/// `growth`/`fixed`/fourth-hitbox `center`/`radius`) are Falco's own
/// exporter-confirmed values, genuinely different from Fox's (see
/// `characters::fox::neutral::{Attributes,Laser}`'s own doc comments and
/// `docs/falco.md`); the bone poses themselves are the same invented
/// two-bone pose `tests/support/neutral_special.rs` uses, not a real
/// Falco animation.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/game/falco-neutral-special.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials::Falco {
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
