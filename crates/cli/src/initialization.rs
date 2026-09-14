//! Build a `skirmish_replay::match_validation::Initialization` from an
//! externally supplied native `MatchData` and a completed Slippi replay.
//!
//! This module reads only Slippi's `GameStart` metadata (occupied ports,
//! starting stocks, external character IDs, stage ID and the recorded random
//! seed) plus the timeline's first selected frame ID. No recorded position,
//! action state or per-frame RNG observation is copied into the simulator;
//! see `docs/replays.md` for the input/observation policy that still applies
//! once `validate-replay` consumes the resulting file.
use anyhow::{Context, Result, bail, ensure};
use skirmish::game::data::MatchData;
use skirmish_replay::{
    match_validation::Initialization,
    slippi::{Replay, Timeline, peppi},
    spawn_policy::{SpawnPolicy, SpawnProvenance, classify_spawn_provenance},
};

/// Public Slippi/CSS external character IDs, in the same order and naming
/// convention as `tests/fixtures/slippi/manifest.json`'s `character` field
/// (kebab-case). This is well-established Slippi/libmelee metadata, distinct
/// from `skirmish_replay::observation::internal_character`'s decomp-derived
/// internal fighter table, which this module does not touch.
pub const CHARACTER_EXTERNAL_IDS: &[(&str, u8)] = &[
    ("captain-falcon", 0),
    ("donkey-kong", 1),
    ("fox", 2),
    ("game-and-watch", 3),
    ("kirby", 4),
    ("bowser", 5),
    ("link", 6),
    ("luigi", 7),
    ("mario", 8),
    ("marth", 9),
    ("mewtwo", 10),
    ("ness", 11),
    ("peach", 12),
    ("pikachu", 13),
    ("ice-climbers", 14),
    ("jigglypuff", 15),
    ("samus", 16),
    ("yoshi", 17),
    ("zelda", 18),
    ("sheik", 19),
    ("falco", 20),
    ("young-link", 21),
    ("dr-mario", 22),
    ("roy", 23),
    ("pichu", 24),
    ("ganondorf", 25),
];

/// Slippi external stage IDs. Currently limited to the six stages already
/// observed in `tests/fixtures/slippi/manifest.json`'s corpus sample; this is
/// intentionally not a claim of exhaustive stage-legality coverage. An
/// unrecognized stage name is a hard error rather than a silent guess; add
/// entries here as more stages are exercised.
pub const STAGE_EXTERNAL_IDS: &[(&str, u16)] = &[
    ("fountain-of-dreams", 2),
    ("pokemon-stadium", 3),
    ("yoshis-story", 8),
    ("dream-land", 28),
    ("battlefield", 31),
    ("final-destination", 32),
];

/// A handful of common names whose mechanical slug does not match the table
/// above.
const NAME_ALIASES: &[(&str, &str)] = &[
    ("mr-game-watch", "game-and-watch"),
    ("game-watch", "game-and-watch"),
];

/// Lowercase, ASCII-fold and hyphenate a display name into the table's
/// kebab-case convention (e.g. "Yoshi's Story" -> "yoshis-story", "Final
/// Destination" -> "final-destination").
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_hyphen = false;
    for ch in name.chars() {
        let ch = match ch {
            '\'' | '\u{2019}' => continue,
            '\u{e9}' | '\u{c9}' => 'e',
            other => other,
        };
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !prev_hyphen && !out.is_empty() {
            out.push('-');
            prev_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    for (alias, canonical) in NAME_ALIASES {
        if out == *alias {
            return (*canonical).to_string();
        }
    }
    out
}

fn character_external_id(name: &str) -> Option<u8> {
    CHARACTER_EXTERNAL_IDS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, id)| *id)
}

fn stage_external_id(name: &str) -> Option<u16> {
    STAGE_EXTERNAL_IDS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, id)| *id)
}

/// `build`'s result: the `Initialization` itself plus a read-only
/// diagnostic naming which (if either) known codeset the replay's own
/// spawn positions happen to match. The diagnostic is derived, not part of
/// the initialization's resource identity; recompute it from
/// `classify_spawn_provenance` if a caller needs it again later.
#[derive(Debug)]
pub struct Built {
    pub initialization: Initialization,
    pub spawn_provenance: SpawnProvenance,
}

