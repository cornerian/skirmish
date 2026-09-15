# Pon embedding probe

This opt-in standalone crate pins Pon at
`ab9067dbd2899c64c4d67a4bc27b8ad49472b126` and verifies the real embedding
surface. It compiles a Python class with a decorator once through Pon's
Cranelift JIT, then executes the compiled module. The decorated event handler
is entered three times from Rust through Pon's runtime ABI and validates its trace before
printing `pon-native-event-hook-ok` and `pon-probe-ok`.

The current public API does not expose a direct IR-function address lookup.
The probe instead uses Pon's public runtime ABI: `pon_load_global` retrieves a
published callable, and `pon_call` invokes it from Rust. This is the intended
native host callback seam for now; both functions use Pon's boxed object ABI,
and `pon_call` roots its operands during dispatch. A production fighter layer
should wrap this unsafe ABI in a typed, rooted handle and define the lifetime
of the compiled module and callback context.

Run it from this directory with the pinned cache and an external target directory:

```sh
env CARGO_HOME=/tmp/pon-cargo CARGO_TARGET_DIR=/tmp/skirmish-pon-target2 \
  cargo run --locked --release
```

The crate has its own workspace declaration so it does not change the
skirmish workspace or game runtime dependency graph. Pon currently pins the
nightly toolchain in its repository (`nightly-2026-04-29`) and requires the
Cranelift dependency set from that lockfile. This probe is therefore an
experimental integration check, not yet a production mod ABI.

Pon's `DynExecHandle` must remain alive while its compiled functions execute;
dropping it tears down the JIT module. Runtime initialization and GC state are
process-wide. The probe attaches its calling thread and publishes a stack
boundary around generated execution so GC can scan native entry frames. A
production fighter layer should retain that lifecycle and wrap the unsafe ABI
in a typed, rooted handle before using arbitrary game worker threads.

The verified environment is Rust/Cargo 1.98.1 (`rustc 1.98.1`, stable), with
the pinned Pon revision above and `--locked --release`; build artifacts remain
under `/tmp/skirmish-pon-target2`.
