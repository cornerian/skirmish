//! Guards `tests/fixtures/slippi/parity/*.slp` against the class of bug that
//! made the original `fox-ps.slp` fixture (Slippi 3.9.0) diverge on frame
//! one against every measurement: newer Slippi netplay builds have shipped
//! modified stage data for at least Pokemon Stadium (confirmed 2026-09-14,
//! see `docs/parity.md`'s "Fixture-selection rule"), so a recording made on
//! such a build can put a fighter at a spawn position the gameplay-export
//! pack disagrees with before any simulation step ever runs -- making the
//! ratchet's "first divergence" measure a fixture-provenance bug, not a
//! Skirmish bug.
//!
//! This asserts every listed recording's own first selected frame places
//! each player bit-exactly at the pack's own `stage.spawns` for that
//! recording's pairing, using the same participant-order assignment
//! `crates/cli/src/initialization.rs::build` uses (ports sorted ascending;
//! the lower port gets `spawns[0]`, the higher port `spawns[1]`). A fixture
//! that fails this check is not a valid parity target regardless of what its
//! own baseline says, since nothing has run yet at the frame this compares.
//!
//! Skipped (like `real_parity.rs`) when `SKIRMISH_GAMEPLAY_DATA` is unset,
//! and per-recording when that pairing's `match-data.{json,bin}` has not
//! been published yet, so this never requires the export to run in CI.
use serde::Deserialize;
use skirmish::game::data::MatchData;
use skirmish_cli::pack;
use skirmish_replay::slippi::{Port, Replay, Timeline};
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

/// Recordings with a known, separately-tracked *pack-data* spawn mismatch --
/// not a fixture problem, so this check must not fail loudly for them.
/// `docs/parity.md`'s "Dream Land / Pokemon Stadium" section traces
/// `fox-dl.slp`'s gap (spawn `.y` `37.0` recorded vs `37.2215`/`37.3215`
/// exported) to the exporter's own spawn-marker parent-chain composition
/// (`skirmish-assets`'s `stage.rs`, fixed in a dedicated exporter-side loop,
/// not here); it is already covered by `fox-dl-baseline.json`'s own
/// `checked_frames: 0`. Remove an id once the pack republishes corrected
/// spawns and this check passes for it unaided.
const KNOWN_PACK_DATA_SPAWN_GAPS: &[&str] = &["fox-dl"];

#[test]
fn fixture_first_frame_spawns_bit_match_the_pack() {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; the fixture-vs-pack spawn check needs the \
             published gameplay export. See docs/gameplay-export.md."
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

    let mut failures = Vec::new();
    for recording in &recordings.recordings {
        if KNOWN_PACK_DATA_SPAWN_GAPS.contains(&recording.id.as_str()) {
            println!(
                "skip: {}: known, separately-tracked pack-data spawn gap (docs/parity.md, \
                 fox-dl-baseline.json), not a fixture problem",
                recording.id
            );
            continue;
        }
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

        for (spawn_index, port) in ports.iter().enumerate() {
            let actor = frame
                .actors
                .iter()
                .find(|actor| actor.port == *port && !actor.follower)
                .expect("port found while building the sorted list above");
            let expected = data.stage.spawns[spawn_index];
            let actual = [actor.pre.position.x, actor.pre.position.y];
            if expected[0].to_bits() != actual[0].to_bits()
                || expected[1].to_bits() != actual[1].to_bits()
            {
                failures.push(format!(
                    "{}: {port:?}'s first-frame position does not bit-match the pack's \
                     stage.spawns[{spawn_index}] (expected [{:#010x}, {:#010x}] = {expected:?}, \
                     fixture has [{:#010x}, {:#010x}] = {actual:?}) -- this fixture may have been \
                     recorded on a Slippi build with modified stage data; see docs/parity.md's \
                     \"Fixture-selection rule\"",
                    recording.id,
                    expected[0].to_bits(),
                    expected[1].to_bits(),
                    actual[0].to_bits(),
                    actual[1].to_bits(),
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixture(s) do not bit-match the pack's spawn points:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
