//! Real-replay check for the laser-muzzle frame-ordering fix in
//! `simulation::advance` (`src/game/simulation.rs`'s `fire_pose` capture).
//!
//! Unlike `real_parity.rs`'s ratchet (which shells out to `validate-replay`
//! and that harness's own `match_validation::validate` explicitly refuses
//! any frame carrying recorded items,
//! `crates/skirmish-replay/src/match_validation.rs`), this test drives
//! `game::Match::step` directly with each frame's real recorded controller
//! inputs (`skirmish_replay::observation::controllers`) and reads the
//! simulator's own `state.projectiles`, so a frame that also happens to
//! carry a recorded item observation does not abort the walk -- the
//! recorded item is never consumed as simulator input either way.
//!
//! `fox-fd-3.slp`'s own P2 (lower port, sorted to simulator index 0 by
//! `initialization::build`, matching this repository's other "P1" muzzle
//! probes) fires an airborne Blaster shot recorded as `FOX_LASER` item
//! spawn id 1 at frame -14, position `(-13.0461, 18.1391)`, velocity
//! `(7.0, 0.0)`; the same item is recorded at `(-6.0461, 18.1391)`,
//! `(0.9539, 18.1391)` and `(7.9539, 18.1391)` on frames -13, -12 and -11
//! (`docs/validation.md`'s muzzle-residual entries; `docs/fox-neutral-
//! special.md`'s own citation of this same spawn). P2's own recorded
//! post-frame position is `(-29.4516, 8.5501)` at frame -15 and
//! `(-28.223993, 8.3501)` at frame -14.
use anyhow::{Context, Result};
use skirmish::game::projectile::ProjectileKind;
use skirmish_cli::{initialization, pack};
use skirmish_replay::{
    match_validation,
    observation::controllers,
    slippi::{Replay, Timeline},
};
use std::{env, fs, path::PathBuf};

const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/slippi/parity/fox-fd-3.slp"
);

