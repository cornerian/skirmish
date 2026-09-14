//! Classifies every `tests/fixtures/slippi/parity/*.slp` fixture's own
//! first selected frame against the two known spawn codesets
//! (`skirmish_replay::spawn_policy`): the resource pack's vanilla NTSC 1.02
//! `stage.spawns`, and Slippi netplay's "Neutral Spawns" table. A fixture
//! whose first frame matches neither -- Dream Land's own unidentified
//! console-era gap is the known case, `docs/parity.md`'s 2026-09-14
//! sections have the measurement -- is not a bug in this check: `make-
//! initialization` (`crates/cli/src/initialization.rs::build`) fills
//! `SpawnPolicy::Explicit` from each recording's own frame -123 post-frame
//! position, not an assumption that the recording started at the pack's
//! vanilla spawn, so an "unknown codeset" fixture is still a perfectly
//! valid parity target: the sim starts at what the replay actually
//! recorded either way.
//!
//! This used to be a hard bit-exact check against the pack's `stage.spawns`
//! alone, with one explicit exception (`fox-dl`) for a gap that turned out
//! not to be an exporter bug at all (see the history in docs/parity.md).
//! Classifying every fixture, rather than excepting the ones that don't
//! match vanilla, removes the need for that kind of exception list going
//! forward: a newly added fixture that starts on a different codeset is
//! automatically "unknown codeset", not a silent fixture-selection mistake
//! that (before `SpawnPolicy::Explicit` existed) would have made every
//! downstream frame count meaningless.
//!
//! Skipped (like `real_parity.rs`) when `SKIRMISH_GAMEPLAY_DATA` is unset,
//! and per-recording when that pairing's `match-data.{json,bin}` has not
//! been published yet, so this never requires the export to run in CI.
use serde::Deserialize;
use skirmish::game::data::MatchData;
use skirmish_cli::pack;
use skirmish_replay::{
    slippi::{Port, Replay, Timeline},
    spawn_policy::{SpawnProvenance, classify_spawn_provenance},
};
use std::{env, fs, fs::File, io::BufReader, path::PathBuf};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity"
);

#[derive(Deserialize)]
struct RecordingsFile {
    recordings: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
    id: String,
    file: String,
    pairing: String,
}

#[test]
fn fixture_first_frame_spawns_are_classified() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; the fixture spawn-provenance check needs \
             the published gameplay export. See docs/gameplay-export.md."
        );
        return;
    };

    let recordings_path = PathBuf::from(FIXTURES).join("recordings.json");
    let recordings: RecordingsFile =
        serde_json::from_slice(&fs::read(&recordings_path).unwrap()).unwrap();
    assert!(
        !recordings.recordings.is_empty(),
        "{} listed no recordings",
        recordings_path.display()
    );

    let mut classified = Vec::new();
    for recording in &recordings.recordings {
        let pairing_dir = PathBuf::from(&root).join(&recording.pairing);
        let Some(match_data_path) = pack::discover_match_data(&pairing_dir) else {
            println!(
                "skip: {}: neither match-data.bin nor match-data.json exists under {} yet",
                recording.id,
                pairing_dir.display()
            );
            continue;
        };
        let data: MatchData = pack::load_match_data(&match_data_path).unwrap_or_else(|error| {
            panic!(
                "{}: failed to load {}: {error}",
                recording.id,
                match_data_path.display()
            )
        });

        let replay_path = PathBuf::from(FIXTURES).join(&recording.file);
        let replay = Replay::read(BufReader::new(File::open(&replay_path).unwrap_or_else(
            |error| {
                panic!(
                    "{}: failed to open {}: {error}",
                    recording.id,
                    replay_path.display()
                )
            },
        )))
        .unwrap_or_else(|error| panic!("{}: failed to parse replay: {error}", recording.id));

        let indices = replay
            .frame_indices(Timeline::LastRecorded)
            .unwrap_or_else(|error| {
                panic!("{}: failed to select a timeline: {error}", recording.id)
            });
        let &first_index = indices
            .first()
            .unwrap_or_else(|| panic!("{}: replay has no selected frames", recording.id));
        let frame = replay
            .frame(first_index)
            .unwrap_or_else(|error| panic!("{}: failed to read frame 0: {error}", recording.id));

        let mut ports: Vec<Port> = frame
            .actors
            .iter()
            .filter(|actor| !actor.follower)
            .map(|actor| actor.port)
            .collect();
        ports.sort();
        assert_eq!(
            ports.len(),
            2,
            "{}: expected exactly two non-follower actors on the first frame, found {}",
            recording.id,
            ports.len()
        );

        // `post`, not `pre`: this is the same field `make-initialization`
        // fills `SpawnPolicy::Explicit` from and the whole validation
        // harness compares (`observation::expected`'s `post.position`).
        let mut explicit = [[0.0_f32; 2]; 2];
        for (spawn_index, port) in ports.iter().enumerate() {
            let actor = frame
                .actors
                .iter()
                .find(|actor| actor.port == *port && !actor.follower)
                .expect("port found while building the sorted list above");
            explicit[spawn_index] = [actor.post.position.x, actor.post.position.y];
        }

        let provenance = classify_spawn_provenance(&data.stage.name, data.stage.spawns, explicit);
        println!(
            "{}: classified {:?} (stage.spawns = {:?}, first-frame post.position = {:?})",
            recording.id, provenance, data.stage.spawns, explicit
        );
        classified.push((recording.id.clone(), provenance));
    }

    assert!(
        !classified.is_empty(),
        "every recording in {} was skipped (no published pack for any pairing)",
        recordings_path.display()
    );
    println!(
        "classified {} fixture(s): {} vanilla, {} slippi_neutral, {} unknown_codeset",
        classified.len(),
        classified
            .iter()
            .filter(|(_, p)| *p == SpawnProvenance::Vanilla)
            .count(),
        classified
            .iter()
            .filter(|(_, p)| *p == SpawnProvenance::SlippiNeutral)
            .count(),
        classified
            .iter()
            .filter(|(_, p)| *p == SpawnProvenance::UnknownCodeset)
            .count(),
    );
}
