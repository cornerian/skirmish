# Pon integration acceptance plan

Pon integration is in progress. The embedded runtime imports the class based
fighter API, and focused tests compile that package and exercise the native
ABI through `Program`; the native Rust event/action path remains the gameplay
authority. A focused clean-process full Fox load now passes in
`tests/pon_runtime_bundle.rs` with the explicitly verified stdlib archive.
Gameplay parity and async rollback are not yet verified. This plan defines the evidence required before a Pon backend
can replace that path for Fox. Passing a compiler smoke test or a single
callback is not integration.

## Required boundary

The supported input is a validated, typed fighter module. `Fighter` owns a
typed `Move` registry and typed `Parameters`, `ActionState`, resource handles,
and hook registrations. Move objects are immutable and shareable; all mutable
execution state belongs to a fighter/action instance and is included in the
checkpoint. The loader resolves and validates the module once, freezes callback
handles and resource references, and stores a content and ABI identity in
`Program` and every checkpoint.

The callback ABI must expose copied, finite event payloads and bounded typed
commands. It must support the existing event vocabulary (input edges, action
availability/entry/exit, animation end, scheduled deadlines/markers,
command-trace changes, hit/receive-hit, projectile contact, landing, surface
contact, ground/air change, and platform-drop decisions). There is no frame or
tick polling callback. Continuous movement, collision, hitlag, projectiles,
staling and stage rules remain native systems; callbacks request commands and
the host applies them transactionally.

Special policies may use finite async move functions as event waiters. Ordinary
actions use `ActionMove`: a frozen descriptor with a required canonical action
string or `ActionDescriptor` and an optional resource, with no `run` or timer
behavior. Event callbacks remain native-dispatched; continuous movement,
collision, hitlag, and resource timeline sampling remain host systems.

The final authoring boundary is one source file per character, such as
`fox.py`, containing the complete class-based fighter declaration and its
behavior. Character authors should express attacks as `ActionMove` slots and
specials as class based event policies through a small declarative API with
typed validation. The current `scripts/fighters/fox.py` is consolidated into
that one-file shape; its remaining gameplay parity and rollback gaps are
tracked below.

## Gates

1. **Load and typing.** Valid Fox source loads; malformed, unknown, untyped,
   duplicate, out-of-range, non-finite, or undeclared fields fail before match
   construction. The same result is obtained after serialization and reload.
   Resource and module identities are deterministic and included in replay
   compatibility checks.
2. **Lifecycle.** Every Fox move can enter, receive its declared events,
   complete, be interrupted, land, change ground/air state, or be cancelled.
   Repeated entry and cancellation leaves no subscriptions, tasks, hitboxes,
   projectiles, or stale state. Event ordering is asserted against the native
   host contract.
3. **Rollback.** Checkpoint/restore reproduces byte-identical state and future
   events for every move, nested async task, timer/deadline, projectile,
   persistent typed state, and resource identity. Rollback must not retain a
   foreign VM heap or execute callbacks twice.
4. **Multi-mod isolation.** Two or more fighters with different modules and
   resources can run in one match. A module cannot see another module's state
   or callbacks; registration order and cloned `Arc` programs do not change
   results. Fox and Falco are the minimum pair.
5. **Runtime loading.** A module can be loaded and validated from a native
   resource bundle without an ISO, DOL, emulator, network, or developer
   checkout. Loading occurs outside `Match::step`; stepping performs no source
   parsing, callback-name lookup, or resource discovery.
6. **Failure atomicity.** A callback error, command-limit violation, invalid
   value, resource failure, or transaction failure restores the pre-dispatch
   host state and reports the first cause. Limits cover source, instructions,
   memory, locals, strings, subscriptions, commands, and nested async work.
7. **Performance.** Benchmark native and Pon-backed Fox on the same fixed
   synthetic match and replay prefix. Report median and tail `Match::step`,
   allocation count, checkpoint/restore cost, and module-load cost separately.
   The Pon path must have a recorded native baseline and meet the user's
   requirement of at least directly compiled native Rust performance for the
   overall match profile, including median and tail `Match::step` time and
   relevant allocation costs. Callback dispatch, module load, and
   checkpoint/restore remain separately reported diagnostics; a passing
   correctness test cannot waive a material frame-time regression.
8. **Parity.** Pon and native Fox produce the same observation trace for all
   accepted cases. Any difference is classified as source behavior, resource
   data, host ordering, arithmetic/f32, or unsupported coverage; it cannot be
   hidden by weakening the observation profile.

## Verification artifacts

The integration suite should include typed API compile/load tests, event-order
and cancellation tests, async deadline tests, failure-atomicity tests,
checkpoint branch tests, two-module isolation tests, runtime bundle tests, and
the performance benchmark. Add these under `tests/pon_acceptance/` only when
the backend exists. Keep original-C differential tests for exact arithmetic
functions and use real replay traces for whole-engine claims.