#[test]
fn laser_muzzle_uses_pre_physics_position_on_fox_fd_3() -> Result<()> {
    let Ok(root) = env::var("SKIRMISH_GAMEPLAY_DATA") else {
        println!(
            "skip: SKIRMISH_GAMEPLAY_DATA is not set; real-replay comparison against the \
             published gameplay export is skipped. See docs/gameplay-export.md."
        );
        return Ok(());
    };
    let pairing_dir = PathBuf::from(&root).join("fox-fd");
    let Some(match_data_path) = pack::discover_match_data(&pairing_dir) else {
        println!(
            "skip: neither match-data.bin nor match-data.json exists under {}; \
             SKIRMISH_GAMEPLAY_DATA={root} is set, but the fox-fd export has not landed there \
             yet.",
            pairing_dir.display()
        );
        return Ok(());
    };
    let data = pack::load_match_data(&match_data_path)?;

    let bytes = fs::read(REPLAY).with_context(|| format!("reading {REPLAY}"))?;
    let replay = Replay::read(bytes.as_slice())?;
    let built = initialization::build(data, &replay, None)?;
    let ports = built.initialization.ports;
    let mut game = match_validation::initialize(&built.initialization)?;

    let indices = replay.frame_indices(Timeline::LastRecorded)?;
    // Recorded laser positions (`x`, `y`), frames -14..=-11, and the
    // recorded fighter position one frame before firing (frame -15) /
    // the frame it fires (-14), all cited in this file's own doc comment
    // above.
    const RECORDED_LASER_X: [(i32, f32); 4] = [
        (-14, -13.0461),
        (-13, -6.0461),
        (-12, 0.9539),
        (-11, 7.9539),
    ];
    const RECORDED_LASER_Y: f32 = 18.1391;
    const RECORDED_FIGHTER_POSITION: [(i32, [f32; 2]); 2] =
        [(-15, [-29.4516, 8.5501]), (-14, [-28.223993, 8.3501])];

    let mut observed_laser = Vec::new();
    let mut observed_fighter = Vec::new();
    for &index in indices {
        let frame = replay.frame(index)?;
        let inputs = controllers(&frame, ports).map_err(anyhow::Error::msg)?;
        let state = game.step(inputs)?;
        if frame.id == -15 || frame.id == -14 {
            observed_fighter.push((frame.id, state.fighters[0].position));
        }
        if (-14..=-11).contains(&frame.id) {
            let laser = state
                .projectiles
                .iter()
                .find(|projectile| {
                    projectile.owner == 0 && projectile.kind == ProjectileKind::FoxLaser
                })
                .map(|projectile| [projectile.position[0], projectile.position[1]]);
            observed_laser.push((frame.id, laser));
        }
        if frame.id >= -11 {
            break;
        }
    }

    // Sanity check: the fighter's own recorded position at -15/-14 must
    // still match (this fix does not touch fighter movement, only which
    // position the muzzle is evaluated against), otherwise a mismatch
    // below would be meaningless.
    for &(id, expected) in &RECORDED_FIGHTER_POSITION {
        let actual = observed_fighter
            .iter()
            .find(|(observed_id, _)| *observed_id == id)
            .unwrap_or_else(|| panic!("frame {id} was not stepped"))
            .1;
        assert!(
            (actual[0] - expected[0]).abs() <= 1e-3 && (actual[1] - expected[1]).abs() <= 1e-3,
            "frame {id}: fighter position {actual:?} != recorded {expected:?}"
        );
    }

    // The fix (evaluating the muzzle from this frame's pre-physics
    // position, `simulation::advance`'s new `fire_pose`) removes the
    // ~1.4-unit same-frame double-move this repository's own
    // `docs/validation.md` muzzle-residual entries measured before it (the
    // post-move position was one full frame of fighter translation ahead
    // of where the decomp's own proc order actually evaluates the muzzle).
    //
    // A smaller residual remains and is asserted exactly below, not hidden
    // behind a loose tolerance. Since `docs/validation.md`'s 2026-09-16
    // model-scale entry, Fox's pose is evaluated with `model_scaling`
    // (`0.96`) applied to the root bone's scale -- Skirmish evaluated every
    // Fox bone 4% too large before that loop, the pack having carried no
    // model scale at all. The residual below is that entry's own
    // oracle-faithful "air frame 5" row, cross-checked bit-for-bit against
    // the real `HSD_JObjMakeMatrix` oracle (`tests/bones_differential.rs`'s
    // `real_pose` module) under both classical-scale configurations the
    // investigation considered, which turned out numerically identical for
    // this bone chain: this pose is genuinely oracle-faithful, not merely
    // unchased. It does *not* reproduce this recording's own Slippi target
    // (the entry's own table, and its "does not reproduce the Slippi-
    // replay-derived targets, reported honestly rather than fudged" -- a
    // hybrid of unscaled rotation with scaled translation would match
    // Slippi to `1e-4`, but no permutation of `HSD_JObjMakeMatrix`'s own
    // scale-removal bookkeeping produces that hybrid for a uniformly-scaled
    // root). Every one of these four frames comes out `(-0.1847, -0.0419)`
    // from its own recorded position (both the recording's and the
    // simulator's own laser move at the identical constant `(7.0, 0.0)`
    // per frame after spawn, so a residual fixed at spawn time stays
    // constant rather than growing).
    const RESIDUAL: [f32; 2] = [-0.1847, -0.0419];
    const TOLERANCE: f32 = 2e-4;

    assert_eq!(observed_laser.len(), 4, "expected exactly frames -14..=-11");
    for (&(id, recorded_x), &(observed_id, position)) in
        RECORDED_LASER_X.iter().zip(observed_laser.iter())
    {
        assert_eq!(id, observed_id);
        let [x, y] = position.unwrap_or_else(|| {
            panic!("frame {id}: no active FoxLaser projectile owned by player 0")
        });
        let expected = [recorded_x + RESIDUAL[0], RECORDED_LASER_Y + RESIDUAL[1]];
        assert!(
            (x - expected[0]).abs() <= TOLERANCE && (y - expected[1]).abs() <= TOLERANCE,
            "frame {id}: laser position ({x}, {y}) != recorded ({recorded_x}, \
             {RECORDED_LASER_Y}) + residual {RESIDUAL:?} = {expected:?}"
        );
    }

    Ok(())
}
