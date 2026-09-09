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

Use stable Rust and a C compiler (Peppi's compression dependencies and the
optional differential tests use native C). In xonsh:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

`/tmp/skirmish-target` is a fallback when shared storage is unavailable. Building,
running and testing this project **must never require an ISO, DOL, emulator or
GameCube runtime**. It targets native execution on modern machines. Game data
will use native resources; missing data remains an explicit migration task.
Tests against host-compiled C establish agreement for the tested functions and
inputs; full-game equivalence remains unverified.

See the [integration coverage and missing-behavior tests](docs/testing.md).
Ignored conformance scenarios describe unimplemented behavior and are expected
to fail when explicitly run; their absence from the passing suite is reported.

## Current translation

The pinned source contains 2,441 C/C++/header/assembly files and 611,321 lines,
including the Dolphin SDK. `upstream.lock.json` records the revision and counts;
`cargo run --bin skirmish -- inventory /path/to/melee` produces a source/hash inventory.

| Library | Translated behavior |
| --- | --- |
| `runtime::random` | HSD and MSL random sequences, explicit seeds and HSD seed-storage forgetting |
| `runtime::ctype` | ASCII classification masks and case conversion, including upstream EOF/byte behavior |
| `runtime::mbstring` | MSL wide-to-byte conversion and low-byte terminators |
| `runtime::bytecode` | All implemented HSD bytecode opcodes, stack operations, branching, arithmetic and RNG |
| `runtime::spline` | Hermite, linear/Bezier/B-spline/cardinal points and arc-length inversion |
| `runtime::id` | Object-ID lookup, replacement, removal and default-table reset using `HashMap` |
| `runtime::quaternion` | Six matrix, axis/Euler rotation, multiplication and interpolation routines, using scalar `glam` plus compatibility wrappers |
| `physics` | Movement, skeletal poses, environmental collision boxes and substeps, directed stage queries, swept capsules, knockback, DI and hitlag in a standalone `no_std` library (poses use `alloc`) |
| `input` | Original controller stick/trigger clamping and four-port processing with owned calibration in a `no_std` library |
| `replay` | Streaming checkpoint/step/observation validation machinery |
| `peppi-adapter` | Peppi 2.1.2 parsing, retained replay primitives/columns, rollback and finalized-frame selection, native-validator transitions |
| `arena` | Experimental native two-player match: stage/ECB collision, animated swept jab, damage and DI, KOs, stocks, respawn, timeout, checkpoints and replay-stepper integration |

Most game, engine, SDK and platform code remains unported. Transcendental
tests allow a documented host-library tolerance; integer and
eligible floating-point operations use exact comparisons. The match slice uses
an explicitly synthetic fixture; a faithful Melee matchup and a training adapter
are not implemented yet.

Run a complete native headless demonstration with
`cargo run --locked --bin skirmish -- demo-match`. It emits a semantic JSONL trace
of a scripted jab sequence through stock loss, respawn and a winner. Bones and
collision run in physics; there is no rendering dependency. See the
[match API, native-data format and coverage limits](docs/match.md). Real Fox and
stage data subsets are preserved with [provenance and missing fields](docs/native-data.md).

Inspect a completed Slippi replay with
`cargo run --locked --bin skirmish -- inspect-replay /path/to/game.slp`.
Add `--finalized-only` to select its explicitly confirmed frame prefix. The
[Peppi adapter](crates/peppi-adapter) preserves recorded fields and produces
validator transitions. Compare those transitions against real native
`Match::step` calls with:

```xonsh
cargo run --locked --bin skirmish -- validate-replay /path/to/game.slp --initialization /path/to/init.json
```

The required initialization embeds native match data, a seed, explicit port
mapping, the next replay frame and deterministic warmup inputs. The command
compares the entire selected suffix, reports the first difference, and exits
unsuccessfully on mismatch or unsupported input. Its `fighter-post-v1` policy
checks exact position, facing, percent, stocks and airborne observations for two
human leaders. Matching those fields does not certify complete state or Melee
gameplay. The current match rules and real Fox resources remain incomplete; see
the [initialization format and comparison limits](docs/replays.md).

See [headless architecture](docs/architecture.md) and
[equivalence testing](docs/equivalence.md), plus the
[replay integration contract](docs/replays.md). Independent library projects live under
`crates/`; rendering and training/coaching adapters are kept outside the runtime.

`skirmish-probe` is a headless RNG executable for testing the comparison harness.
The suite compiles a separate executable from original C, compares it with the
Rust executable at two C optimization levels, and verifies that deliberate
divergence, input mutation, nonzero exit and timeout all fail. It is not a game.
