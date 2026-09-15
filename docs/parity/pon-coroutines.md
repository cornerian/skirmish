# Pon coroutine parity

This note records the coroutine surface of the pinned Pon checkout
`ab9067dbd2899c64c4d67a4bc27b8ad49472b126` (2026-09-14). The source of record
is `/mnt/shared/Projects/Code/External/pon`, whose `HEAD` matches that commit.

Pon has a real stackless coroutine representation. Its lowering tests classify
an `async def` body as `is_coroutine`, retain it as a resumable generator-family
state machine, and lower `await` to an `Await` instruction. The runtime creates
a dedicated coroutine type whose `__await__` slot returns the coroutine itself
(`pon-runtime/src/types/coroutine.rs`), then exposes the following native
driving operations in `pon-runtime/src/abi/gen.rs`:

| Requirement | Pinned source evidence | Result |
| --- | --- | --- |
| `async def` produces a callable coroutine object | `pon-ir/src/lower.rs` tests around `lowers_async_function_def_as_coroutine`; `types/coroutine.rs` | Implemented and exercised natively |
| Await custom awaitable | `pon_await` checks `am_await`, then `__await__`, and gets an iterator | Implemented in runtime ABI |
| Suspend and resume | `pon_gen_send` dispatches the heap frame state machine | Implemented in runtime ABI |
| Throw/cancellation | `pon_gen_throw` and `pon_gen_close` inject exceptions / `GeneratorExit` | Implemented in runtime ABI |
| Cleanup across suspend | generator lowering carries `finally` handlers through suspend states; close path is explicit | Exercised: close runs `finally` |
| Synchronous completion vs pending | NULL plus pending `StopIteration` denotes completion; non-NULL denotes a yielded value | ABI contract present; facade must decode both paths |
| Error propagation | NULL sentinel and thread-local pending exception are used throughout | Exercised: exception from `finally` reaches Rust |
| Deterministic simulation clock | No game clock or scheduler integration in Pon | Missing; host-owned design required |
| Rollback reconstruction / snapshot | Heap coroutine frames contain resume state, sent/thrown payloads, and spill slots, but no host snapshot API | Missing; host must serialize/recreate state or forbid live coroutines across rollback |

The native conformance probes in
`crates/pon-runtime/tests/coroutines.rs` exercise the host seam.
It compiles an `async def`, acquires it through `pon_await`, observes a
deterministic `"pending"` event token, resumes it from a Rust native callback,
and verifies the returned `StopIteration.value`. A second case starts the
coroutine, closes it through `pon_gen_close`, and verifies that its `finally`
block appends `"closed"`. All four probes pass with `--test-threads=1` using the
pinned checkout and external target directory.

The same compiled callback also drives two independently-created coroutine
instances. Each yields an event token and resumes separately, demonstrating
that the callback can be reused without sharing continuation state. A final
probe raises from `finally` during cancellation and confirms that the error
crosses the native boundary.

The first run exposed and the runtime owner fixed an anonymous lifetime in the
facade's native-module registration signature. After the pinned Pon and Ruff
sources were fetched into `/tmp/skirmish-pon-cargo`, the focused native tests
passed in 0.04 seconds. The broader project suite still has unrelated runtime
warnings and should be validated by the owning integration task.

Coroutine support is a Pon runtime capability, not fighter parity. An event
subscription must be represented by a copied, immutable token such as
`(fighter_id, action_id, generation, event_kind, deadline_frame)`. A suspended
coroutine must never retain a host-owned event handle across frames; the engine
rebinds the token to the current event registry when the matching generation
completes. Cancellation invalidates that generation before closing the
coroutine, so a late event cannot resume it.

This ABI is not yet a gameplay executor for the class API's `Move.run`. The
ordinary move declarations can export identities and metadata, but no current
runtime path invokes an async `Move.run` and no public Pon API snapshots,
clones, or restores a `GenFrame`. Retaining a live coroutine in rollback state
would retain raw heap pointers, closure/function pointers, and arbitrary boxed
spill values. Replaying effects from the beginning after rollback is also
incorrect because commands before the await could execute twice.

The proposed implementation seam is a restricted external-state lowering
pass over Pon's existing IR: `pon_ir::lower_source` remains the parser and
front end, while a compiler adapter identifies the exported coroutine
function, its `Terminator::Suspend` states, supported await boundaries, and
serializable locals. The adapter must emit a native step table, not a second
Python bytecode VM:

```text
(source/ABI identity, move identity, resume tag, typed locals, event context)
    -> commands + Complete
    -> or commands + Wait(native await owner/deadline, next resume tag, locals)
```

The continuation record belongs in the cloneable native event state and must
carry source/ABI identity, move identity, resume tag, typed serializable
locals, and native await ownership/generation. Existing action-relative
deadlines and generation cancellation in `src/game/script/scheduler.rs` are
the intended clock and invalidation boundary. The current integration now has
an experimental scalar adapter in
`crates/pon-runtime/src/continuation.rs`. It uses Pon's lowered IR to emit
disposable native pre- and post-await steps. The acceptance test executes the
pre-step with `None`, copies the resulting `(None, 3)` state into a Rust-owned
scalar checkpoint, serializes and deserializes that checkpoint, drops the
native image, rebuilds from the same source, and verifies `3 + 2 == 5` after
resume. Supplying an altered copied value (`7`) produces `9`, proving that the
post-step performs the original arithmetic rather than returning a synthetic
constant. The six focused scalar continuation tests, including the zero-live-
local case, pass with:

```text
CARGO_HOME=/tmp/skirmish-pon-cargo CARGO_TARGET_DIR=/tmp/skirmish-pon-target \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
cargo test --locked --offline -p skirmish-pon-runtime \
  --features experimental-continuations --test continuation -- --test-threads=1
```

The experimental driver adapter also carries a cloneable `PendingMove` with
source/ABI identity, move identity, resume tag, event token, and scalar locals.
Three driver tests cover restoring a fresh native image, resuming at the
matching event, and preserving the pending state when resume fails. These
tests exercise the class-named move adapter only; they do not establish
integration with the SDK `FighterMove.run`, the actual `Match` event registry,
or gameplay rollback.

This is a scalar lowering proof, not full gameplay rollback integration. The
tests do not invoke `Move.run`, deliver a native event token through the game
registry, or compare a before/after command trace. The checkpoint is assembled
in the test from the copied scalar value; it is not yet produced by a gameplay
snapshot pipeline. The copied-token cancellation driver is covered, while
nested async, nested-finally cancellation, and multiple-await functions remain
unsupported. The adapter supports zero or one scalar live local and rejects
multiple scalar locals because its current resume ABI has one slot. No claim
of full rollback support follows from this milestone.
