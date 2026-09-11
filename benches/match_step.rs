//! Criterion benchmarks for `Match::step` and the surrounding checkpoint and
//! observation paths, used to baseline performance ahead of the parallel
//! `src/game/**`/`src/fighter/**` refactors. See `docs/performance.md` for
//! the measured numbers, allocation findings and optimization plan. This
//! file and the `[[bench]]`/dev-dependency wiring in `Cargo.toml` are the
//! only changes this task makes outside test/doc helpers; no gameplay
//! behavior is touched.
//!
//! Scenarios:
//! (a) `minimal_fixture_idle` -- the smallest fixture (`integration-match.json`,
//!     jab only) stepped with idle input.
//! (b) `featured_match` -- every profile the `tests/support/*.rs` builders
//!     expose (locomotion, shield, grab, escape, tilt, smash, edge, dash,
//!     jab, walk/run animation, ledge, escape-air, neutral special, Fox side
//!     special, idle-animation, taunt) driven by a scripted 600-frame input
//!     sequence that closes distance, walks, dashes, jumps, attacks an
//!     in-reach opponent, shields, grabs/throws, escapes and casts specials.
//! (c) `checkpoint_create`/`checkpoint_restore` on a mid-match featured state.
//! (d) `observation_extract` (`skirmish_replay::observation::observe`, the
//!     Slippi post-frame extraction) and `replay_comparison_loop`
//!     (`replay_validation::validate` over a 600-frame self-recorded file).
//! (e) Targeted micro-benchmarks for suspected hot spots that have a public
//!     API (`perf` and `flamegraph`/`samply` are unavailable, see
//!     `docs/performance.md`): `bones_pose_evaluate` (per-pose bone matrix
//!     evaluation, `src/collision/bones.rs`, no frame cache by design),
//!     `sweep_capsule_capsule` (hitbox/hurtbox narrow phase,
//!     `src/collision/sweep.rs`), and `ecb_load_and_interpolate` (ECB
//!     rebuild + interpolation, `src/collision/ecb.rs`). Collision
//!     projection and staling sampling are `pub(crate)`/private to
//!     `src/game`, so they are only covered indirectly through (a)-(d).
//!
//! Reuses the same `tests/support/*.rs` fixture builders the integration
//! tests use (via `#[path]`, exactly as the test binaries already do) so the
//! benchmarked match is the same synthetic world the correctness suite
//! exercises, not a bench-only stand-in.

#[path = "../tests/support/conformance.rs"]
mod conformance;
#[path = "../tests/support/dash.rs"]
mod dash_support;
#[path = "../tests/support/edge.rs"]
mod edge_support;
#[path = "../tests/support/escape_air.rs"]
mod escape_air_support;
#[path = "../tests/support/escape.rs"]
mod escape_support;
#[path = "../tests/support/fox_side_special.rs"]
mod fox_side_special_support;
#[path = "../tests/support/grab.rs"]
mod grab_support;
#[path = "../tests/support/idle.rs"]
mod idle_support;
#[path = "../tests/support/jab.rs"]
mod jab_support;
#[path = "../tests/support/ledge.rs"]
mod ledge_support;
#[path = "../tests/support/run.rs"]
mod run_support;
#[path = "../tests/support/smash.rs"]
mod smash_support;
#[path = "../tests/support/special.rs"]
mod special_support;
#[path = "../tests/support/taunt.rs"]
mod taunt_support;
#[path = "../tests/support/tilt.rs"]
mod tilt_support;
#[path = "../tests/support/walk.rs"]
mod walk_support;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use replay_validation::{Checkpoint as ValidationCheckpoint, FrameStepper, Transition, validate};
use skirmish::game::{
    self, BUTTON_A, BUTTON_B, BUTTON_DPAD_UP, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Match,
    data::MatchData,
};
use skirmish_replay::observation::{self, Observation};
use skirmish_replay::slippi::Port;
use std::{hint::black_box, io::Write, path::PathBuf};

const FEATURED_FRAMES: usize = 600;
const SEED: u32 = 7;
const PORTS: [Port; 2] = [Port::P1, Port::P2];
const CHARACTERS: [u8; 2] = [0, 0];

