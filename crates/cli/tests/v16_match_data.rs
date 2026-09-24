//! Compatibility checks for the generic-roster gameplay export.
//!
//! The large archive is deliberately kept outside the repository.  When it is
//! present, the test decodes one all-generic pairing and one pairing carrying
//! a specialized resource.  The compact shape fixture below pins the fields
//! that made the v16 export newer than the original synthetic fixture.

use serde_json::{Value, json};
use skirmish::game::data::MatchData;
use skirmish_cli::pack;
use std::{env, path::PathBuf};

const DEFAULT_ROOT: &str =
    "/mnt/archive/datasets/melee/skirmish-gameplay/v16-roster-generic-20260919";

fn v16_shape_fixture() -> MatchData {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    let fighter = &mut value["fighters"][0];
    fighter["model_scaling"] = json!(0.8);
    fighter["wall_jump"] = json!({
        "can_walljump": true,
        "minimum_approach_speed": 0.5,
        "horizontal_velocity": 1.3,
        "vertical_velocity": 2.3,
        "blend_frames": 0,
        "dynamics_variant": 0,
        "frames": []
    });
    fighter["movement_poses"] = json!({
        "blend": {
            "wait": {"blend_frames": 0, "dynamics_variant": 0}
        }
    });
    fighter["jab"]["blend_frames"] = json!(0);
    fighter["jab"]["dynamics_variant"] = json!(0);
    serde_json::from_value(value).unwrap()
}

#[test]
fn v16_motion_metadata_is_typed_and_lossless() {
    let data = v16_shape_fixture();
    let fighter = &data.fighters[0];
    let wall_jump = fighter.wall_jump.as_ref().unwrap();
    assert_eq!(wall_jump.blend_frames, 0);
    assert_eq!(wall_jump.dynamics_variant, 0);
    assert_eq!(
        fighter
            .movement_poses
            .as_ref()
            .unwrap()
            .blend
            .as_ref()
            .unwrap()["wait"]
            .blend_frames,
        0
    );
    assert_eq!(fighter.jab.blend_frames, 0);
    assert_eq!(fighter.model_scaling, 0.8);
}

#[test]
fn v16_generic_and_specialized_archives_load_when_available() {
    let root = env::var_os("SKIRMISH_V16_GENERIC_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT));
    let generic_path = root.join("ice-climbers-pikachu-fd/match-data.json");
    let specialized_path = root.join("mario-fox-fd/match-data.json");
    if !generic_path.is_file() || !specialized_path.is_file() {
        println!(
            "skip: v16 archive is not available at {}; set SKIRMISH_V16_GENERIC_ROOT to override",
            root.display()
        );
        return;
    }

    let generic: MatchData = pack::load_match_data(&generic_path).unwrap();
    assert_eq!(generic.fighters[0].name, "ice-climbers");
    assert_eq!(generic.fighters[1].name, "pikachu");
    assert!(
        generic
            .fighters
            .iter()
            .all(|fighter| fighter.specials.is_none())
    );

    let specialized: MatchData = pack::load_match_data(&specialized_path).unwrap();
    assert_eq!(specialized.fighters[0].name, "mario");
    assert_eq!(specialized.fighters[1].name, "fox");
    assert!(specialized.fighters[0].specials.is_none());
    assert!(specialized.fighters[1].specials.is_some());
}
