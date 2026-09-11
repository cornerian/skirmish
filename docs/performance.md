# Performance baseline

Measured against commit `0e48217f1d6274405a3ac774229e01cd9bc42610` on
2026-09-11, ahead of the parallel `src/game/**`/`src/fighter/**` refactors,
so later work can be judged against real numbers instead of intuition. This
document only measures and reports; none of the optimizations below are
applied here.

## Method

- Machine: AMD Ryzen 7 8700G (8 cores / 16 threads, 422-5177 MHz, 256 KiB
  L1d+L1i per core, 8 MiB L2 per core, 16 MiB shared L3), 60 GiB RAM, Linux
  7.2.3-1-cachyos, `rustc 1.98.1`. This is a shared interactive desktop, not
  an isolated benchmarking rig (no CPU pinning, no governor lock, no quiet
  boot) -- treat absolute numbers as indicative, not lab-grade, and prefer
  the relative comparisons between scenarios and micro-benchmarks.
- Build: `env CARGO_TARGET_DIR=/mnt/shared/tmp/skirmish-target-perf
  CARGO_INCREMENTAL=0 cargo bench --locked --bench match_step --
  --warm-up-time 1 --measurement-time 3`, 100 samples per benchmark
  (criterion default). `[profile.bench]` in `Cargo.toml` sets `debug = true`
  (symbols for any future profiler) and `overflow-checks = true`, matching
  `[profile.release]`, so these numbers include the same checked arithmetic
  a real `cargo build --release` pays for -- they are not an
  overflow-checks-off best case.
- Full criterion output: `bench_output.txt` next to this file's copy under
  `/mnt/archive/runs/skirmish-perf-baseline-20260911/`. The `change:` /
  `Performance has regressed` lines criterion prints are a self-comparison
  against an earlier, shorter smoke-test run (`--sample-size 10`) on this
  same shared machine while other work was running, not a real regression
  against a prior commit; there is no earlier baseline to compare against.
  Ignore them and read the `time:`/`thrpt:` lines directly.
- Allocation counts: `cargo test --locked --release --test alloc_profile --
  --nocapture` (a test-only counting `GlobalAlloc`, see
  `tests/alloc_profile.rs`), release profile since allocation *counts*
  measurably differ from the debug profile (see below). Output saved as
  `alloc_profile_output.txt` (release) and `alloc_profile_output_debug.txt`
  (debug, for comparison) in the same archive directory.
- `perf` and `valgrind` are not installed and this task may not install
  system packages. `samply` *was* installable as a pure cargo tool
  (`cargo install --root /mnt/shared/tmp/skirmish-perf-tools --locked
  samply`, no system package, succeeded) but cannot record: this sandbox's
  `/proc/sys/kernel/perf_event_paranoid` is `2`, and samply requires `1` or
  lower plus `sudo` to change it, which this task does not have and should
  not request. No `perf`/`flamegraph`/`samply` profile could be produced;
  CPU hot spots below come from criterion timings, the counting-allocator
  bisections, and reading the code the timings point at, as the task's
  fallback plan allows.

## Baseline: `Match::step` scenarios (600 frames each)

| Benchmark | Scenario | Median time / 600 frames | ns/frame | frames/sec |
|---|---|---:|---:|---:|
| `match_step/minimal_fixture_idle` | (a) smallest fixture (`integration-match.json`, jab only), idle input | 1.8429 ms | 3072 | ~325,600 |
| `match_step/featured_match_scripted` | (b) every `tests/support` profile, scripted 600-frame combat sequence | 2.1178 ms | 3530 | ~283,300 |
| `match_step/replay_comparison_loop` | (d) `Match::step` + `observation::observe` + field compare, 600 recorded frames | 3.6255 ms | 6043 | ~165,500 |

