# Pon match performance baseline

This is a limited baseline for the Pon integration boundary. It measures a
real `Match::step` over the shared synthetic conformance world with the jab
resource profile installed (`tests/support/conformance.rs` and
`tests/support/jab.rs`). The native arm has no script. The Pon arm loads a
small class based fighter whose `PressMove` is explicitly bound to native
`"jab"`; its `@hook.press("A")` callback returns `None`, allowing native action
selection, movement, collision, and attack resources to proceed. The test
first steps both matches through the same fixed 600-frame input trace and
compares `skirmish_replay::observation::observe` after every step. This is the
repository's documented gameplay observation profile. Full internal `State`
serialization differs because Pon's class registration assigns a different
internal action-instance identity even when the observed action/physics fields
match; that difference is retained and reported rather than discarded. This
establishes equivalence for this fixture and profile, but does not establish
complete Fox behavior or full game parity.

Run the ignored release benchmark from `skirmish/` with the shared, stable
environment:

```text
xonsh --no-rc -c 'env TMPDIR=/mnt/shared/tmp/skirmish-build-temp CARGO_TARGET_DIR=/mnt/shared/tmp/skirmish-release-target CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 SKIRMISH_PON_STDLIB_ARCHIVE=/mnt/archive/runs/skirmish-pon-stdlib-v3.14.0-20260923/pon-stdlib.tar.gz SKIRMISH_PON_STDLIB_SHA256=91aab4eb859690ebe1ecf8e3e21957378fea86b1ebac0a61e7559145f64c642d cargo test --locked --release --test pon_match_performance -- --ignored --nocapture'
```

The archive is the verified durable release input. Its manifest identity is
`b14b1766c1138f9aad84664dbbd4f52242b874a1003810a7ec69410cc5f83782`.
It was built from Pon commit `ab9067dbd2899c64c4d67a4bc27b8ad49472b126`
and CPython `v3.14.0`. The retained provenance record is
`/mnt/archive/runs/skirmish-pon-stdlib-v3.14.0-20260923/README.md`.

The 2026-09-23 release attempt used commit `951a4cf5351041100a0d8aa199b900d892acc70f`
with a dirty working tree (the source changes were pre-existing and were not
modified by the benchmark). Its durable run record is
`/mnt/archive/runs/skirmish-pon-release-20260923/SUMMARY.md`, with terminal
output in `benchmark.log`. The completed timing sections reported:

```text
native step/frame: median=3150ns p95=4670ns p99=5770ns; allocations/frame=26.48
pon dispatch split: callback frames=36/600; callback average approximately 326–360µs and 4,904 allocations; non-callback average approximately 4.66–5.09µs and 36–51 allocations
pon step/frame: median=3290ns p95=317647ns p99=346857ns; allocations/frame=348.83
native checkpoint: median=520ns p95=530ns p99=640ns
native restore: median=590ns p95=610ns p99=820ns
pon checkpoint: median=520ns p95=540ns p99=1000ns
pon restore: median=580ns p95=610ns p99=690ns
```

This was not a passing full ignored-test run. Three additional ignored phase
profile tests failed immediately because `SKIRMISH_PON_PROFILE_PHASES=1` was
unset. The baseline then remained CPU-bound in its 16-iteration Pon module-load
section for approximately 22 minutes without emitting module-load quantiles and
was interrupted. Consequently, no module-load timing is claimed, and these
figures must not be treated as a complete benchmark pass or as a speed claim
against the original decompilation.

The successful 2026-09-15 post-dispatch-fast-path release run printed these per-frame step samples
(32 repetitions of the 600-frame trace), allocation counts, and checkpoint/restore
quantiles. The run was performed while other workspace tests were compiling, so
the callback wall time is a noisy upper-bound profile; allocation counts are the
more stable signal:

```text
native step/frame: median=2990ns p95=4710ns p99=5690ns; allocations/frame=23.09
pon dispatch split (36 callback / 564 non-callback frames): callback averages=230–402µs and 4,179–5,001 allocations; non-callback averages=4.64–5.52µs and 38–52 allocations
pon step/frame: median=3380ns p95=240815ns p99=374978ns; allocations/frame=311.44
native checkpoint: median=490ns p95=510ns p99=1350ns
native restore: median=560ns p95=590ns p99=1890ns
pon checkpoint: median=490ns p95=510ns p99=2740ns
pon restore: median=570ns p95=590ns p99=980ns
```

Construction/setup is outside the step timing loop; module loading is timed
as its own operation. The release profile was optimized with overflow checks,
and the test-only global allocator counts allocation calls. These counts
include all allocations made during each step, including the Pon bridge and
runtime. Outputs are terminal-only and should be copied to a run record under
`/mnt/archive/runs` when a durable measurement is needed.

Before the dispatch fast path, this same benchmark measured Pon at 10,670ns
median, 199,534ns p95, and 758.21 allocations/frame. After it, Pon's median
is 2,880ns (near the 2,810ns native median) and 310.36 allocations/frame.
The tail remains 190.233µs p95 versus 4.270µs native. The scoped split
attributes that tail directly to callback frames, which averaged 195–220µs
and 4,151–4,878 allocations, versus 4.26–4.50µs and 38–55 allocations for
non-callback frames. The dominant next optimization target is therefore
callback dispatch and its temporary typed host/value collections; module load
is a separate cold cost and checkpoint/restore is not the bottleneck in this
fixture. This is an
indicative shared-machine baseline.
and this benchmark must not be described as a full-Fox parity or replacement
gate. The measured callback path is still far above the explicit requirement
to reach native Rust callback performance. The open work remains complete Fox
source coverage, multi-module gameplay parity, and async rollback under match
load.