/// Build an `Initialization` from `data` and `replay`'s `GameStart`/timeline.
///
/// `seed_override`, when given, is used instead of the replay's own recorded
/// `random_seed`. Peppi decodes `GameStart.random_seed` unconditionally for
/// every Slippi version the importer accepts (2.0.0 through 3.18.0; see
/// `peppi-adapter`'s `Replay::read`), so there is currently no in-range
/// version for which the field is genuinely absent. `seed_override` exists
/// for a future format that drops the field and for deliberately reproducing
/// a match under a different seed; it is not required for any file this
/// importer currently accepts.
///
/// `warmup` is always empty: the native `Match::new` checkpoint already
/// corresponds to Melee's own pre-game state, which is also where Slippi's
/// own frame numbering starts (`peppi::frame::FIRST_INDEX`, -123; every
/// supported recording's frame IDs must advance contiguously from there, see
/// `docs/replays.md`). Since no warmup-stepping is implemented yet, this
/// function requires the replay's first selected frame to already be -123
/// and refuses otherwise, rather than silently misaligning the checkpoint.
///
/// `spawn_policy` is always `SpawnPolicy::Explicit`, filled from each
/// port's own frame -123 *post*-frame position (participant order: ports
/// sorted ascending assigned to index 0/1, matching `data.stage.spawns`'s
/// own convention) -- the replay's own recorded first frame, not an
/// assumption that the recording started at the resource pack's vanilla
/// `stage.spawns`. A real recording can start somewhere else
/// (`crate::spawn_policy`'s module doc; `docs/parity.md`'s 2026-09-14
/// sections), so a validated replay must always begin from what it actually
/// recorded.
pub fn build(mut data: MatchData, replay: &Replay, seed_override: Option<u32>) -> Result<Built> {
    let start = &replay.game().start;
    ensure!(!start.is_teams, "team matches are not implemented");
    ensure!(
        start.players.len() == 2,
        "replay must have exactly two occupied ports, found {}",
        start.players.len()
    );
    ensure!(
        start
            .players
            .iter()
            .all(|player| player.r#type == peppi::game::PlayerType::Human),
        "make-initialization requires two human-controlled players"
    );

    let mut players = start.players.clone();
    players.sort_by_key(|player| player.port);
    let ports = [players[0].port, players[1].port];
    ensure!(ports[0] != ports[1], "duplicate player ports");

    // Characters are checked before the stage: a mismatched matchup is the
    // more common authoring mistake, and reporting it first keeps the error
    // for a wrong-fighters-right-stage file from being masked by a
    // coincidentally-also-wrong stage name.
    for (index, player) in players.iter().enumerate() {
        let fighter_name = &data.fighters[index].name;
        let expected = character_external_id(&slug(fighter_name));
        if expected != Some(player.character) {
            bail!(
                "character mismatch at replay port {:?}: match data fighter {:?} does not correspond to replay external character id {} (mapped id: {:?})",
                player.port,
                fighter_name,
                player.character,
                expected
            );
        }
    }

    let stage_slug = slug(&data.stage.name);
    let expected_stage = stage_external_id(&stage_slug);
    if expected_stage != Some(start.stage) {
        bail!(
            "stage mismatch: match data stage {:?} does not correspond to replay external stage id {} (mapped id: {:?}, known stage names: {:?})",
            data.stage.name,
            start.stage,
            expected_stage,
            STAGE_EXTERNAL_IDS
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
        );
    }

    for player in &players {
        ensure!(
            u16::from(player.stocks) == u16::from(data.rules.stocks),
            "stock count mismatch at replay port {:?}: match data rules.stocks is {}, replay recorded {} starting stocks",
            player.port,
            data.rules.stocks,
            player.stocks
        );
    }

    // Peppi decodes `GameStart.players[_].handicap` unconditionally (it is
    // not gated by Slippi version, unlike e.g. `netplay`), and the source
    // pins every slot to 9 whenever the handicap rule is off
    // (`docs/grab-escape-timer.md`), so this is always a real value, not a
    // placeholder that needs a presence check.
    data.players = Some([
        skirmish::game::data::PlayerSettings {
            handicap: players[0].handicap,
        },
        skirmish::game::data::PlayerSettings {
            handicap: players[1].handicap,
        },
    ]);

    let seed = seed_override.unwrap_or(start.random_seed);

    let summary = replay.summary(Timeline::LastRecorded)?;
    let next_frame = summary
        .first_frame
        .context("replay has no selected frames")?;
    ensure!(
        next_frame == peppi::frame::FIRST_INDEX,
        "replay's first selected frame is {next_frame}, not {}; make-initialization does not model warmup to reach a later start, so the native Match::new() checkpoint (Melee's own pre-game state) cannot be aligned with it. Pass --seed and construct the initialization by hand if a later start is genuinely required.",
        peppi::frame::FIRST_INDEX
    );

    // `spawn_policy` is filled from this same first frame's own *post*-frame
    // position, not the resource pack's `data.stage.spawns`: a real
    // recording's spawn point is match-start state, not purely stage data
    // (`crate spawn_policy`'s module doc). `post`, not `pre`, matches the
    // field the whole validation harness compares against
    // (`observation::expected`'s `post.position`,
    // `match_validation::Report`'s `"fighter-post-v11"` policy): during the
    // match-start warp-in nothing moves within a frame, so a fixture whose
    // recorded spawn is genuinely vanilla already has `post.position ==
    // data.stage.spawns` at this frame, and `Explicit` reproduces that
    // bit-exactly for every codeset, identified or not.
    let indices = replay.frame_indices(Timeline::LastRecorded)?;
    let &first_index = indices.first().context("replay has no selected frames")?;
    let frame = replay
        .frame(first_index)
        .context("failed to read the replay's first selected frame")?;
    ensure!(
        frame.id == next_frame,
        "replay's first selected frame index does not correspond to frame id {next_frame} (found {})",
        frame.id
    );
    let mut spawns = [[0.0_f32; 2]; 2];
    for (index, port) in ports.iter().enumerate() {
        let actor = frame
            .actors
            .iter()
            .find(|actor| actor.port == *port && !actor.follower)
            .with_context(|| {
                format!("replay's first frame has no non-follower actor at port {port:?}")
            })?;
        spawns[index] = [actor.post.position.x, actor.post.position.y];
    }
    let spawn_provenance = classify_spawn_provenance(&data.stage.name, data.stage.spawns, spawns);

    Ok(Built {
        initialization: Initialization {
            data,
            seed,
            ports,
            next_frame,
            warmup: Vec::new(),
            spawn_policy: SpawnPolicy::Explicit { spawns },
        },
        spawn_provenance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use skirmish::game::data::MatchData;
    use skirmish_replay::slippi::Port;
    use std::{fs::File, io::BufReader};

    fn fixture_replay() -> Replay {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/slippi/parity/fox-fd.slp"
        );
        Replay::read(BufReader::new(File::open(path).unwrap())).unwrap()
    }

    fn synthetic_data() -> MatchData {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap()
    }

    #[test]
    fn synthetic_match_data_rejects_the_real_fox_fd_replay_on_character_mismatch() {
        let replay = fixture_replay();
        let data = synthetic_data();
        let error = build(data, &replay, None).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("character mismatch"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn fox_named_stand_in_match_data_builds_against_the_real_fox_fd_replay() {
        let replay = fixture_replay();
        let mut data = synthetic_data();
        data.stage.name = "Final Destination".to_string();
        data.rules.stocks = 4;
        for fighter in &mut data.fighters {
            fighter.name = "Fox".to_string();
        }
        let built = build(data, &replay, None).unwrap();
        let initialization = built.initialization;
        assert_eq!(initialization.ports, [Port::P1, Port::P4]);
        assert_eq!(initialization.seed, 3_778_252_302);
        assert_eq!(initialization.next_frame, peppi::frame::FIRST_INDEX);
        assert!(initialization.warmup.is_empty());
        // The synthetic fixture's own `stage.spawns` (-2,0)/(2,0) is not the
        // real recording's spawn point, so `spawn_policy` must be `Explicit`
        // with the replay's own frame -123 post-frame positions, not
        // whatever this stand-in match data happened to carry.
        let SpawnPolicy::Explicit { spawns } = initialization.spawn_policy else {
            panic!(
                "expected an Explicit spawn_policy, got {:?}",
                initialization.spawn_policy
            );
        };
        assert_ne!(spawns, [[-2.0, 0.0], [2.0, 0.0]]);
        // Final Destination's real spawn points are (-60,10)/(60,10), which
        // is both the pack's own vanilla value and (coincidentally, per
        // `docs/parity.md`) UnclePunch's Neutral Spawns singles entry for
        // FD; a synthetic `MatchData` never carries the real pack's
        // `stage.spawns`, so this recording classifies against the table,
        // not against this fixture's own arbitrary "vanilla".
        assert_eq!(spawns, [[-60.0, 10.0], [60.0, 10.0]]);
        assert_eq!(built.spawn_provenance, SpawnProvenance::SlippiNeutral);
    }

    #[test]
    fn slug_matches_the_manifest_kebab_case_convention() {
        assert_eq!(slug("Fox"), "fox");
        assert_eq!(slug("Yoshi's Story"), "yoshis-story");
        assert_eq!(slug("Final Destination"), "final-destination");
        assert_eq!(slug("Dr. Mario"), "dr-mario");
        assert_eq!(slug("Mr. Game & Watch"), "game-and-watch");
    }
}