(a) and (b) both run `Match::step` 600 times with no other work timed
(construction happens in `iter_batched`'s untimed setup phase); (d) times
the same step cost plus post-frame observation extraction and comparison,
roughly double (a)/(b) per frame, which is expected -- it is doing strictly
more work per frame, not evidence of a separate slowdown.

## Baseline: checkpoint and observation (c, d)

| Benchmark | Median time |
|---|---:|
| `match_step/checkpoint_create` (`Match::checkpoint`) | 258.99 ns |
| `match_step/checkpoint_restore` (`Match::restore_checkpoint`) | 237.33 ns |
| `observation_extract_frame` (`skirmish_replay::observation::observe`) | 89.13 ns |

Checkpoint create/restore and observation extraction are all sub-microsecond
and small next to a full `Match::step` (~3.1-3.5 us): none of the three is a
priority target on its own, though checkpoint create/restore both clone the
whole `State` (see "Hot spots" below) and would shrink for free if `State`
itself got cheaper to clone.

## Micro-benchmarks (e): suspected hot spots with a public API

| Benchmark | What it calls | Median time |
|---|---|---:|
| `bones_pose_evaluate` | `collision::bones::Pose::evaluate` on fighter 0's real bone hierarchy | 219.85 ns |
| `sweep_capsule_capsule` | `collision::sweep::capsule_capsule`, one overlapping pair | 44.53 ns |
| `ecb_load_and_interpolate` | `collision::ecb::State::load_joints` + `interpolate` | 29.05 ns |

`simulation::pose` (the per-frame wrapper around `Pose::evaluate`) and
`game::collision`'s projection/contact code are `pub(crate)`/private to
`src/game`, so they could not be micro-benchmarked directly from `benches/`
without adding a public seam to `src/game` -- disallowed by this task's
scope. `simulation::pose` is called exactly twice per frame, once per
fighter (`src/game/simulation.rs:576-577`), so `bones_pose_evaluate`'s
219.85 ns is a direct, representative per-call cost, just not captured
in-process.

## Allocation profile

`tests/alloc_profile.rs` measures allocations by diffing a counting
`GlobalAlloc` around each `Match::step` call, then attributes cost by
bisecting inputs and resource profiles (per this task's constraint: no
counting hook was added to `skirmish::game` itself).

| Scenario | allocs/frame | bytes/frame | max single-frame allocs |
|---|---:|---:|---:|
| Minimal fixture, idle, steady state (post-landing `Wait`) | 85.9 | 6,569 | 176 |
| Featured match (every profile), idle, steady state | 25.1 | 1,853 | 31 |
| Minimal fixture, whiffed jabs (1-in-6 frames press A, out of range) | 123.3 | 9,046 | 195 |
| Minimal fixture, landed jabs (point-blank, 1-in-6 frames press A) | 140.5 | 10,584 | 273 |
| Featured match, full 600-frame scripted scenario (walk/dash/jump/attack/shield/grab/special mix) | 25.9 | 1,674 | 40 |

(release profile; the debug-profile run of the same five scenarios gave
80.4/57.0/130.9/131.6/25.9 allocs/frame respectively, with a much higher
landed-hit spike of 3,411 vs. release's 273 -- see "debug vs. release
allocation counts differ" below.)

Reading these together:

- **The floor is not zero.** Even a fighter already at rest in `Wait`, with
  no events and no action transitions, allocates on every single frame (86
  and 25 allocs/frame in the two idle scenarios). Since `State::clone()`
  itself should be near-allocation-free when its `Vec` fields are empty
  (`Vec::clone` allocates a buffer sized to the source's *length*, and an
  idle fighter's `events`/`staling::transitions`/`action_instance::pending`
  are all empty between frames), this floor has to come from work `advance`
  always does, not from the per-step state clone. `src/collision/bones.rs`'s
  `Pose::evaluate` is the direct, code-cited match: its own doc comment says
  "every call evaluates the supplied pose completely; there is no hidden
  frame cache," and it allocates four separate `Vec`s every call (`world`,
  `scales`, `visited`, `path`; `src/collision/bones.rs:80-83`) plus one more
  `Vec` in the resource-to-physics bone conversion feeding it
  (`src/game/simulation.rs:1614`). `simulation::pose` runs twice per frame
  unconditionally (`src/game/simulation.rs:576-577`), so pose evaluation is
  the direct, code-cited explanation for a meaningful share of this floor
  every frame, with the rest most likely other per-frame `pub(crate)`
  collection building inside `advance` that this task could not attribute
  more precisely without instrumenting `src/game` (disallowed).
- **Debug vs. release allocation *counts* differ, not just timing.**
  Allocation counts should in principle be a deterministic function of the
  code and inputs (same seed, same script), independent of optimization
  level; measured, they are not: e.g. minimal-fixture landed jabs peak at
  3,411 allocations on its worst frame in debug vs. 273 in release, and
  featured-idle drops from 57.0 to 25.1 allocs/frame. The likely cause is
  that release-mode inlining lets LLVM prove some short-lived `Vec`s
  (built, read, and dropped without escaping a function) are dead and
  elide them, while debug's per-function-boundary codegen keeps them
  concrete; this was not verified with a disassembly, so it is reported as
  an observation, not a proven mechanism. Practically: **debug-mode
  allocation counts substantially overstate real (release) cost here**, so
  the release-profile numbers above are the ones to optimize against, and
  any future allocation profiling on this codebase should default to
  `--release` for the same reason.
- **The minimal fixture allocates more than the featured match at idle**
  (85.9 vs. 25.1 allocs/frame), which is counterintuitive given the
  featured match carries far more optional resources. This was measured,
  not root-caused: the two fixtures differ in more than profile count (the
  minimal fixture has no `locomotion`/`idle` profile, so its physics and
  idle-animation code paths differ during the identical-length warmup
  window used before each measurement). Reported as an open finding rather
  than explained away.
- **Whiffed vs. landed hits cost more for a landed hit, both on average and
  at peak** (123.3 vs. 140.5 allocs/frame at an identical 1-in-6 press
  cadence in release; 195 vs. 273 on the worst single frame). The extra
  cost of a *landed* hit is
  concentrated in a couple of frames (contact resolution, damage-motion
  pose selection, staling/combo bookkeeping) rather than spread evenly, so
  an averaged per-frame number understates its worst-case cost -- relevant
  for any RL/coaching rollout that branches specifically at hit frames.
- **The realistic combat scenario's average (25.9 allocs/frame) is lower
  than the synthetic isolation scenarios above it.** That is expected, not
  a contradiction: roughly a third of the 600 scripted frames are spent
  closing distance (dash/run, no attacks), which dilutes the average
  relative to a script that presses an attack button every 6th frame for
  its entire length.

Byte volumes are all small (roughly 1.6-13 KB/frame); the count, not the
size, is what matters here, since each allocation is a `malloc`/`free`
round trip regardless of how few bytes it moves.

## Hot spots ranked, with evidence

1. **Bone pose evaluation has no cache, by design, and runs at least twice
   every frame.** `collision::bones::Pose::evaluate` (`src/collision/bones.rs:76-139`)
   allocates 4 `Vec`s per call and does a full topological walk of the bone
   hierarchy; `simulation::pose` (`src/game/simulation.rs:1578-1646`) wraps
   it with a 5th allocation (the bone-to-physics `.collect::<Vec<_>>()` at
   line 1614) and is called once per fighter per frame
   (`src/game/simulation.rs:576-577`). Measured directly at 219.85 ns/call
   (`bones_pose_evaluate`) and indirectly as a contributor to the
   ~25-86 allocs/frame idle floor above. This is the single largest,
   best-evidenced target: both a CPU cost (topological sort + matrix
   concatenation every frame, even when the pose has not changed) and an
   allocation cost (5 heap round-trips per fighter per frame minimum).
2. **`Match::step` clones the entire `State` on every call**
   (`self.state.clone()`, `src/game/mod.rs:531`), and `checkpoint()`/`reset()`
   clone it again (`src/game/mod.rs:513-522`). `State` embeds both
   `Fighter`s, each carrying a dozen+ nested sub-state structs. Measured
   indirectly: `checkpoint_create`/`checkpoint_restore` (258.99 ns / 237.33 ns)
   are pure `State` clones with no simulation work, so that is the clone's
   standalone floor; it is paid again, on top of `advance`'s own cost, on
   every `Match::step`. Not a large fraction of the ~3.1-3.5 us/frame total,
   but entirely avoidable overhead once double-buffering or copy-on-write
   state is in place, and it disproportionately dominates the *cheap*
   scenarios (checkpoint/restore, and any RL rollout that checkpoints often
   relative to how much it steps).
3. **One unconditional `Vec` allocation for hit contacts every frame,
   regardless of whether any hit occurs.** `let mut hits =
   Vec::with_capacity(2);` (`src/game/simulation.rs:642`) runs once per
   `advance()` call, allocating a 2-element buffer even on a frame with zero
   contacts. Small in isolation (one allocation/frame), but it is a clean,
   fully-understood, zero-risk target: the capacity is already a hard-coded
   2 (one contact per attacker slot in a 2-player match), so it maps
   directly onto a fixed-size `[Option<HitContact>; 2]` with no behavior
   change.
4. **Landed-hit frames allocate far more than their neighbors (up to 273
   allocations on one frame in release, 3,411 in debug)** in
   `landed_hits_cost_more_than_whiffs`, vs. steady-state's tens,
   concentrated rather than smoothed across
   frames. This was measured but not fully bisected further (would require
   instrumenting `src/game::collision`/`staling`/`damage`'s private
   functions individually, disallowed by this task); the queued-then-drained
   `action_instance::pending: Vec<u8>` (`src/fighter/action_instance.rs:12`)
   and `staling::State::transitions: Vec<Transition>`
   (`src/game/staling.rs:29`) are both first-push allocations that occur
   exactly on action-transition frames (their doc comments: "deferred only
   within one native callback turn... flushed before contacts and before
   publishing a frame/checkpoint"), which a landed hit is more likely to
   trigger (damage transition, staling transition) than a whiff.
5. **`replay_comparison_loop` roughly doubles per-frame cost over plain
   `Match::step`** (6043 ns/frame vs. 3072-3530 ns/frame), from adding
   `observation::observe` (89.13 ns, cheap) plus the `PartialEq` comparison
   and `Transition`/`Observation` cloning `validate`'s generic loop does per
   frame (`crates/replay-validation/src/lib.rs:107-174`,
   `benches/match_step.rs`'s `transitions.clone()` per batch). Not a
   `Match::step` hot spot itself, but relevant to anyone driving many
   counterfactual replay/coaching rollouts: the comparison harness's own
   overhead is comparable to the simulation it is checking.

## Optimization plan (not applied here; ranked by expected impact / risk)

Each item is behavior-neutral: it changes representation or caching, not
game rules, and is guarded by the existing correctness suite plus the
benchmarks/allocation test added in this task.

1. **Cache bone world matrices across frames when the pose is unchanged.**
   `Pose::evaluate`'s own doc comment already flags the "no hidden frame
   cache" design; add a per-`Fighter` cached `Pose` invalidated only when
   the sampled bone list or root transform actually changes (most frames in
   `Wait`, `Guard`, `Fall`, etc. reuse the same static/animated pose frame).
   Expected gain: this is the single largest identified cost (2 calls x 5
   allocations/frame minimum, ~220 ns/call measured), so a working cache
   should remove a large share of the idle-frame allocation floor and a
   comparable share of `bones_pose_evaluate`'s CPU cost on unchanged-pose
   frames. Guard: `bones_differential.rs`, `ecb_differential.rs`, and every
   `game_*_differential`/`game_hurtbox_geometry.rs`/`game_swept_hitboxes.rs`
   test that depends on world-space bone output must still pass bit-for-bit
   -- the cache must be proven invalidated correctly on every input that
   changes the pose (action transition, animation frame advance, facing
   flip, position/root change), not just spot-checked.
2. **Replace `let mut hits = Vec::with_capacity(2);`
   (`src/game/simulation.rs:642`) with a fixed `[Option<HitContact>; 2]` (or
   `arrayvec`-style inline buffer).** Expected gain: removes one
   unconditional allocation from every single frame of every match (100% of
   frames, per the allocation profile above), for the cost of an `if let
   Some`/index-based push instead of `Vec::push`. Guard:
   `combat_collision_differential.rs`,
   `game_hurtbox_states.rs`/`game_hurtbox_eligibility.rs`, and the
   `staling`/`combo`/`Event::Hit` integration tests, since this list feeds
   damage/hitlag/event dispatch directly.
3. **Avoid the full `State::clone()` in `Match::step`/`checkpoint`/`reset`.**
   Either double-buffer two `State`s and swap instead of clone-then-replace,
   or move to copy-on-write/`Arc`-shared substructures for the parts that
   rarely change within a frame. Expected gain: removes the ~240-260 ns
   clone cost (measured via `checkpoint_create`/`restore`) from every
   `Match::step`, `checkpoint()` and `reset()` call; proportionally largest
   for RL-style workloads that checkpoint/restore far more often than they
   advance. Guard: `mid_hitlag_checkpoint_replays_exactly_and_branches_without_shared_state`
   (`tests/game_matches.rs`) and every differential test that checkpoints
   mid-match are the direct oracle -- they already assert bit-exact replay
   from a checkpoint, which a double-buffer/COW change must continue to
   satisfy exactly.
4. **Arena or reuse `action_instance::pending`/`staling::transitions`
   buffers instead of letting them allocate-then-drop every action
   transition.** Both are drained every frame they are used
   (`src/fighter/action_instance.rs:16-22`, `src/game/staling.rs`); since
   they hold at most a couple of `u8`/`Transition` entries, a fixed-size
   `[Option<T>; N]` (mirroring `Fighter::hitboxes: [Track; 4]`'s existing
   pattern in the same struct) would remove their allocation entirely.
   Expected gain: smaller than items 1-3 individually, but directly
   explains part of the landed-hit spike (up to 273 allocations on one
   frame in release, 3,411 in debug); most valuable for hit-heavy/combo-heavy
   workloads. Guard:
   `action_instance_differential.rs`, `stale_differential.rs`,
   `game_staling.rs`, and the combo/instance-id fields in every Slippi
   observation differential test (`instance_id`/`last_hit_by_instance` are
   externally observable via `skirmish_replay::observation`).
5. **Give `events: Vec<Event>` a small inline capacity (e.g.
   `smallvec`-style or a fixed `[Option<Event>; N]` for the common case,
   falling back to heap only past N)**, since most frames produce 0-1
   events and `Vec::clone` already reallocates to fit the previous frame's
   length every step. Expected gain: modest and scenario-dependent (this
   task's allocation profile did not isolate `events` specifically from the
   pose-evaluation floor), lowest priority of the five; worth revisiting
   after 1-3 land and a fresh allocation profile can isolate its share
   cleanly. Guard: every test asserting on `state.events` contents
   (`Event::Hit`/`Grabbed`/`Knockout`/etc. across the `game_*.rs` suite) and
   `replay_validation`'s `Observation` comparisons.

Explicitly **not** on this plan: `serde_json` does not appear in
`Match::step`'s own path today (only once, at `Match::new`, to hash
resources into `resource_id`; see `src/game/mod.rs:486-489`). It is used
per-frame in `crates/skirmish-replay`/CLI trace *emission* paths, which are
outside this task's benchmarked `Match::step`/observation/checkpoint
scope -- worth a follow-up baseline of its own if trace-emission throughput
becomes a bottleneck, but not claimed as a `Match::step` hot spot here.

## What could not be measured

- No `perf`/`flamegraph`/`samply` call-graph profile (see "Method" above);
  hot spots are evidenced by targeted micro-benchmarks and code reading
  instead of a sampled profile.
- `simulation::pose`, `game::collision`'s projection/contact resolution, and
  `staling::transitions`'s exact sampling cost are `pub(crate)`/private to
  `src/game` and were not independently micro-benchmarked (would require a
  public seam into `src/game`, out of this task's scope); they are covered
  only indirectly, through the full `Match::step` scenarios and the
  allocation bisections.
- The idle-allocation gap between the minimal and featured fixtures (85.9
  vs. 25.1 allocs/frame, release) is reported as measured but not fully
  root-caused, and the debug-vs-release allocation-count gap itself was not
  confirmed against a disassembly.
- No allocation *size histogram* per call site was captured, only per-frame
  totals; the counting allocator records counts and byte totals but not a
  call-stack, so "the top allocation sites" above are identified by reading
  the code paths that must run every frame, cross-checked against the
  measured per-scenario totals, not by a live call-site breakdown.
