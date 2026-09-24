#![allow(dead_code)] // Shared by integration targets with different setup paths.

use skirmish::fighter::specials::Rules;
use skirmish::game::script::resources::{Resources, Specials};
use skirmish::game::wall_jump::{Attributes as WallJumpAttributes, Rules as WallJumpRules};
use skirmish::game::{Controller, Match, data::MatchData};
use std::collections::BTreeMap;

/// Install the invented `tests/fixtures/game/falco-neutral-special.json`
/// motion on both fighters, wired through `Specials` with the Falco character
/// key (rather than Fox's).
/// Numeric attributes (`speed`/`laser.lifetime`/every hitbox's own
/// `growth`/`fixed`/fourth-hitbox `center`/`radius`) are Falco's own
/// exporter-confirmed values, genuinely different from Fox's (see the
/// `neutral` resource in `scripts/fighters/fox.luau` and `docs/falco.md`);
/// the bone poses themselves are the same invented
/// two-bone pose `tests/support/neutral_special.rs` uses, not a real
/// Falco animation.
pub fn profile(mut data: MatchData) -> MatchData {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/game/falco-neutral-special.json")).unwrap();
    let mut values = BTreeMap::new();
    // The neutral B dispatcher shares the common special-input gate. Keep the
    // synthetic Falco profile's thresholds aligned with the sibling Fox
    // fixture so the resource-backed move can be entered from neutral.
    data.rules.specials = Some(Rules {
        side_stick_threshold: 0.3,
        turn_threshold: 0.1,
        vertical_threshold: 0.6,
        air_drift_recovery_step: 0.02,
    });
    // Falco's source registration exports `can_walljump`; provide the
    // matching synthetic profile so attribute projection remains valid while
    // this neutral-special fixture uses the real Falco roster identity.
    data.rules.wall_jump = Some(WallJumpRules {
        tilt_deadzone: 0.3,
        input_window: 5.0,
        stick_threshold: 0.7,
        tilt_window: 3.0,
        startup_frames: 0,
        vertical_velocity_base: 0.5,
    });
    let mut neutral = fixture["parameters"].clone();
    // The neutral policy consumes the exported command-variable trace.  A
    // command 2 sample on Loop's first animation frame is the fixture's real fire
    // event, replacing the removed implicit legacy-fire path.
    let trace = |len: usize, slot: usize, frame: Option<usize>| {
        let mut cmd_vars = vec![vec![serde_json::Value::Null; 4]; len];
        if let Some(frame) = frame {
            cmd_vars[frame][slot] = serde_json::json!(1);
        }
        serde_json::json!({ "cmd_vars": cmd_vars, "allow_interrupt": vec![false; len] })
    };
    neutral["script"] = serde_json::json!({
        "start": {
            "ground": trace(neutral["start"]["ground"]["frames"].as_array().unwrap().len(), 0, None),
            "air": trace(neutral["start"]["air"]["frames"].as_array().unwrap().len(), 0, None)
        },
        "loop_phase": {
            "ground": trace(neutral["loop_phase"]["ground"]["frames"].as_array().unwrap().len(), 2, Some(0)),
            "air": trace(neutral["loop_phase"]["air"]["frames"].as_array().unwrap().len(), 2, Some(0))
        },
        "end": {
            "ground": trace(neutral["end"]["ground"]["frames"].as_array().unwrap().len(), 1, None),
            "air": trace(neutral["end"]["air"]["frames"].as_array().unwrap().len(), 1, None)
        }
    });
    values.insert("neutral".to_owned(), neutral);
    let specials = Specials {
        character: "Falco".to_owned(),
        special_attributes: None,
        animations: None,
        articles: None,
        resources: Resources::new(values).unwrap(),
    };
    for fighter in &mut data.fighters {
        // Bundled source selection is keyed by FighterData.name; the generic
        // conformance fixture starts with synthetic names, so install the
        // Falco roster identity alongside its Specials resource.
        fighter.name = "Falco".to_owned();
        fighter.wall_jump = Some(WallJumpAttributes {
            can_walljump: false,
            minimum_approach_speed: 0.2,
            horizontal_velocity: 3.0,
            vertical_velocity: 4.0,
            blend_frames: 0,
            dynamics_variant: 0,
            frames: vec![fighter.bones.clone()],
        });
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
