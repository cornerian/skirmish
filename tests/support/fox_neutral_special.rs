#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::game::script::resources::{Resources, Specials};
use skirmish::game::{Controller, Match, data::MatchData};
use std::collections::BTreeMap;

/// Install the invented `tests/fixtures/game/fox-neutral-special.json`
/// motion on both fighters. Numeric attributes (`angle`/`speed`/
/// `landing_lag`/`laser` damage/knockback/lifetime) mirror this batch's own
/// exporter- and real-recording-confirmed values (see
/// `docs/fox-neutral-special.md`); the bone poses themselves are the same
/// invented two-bone pose every other synthetic fixture in this suite uses.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/game/fox-neutral-special.json")).unwrap();
    let mut values = BTreeMap::new();
    let mut neutral = fixture["parameters"].clone();
    // The neutral policy consumes the exported command-variable trace.  A
    // command 2 sample on Loop's first animation frame is the fixture's real fire
    // event, replacing the removed implicit legacy-fire path. Loop also keeps
    // command 0 armed so the synthetic repeat-input test can exercise the
    // native repeat gate without changing the exported gameplay resource.
    let trace = |len: usize, slot: usize, frame: Option<usize>| {
        let mut cmd_vars = vec![vec![serde_json::Value::Null; 4]; len];
        if let Some(frame) = frame {
            cmd_vars[frame][slot] = serde_json::json!(1);
        }
        serde_json::json!({ "cmd_vars": cmd_vars, "allow_interrupt": vec![false; len] })
    };
    let loop_trace = |len: usize| {
        let mut cmd_vars = vec![vec![serde_json::Value::Null; 4]; len];
        cmd_vars[0][0] = serde_json::json!(1);
        cmd_vars[0][2] = serde_json::json!(1);
        serde_json::json!({ "cmd_vars": cmd_vars, "allow_interrupt": vec![false; len] })
    };
    neutral["script"] = serde_json::json!({
        "start": {
            "ground": trace(neutral["start"]["ground"]["frames"].as_array().unwrap().len(), 0, None),
            "air": trace(neutral["start"]["air"]["frames"].as_array().unwrap().len(), 0, None)
        },
        "loop_phase": {
            "ground": loop_trace(neutral["loop_phase"]["ground"]["frames"].as_array().unwrap().len()),
            "air": loop_trace(neutral["loop_phase"]["air"]["frames"].as_array().unwrap().len())
        },
        "end": {
            "ground": trace(neutral["end"]["ground"]["frames"].as_array().unwrap().len(), 1, None),
            "air": trace(neutral["end"]["air"]["frames"].as_array().unwrap().len(), 1, None)
        }
    });
    values.insert("neutral".to_owned(), neutral);
    let specials = Specials {
        character: "Fox".to_owned(),
        resources: Resources::new(values).unwrap(),
    };
    for fighter in &mut data.fighters {
        fighter.specials = Some(specials.clone());
    }
    data
}

pub fn airborne_game(mut data: MatchData) -> Match {
    data.stage.spawns[0][1] = 6.0;
    let mut game = Match::new(profile(data), 0).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    game
}
