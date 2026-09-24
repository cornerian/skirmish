//! Captain Falcon's real gameplay export binds both Falcon Dive entry moves.
//!
//! The export is intentionally an optional local test input, like the other
//! real-parity tests in this crate.  CI and ordinary checkouts skip it until
//! `SKIRMISH_GAMEPLAY_DATA/captain-marth-fd` is available.

use skirmish_cli::pack;
use std::{env, path::PathBuf};

const PAIRINGS: &[&str] = &[
    "captain-marth-fd-initialization-compatible",
    "captain-marth-fd",
];

fn match_data_path(root: &str) -> Option<PathBuf> {
    PAIRINGS
        .iter()
        .find_map(|pairing| pack::discover_match_data(&PathBuf::from(root).join(pairing)))
}

#[test]
fn real_export_contains_ground_and_air_dive_entries_and_capture_payload() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; Captain Dive export coverage is skipped"
        );
        return;
    };
    let Some(path) = match_data_path(&root) else {
        println!(
            "skip: {} does not exist; Captain Dive export has not landed there yet",
            PAIRINGS
                .iter()
                .map(|pairing| PathBuf::from(&root).join(pairing).display().to_string())
                .collect::<Vec<_>>()
                .join(" or ")
        );
        return;
    };

    let data = pack::load_match_data(&path).expect("Captain Dive export must decode");
    let captain = &data.fighters[0];
    let specials = captain
        .specials
        .as_ref()
        .expect("Captain export must carry specials");
    assert_eq!(specials.character_key(), "captain-falcon");

    for path in ["up.ground", "up.air"] {
        let attack = specials
            .attack(path)
            .unwrap_or_else(|| panic!("real Captain export is missing {path}"));
        assert_eq!(attack.move_id, Some(20), "unexpected {path} move id");
        assert_eq!(attack.frames.len(), 65, "unexpected {path} frame count");
        let active: Vec<usize> = attack
            .frames
            .iter()
            .enumerate()
            .filter_map(|(frame, sample)| (!sample.hitboxes.is_empty()).then_some(frame))
            .collect();
        assert_eq!(active.first(), Some(&13), "unexpected {path} active start");
        assert_eq!(active.last(), Some(&33), "unexpected {path} active end");
    }

    let capture = specials
        .lookup("up.capture")
        .expect("real Captain export is missing up.capture");
    assert_eq!(capture["throw"]["release_frame"], 0);
    assert_eq!(capture["throw"]["hit"]["damage"], 12);
    assert_eq!(capture["throw"]["hit"]["element"], 1);
}
