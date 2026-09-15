//! Limited native/Pon match performance baseline.
//!
//! This is an ignored release benchmark because it deliberately runs a real
//! `Match::step` path over the shared synthetic combat fixture.  It is not a
//! full Fox parity benchmark; the Pon module only exercises the currently
//! integrated press-hook boundary while native movement, collision, and jab
//! resources remain the same.

#![allow(dead_code)]
#![allow(unsafe_code)]

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/jab.rs"]
mod jab;

use skirmish::game::{
    BUTTON_A, Controller, Match,
    script::{FighterView, HitView, Hook, LocalState, Program},
};
use skirmish_pon_runtime::{callback_phase_profile, reset_callback_phase_profile};
use skirmish_replay::{observation, slippi::Port};
use skirmish_script_runtime::{CallbackHandle, CompiledProgram, SourceBundle, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct CountingAllocator;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const FRAMES: usize = 600;

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class PressMove(Move):
    action = "jab"
    @hook.press("A")
    def pressed(self, fighter, context):
        # Observe the input without claiming it from native action selection.
        pass
ordinary = PressMove()
@register
class BaselineFighter(Fighter):
    name = "pon_match_performance"
    attributes = Attributes
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn data(with_pon: bool) -> skirmish::game::data::MatchData {
    let mut data = jab::profile(conformance::data());
    if with_pon {
        let program = Program::new(SOURCE).expect("performance Pon source compiles");
        for fighter in &mut data.fighters {
            fighter.script = Some(program.clone());
        }
    }
    data
}

fn inputs() -> Vec<[Controller; 2]> {
    (0..FRAMES)
        .map(|frame| {
            let buttons = if frame % 17 == 0 { BUTTON_A } else { 0 };
            [
                Controller {
                    buttons,
                    stick: [if frame % 31 < 15 { 0.7 } else { -0.7 }, 0.0],
                    ..Default::default()
                },
                Controller::default(),
            ]
        })
        .collect()
}

fn quantiles(values: &mut [u128]) -> (u128, u128, u128) {
    values.sort_unstable();
    let at = |percent: usize| values[(values.len() * percent / 100).min(values.len() - 1)];
    (at(50), at(95), at(99))
}

type SampleStats = (usize, u128, usize);

fn step_samples(
    mut game: Match,
    trace: &[[Controller; 2]],
) -> (Vec<u128>, usize, SampleStats, SampleStats) {
    let mut samples = Vec::with_capacity(trace.len());
    let mut allocations = 0;
    let mut callback = (0, 0, 0);
    let mut idle = (0, 0, 0);
    for input in trace {
        let before = ALLOCS.load(Ordering::Relaxed);
        let start = Instant::now();
        black_box(game.step(*input).unwrap());
        let elapsed = start.elapsed().as_nanos();
        let count = ALLOCS.load(Ordering::Relaxed) - before;
        samples.push(elapsed);
        allocations += count;
        let bucket = if input[0].buttons & BUTTON_A != 0 {
            &mut callback
        } else {
            &mut idle
        };
        bucket.0 += 1;
        bucket.1 += elapsed;
        bucket.2 += count;
    }
    (samples, allocations, callback, idle)
}

#[test]
#[ignore = "manual release benchmark; run with the documented shared Cargo environment"]
fn native_and_pon_match_performance_baseline() {
    let trace = inputs();
    let mut native = Match::new(data(false), 7).expect("native match loads");
    let mut pon = Match::new(data(true), 7).expect("Pon match loads");

    // Correctness gate is deliberately before timing.  Script source identity
    // differs, so compare the complete observable state instead of resource id.
    for input in &trace {
        native.step(*input).expect("native step");
        pon.step(*input).expect("Pon step");
        assert_eq!(
            observation::observe(&native, [Port::P1, Port::P2], [0, 0]),
            observation::observe(&pon, [Port::P1, Port::P2], [0, 0]),
            "native/Pon observable state diverged"
        );
    }

    let native_data = data(false);
    let pon_data = data(true);
    for (name, fixture) in [("native", &native_data), ("pon", &pon_data)] {
        let mut samples = Vec::with_capacity(32 * FRAMES);
        let mut allocations = 0usize;
        for _ in 0..32 {
            let (mut run_samples, count, callback, idle) =
                step_samples(Match::new(fixture.clone(), 7).unwrap(), &trace);
            samples.append(&mut run_samples);
            allocations += count;
            if name == "pon" {
                println!(
                    "pon dispatch split: callback frames={} avg={}ns {}allocs; non-callback frames={} avg={}ns {}allocs",
                    callback.0,
                    callback.1 / callback.0 as u128,
                    callback.2 / callback.0,
                    idle.0,
                    idle.1 / idle.0 as u128,
                    idle.2 / idle.0
                );
            }
        }
        let (median, p95, p99) = quantiles(&mut samples);
        println!(
            "{name} step/frame: median={median}ns p95={p95}ns p99={p99}ns; allocations/frame={:.2}",
            allocations as f64 / (32 * FRAMES) as f64
        );
    }

    for (name, fixture) in [("native", &native_data), ("pon", &pon_data)] {
        let mut game = Match::new(fixture.clone(), 7).unwrap();
        for input in trace.iter().take(240) {
            game.step(*input).unwrap();
        }
        let checkpoint = game.checkpoint();
        let mut create = Vec::with_capacity(32);
        let mut restore = Vec::with_capacity(32);
        for _ in 0..32 {
            let start = Instant::now();
            let created = black_box(game.checkpoint());
            create.push(start.elapsed().as_nanos());
            let start = Instant::now();
            game.restore_checkpoint(black_box(&checkpoint)).unwrap();
            restore.push(start.elapsed().as_nanos());
            black_box(created);
        }
        let (cm, cp95, cp99) = quantiles(&mut create);
        let (rm, rp95, rp99) = quantiles(&mut restore);
        println!("{name} checkpoint: median={cm}ns p95={cp95}ns p99={cp99}ns");
        println!("{name} restore: median={rm}ns p95={rp95}ns p99={rp99}ns");
    }

    let mut loads = Vec::with_capacity(16);
    for _ in 0..16 {
        let start = Instant::now();
        black_box(Program::new(SOURCE).expect("Pon module load"));
        loads.push(start.elapsed().as_nanos());
    }
    let (lm, lp95, lp99) = quantiles(&mut loads);
    println!("pon module load: median={lm}ns p95={lp95}ns p99={lp99}ns");

    // Isolate the compiled callback/ABI path from Match host commit and
    // physics. This direct dispatch uses the same prepared Program and empty
    // typed host state as the performance fixture.
    // Use an unfiltered combat hook here: `Program::dispatch` has no input
    // mask context, so the @hook.press selector would correctly be skipped.
    let direct_source = SOURCE
        .replace("@hook.press(\"A\")", "@hook.before_hit")
        .replace(
            "def pressed(self, fighter, context):",
            "def pressed(self, fighter, hit):",
        )
        .replace(
            "# Observe the input without claiming it from native action selection.\n        pass",
            "hit.damage = hit.damage + 1.0",
        );
    let program = Program::new(direct_source).unwrap();
    reset_callback_phase_profile();
    let fighter = FighterView::default();
    let locals = LocalState::default();
    let hit = HitView::default();
    let probe = program
        .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
        .unwrap();
    assert_eq!(probe.hit.as_ref().unwrap().damage, 1.0);
    let mut direct_times = Vec::with_capacity(64);
    let mut direct_allocs = 0usize;
    for _ in 0..64 {
        let before = ALLOCS.load(Ordering::Relaxed);
        let start = Instant::now();
        black_box(
            program
                .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
                .unwrap(),
        );
        direct_times.push(start.elapsed().as_nanos());
        direct_allocs += ALLOCS.load(Ordering::Relaxed) - before;
    }
    let (dm, dp95, dp99) = quantiles(&mut direct_times);
    println!(
        "direct Pon dispatch: median={dm}ns p95={dp95}ns p99={dp99}ns allocations={:.2}",
        direct_allocs as f64 / 64.0
    );
    if std::env::var_os("SKIRMISH_PON_PROFILE_PHASES").is_some() {
        for (phase, calls, nanos) in callback_phase_profile() {
            println!(
                "direct Pon phase: {phase} calls={calls} total={nanos}ns avg={}ns",
                nanos / calls.max(1)
            );
        }
    }
}

#[test]
#[ignore = "manual release callback phase profile; run with the documented shared Cargo environment"]
fn direct_callback_phase_profile() {
    assert!(
        std::env::var_os("SKIRMISH_PON_PROFILE_PHASES").is_some(),
        "set SKIRMISH_PON_PROFILE_PHASES=1 to enable phase timing"
    );
    let direct_source = SOURCE
        .replace("@hook.press(\"A\")", "@hook.before_hit")
        .replace(
            "def pressed(self, fighter, context):",
            "def pressed(self, fighter, hit):",
        )
        .replace(
            "# Observe the input without claiming it from native action selection.\n        pass",
            "hit.damage = hit.damage + 1.0",
        );
    let program = Program::new(direct_source).expect("direct callback source compiles");
    let fighter = FighterView::default();
    let locals = LocalState::default();
    let hit = HitView::default();
    let probe = program
        .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
        .expect("direct callback probe succeeds");
    assert_eq!(probe.hit.as_ref().unwrap().damage, 1.0);
    reset_callback_phase_profile();
    let mut times = Vec::with_capacity(64);
    let mut allocations = 0usize;
    for _ in 0..64 {
        let before = ALLOCS.load(Ordering::Relaxed);
        let started = Instant::now();
        black_box(
            program
                .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
                .expect("direct callback succeeds"),
        );
        times.push(started.elapsed().as_nanos());
        allocations += ALLOCS.load(Ordering::Relaxed) - before;
    }
    let (median, p95, p99) = quantiles(&mut times);
    println!(
        "direct Pon phase profile: median={median}ns p95={p95}ns p99={p99}ns allocations={:.2}",
        allocations as f64 / 64.0
    );
    for (phase, calls, nanos) in callback_phase_profile() {
        println!(
            "direct Pon phase: {phase} calls={calls} total={nanos}ns avg={}ns",
            nanos / calls.max(1)
        );
    }
}

#[test]
#[ignore = "manual release callback bridge profile; run with the documented shared Cargo environment"]
fn direct_callback_bridge_variants() {
    assert!(std::env::var_os("SKIRMISH_PON_PROFILE_PHASES").is_some());
    let fighter = FighterView::default();
    let locals = LocalState::default();
    let hit = HitView::default();
    for (label, body) in [
        ("empty", "pass"),
        ("host_write", "hit.damage = hit.damage + 1.0"),
    ] {
        let source = SOURCE
            .replace("@hook.press(\"A\")", "@hook.before_hit")
            .replace("def pressed(self, fighter, context):", "def pressed(self, fighter, hit):")
            .replace(
                "# Observe the input without claiming it from native action selection.\n        pass",
                body,
            );
        let program = Program::new(source).expect("bridge variant source compiles");
        let probe = program
            .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
            .expect("bridge variant probe succeeds");
        if label == "host_write" {
            assert_eq!(probe.hit.as_ref().unwrap().damage, 1.0);
        }
        reset_callback_phase_profile();
        let mut times = Vec::with_capacity(64);
        for _ in 0..64 {
            let started = Instant::now();
            black_box(
                program
                    .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
                    .expect("bridge variant succeeds"),
            );
            times.push(started.elapsed().as_nanos());
        }
        let (median, p95, p99) = quantiles(&mut times);
        println!("bridge variant {label}: median={median}ns p95={p95}ns p99={p99}ns");
        for (phase, calls, nanos) in callback_phase_profile() {
            println!(
                "bridge variant {label} phase: {phase} avg={}ns",
                nanos / calls.max(1)
            );
        }
    }
}

#[test]
#[ignore = "manual release raw callback profile; run with the documented shared Cargo environment"]
fn raw_callback_vs_sdk_dispatch_profile() {
    assert!(std::env::var_os("SKIRMISH_PON_PROFILE_PHASES").is_some());
    let bundle = SourceBundle::from_directory(
        "fighter-pack-v1",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/api"),
    )
    .expect("SDK bundle loads");
    let raw_source = format!("{SOURCE}\ndef __bench_noop(*args):\n    return None\n");
    let raw_program = CompiledProgram::new_with_bundle(
        raw_source,
        "raw_callback_profile.py",
        [CallbackHandle::new("__bench_noop")],
        Some(bundle),
    )
    .expect("raw callback program compiles");
    let raw_callback = raw_program
        .callback("__bench_noop")
        .expect("raw callback is retained");
    let mut raw_times = Vec::with_capacity(64);
    reset_callback_phase_profile();
    for _ in 0..64 {
        let started = Instant::now();
        black_box(
            raw_program
                .invoke_values(&raw_callback, &[Value::None, Value::None])
                .expect("raw retained callback succeeds"),
        );
        raw_times.push(started.elapsed().as_nanos());
    }
    let (raw_median, raw_p95, raw_p99) = quantiles(&mut raw_times);
    println!("raw retained callback: median={raw_median}ns p95={raw_p95}ns p99={raw_p99}ns");
    for (phase, calls, nanos) in callback_phase_profile() {
        println!("raw retained phase: {phase} avg={}ns", nanos / calls.max(1));
    }

    let sdk_source = SOURCE
        .replace("@hook.press(\"A\")", "@hook.before_hit")
        .replace(
            "def pressed(self, fighter, context):",
            "def pressed(self, fighter, hit):",
        );
    let sdk_program = Program::new(sdk_source).expect("SDK callback program compiles");
    let fighter = FighterView::default();
    let locals = LocalState::default();
    let hit = HitView::default();
    reset_callback_phase_profile();
    let mut sdk_times = Vec::with_capacity(64);
    for _ in 0..64 {
        let started = Instant::now();
        black_box(
            sdk_program
                .dispatch(Hook::BeforeHit, &fighter, Some(&hit), &locals)
                .expect("SDK dispatch callback succeeds"),
        );
        sdk_times.push(started.elapsed().as_nanos());
    }
    let (sdk_median, sdk_p95, sdk_p99) = quantiles(&mut sdk_times);
    println!("SDK dispatch callback: median={sdk_median}ns p95={sdk_p95}ns p99={sdk_p99}ns");
    for (phase, calls, nanos) in callback_phase_profile() {
        println!("SDK dispatch phase: {phase} avg={}ns", nanos / calls.max(1));
    }
}
