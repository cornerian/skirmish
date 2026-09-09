# Headless simulation first

The intended applications are reinforcement learning and player coaching: many
independent games, decisions at every frame, and reproducible counterfactual
rollouts from a recorded state. The implementation therefore separates reusable
numerics, gameplay state, environment adapters and presentation.

Builds, execution and tests target modern machines directly. An ISO, DOL,
emulator or GameCube runtime must never be a dependency. Native resources will
carry the gameplay data; unresolved data and rules must not be silently filled
with approximate defaults to make a match appear playable.

`crates/melee-runtime` is an independent library project for the translated HAL
and Metrowerks algorithms. `crates/melee-physics` contains 25 isolated fighter
movement routines. Cargo workspace packages keep builds and
tests reproducible while giving each library a separate dependency boundary.
They can later move to separate Git repositories without changing their APIs.

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
reference. No RL environment, replay importer, full match simulation, or coaching
model is implemented yet.
