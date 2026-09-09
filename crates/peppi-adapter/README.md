# peppi-adapter

Native Slippi import using pinned **Peppi 2.1.2**. This independently buildable
crate reuses Peppi's Arrow storage, parser and field types and emits transitions
for `replay`. It requires no ISO, DOL, emulator or GameCube runtime.

```rust,no_run
use peppi_adapter::{Replay, Timeline};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let replay = Replay::read(std::fs::File::open("match.slp")?)?;
    let summary = replay.summary(Timeline::LastRecorded)?;
    println!("{} frames", summary.selected_frames);
    for transition in replay.transitions(Timeline::LastRecorded)? {
        let transition = transition?;
        assert_eq!(transition.frame, transition.expected.id);
    }
    Ok(())
}
```

Completed Slippi 2.0.0–3.18.0 files up to 512 MiB are supported; malformed,
truncated, live and future-version files are rejected. `Replay::game()` exposes
the original immutable Peppi game. The `peppi`, `row`, `Port` and `Version`
reexports let consumers use the same types without duplicating their schemas.
Legacy 2.0/2.1 has no FrameStart event and permits only a contiguous timeline;
intermittently absent actors are rejected to avoid Peppi's legacy row shifting.
Fields unavailable in an older format remain absent.

`LastRecorded` truncates the old tail when rollback replaces a frame.
`FinalizedOnly` selects an explicit bookend-finalized prefix and requires 3.7+;
`GameEnd` does not promote its unfinalized tail. Original frame IDs, physical
ports, followers, missing actors and optional versioned fields are preserved.
Pre/post observations sharing an ID form one input-to-post-state transition.
Input adapters choose between recorded raw and processed controller fields;
observed pre-state is not an instruction to overwrite simulator state.

Peppi loads each full replay into Arrow columns. Iterating transitions avoids a
second row table, but does not make parsing constant-memory. Import corpus files
one at a time; parsed arrays consume memory in addition to the bounded input.

This crate does not reconstruct hidden state or synthesize a checkpoint. The root
application's `validate-replay` harness connects its transitions to actual native
`Match::step` calls, using a separate complete initialization or in-memory checkpoint
and a fixed selected-field comparison policy. The native adapter supports two
human leaders and processed sticks with A/X/Y; it rejects followers and unsupported
controls. Complete Fox behavior remains unimplemented. Neither import success nor
selected-field agreement establishes full gameplay equivalence.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked -p peppi-adapter
```

From the repository root, `cargo run --locked --bin skirmish -- inspect-replay match.slp`
prints provenance and timeline counts; add `--finalized-only` for the explicit
finalized prefix. `cargo test --locked --test slippi_cli` exercises the real
executable using synthetic files written by Peppi.

`cargo run --locked --bin skirmish -- validate-replay match.slp --initialization init.json`
runs the file-backed simulator comparison. The initialization embeds complete
native match data, seed, port mapping, next replay frame and explicit warmup inputs.
It is not constructed from observed replay state.

See [the replay contract](../../docs/replays.md) for initialization requirements,
validation limits and how these observations connect to native physics.
