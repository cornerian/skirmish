//! Regression pin for the ECB per-path load-flags fix (`docs/
//! ecb-load-flags.md`): the real recording's entry fall (P1 frames -52..-49),
//! replayed with the fighter's own actual recorded controller inputs through
//! the native `Match::step`, not a synthetic fixture. Ground-truth values
//! below were read directly from `tests/fixtures/slippi/parity/fox-fd.slp`
//! via `skirmish_replay::observation::expected`, independent of this
//! simulator.
//!
//! The jump landing at frames 418-421 (also pinned through pack v4) is not
//! exercised here: stepping this same match past frame 70 hits an unrelated
//! data gap (`docs/ecb-load-flags.md`'s "Known gap" note -- P1 transitions
//! out of GuardOn around frame 71 into an action `game::staling::flush`
//! classifies as an attack with no `move_id`, which it requires once any
//! rules profile enables staling). Reaching frame 418 through this pack's
//! own recorded inputs is blocked by that gap, not by anything this batch
//! changed; only the *recorded* (ground-truth, from the replay file itself)
//! position at frame 421 -- 0.0001001358, landed, matching the entry fall's
//! same mechanism -- has been confirmed, not Skirmish's own value there.
//! `docs/parity.md`'s own real-replay ratchet also cannot reach 418 yet, for
//! the same reason.
//!
//! Gated on the v4 gameplay export existing at a fixed archive path (not
//! `SKIRMISH_GAMEPLAY_DATA`, unlike `real_parity.rs`): this pins the specific
//! joint-index/flags combination that export's manifest records, so a
//! differently-versioned export at that env var wouldn't exercise the same
//! regression. Skips (without failing) when the export is not present, the
//! same convention as `real_parity.rs`.
use skirmish::game::data::MatchData;
use skirmish_cli::initialization;
use skirmish_replay::{
    match_validation,
    slippi::{Port, Replay, Timeline},
};
use std::{fs::File, io::BufReader};

const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/fox-fd.slp"
);
const V4_MATCH_DATA: &str =
    "/mnt/archive/datasets/melee/skirmish-gameplay/v4-snapshot-20260911/fox-fd/match-data.json";

#[test]
fn entry_fall_matches_the_recording_under_pack_v4() {
    let Ok(bytes) = std::fs::read(V4_MATCH_DATA) else {
        println!(
            "skip: {V4_MATCH_DATA} is not available in this environment; \
             see docs/gameplay-export.md."
        );
        return;
    };
    let data: MatchData = serde_json::from_slice(&bytes).unwrap();
    let replay = Replay::read(BufReader::new(File::open(REPLAY).unwrap())).unwrap();
    let init = initialization::build(data, &replay, None).unwrap();
    assert_eq!(init.ports, [Port::P1, Port::P4]);
    let mut game = match_validation::initialize(&init).unwrap();

    let indices = replay.frame_indices(Timeline::LastRecorded).unwrap();
    let mut checked = 0;
    for &index in indices {
        let frame = replay.frame(index).unwrap();
        let input = skirmish_replay::observation::controllers(&frame, init.ports).unwrap();
        let state = game
            .step(input)
            .unwrap_or_else(|e| panic!("frame {}: {e}", frame.id));
        let p1 = &state.fighters[0];
        match frame.id {
            // TopN/position.y (docs/ecb-load-flags.md's evidence): no
            // two-unit padding is applied to the airborne ECB, so these
            // three falling frames land on the recorded values exactly
            // (mode 6, `mpColl_LoadECB_inline(coll, 6)`).
            -52 => assert_position_y(p1.position[1], 1.764_920_4),
            -51 => assert_position_y(p1.position[1], -0.305_079_58),
            -50 => assert_position_y(p1.position[1], -2.605_079_7),
            // The recording lands here, not one frame early: the grounded
            // ECB anchors the bottom to 0 (mode 5) and the airborne floor
            // snap rests `position` itself on the floor whenever the raw
            // (unanchored) ECB bottom samples above position
            // (`ecb_unlocked` in `mpColl_80046904`, ported in
            // `game::collision::resolve`).
            -49 => {
                assert!(p1.grounded, "expected P1 grounded at frame -49");
                assert_position_y(p1.position[1], 0.0001);
                checked += 1;
                break;
            }
            _ => {}
        }
    }
    assert_eq!(checked, 1, "expected frame -49 to be reached");
}

fn assert_position_y(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "position.y {actual} does not match recorded {expected}"
    );
}
