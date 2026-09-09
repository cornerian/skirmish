# melee-physics

An independently buildable, allocation-free Rust translation of selected movement
functions from `src/melee/ft/ftcommon.c` at the revision in `../../upstream.lock.json`.
It supports headless consumers and has no renderer or platform dependencies.

`Movement` owns only the fighter fields read or written by these functions.
Its attributes are caller-provided game data; this crate invents no character
constants, stage geometry, collision results, or frame scheduler. Acceleration
methods set the original acceleration fields and do not silently integrate them.
Each public method documents its original function name. Shared implementation
removes duplicate C arithmetic while preserving comparison order and signed zero.

These helpers are not a complete game simulation. The source function allowlist
for differential testing is `../../tests/oracle/physics.functions.json`; other
functions in `ftcommon.c` remain untranslated. The C oracle uses verbatim selected
functions from a pinned snapshot with a compact host representation of the fields
they access. Host agreement does not prove whole-game or PowerPC equivalence.

Build and verify from this directory:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo build --locked
cargo test --locked
```

Run the original C comparisons from the repository root:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo test --locked --features c-oracle --test physics_differential
```

Use `/tmp/skirmish-target` when the shared scratch directory is not writable.
