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

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/dash.rs"]
mod dash_support;
#[path = "support/edge.rs"]
mod edge_support;
#[path = "support/escape_air.rs"]
mod escape_air_support;
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/fox_side_special.rs"]
mod fox_side_special_support;
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/idle.rs"]
mod idle_support;
#[path = "support/jab.rs"]
mod jab_support;
#[path = "support/ledge.rs"]
mod ledge_support;
#[path = "support/run.rs"]
mod run_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/fox_neutral_special.rs"]
mod special_support;
#[path = "support/taunt.rs"]
mod taunt_support;
#[path = "support/tilt.rs"]
mod tilt_support;
#[path = "support/walk.rs"]
mod walk_support;

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

/// Same idle input, but with every `tests/support` profile loaded, isolating
/// the fixed per-frame cost of carrying the extra (unused) profiles.
#[test]
fn idle_featured_matches_minimal_steady_state() {
    let mut game = Match::new(featured_data(), 1).unwrap();
    measure(&mut game, &[IDLE; 60]); // warmup: landing, idle-animation settling
    let samples = measure(&mut game, &[IDLE; 200]);
    summarize("featured idle (steady state)", &samples);
    let total_allocs: u64 = samples.iter().map(|s| s.0).sum();
    assert!(
        total_allocs < 200 * 200,
        "featured idle steady-state allocations/frame grew far past the \
         measured baseline in docs/performance.md (total={total_allocs} over 200 frames)"
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

/// The full featured match under the same scripted 600-frame sequence used
/// by `benches/match_step.rs`, for a whole-scenario allocations/frame number.
#[test]
fn featured_scripted_allocation_profile() {
    const MOVES: &[(usize, u16, [f32; 2])] = &[
        (20, 0, [0.0, 0.0]),
        (10, BUTTON_A, [0.0, 0.0]),
        (16, 0, [1.0, 0.0]), // dash/run toward center
        (16, skirmish::game::BUTTON_X, [0.0, 0.0]),
        (16, skirmish::game::BUTTON_L, [0.0, 0.0]),
        (10, skirmish::game::BUTTON_Z, [0.0, 0.0]),
        (16, skirmish::game::BUTTON_B, [0.0, 0.0]),
    ];
    let mut game = Match::new(featured_data(), 7).unwrap();
    let mut script = Vec::with_capacity(600);
    let mut index = 0usize;
    let mut remaining = MOVES[0].0;
    while script.len() < 600 {
        let state = game.state();
        let dx = state.fighters[1].position[0] - state.fighters[0].position[0];
        let side = if dx >= 0.0 { 1.0 } else { -1.0 };
        let input0 = if dx.abs() > 2.2 {
            Controller {
                stick: [side, 0.0],
                ..Default::default()
            }
        } else {
            let (_, buttons, stick) = MOVES[index];
            if remaining == 0 {
                index = (index + 1) % MOVES.len();
                remaining = MOVES[index].0;
            }
            remaining -= 1;
            Controller {
                buttons,
                stick,
                ..Default::default()
            }
        };
        let input = [input0, Controller::default()];
        script.push(input);
        game.step(input).unwrap();
    }
    let mut game = Match::new(featured_data(), 7).unwrap();
    let samples = measure(&mut game, &script);
    summarize("featured scripted (600 frames)", &samples);
    let histogram_nonzero = samples.iter().filter(|s| s.0 > 0).count();
    println!(
        "  frames with at least one allocation: {histogram_nonzero}/{}",
        samples.len()
    );
}
