# replay

Streaming machinery for validating a replay adapter against a headless
simulation. It contains no Slippi parser, no complete Melee frame scheduler,
and no reconstruction of hidden state from observed replay fields.
The separate [peppi-adapter](../peppi-adapter) crate supplies Peppi-backed
recorded transitions, leaving this library independent of replay file formats.

The caller supplies a **complete native simulator checkpoint** separately, along
with native input and observation data. No external game runtime or executable
is required to build, run, or test this crate. `Checkpoint::next_frame`
labels the observation produced by the first subsequent `advance(input)` call.
Each `Transition` supplies that frame's input and expected **post-step** observation.
Indices are signed; negative opening frames need no special conversion.

Implement `FrameStepper` for the simulation. Its checkpoint must restore every
state value that can affect later behavior, including RNGs, object identities,
timers and other hidden state. Select finalized replay transitions before calling
the validator; duplicate rollback records are rejected as noncontiguous input.

`validate` consumes an iterator without collecting it. `validate_fallible` also
propagates parser/read errors, so a failed read cannot turn a matching prefix into
a passing run. The first transition must match `next_frame`, and subsequent frames
must increment by one without overflow. Empty, gapped, duplicate and out-of-order
streams fail. The caller must independently establish that the supplied stream is
complete; a plain iterator cannot distinguish intended completion from truncation.

Comparators receive `(expected, actual)` and return an optional custom difference.
The first mismatch includes its signed frame, matched-prefix count and that
difference. `compare_f32_bits` and `compare_f64_bits` preserve signed zero and NaN
payloads. Compose them field by field for structured observations and include the
field name in your difference type. Success covers only the observations and
transitions supplied, and does not prove whole-game equivalence.

On an advance error or mismatch, the stepper remains at the state reached by that
attempt, which may be partially updated on error. Use `branch_from` to clone and
restore a checkpoint for a counterfactual branch while retaining the original.
Its `Clone` implementation must create independent mutable simulation state.

The tests execute explicitly scripted sequences of existing `physics`
helpers and HSD RNG draws. These sequences exercise validation/restoration;
they are not an invented Melee frame loop or a claim of replay compatibility.

From the repository root, with the project build directory configured:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked -p replay
cargo clippy --locked -p replay --all-targets -- -D warnings
```