/// Every profile exposed by the `tests/support/*.rs` builders, composed the
/// same way the most feature-complete integration tests already do (see
/// `tests/game_taunt.rs`, `tests/game_edges.rs`) plus walk/run animation,
/// ledge, air-dodge, neutral special and Fox side special on top.
fn featured_data() -> MatchData {
    let data = conformance::data();
    let data = grab_support::profile(data);
    let data = escape_support::profile(data);
    let data = tilt_support::profile(data);
    let data = smash_support::profile(data);
    let data = edge_support::profile(data);
    let data = dash_support::profile(data);
    let data = jab_support::profile(data);
    let data = walk_support::profile(data);
    let data = run_support::profile(data);
    let data = escape_air_support::profile(data);
    let data = ledge_support::profile(data);
    let data = special_support::profile(data);
    let data = fox_side_special_support::profile(data);
    let data = idle_support::profile(data);
    taunt_support::profile(data)
}

fn minimal_data() -> MatchData {
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    // Measure steady-state `Playing`-phase steps, not the two-frame countdown
    // or an early match-end, so the per-frame cost is comparable across scenarios.
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 999_999;
    data
}

#[derive(Clone, Copy)]
enum Move {
    Idle,
    Jab,
    TiltUp,
    TiltDown,
    Smash,
    Shield,
    EscapeRoll,
    Grab,
    Jump,
    AirDodge,
    NeutralSpecial,
    SideSpecial,
    Taunt,
}

impl Move {
    fn controller(self, side: f32) -> Controller {
        match self {
            Move::Idle => Controller::default(),
            Move::Jab => Controller {
                buttons: BUTTON_A,
                ..Default::default()
            },
            Move::TiltUp => Controller {
                buttons: BUTTON_A,
                stick: [0.0, 0.6],
                ..Default::default()
            },
            Move::TiltDown => Controller {
                buttons: BUTTON_A,
                stick: [0.0, -0.6],
                ..Default::default()
            },
            Move::Smash => Controller {
                cstick: [side, 0.0],
                ..Default::default()
            },
            Move::Shield => Controller {
                buttons: BUTTON_L,
                ..Default::default()
            },
            Move::EscapeRoll => Controller {
                buttons: BUTTON_L,
                stick: [side, 0.0],
                ..Default::default()
            },
            Move::Grab => Controller {
                buttons: BUTTON_Z,
                ..Default::default()
            },
            Move::Jump => Controller {
                buttons: BUTTON_X,
                ..Default::default()
            },
            Move::AirDodge => Controller {
                buttons: BUTTON_L,
                stick: [side, 0.0],
                ..Default::default()
            },
            Move::NeutralSpecial => Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Move::SideSpecial => Controller {
                buttons: BUTTON_B,
                stick: [side, 0.0],
                ..Default::default()
            },
            Move::Taunt => Controller {
                buttons: BUTTON_DPAD_UP,
                ..Default::default()
            },
        }
    }
}

/// Build a 600-frame scripted input sequence by actually driving a throwaway
/// match: close the gap toward the opponent whenever too far to reach, then
/// cycle through jab/tilts/smash/shield/grab/jump/air-dodge/specials/taunt
/// once in range. Recorded once (outside any timed benchmark) so the
/// benchmarked routines replay a fixed, representative sequence.
fn scripted_inputs() -> Vec<[Controller; 2]> {
    const MOVES: &[(usize, Move)] = &[
        (20, Move::Idle),
        (10, Move::Jab),
        (12, Move::TiltUp),
        (14, Move::TiltDown),
        (16, Move::Smash),
        (20, Move::Shield),
        (10, Move::EscapeRoll),
        (10, Move::Grab),
        (16, Move::Jump),
        (14, Move::AirDodge),
        (16, Move::NeutralSpecial),
        (16, Move::SideSpecial),
        (10, Move::Taunt),
    ];
    let mut game = Match::new(featured_data(), SEED).unwrap();
    let mut script = Vec::with_capacity(FEATURED_FRAMES);
    let mut move_index = 0usize;
    let mut move_remaining = MOVES[0].0;
    while script.len() < FEATURED_FRAMES {
        let state = game.state();
        let dx = state.fighters[1].position[0] - state.fighters[0].position[0];
        let side = if dx >= 0.0 { 1.0 } else { -1.0 };
        let input0 = if dx.abs() > 2.2 {
            Controller {
                stick: [side, 0.0],
                ..Default::default()
            }
        } else {
            let kind = MOVES[move_index].1;
            if move_remaining == 0 {
                move_index = (move_index + 1) % MOVES.len();
                move_remaining = MOVES[move_index].0;
            }
            move_remaining -= 1;
            kind.controller(side)
        };
        let input = [input0, Controller::default()];
        script.push(input);
        game.step(input)
            .expect("scripted input must always be accepted by validation");
    }
    script
}