The first integrated module should be the complete Fox module, with every
move listed in `docs/parity/fox-acceptance.md`. A special-only or callback-only
demo is an intermediate milestone and must remain labeled as such.

## Bundled package and imports

The distributable Skirmish binary must carry the Pon compiler/runtime as Cargo
native code. The pinned implementation is the `pon-jit` and `pon-runtime`
crates at commit `ab9067dbd2899c64c4d67a4bc27b8ad49472b126`; its transitive
runtime includes the Ruff parser/AST, Cranelift JIT, GC, and the native Python
object/type/ABI implementation. Cargo embeds or links these Rust artifacts in
the Skirmish executable. No CPython installation, `libpython`, PyO3, maturin,
or build-machine checkout is a runtime dependency.

The package also needs two native resource sets:

1. The curated native Pon module registry and the Skirmish bridge module. The
   registry currently provides the eager core (`builtins`, `sys`, `_io`,
   `time`, `os`, `_thread`) and lazy modules such as `math`, `itertools`,
   `random`, `struct`, `re` support, `weakref`, and `asyncio` support. The
   bridge must be registered before compiling a fighter module and must expose
   only the typed host ABI and resource handles.
2. A content-pinned pure-Python standard-library tree. The pinned checkout's
   source is `pon-conformance/vendor/cpython-3.14/Lib` (currently about 1,845
   files and 38 MiB). At minimum the fighter authoring package imports
   `abc.py`, `dataclasses.py`, `typing.py`, `types.py`, `enum.py`,
   `weakref.py`, `math.py`/`re` support, and their transitive imports. Bundle
   the complete curated `Lib` tree rather than guessing a transitive subset;
   importlib, encodings, collections, inspect and exception/traceback support
   are loaded lazily and are easy to omit accidentally. Exclude tests,
   `site-packages`, `.pyc`, and host-specific extension binaries unless a
   separately hashed module is intentionally supported.

The runtime's current resolver searches native modules first, then installed
packages/source roots, then the vendored stdlib. A packaged game must replace
the development defaults (current directory, `.pon/packages/site-packages`,
`PYTHONPATH`, `PONPATH`, and `PON_IMPORT_PATH`) with one controlled, read-only
mod root selected by the game resource manifest. `PON_STDLIB_PATH` may select
the bundled stdlib during development, but an unset or invalid environment
variable must never make a release silently search a developer checkout.
Normalize and contain every module path, reject traversal and symlink escapes,
and hash the exact source bytes before compilation. Mods may import only the
standard library and declared Skirmish/fighter packages; arbitrary native
extensions and ambient site-packages are rejected.

The package manifest should record the Pon commit, compiler/runtime ABI,
CPython `Lib` tree hash, target triple/CPU feature set, bridge ABI version,
mod source hashes, and gameplay resource hashes. JIT cache keys include all of
these values plus the module filename/import-root identity. Cache entries are
target-specific executable code and are disposable; a mismatch forces a fresh
compile. The cache must live in the platform cache directory, never beside a
source checkout or in the repository.

`skirmish-pon-runtime` currently reflects the required ownership: `Program` is
sendable source identity, while `PreparedProgram` and Pon host capabilities
are thread-affine and must be prepared, invoked, and dropped on one attached
OS thread. A production loader needs an explicit per-thread attach/teardown
owner, serialized module registration, and a test that concurrent loads cannot
race Pon's process-global import/module tables. Callback host capabilities are
scoped to one dispatch and expire when that scope closes.

JIT and AoT are separate distribution modes. JIT loading ships source modules,
the bundled stdlib, compiler/runtime code, and a target-compatible Cranelift
JIT; compilation occurs once at mod load and callbacks use retained handles.
AoT loading ships generated native module bodies plus the Pon runtime and its
generated name/module initializers; the reachable pure-Python imports must be
resolved and embedded at build time using the same native-shadowing and import
order rules. AoT does not justify shipping a CPython installation, and it must
fail closed when a reachable import is absent rather than falling back to the
build host. The acceptance suite should test both modes when AoT support is
enabled, while gameplay can initially standardize on JIT.

Current bootstrap obstacles are concrete: Cargo resolves Pon from its pinned
Git revision, while the vendored `Lib` tree is outside the Skirmish repository;
the development resolver still has ambient env/cwd fallbacks. The focused
clean-process full Fox load in `tests/pon_runtime_bundle.rs` passes with the
explicitly verified stdlib archive. The native typed host fields and
the `PreparedProgram` to `Match::step` bridge are covered by current tests.
The CLI now verifies and materializes an explicitly configured stdlib archive
before startup, and automatically discovers a verified packaged default beside
the executable when present. The async run executor/rollback path, complete Fox
gameplay parity, and the performance gate remain open. Packaging is therefore
not complete until the Pon sources/
dependency lock and stdlib tree are shipped under content hashes, the
controlled import root is wired as the release default, and a clean machine
with no Python installation can load and execute a complete Fox module.