## Callback phase attribution

The split is measured at two boundaries: `Match::step` is timed with the
fixture's input trace, and `Program::dispatch(Hook::BeforeHit, ...)` is timed
directly with a callback that increments `hit.damage`. The direct callback
probe includes the prepared-runtime call while excluding native movement,
collision, and host commit. It also checks that the returned damage is `1.0`.

Source inspection attributes the direct probe to these phases, in order:

1. Install the stack boundary, bundle module guard, and prepared import-policy
   guard.
2. Box the fighter and optional hit arguments into rooted Pon values.
3. Enter module execution and call the compiled callback.
4. Capture changed `sys.modules` entries, validate them against import policy,
   and rebuild persistent roots. `root_module_values` walks every current
   module attribute on every callback; this is the strongest allocation and
   latency candidate visible in the current implementation.
5. Drop temporary host references, recover the staged `CombatHost`, validate
   state, and convert the staged result back to native types.

The callback/non-callback split, allocation counter, and phase timers are
measured. The isolated 2026-09-15 direct
profile (64 callbacks, with `SKIRMISH_PON_PROFILE_PHASES=1`) measured:

```text
direct Pon phase profile: median=185124ns p95=286125ns p99=350307ns allocations=1404.09
guards:             8,369ns/call
boxing:             6,401ns/call
compiled_call:    113,887ns/call
recapture_rooting: 48,622ns/call
guard_teardown:    10,814ns/call
recovery_unbox:        28ns/call (Pon result decoding only)
```

The phase totals are deliberately reported separately: the direct wall time
also includes phase boundary overhead and unmeasured work between phases.
`recovery_unbox` covers only Pon result decoding; staged `CombatHost` recovery
and native state validation occur in the caller outside this runtime phase
counter. The compiled callback and recapture/root rebuilding are the largest
measured components. Guard teardown is included explicitly and retains the
original module-policy destruction order. Any root-snapshot optimization must
preserve module-guard restoration order and GC roots for parked modules.

An additional isolated bridge comparison used the same 64-call direct probe
with an empty callback and with `hit.damage = hit.damage + 1.0`. The empty
variant measured 92,832 ns compiled-call and 95,394 ns recapture/rooting per
call; the host-write variant measured 104,051 ns and 33,277 ns respectively.
Because these runs were noisy and the supposed host-write cost was smaller in
the second run, they do not identify an actionable SDK dispatcher or native
descriptor cost. The current evidence supports profiling the compiled callback
and root rebuilding further with a less contended machine before changing the
SDK bridge.

A raw retained-call comparison then isolated the SDK dispatch wrapper. Both
variants used the same SDK bundle and prepared-runtime guards; the raw variant
invoked a retained top-level `__bench_noop(*args)` through
`CompiledProgram::invoke_values`, while the SDK variant used the normal
`Program::dispatch` path. In the same 2026-09-15 release run:

```text
raw retained callback: median=171593ns p95=281595ns p99=335226ns
raw compiled_call:       1849ns/call
SDK dispatch callback: median=251934ns p95=461268ns p99=859005ns
SDK compiled_call:      77591ns/call
```

The raw and SDK runs had noisy guard and teardown timings, but the compiled
call phase separated cleanly: the SDK dispatch wrapper added about 76 µs per
call over the retained no-op callback. The wrapper includes `_loader.dispatch`
callback-index handling, argument wrapping, callback invocation, and result
unwrapping. This is actionable evidence for an SDK-dispatch optimization while
preserving the existing ABI and isolation guards.

The first SDK change replaced the callback argument generator expression with
an eager list comprehension in `_loader.dispatch`, preserving the same
left-to-right wrapping before invoking the callback. The isolated release
probe improved the SDK compiled phase from 77,591 ns/call to 53,804 ns/call
and its median wall time from 251,934 ns to 184,293 ns. The raw retained
callback's compiled phase was 1,147 ns/call in the same run, so the wrapper is still the
dominant cost. Focused `callback_dispatch` regression coverage passed.

The second SDK change added identity fast paths for `None` in `_native.wrap`
and `_native.unwrap`; these return the same primitive value without touching
host proxy, scalar, collection, or enum handling. The isolated release probe
then measured SDK compiled dispatch at 36,549 ns/call and median wall time at
146,172 ns, versus 53,804 ns/call and 184,293 ns before this change. The raw
retained callback's compiled phase was 871 ns/call in the same run. The direct callback damage
assertion and focused callback-dispatch regression test remained passing.

## Architecture boundary for the remaining cost

The latest isolated profile measured normal dispatch at 146,172 ns median, with
these averages per call:

```text
guards             49,934 ns
boxing              3,703 ns
compiled_call      36,549 ns
recapture_rooting  13,681 ns
guard_teardown     39,390 ns
recovery_unbox         34 ns
```

The retained raw callback was 871 ns in the compiled phase, so the historical
normal-dispatch cost was split between the SDK wrapper and the per-call
module/import scope. The prepared-program callback batching boundary described
above is now implemented in `src/game/script.rs` and the script runtime. The
runtime's invocation scope is reused by contiguous callbacks from one prepared
program, while callback writes remain visible through the shared staged host.
The scope closes on success or error and retains the existing policy lock,
frozen import checks, module ownership checks, root snapshots, and restoration
order. Scope ownership still prevents callbacks from another program,
unrelated native work, or another match frame from sharing it.

The 2026-09-15 figures in this document are retained historical measurements.
The 2026-09-23 run record above adds a partial current release measurement with
an explicit interrupted module-load section and failing profile tests. Neither
record establishes decomp-speed or complete behavioral parity.
