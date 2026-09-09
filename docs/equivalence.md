# Behavioral equivalence

The release condition is identical observable behavior for identical initial
state, assets, inputs and timing. Matching PowerPC assembly is not required.
No finite test suite proves equivalence for all possible game executions.

## What runs now

`cargo test --features c-oracle` compiles small, verbatim upstream C snapshots
with documented host ABI adaptations and tests translated functions against
them. Integer outputs and eligible floating-point results are compared by bits.
Bytecode transcendental comparisons use 2e-6 absolute/relative tolerance;
quaternion, bone Euler, damage and rotated ECB transcendental comparisons use 4e-6. These are host-library checks,
not exact equivalence claims. Special NaN results in numerical kernels compare
NaN classification; storage and trace comparisons preserve payload bits.
The normal Rust tests cover malformed input and new safe API contracts too.
Source snapshots are identified by SHA-256 and a pinned upstream Git revision.

The headless match fixture separately tests full lifecycle events, deterministic
checkpoints, counterfactual branches and strict trace-adapter behavior. CI compares
debug and optimized native match executable traces. This checks the experimental
slice's consistency across compiler optimization; it does not establish agreement
with original Melee. Original-C differential coverage includes the selected bone,
contact, knockback, DI, hitlag, walking, jump, neutral-special input, blast-death selection and RNG, rebirth travel, stage query/remapping and ECB
arithmetic used by the slice. Generated cases also exercise directed crossings,
adjacency/tie order, collision-box state mutations and subdivision thresholds.
Match integration tests cover slopes, walls, ceilings, static and moving platforms, swept attack
history and bone-sampled environmental geometry using synthetic resources.
The file-backed replay harness also advances the actual native match and reports
the first differing selected Slippi post-field. Its synthetic recordings verify
the harness, including negative cases; they are not independent gameplay oracles.
See [the integration/conformance coverage map](testing.md) for passing,
unimplemented and reference-blocked tests. Function-parity unit tests remain at
retained C/Rust boundaries; refactored subsystems use observable integration
contracts.

These tests run natively and require no game image or original executable. Gekko
paired-single operations, fused arithmetic, the original math library, floating
point status and hardware timing are migration concerns: preserve their gameplay
effects through native code, explicit compatibility functions and observable
regressions. Host `libm` agreement alone is not proof of full-game parity.

## Executable comparison protocol

`skirmish compare-binaries --reference reference.json --candidate candidate.json
--input scenario.bin --output /mnt/archive/runs/skirmish-scenario --timeout-seconds 60`
runs both adapters with the same bytes on stdin. Each adapter specification is:

```json
{"program":"/absolute/path/to/adapter", "args":[], "env":{}}
```

Adapters emit one JSON object per line on stdout and diagnostics on stderr.
The comparison checks semantic fields, not raw text order, executable hashes or
assembly. Executable/input hashes and the adapter invocation are recorded as
provenance, and traces and stderr are retained in a new output directory.
A timeout or nonzero process exit fails; adapters must wait for their own child
processes and terminate them when exiting. The current runner bounds the direct
adapter process, not arbitrary detached descendants.

```jsonl
{"kind":"header","schema":1,"upstream_commit":"0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9","scenario":"example","initial_state":{"seed":1}}
{"kind":"frame","frame":0,"inputs":{"port0.buttons":256},"state":{"fighter0.x":"f32:80000000","fighter0.damage":"f32:00000000","rng":2745024},"events":[]}
{"kind":"end","frames":1}
```

Frames start at zero and are contiguous. An explicit end record must give the
exact nonzero frame count. Unknown fields, missing state, truncated files and
extra records fail even when both sides make the same mistake. Float fields
must use bit strings (`f32:XXXXXXXX` / `f64:XXXXXXXXXXXXXXXX`); preserve signed
zero and NaN payloads. Normalize pointers to stable object IDs. Do not omit a
divergent field to make a comparison pass. Use `compare-traces` for existing files.
Decimal float observations are rejected recursively; integer JSON observations
retain their full signed/unsigned 64-bit range. Failed runs retain their invocation
and input/executable hashes even when no successful comparison report is produced.

## Full-game gate, pending

1. Continue extracting original C function groups into native test adapters with
   explicit data and dependencies. Generate valid states and operation sequences;
   compare every observable mutation after every call. Invalid C/ABI domains must
   be excluded or tested only against the new checked Rust API.
2. Complete the native game simulation and native gameplay resources, recording
   their provenance and hashes. No ISO, DOL, emulator or GameCube runtime may be
   required for building, running or testing the project.
3. Expand the existing Peppi-to-native-match checkpoint harness beyond its
   experimental two-player input policy and six selected post fields. Apply
   each frame's inputs and compare the simulated next observation with the
   recorded post-frame. Initial state reconstruction and observation coverage
   must be explicit.
4. Capture fighter/item/stage state, RNG state and call order, hitboxes and
   hurtboxes, collisions, action transitions, camera/animation, sound commands,
   saves and scene/menu changes. Validate video/audio separately with specified
   capture settings; snapshots alone do not cover these outputs.
5. Run a recorded corpus and generated inputs covering every character, action,
   stage, mode, item, event ordering and boundary condition. Store minimized
   divergent inputs as regression fixtures with provenance.

Slippi replay state can supply additional regression cases, but does not expose
every internal state or all game output and cannot alone certify the full port.
