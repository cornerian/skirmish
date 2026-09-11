#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::game::{
    Controller, Match, characters::Specials, data::MatchData, specials::neutral::Parameters,
};

pub fn profile(mut data: MatchData) -> MatchData {
    for fighter in &mut data.fighters {
        fighter.specials = Some(Specials::Fox {
            neutral: Some(Parameters {
                neutral_thresholds: [0.6, 0.6],
                ground: fighter.jab.clone(),
                air: fighter.jab.clone(),
            }),
            side: None,
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
