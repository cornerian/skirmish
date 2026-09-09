# melee-runtime

An independently buildable Rust library of translated HAL/Metrowerks algorithms.
It has no rendering, audio device, filesystem, emulator, or command-line dependency.
Game-specific random sequences and arithmetic remain explicit; `Vec`, slices and
standard ASCII operations replace manual storage and tables where equivalent.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test -p melee-runtime
```

The parent project's `cargo test --features c-oracle` runs differential tests
against pinned original C. Those tests do not yet establish PowerPC equivalence.
Stateful algorithms own or explicitly borrow their state, allowing independent
headless environments. Bytecode `libm` transcendentals still need target validation.
