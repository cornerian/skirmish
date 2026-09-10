# Headless simulation first

The intended applications are reinforcement learning and player coaching: many
independent games, decisions at every frame, and reproducible counterfactual
rollouts from a recorded state. The implementation therefore separates reusable
numerics, gameplay state, environment adapters and presentation.

Builds, execution and tests target modern machines directly. An ISO, DOL,
emulator or GameCube runtime must never be a dependency. Native resources will
carry the gameplay data; unresolved data and rules must not be silently filled
with approximate defaults to make a match appear playable.

Resource extraction and visual conversion are owned by the separate
`skirmish-assets` task. Skirmish consumes native exports while continuing gameplay
work; it does not own that project's directory layout. The
[resource integration contract](resources.md) separates exact gameplay data
from vector artwork and procedural presentation effects.

The main crate groups native code by responsibility. Focused top-level modules
hold translated HSD and Metrowerks algorithms. `fighter` contains scalar
movement, walking/jump launch, and damage arithmetic; `collision` contains bone
hierarchy transforms, environmental collision boxes, sampled stage queries,
moving-line remapping and swept contacts. Bone poses and bone-attached
hitboxes/hurtboxes remain available headless. `game` composes the experimental
match slice and owns gameplay state and frame scheduling. Renderer-independent
presentation timing and mutable scene-instance state live under
`src/presentation`; feature-gated GPU, window, input-host, and audio adapters
live under `src/renderer`. Separate crates remain where a real dependency
boundary exists, such as replay parsing.

`crates/peppi-adapter` uses Peppi for parsing and columnar replay storage,
reusing its port, version, pre/post-frame and vector types. This keeps replay
formats and Arrow dependencies outside the headless game and the generic
`replay-validation` engine. Native vectors retain their existing arithmetic; replay
vectors carry recorded observations.
`crates/skirmish-replay` connects these boundaries: it
restores an explicit native checkpoint, maps supported recorded inputs, calls
`Match::step` and compares a named subset of post-frame fields. The `validate-replay`
CLI reads a real `.slp` file plus a separately supplied deterministic initialization
and reports provenance and the first difference. `crates/equivalence` owns
semantic traces, process comparison, native match trace adapters and differential
probes. `crates/cli` composes these tools while Peppi/Arrow and test tooling stay
outside the native match and physics dependencies.

The game simulation must own the complete mutable state of one match, including
RNG state, action timers, object identities and event queues. It must advance one
simulation frame from explicit controller inputs without a wall clock, renderer,
audio device or global mutable singleton. Checkpoint/restore must preserve that
entire state so rollouts from a saved frame consume the same RNG sequence and
produce the same state and events. Parallel environments must share only immutable
assets and must not change behavior when their scheduling changes.

Physics computes movement and collision results; gameplay decides action state
transitions, hitlag, knockback and rules. Avoid substituting a general physics
engine until its exact behavior is shown to agree. Rendering and audio consume
read-only state/events. They may be disabled without changing simulation state
or RNG consumption. The CLI and C oracles are tooling, outside these libraries.

The eventual RL interface should expose reset, one-frame step, observation,
termination, checkpoint and restore. Keep reward definitions in the training
adapter so shaping choices do not alter game rules. A frame's best move depends
on the objective, policy and opponent model; training estimates that value.
Observations must distinguish information available to a player from privileged
training state. Controller actions need the game's analog and digital input
semantics, not only named moves.

Coaching should branch counterfactual rollouts from the same restored state and
rank the loss in estimated outcome value between the recorded action and its
alternatives. Report uncertainty and the time horizon with that estimate. A
replay snapshot alone is not a complete restorable game state; importing it
requires reconstructing or replaying hidden state and validating against the
reference. An experimental two-player match now implements reset, frame steps,
termination and checkpoint branching; `skirmish-replay` adapts it to the generic
validation interface. It
uses synthetic native resources and a limited ruleset; see
[the match contract](match.md). The Peppi importer supplies recorded transitions,
and [file-backed comparison](replays.md) now drives the real simulator from
caller-supplied initialization or an existing complete checkpoint. Automatic
hidden-state reconstruction, a faithful full Melee match, the training framework
adapter and coaching model remain unimplemented. A passing selected-field report
is not a claim of whole-state or Melee gameplay equivalence.
