# Native match slice

`skirmish-match` now runs an **experimental two-player match** from reset through
countdown, per-frame movement, a bone-animated jab, contact, damage, hitlag,
knockback, stock loss, respawn and termination. The CLI's demonstration uses an
explicitly synthetic dataset, not Fox or a certified Melee matchup. Neither
builds nor execution need an ISO, DOL, emulator, renderer or audio device.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --bin skirmish -- demo-match > /mnt/shared/tmp/skirmish-demo.jsonl
cargo run --locked --bin skirmish -- run-match --data /path/to/native-match.json --inputs /path/to/inputs.jsonl
```

`run-match` consumes one controller pair per JSONL line, from stdin when
`--inputs` is omitted. Each controller has
`buttons` (A=256, X=1024, Y=2048) and normalized `stick: [x, y]`. Unsupported
buttons and nonfinite/out-of-range sticks produce errors. Example:

```json
[{"buttons":256,"stick":[0.0,0.0]},{"buttons":0,"stick":[0.0,0.0]}]
```

A run can cover a bounded input scenario or an entire match. EOF ends the
scenario; inspect the final `phase` to distinguish these cases. Empty input,
malformed lines, extra input after match completion and physics failures never
produce a successful trace `end`. Outputs use the existing strict trace format,
including float bit strings, a native-resource SHA-256 identity and explicit
experimental profile. They can be passed to `compare-traces` or emitted by
`compare-binaries` adapters. Whole-match agreement with original C is **not**
established; two copies of this simulator agreeing is a determinism test.

## State and branching

`Match::step([Controller; 2])` advances one frame. `state()` exposes privileged
simulator state; it is not a player-information observation policy. `reset(seed)`
restarts the match. `checkpoint()` and `restore_checkpoint()` preserve inputs,
positions, velocities, action clocks, hitlag/hitstun, contact history, stocks,
invincibility, match clock, reserved RNG seed and events. This slice has no
random events and consumes no RNG draws. Checkpoints are opaque in-memory
values, and restoration rejects different resource/rule identities. Persistent
checkpoint encoding is a later versioned interface.

Clones share only immutable native resources. Each branch owns its mutable state;
stepping errors leave it unchanged. The crate implements `skirmish-replay`'s
`FrameStepper`, so streaming expected observations and counterfactual branches
already use this match implementation. Replay observations still do not supply
all hidden state. Slippi parsing, player observations, batched Python/Gym
interfaces, rewards and coaching value estimation remain separate work.

## Physics and execution order

Bone hierarchy evaluation, local/world transforms, parent-scale compensation,
bone-attached capsules, capsule/sphere intersection, walking and jump launch
arithmetic live in `melee-physics`. The native match schema carries all pose
samples for its jab, including startup/recovery frames. The same evaluated pose
places both hurtboxes and hitboxes; rendering is not involved. Local native
coordinates use +X forward, +Y up and +Z depth; resource imports must convert
source coordinate conventions explicitly.

The experimental scheduler first handles countdown or respawn/freeze counters,
then updates actions and physics for both players, resolves its flat floor, and
evaluates poses. It collects contact decisions before applying damage so both
players can trade on the same frame. Hit-group history suppresses repeated
contacts during one attack. It then resolves stock losses together, checks the
clock and emits events. A timeout compares stocks, then percent; exact ties
finish as a draw. This schedule and these match policies are integration code,
not a claimed translation of the complete original callback graph.

## Explicit coverage limits

The current match profile supports walking directly in either direction, button
jumps/full and short hops, airborne drift/fast fall, a single jab, unarmored
damage and ordinary fixed-angle launch. Its inputs do not yet reproduce the
full PAD-to-fighter input history: tap-jump, tilt windows, crouch, dash/turn,
shield, grabs, specials, aerial attacks, double jumps and ledge actions are
unported. Some accepted stick/button combinations consequently have no action
in this experimental profile.

Its stage is one horizontal floor with a point-foot sweep. ECB geometry, slopes,
walls, ceilings, platforms, ledges, moving stages, collision substeps, fighter
push/nudge and moving hitbox sweeps remain unported. Crossing any blast boundary
kills here, including the top; this does not implement Melee's conditional top
KOs or star/screen deaths. Respawn delay/invincibility are configured integration
policies, without the original rebirth platform. Damage landing does not yet
implement techs, bounces or knockdown.

Combat omits stale moves, priority/clanks, armor, vulnerability/target flags,
shield responses, throws, DI/SDI/ASDI and special launch angles. Outside jab,
fighters currently use a static supplied pose; authentic walking, jumping and
damage collision require those animation resources. The schema exposes ordinary
Euler scale inheritance but not all HSD joint flags, IK or animation scripting.
Character-specific radius/model scaling is not implemented. Tests exercise the
explicit slice; they do not certify those missing rules.

The [native-data audit](native-data.md) preserves real Fox numeric attributes,
raw jab commands, captured jab observations and Final Destination bounds with
source hashes. Those files are **incomplete resources** and are intentionally
not loaded as a playable Fox profile. Missing skeletons, genuine hurtboxes, ECB,
animation tracks and stage topology must be filled before a real matchup can
be validated. Conflicting move-data extraction conventions must be resolved
against raw commands and source arithmetic, not blended into defaults.
