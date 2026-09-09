# Skirmish

A Rust rewrite of [doldecomp/melee](https://github.com/doldecomp/melee), in progress.
The complete game is **not implemented** yet. This tree replaces the earlier
Skirmish prototype. Its history is archived on `archive/pre-rust-rewrite-20260909`;
`main` starts from a new root commit.

The migration prioritizes behavior over matching PowerPC assembly. Rust's
standard library replaces manual allocation and containers; established crates
provide parsing, command-line handling, math, hashing and test generation.
Game-specific arithmetic is retained when a library would change its results.

## Development

Use stable Rust and a C compiler for the optional differential tests. In xonsh:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

`/tmp/skirmish-target` is a fallback when shared storage is unavailable. No game
image is required for the host tests. The reference GameCube build needs your
Melee 1.02 GALE01 `main.dol`; it and game assets must stay outside this repository.
Tests against host-compiled C establish agreement for the tested functions and
inputs, not full-game or PowerPC equivalence.

## Current translation

The pinned source contains 2,441 C/C++/header/assembly files and 611,321 lines,
including the Dolphin SDK. `upstream.lock.json` records the revision and counts;
`cargo run --bin skirmish -- inventory /path/to/melee` produces a source/hash inventory.

| Library | Translated behavior |
| --- | --- |
| `melee-runtime::random` | HSD and MSL random sequences, explicit seeds and HSD seed-storage forgetting |
| `melee-runtime::ctype` | ASCII classification masks and case conversion, including upstream EOF/byte behavior |
| `melee-runtime::mbstring` | MSL wide-to-byte conversion and low-byte terminators |
| `melee-runtime::bytecode` | All implemented HSD bytecode opcodes, stack operations, branching, arithmetic and RNG |
| `melee-runtime::spline` | Hermite, linear/Bezier/B-spline/cardinal points and arc-length inversion |

The remaining game, engine, SDK and platform code is unported. Bytecode
transcendental tests allow a documented host-library tolerance; integer and
eligible floating-point operations use exact comparisons. This is not a playable
game or an RL environment yet.

See [headless architecture](docs/architecture.md) and
[equivalence testing](docs/equivalence.md). Independent library projects live under
`crates/`; rendering and training/coaching adapters are kept outside the runtime.
