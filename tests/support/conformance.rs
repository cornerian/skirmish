//! Integration scenarios use an explicit synthetic world. These fixtures test
//! required behavior, not numeric fidelity of authentic character resources.
#![allow(dead_code)] // Each integration target uses a different subset.
use skirmish::game::{Controller, Match, State, data::MatchData};

pub fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("../fixtures/game/integration-match.json")).unwrap();
    data.rules.stocks = 4;
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.top_ko_min_knockback = Some(0.5);
    // Explicit invented values, like integration-match.json; these are not
    // extracted Melee common data or authentic character timings.
    let locomotion =
        serde_json::from_str(include_str!("../fixtures/game/locomotion.json")).unwrap();
    #[derive(serde::Deserialize)]
    struct ShieldFixture {
        rules: skirmish::game::shield::Rules,
        attributes: skirmish::game::shield::Attributes,
    }
    let shield: ShieldFixture =
        serde_json::from_str(include_str!("../fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(shield.rules);
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(locomotion);
        fighter.shield = Some(shield.attributes.clone());
    }
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.floor.y = 0.0;
    data.stage.spawns = [[-10.0, 0.0], [10.0, 0.0]];
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data
}
pub fn game() -> Match {
    Match::new(data(), 0).unwrap()
}
pub fn idle() -> [Controller; 2] {
    [Controller::default(); 2]
}
pub fn step(game: &mut Match, input: [Controller; 2]) -> State {
    game.step(input)
        .expect("required gameplay input must be supported")
        .clone()
}
pub fn action(state: &State, player: usize) -> String {
    serde_json::to_value(state.fighters[player].action)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}
