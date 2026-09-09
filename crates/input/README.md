# input

An independent, allocation-free `no_std` translation of Dolphin's entire
`extern/dolphin/src/dolphin/pad/PadClamp.c`, pinned by `../../upstream.lock.json`.
It conditions raw controller bytes for headless consumers without a renderer,
controller device, global calibration state, or invented analog normalization.

`clamp` implements `PADClamp` for exactly four controller statuses using the
original default region. Controllers with an error retain every field. The
buttons and analog A/B bytes are unchanged. `ClampRegion::apply`,
`StickRegion::clamp`, and `ClampRegion::clamp_trigger` expose the same operations
with caller-owned calibration. Integer division, signed-byte casts, and trigger
wraparound match C. Custom regions that cause C integer division by zero return
an error without changing the caller's controller batch.

This is input conditioning, not a controller poller, input history, game action
model, or complete simulation. Passing already conditioned input through it
again applies the deadzone again, exactly as the original does.

From this directory in xonsh:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo build --locked
cargo test --locked
```

From the repository root, run exhaustive comparisons of all signed-byte stick
pairs and trigger bytes, randomized calibration, and four-controller batches:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo test --locked --features c-oracle --test input_differential
```

Use `/tmp/skirmish-target` if shared scratch is not writable. The C fixture is an
exact snapshot with only platform includes replaced by host scalar declarations.
Passing these tests establishes host-C agreement for this module, not whole-game
or hardware polling equivalence.
