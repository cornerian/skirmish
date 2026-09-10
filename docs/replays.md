# Slippi replay import and validation

`peppi-adapter` imports completed `.slp` files through **Peppi 2.1.2**. It keeps
Peppi's match settings, metadata, Gecko bytes and Arrow frame columns, then builds
an index selecting the surviving timeline. The parser, native physics libraries
and experimental headless match run on modern machines without an ISO, DOL,
emulator or GameCube runtime.

Inspect a replay from the repository root:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --bin skirmish -- inspect-replay /path/to/replay.slp
cargo run --locked --bin skirmish -- inspect-replay /path/to/replay.slp --finalized-only
```

The JSON summary records the exact file's SHA-256 and byte count, parser/format
versions, stage, physical player ports, recorded/surviving/discarded frame counts,
selected frame range and finalization watermark. Ports retain their original
identities: a game on P1/P3 has zero-based ports 0/2, not an invented contiguous
player mapping. Followers remain distinct actors on their leader's port.
Import errors exit unsuccessfully without a success summary.

## Accepted files and memory use

The importer accepts completed Slippi **2.0.0 through 3.18.0** files up to
**512 MiB**, with a nonzero declared raw-event length and a complete `GameEnd`.
It checks the wrapper, event boundaries, actor routing, paired pre/post events,
frame continuity and version-dependent columns. Future formats, live files,
truncated events and unsupported event forms are errors. Recorded absent actors
remain absent; nullable columns do not become zero-valued fighters.

Legacy 2.0/2.1 files have no FrameStart records: a new pre-frame ID opens each
frame, and IDs must advance contiguously from -123 without rollback. Intermittent
actor absence in these legacy files is rejected because Peppi would shift later
rows; trailing absence is supported. FrameEnd and item records are absent before
3.0, and post-frame hurtbox state is absent before 2.1. Version-dependent fields
remain optional rather than being synthesized.

Peppi parses a whole file into Arrow columns. The input bytes and parsed arrays
can coexist during import, so the byte limit is not a total-memory limit.
Transitions are materialized one selected frame at a time from those columns;
this is not a constant-memory streaming file parser. Process a large corpus one
file at a time and release each `Replay` before loading the next.

## Timeline and input timing

`Timeline::LastRecorded` selects the last surviving recording of each frame.
A rollback to an earlier frame removes the old tail before replacement frames
are appended. Original recorded rows and indices remain accessible through
`Replay::game()` and `frame_indices()`. Negative frame IDs are preserved.

`Timeline::FinalizedOnly` requires Slippi 3.7 or later and selects only the prefix
covered by explicit frame-bookend watermarks. `GameEnd` does not finalize the
remaining tail. A completed replay can therefore have an empty finalized prefix;
inspection reports zero selected frames, while replay validation rejects an empty
transition stream. Rewriting already-finalized history is an import error.

Pre/post events with the **same frame ID** produce one
`replay_validation::Transition<Inputs, Frame>`: apply that frame's inputs, then
compare its post-frame observation. Actors are identified by `(port, follower)`.
Peppi's field types and float values are retained, including available raw analog
samples, processed sticks/triggers, logical buttons and physical buttons.
Version-dependent fields remain optional. A simulation input adapter must choose
and document which representation it consumes; import does not silently recalibrate
already processed sticks or replace missing raw samples with zero.

`Inputs` carries the original pre-frame records for that choice. Those records
also contain observed state and RNG fields: an adapter must select controller
inputs explicitly, rather than overwrite simulated position, action or RNG state
with the reference values. Frame start/end data, items and stage-event observations
remain available when recorded.

## Compare a file against Skirmish

```xonsh
cargo run --locked --bin skirmish -- validate-replay /path/to/game.slp --initialization /path/to/init.json
```

Add `--finalized-only` to use the explicit finalized prefix. The harness restores
the supplied complete native checkpoint, converts selected replay inputs, calls
the real `Match::step`, and compares the resulting observations. It starts at the
declared `next_frame` and continues through the selected timeline's last frame.
An absent start frame, empty suffix, unsupported input, simulation error or
observation difference fails; a matching prefix is not silently accepted.

The required initialization JSON has these fields, with no defaults:

| Field | Meaning |
| --- | --- |
| `data` | Embedded complete native `MatchData`, in the [match resource format](match.md); not a filename or replay-derived fighter snapshot |
| `seed` | Explicit unsigned 32-bit seed passed to `Match::new` |
| `ports` | Two distinct Peppi port names, such as `["P1", "P3"]`, mapping native fighters 0/1 to physical replay ports |
| `next_frame` | Signed Slippi frame ID whose input is applied by the next native step |
| `warmup` | Array of native controller pairs applied after initialization and before checkpointing; `[]` means no warmup |

For example, construct the file from an existing native resource bundle in xonsh:

```xonsh
import json
from pathlib import Path
initialization = {
    'data': json.loads(Path('/path/to/native-match.json').read_text()),
    'seed': 0,
    'ports': ['P1', 'P3'],
    'next_frame': -123,
    'warmup': [],
}
Path('/mnt/shared/tmp/skirmish-replay-init.json').write_text(json.dumps(initialization))
```

These seed/port/frame choices are explicit example inputs, not values recovered
from the replay. Resources and warmup must actually establish the native state
corresponding to the declared frame. Warmup uses `run-match`'s controller-pair
format; it does not infer earlier actions or hidden state.

Library callers can pass an existing complete in-memory checkpoint to
`skirmish_replay::match_validation::validate`, together with the `Match`, replay, port mapping
and timeline policy. Its `next_frame` labels the first post-step observation to
compare. Persistent checkpoint encoding is not provided.

## Input and observation policy

Native comparison requires exactly two human-controlled leaders in a non-team
match. Peppi preserves
Nana/follower data, but this adapter rejects followers. Processed joystick X/Y
supplies normalized main-stick input; processed C-stick and analog trigger
values pass through without recalibration. Physical button bits supply
A/B/X/Y/Z/L/R.
Derived main-stick, C-stick and logical-trigger flags are accepted; a logical
trigger flag without analog pressure or digital L/R is rejected. Other buttons
remain unsupported. Raw stick and physical analog samples remain preserved by
the importer, but do not replace processed input. This channel support enables
C-stick ASDI and, with explicit aerial resources, aerial selection. Pre-frame
position, action and RNG never overwrite the simulation.

The named **`fighter-post-v3`** policy compares these post-frame fields for each
mapped actor:

| Replay field | Native observation | Comparison |
| --- | --- | --- |
| `state` | Common action-state mapping | Exact `u16` |
| `state_age` | Action frame | Exact `f32` bits |
| `position.x`, `position.y` | Fighter position | Exact `f32` bits |
| `direction` | Fighter facing | Exact `f32` bits |
| `percent` | Damage percent | Exact `f32` bits |
| `shield` | Shield health | Exact `f32` bits |
| `stocks` | Remaining stocks | Exact integer |
| `airborne` | Inverse of native grounded state | Exact boolean |
| `jumps` | Configured maximum minus jumps used | Exact integer |
| `ground` | Last contacted collision-line ID, retaining `0xffff` before first contact | Exact `u16` |
| `l_cancel` | Per-frame aerial-landing result | Exact integer |
| `hurtbox_state` (Slippi 2.1+) | Timed vulnerable, invulnerable or intangible state | Exact integer |
| `velocities.*` (Slippi 3.5+) | Air X/Y, knockback X/Y and ground X velocity | Exact `f32` bits |
| `hitlag` (Slippi 3.8+) | Remaining hitlag | Exact `f32` bits |

The report's `fields` array follows the replay version, so fields absent from an
older Slippi schema are visible rather than silently claimed. The refactored
action enum collapses some original motion states. Those actions map to one
documented common-state ID. Fox's current neutral-special shell maps to its
ground and air startup families using GameStart character metadata; other
character-specific specials and internal respawn or elimination phases remain
unmapped and therefore produce an action-state mismatch. RNG, ground-line
identity, state flags, items and stage state are not compared.
Nonempty item and dynamic stage-event records are
rejected as unsupported simulation. Equality of the selected fields does not
establish equality of hidden state, complete frame behavior or Melee gameplay.
The match remains an experimental ruleset with incomplete character resources;
the current Fox subset cannot validate arbitrary real Fox matches.

## Reports and regression cases

Reports identify the observation policy and fields, input policy, replay summary
and hash, `resources_sha256`, explicit `ports` and `checkpoint_next_frame`. The CLI
adds `initialization_sha256`, computed over the exact initialization file bytes.
Its `outcome.status` is:

- `matched`: includes `first_frame`, `last_frame` and `checked_frames`; every frame
  from the declared start through the selected timeline end matched.
- `mismatch`: includes `frame`, matched-prefix `checked_frames`, and `difference`
  containing `port`, `field`, `expected` and `actual` as hexadecimal bit strings.
- `error`: includes a frame when available, matched-prefix count and the
  unsupported condition or failed step's explanation.

Mismatch and error reports exit unsuccessfully. File/JSON parsing, initialization
and preflight checks can fail before a report exists. Keep the report, replay,
initialization and inputs
through the failing frame as a regression case. Complete checkpoints also support
native counterfactual branches without depending on rendering.

The file-backed harness tests write synthetic `.slp` files, run the actual native
simulator and verify successful comparisons and deliberate corruptions. They test
the comparison path and failure reporting, not fidelity to recorded Melee gameplay.
