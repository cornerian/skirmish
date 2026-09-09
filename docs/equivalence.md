# Behavioral equivalence

The release condition is identical observable behavior for identical initial
state, assets, inputs and timing. Matching PowerPC assembly is not required.
No finite test suite proves equivalence for all possible game executions.

## What runs now

`cargo test --features c-oracle` compiles small, verbatim upstream C snapshots
with documented host ABI adaptations and tests translated functions against
them. Integer outputs and eligible floating-point results are compared by bits.
The normal Rust tests cover malformed input and new safe API contracts too.
Source snapshots are identified by SHA-256 and a pinned upstream Git revision.

These tests compare against host-compiled C, not the GameCube executable. Gekko
paired-single operations, fused arithmetic, the original math library, floating
point status, hardware timing and memory-mapped peripherals require a PowerPC
reference run. A host `libm` result must not be treated as proof of target parity.

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

## Full-game gate, pending

1. Supply a local GALE01 1.02 `main.dol` and game assets; validate the DOL using
   `skirmish verify-dol`. Expected SHA-1:
   `08e0bf20134dfcb260699671004527b2d6bb1a45`.
2. Build the pinned upstream checkout following its own instructions. Keep the
   original executable, built reference and assets outside Git, with hashes.
3. Implement a deterministic Dolphin/hardware adapter and a completed Rust game
   adapter. Neither full-game adapter exists yet. Both must expose the same
   logical state and all externally visible events at fixed simulation steps.
4. Capture fighter/item/stage state, RNG state and call order, hitboxes and
   hurtboxes, collisions, action transitions, camera/animation, sound commands,
   saves and scene/menu changes. Validate video/audio separately with specified
   capture settings; snapshots alone do not cover these outputs.
5. Run a recorded corpus and generated inputs covering every character, action,
   stage, mode, item, event ordering and boundary condition. Store minimized
   divergent inputs as regression fixtures with provenance.

Slippi replay state can supply additional regression cases, but does not expose
every internal state or all game output and cannot alone certify the full port.