fn run_script(game: &mut Match, script: &[[Controller; 2]]) {
    for input in script {
        game.step(black_box(*input)).unwrap();
    }
}

fn bench_minimal(c: &mut Criterion) {
    let data = minimal_data();
    let base = Match::new(data, SEED).unwrap();
    let idle = [Controller::default(); 2];
    let script = vec![idle; FEATURED_FRAMES];
    let mut group = c.benchmark_group("match_step");
    group.throughput(Throughput::Elements(FEATURED_FRAMES as u64));
    group.bench_function("minimal_fixture_idle", |b| {
        b.iter_batched(
            || base.clone(),
            |mut game| run_script(&mut game, &script),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_featured(c: &mut Criterion) {
    let base = Match::new(featured_data(), SEED).unwrap();
    let script = scripted_inputs();
    let mut group = c.benchmark_group("match_step");
    group.throughput(Throughput::Elements(FEATURED_FRAMES as u64));
    group.bench_function("featured_match_scripted", |b| {
        b.iter_batched(
            || base.clone(),
            |mut game| run_script(&mut game, &script),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_checkpoint(c: &mut Criterion) {
    let mut game = Match::new(featured_data(), SEED).unwrap();
    let script = scripted_inputs();
    // Checkpoint from a mid-match, feature-exercised state (roughly halfway
    // through the scripted sequence), not the trivial initial state.
    run_script(&mut game, &script[..FEATURED_FRAMES / 2]);

    let mut group = c.benchmark_group("match_step");
    group.bench_function("checkpoint_create", |b| {
        b.iter(|| black_box(game.checkpoint()));
    });
    let checkpoint = game.checkpoint();
    let mut target = game.clone();
    group.bench_function("checkpoint_restore", |b| {
        b.iter(|| {
            target.restore_checkpoint(black_box(&checkpoint)).unwrap();
        });
    });
    group.finish();
}

struct Stepper {
    game: Match,
}

impl Clone for Stepper {
    fn clone(&self) -> Self {
        Stepper {
            game: self.game.clone(),
        }
    }
}

impl FrameStepper for Stepper {
    type Checkpoint = game::Checkpoint;
    type Input = [Controller; 2];
    type Observation = Observation;
    type Error = game::Error;

    fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<(), Self::Error> {
        self.game.restore_checkpoint(checkpoint)
    }

    fn advance(&mut self, input: &Self::Input) -> Result<Self::Observation, Self::Error> {
        self.game.step(*input)?;
        Ok(observation::observe(&self.game, PORTS, CHARACTERS))
    }
}

type SelfPlayTransitions = Vec<Transition<[Controller; 2], Observation>>;

/// Record a 600-frame self-played match to an on-disk JSONL file (for
/// provenance/inspection) and return the in-memory transitions the
/// benchmark actually replays, so the timed loop measures comparison CPU
/// cost rather than file I/O and JSON parsing.
fn record_self_play() -> (
    PathBuf,
    ValidationCheckpoint<game::Checkpoint>,
    SelfPlayTransitions,
) {
    let mut game = Match::new(featured_data(), SEED).unwrap();
    let script = scripted_inputs();
    let checkpoint = ValidationCheckpoint {
        next_frame: 1,
        state: game.checkpoint(),
    };
    let dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/mnt/shared/tmp"));
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("skirmish-bench-self-recorded.jsonl");
    let mut file =
        std::fs::File::create(&path).expect("self-recorded replay file must be writable");
    let mut transitions = Vec::with_capacity(FEATURED_FRAMES);
    for (index, input) in script.iter().enumerate() {
        game.step(*input).unwrap();
        let expected = observation::observe(&game, PORTS, CHARACTERS);
        let frame = (index + 1) as i32;
        writeln!(
            file,
            "{}",
            serde_json::json!({"frame": frame, "input": input, "expected": expected})
        )
        .unwrap();
        transitions.push(Transition {
            frame,
            input: *input,
            expected,
        });
    }
    (path, checkpoint, transitions)
}

fn bench_observation(c: &mut Criterion) {
    let mut game = Match::new(featured_data(), SEED).unwrap();
    let script = scripted_inputs();
    run_script(&mut game, &script);
    c.bench_function("observation_extract_frame", |b| {
        b.iter(|| black_box(observation::observe(black_box(&game), PORTS, CHARACTERS)));
    });
}

fn bench_replay_loop(c: &mut Criterion) {
    let (path, checkpoint, transitions) = record_self_play();
    eprintln!(
        "match_step bench: self-recorded {} frames to {}",
        transitions.len(),
        path.display()
    );
    let base = Stepper {
        game: Match::new(featured_data(), SEED).unwrap(),
    };
    let mut group = c.benchmark_group("match_step");
    group.throughput(Throughput::Elements(transitions.len() as u64));
    group.bench_function("replay_comparison_loop", |b| {
        b.iter_batched(
            || base.clone(),
            |mut stepper| {
                validate(
                    &mut stepper,
                    &checkpoint,
                    transitions.clone(),
                    |expected, actual| (expected != actual).then_some(()),
                )
                .unwrap();
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_micro(c: &mut Criterion) {
    use skirmish::collision::{bones, ecb, sweep};

    // (e.1) Bone pose evaluation: fighter 0's real bone hierarchy from the
    // featured resources, matching per-frame evaluation cost exactly since
    // the same public `Pose::evaluate` runs from `game::simulation::pose`.
    // `data::Bone::physics` is crate-private, so convert with the same field
    // mapping by hand (`translation`/`rotation`/`scale` -> `LocalTransform`).
    let physics_bones: Vec<bones::Bone> = featured_data().fighters[0]
        .bones
        .iter()
        .map(|bone| bones::Bone {
            parent: bone.parent,
            local: bones::LocalTransform {
                translation: bone.translation,
                rotation: bone.rotation,
                scale: bone.scale,
            },
            classical_scale: bone.classical_scale,
        })
        .collect();
    c.bench_function("bones_pose_evaluate", |b| {
        b.iter(|| black_box(bones::Pose::evaluate(black_box(&physics_bones)).unwrap()));
    });

    // (e.2) Hitbox/hurtbox narrow phase: a representative overlapping pair.
    let a = skirmish::fighter::combat::Capsule {
        start: [0.0, 0.0, 0.0],
        end: [0.0, 1.0, 0.0],
        radius: 0.5,
    };
    let b_capsule = skirmish::fighter::combat::Capsule {
        start: [0.3, 0.2, 0.0],
        end: [0.3, 1.2, 0.0],
        radius: 0.5,
    };
    c.bench_function("sweep_capsule_capsule", |b| {
        let mut closest = sweep::ClosestPair::default();
        b.iter(|| {
            black_box(sweep::capsule_capsule(
                black_box(&a),
                black_box(&b_capsule),
                &mut closest,
            ))
        });
    });

    // (e.3) ECB rebuild (six bone-joint world samples) and interpolation,
    // the per-frame collision-box source for movement/contact resolution.
    let world: [[f32; 2]; 6] = [
        [0.0, 0.0],
        [0.3, 1.5],
        [-0.3, 1.5],
        [0.0, 2.0],
        [0.4, 0.5],
        [-0.4, 0.5],
    ];
    let parameters = ecb::JointParameters {
        side_y_offset: 0.2,
        height_threshold: 4.0,
        width_threshold: 4.0,
    };
    c.bench_function("ecb_load_and_interpolate", |b| {
        b.iter(|| {
            let mut state = ecb::State::default();
            state.load_joints(black_box(world), [0.0, 0.0], &parameters, 0);
            state.interpolate(black_box(0.5)).unwrap();
            black_box(&state);
        });
    });
}

criterion_group!(
    benches,
    bench_minimal,
    bench_featured,
    bench_checkpoint,
    bench_observation,
    bench_replay_loop,
    bench_micro,
);
criterion_main!(benches);
