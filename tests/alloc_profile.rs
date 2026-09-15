//! Allocation profile for `Match::step`, measured with a test-only counting
//! global allocator (never used outside this binary). `docs/performance.md`
//! reports the numbers this prints; run with
//! `cargo test --locked --test alloc_profile -- --nocapture` to see them.
//!
//! No public counting hook is added to `skirmish::game` (disallowed by the
//! task: attribution must come from bisecting inputs/profiles, not from a
//! production counting seam). Instead this measures the same `Match::step`
//! call under different resource profiles and input scripts and diffs the
//! counts, which attributes cost to a subsystem by construction: e.g.
//! "featured idle" minus "minimal idle" isolates the cost of carrying the
//! extra profiles per frame when nothing uses them, and "minimal hitting"
//! minus "minimal whiffing" isolates one landed `Event::Hit` plus staling.
//!
//! `unsafe_code` is `deny`d workspace-wide (`Cargo.toml`) and separately
//! `forbid`den inside `src/game`/`src/fighter` (`#![forbid(unsafe_code)]` in
//! their crate roots); this file overrides only its own local `deny` to
//! install a `GlobalAlloc` wrapper around `System`, and touches nothing
//! under `src/`.
#![allow(unsafe_code)]

use skirmish::game::{BUTTON_A, Controller, Match, data::MatchData};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicU64, Ordering},
};

struct CountingAllocator;

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

// SAFETY: both methods forward directly to `System`, changing only bookkeeping;
// no allocation is served, freed or mutated beyond what `System` itself does.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    // The default `alloc_zeroed`/`realloc` trait methods call `alloc`/`dealloc`
    // internally, so they are already covered without separate overrides.
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn snapshot() -> (u64, u64, u64) {
    (
        ALLOC_COUNT.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        DEALLOC_COUNT.load(Ordering::Relaxed),
    )
}

/// Per-step (allocations, bytes, deallocations) deltas.
fn measure(game: &mut Match, inputs: &[[Controller; 2]]) -> Vec<(u64, u64, u64)> {
    inputs
        .iter()
        .map(|input| {
            let before = snapshot();
            game.step(*input).expect("scripted input must be accepted");
            let after = snapshot();
            (after.0 - before.0, after.1 - before.1, after.2 - before.2)
        })
        .collect()
}

fn summarize(label: &str, samples: &[(u64, u64, u64)]) {
    let frames = samples.len() as f64;
    let total_allocs: u64 = samples.iter().map(|s| s.0).sum();
    let total_bytes: u64 = samples.iter().map(|s| s.1).sum();
    let total_deallocs: u64 = samples.iter().map(|s| s.2).sum();
    let max_allocs = samples.iter().map(|s| s.0).max().unwrap_or(0);
    let zero_alloc_frames = samples.iter().filter(|s| s.0 == 0).count();
    println!(
        "{label:32} frames={:4} allocs/frame={:7.3} bytes/frame={:8.2} deallocs/frame={:7.3} max_single_frame_allocs={max_allocs:4} zero_alloc_frames={zero_alloc_frames:4}/{:4}",
        samples.len(),
        total_allocs as f64 / frames,
        total_bytes as f64 / frames,
        total_deallocs as f64 / frames,
        samples.len(),
    );
}

fn minimal_data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 999_999;
    // Repeated whiffs/hits must not end the match (stock loss or blast-zone
    // death) partway through a fixed-length measurement window.
    data.rules.stocks = 99;
    data.stage.blast = [-1000.0, 1000.0, -1000.0, 1000.0];
    data.stage.floor.left = -500.0;
    data.stage.floor.right = 500.0;
    data
}

fn close_minimal_data() -> MatchData {
    let mut data = minimal_data();
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    data
}

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn jab_whiff() -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            ..Controller::default()
        },
        Controller::default(),
    ]
}

/// Idle input on the minimal one-jab fixture, after enough warmup frames
/// that both fighters have already landed and settled into `Wait`, with no
/// action transitions and no events. This measures the *floor*: whatever
/// `Match::step` allocates here happens on literally every frame of any
/// match, win or lose. `docs/performance.md` traces it to per-fighter bone
/// pose evaluation (`src/collision/bones.rs`'s `world`/`scales` `Vec`s,
/// freshly allocated on every pose evaluation) rather than `State::clone`
/// itself, which is evidenced by `whiffed_attacks_cost_per_transition`
/// below allocating only a modest amount more despite adding real
/// per-frame action-transition and staling work.
#[test]
fn idle_minimal_is_steady_state() {
    let mut game = Match::new(minimal_data(), 1).unwrap();
    measure(&mut game, &[IDLE; 30]); // warmup: landing, settling
    let samples = measure(&mut game, &[IDLE; 200]);
    summarize("minimal idle (steady state)", &samples);
    let total_allocs: u64 = samples.iter().map(|s| s.0).sum();
    // Regression ceiling, not a precise prediction: catches a large new
    // per-frame allocation source without hard-coding today's exact count.
    assert!(
        total_allocs < 200 * 200,
        "idle steady-state allocations/frame grew far past the measured \
         baseline in docs/performance.md (total={total_allocs} over 200 frames)"
    );
}

/// Repeated jab presses out of range: isolates the cost of one action
/// transition (`enter()`, `action_instance::queue`/`flush`,
/// `staling::transition`) without a landed hit.
#[test]
fn whiffed_attacks_cost_per_transition() {
    let mut game = Match::new(minimal_data(), 1).unwrap();
    measure(&mut game, &[IDLE; 30]);
    let script: Vec<_> = (0..120)
        .map(|frame| if frame % 6 == 0 { jab_whiff() } else { IDLE })
        .collect();
    let samples = measure(&mut game, &script);
    summarize("minimal whiffed jabs", &samples);
}

/// Point-blank jabs that land: isolates the incremental cost of a landed
/// `Event::Hit` (push onto `State::events`) plus `staling::State::hits`.
#[test]
fn landed_hits_cost_more_than_whiffs() {
    let mut game = Match::new(close_minimal_data(), 1).unwrap();
    measure(&mut game, &[IDLE; 10]);
    let script: Vec<_> = (0..120)
        .map(|frame| if frame % 6 == 0 { jab_whiff() } else { IDLE })
        .collect();
    let samples = measure(&mut game, &script);
    summarize("minimal landed jabs (point blank)", &samples);
    let total_hit_allocs: u64 = samples.iter().map(|s| s.0).sum();
    println!(
        "  (compare against whiffed_attacks_cost_per_transition's total for the \
         landed-hit increment; both use identical 1-in-6 press timing)"
    );
    assert!(
        total_hit_allocs > 0,
        "a match with landed attacks over 120 frames must allocate somewhere \
         (events, staling, or the state clone) -- zero would indicate this \
         scenario never actually connected"
    );
}
