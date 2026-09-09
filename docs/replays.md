# Replay validation integration

The `skirmish-replay` library is the seam between a future replay importer and the
headless game simulation. It has no replay-file or rendering dependency. The
importer will supply a stream of expected transitions; the simulator will supply
checkpoint restoration and one-frame advancement.

Each transition explicitly labels the frame **after** applying its input. A
checkpoint records the next expected frame, including negative frame numbers.
The validator requires consecutive frames and stops at the first source error,
simulation error or observation difference. It never treats an empty stream or a
matching prefix followed by an error as a successful replay. A completed run
establishes agreement for the observations supplied by that stream.

The initial checkpoint must contain complete simulation state. A future Slippi
importer must not turn a subset of observed fighter fields into an allegedly
complete checkpoint. Start from deterministic game initialization, or restore a
checkpoint produced by the validated simulator/reference. Replay available inputs
to reach a later frame. Missing asset data, hidden state, RNG call order and scene
initialization must remain explicit unsupported conditions until implemented.

The importer is responsible for normalizing rollback/repeated frames into its
chosen final timeline and documenting input-to-observation timing. Keep the raw
replay hash, format/parser version, match settings, asset hashes and initialization
procedure with each validation run. Import and validate one transition at a time
so large replay corpora do not require loading their contents into memory.

An observation adapter projects simulator state into the exact fields available
in the replay. Compare integer/enum values directly and float fields by raw bits
using the library's comparators. If a comparison deliberately tolerates numerical
differences or observes fewer fields, record that policy with the result; it does
not establish exact or complete state equivalence.

For a failure, retain the initial checkpoint or its reproducible construction,
inputs through the failing frame, expected and actual observations, and the
reported field difference. This becomes a small regression case. Later coaching
and RL rollouts can restore the same checkpoint for alternate input sequences
without making observation comparison depend on presentation.

No Slippi importer, hidden-state reconstruction, playable match simulation or
training loop is implemented yet. The current tests exercise this integration
contract using the translated headless primitives.
