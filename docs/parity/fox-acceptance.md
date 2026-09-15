# Fox observable-behavior acceptance

This is the completion gate for Fox. “Fox parity” means the native simulator
can start from an independently produced resource export, consume arbitrary
valid controller input, and reproduce the original game's observable state,
events, contacts, projectiles, and terminal results across the supported
timeline. A passing unit test, a self-recorded replay, or a special-only trace
does not satisfy this gate.

## Coverage inventory

The acceptance matrix includes every Fox action family and its transitions:

| Area | Required behavior |
| --- | --- |
| Common locomotion | Wait/idle, walk and walk IASA, dash, run, run brake, turn, turn-run, squat/squat-wait/reverse, platform drop, jump variants, fall, fast fall, landing/autocancel, wall jump, passive collisions, blast/death and rebirth |
| Ground attacks | Jab 1/2/3 and rapid-jab loop, dash attack, forward/up/down tilt including stick variants and buffering, forward/up/down smash including charge/release |
| Aerial attacks | Neutral, forward, back, up and down aerials; startup, active/late/clear windows, landing lag, autocancel, drift and interruption |
| Grab and throws | Standing/dash grab, catch pull/wait/attack, pummel, forward/back/up/down throw, mash/escape, victim and attacker transitions |
| Defense | Shield startup/hold/drop, powershield/parry, shield stun and damage, roll, spot dodge, air dodge, intangibility, ledge snap and ledge options |
| Damage and physics | Hit direction, knockback, damage motion, hitlag and displacement, clanks, rebound, SDI/ASDI, ECB/bone contacts, stage surfaces, moving platforms, walls/ceilings, blast zones and staling |
| Fox specials | Blaster ground/air Start, Loop and End; Illusion ground/air Start, Dash and End; Fire Fox Charge, Hold, Travel, Fall/End, Bound and landing/fall paths; Reflector Start, Hold/loop, End and release/turn/jump/landing paths |
| Projectiles | Fox laser spawn timing, position, angle, speed, lifetime, hitbox/damage/knockback, terrain despawn, hurtbox/shield contact, Reflector bounce and owner transfer, staling identity and observation |
| Resources and poses | Complete attributes, action command streams, animation/marker timing, ground/air variants, bone hierarchy and transforms, hurtboxes/ECBs, hitboxes, stage topology, ledges and projectile descriptors with source revision, hashes, precision and explicit coverage |
| Runtime concerns | Typed Pon moves/hooks/async execution, cancellation, rollback, multiple modules, deterministic resource loading, no frame polling, f32 semantics and native performance (tracked by `docs/pon-integration-plan.md`) |

Each row requires input-to-observation integration traces, focused source
differentials where a function is deliberately transcribed, and at least one
independent whole-game trace. “Not reached” and “resource unavailable” are
coverage failures, not passes.

## Evidence hierarchy

Authoritative independent inputs are, in order of scope: the pinned decomp at
revision `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9` (`upstream.lock.json`),
retail/disassembly evidence where source leaves ABI or instruction behavior
uncertain, independently captured native data and replay observations (the
libmelee capture and source manifests), and real human-played Slippi files
from the CC0 `erickfm/slippi-public-dataset-v3.7` corpus. Host-compiled C
oracles prove a function's arithmetic only. Self-recorded replay tests prove
the validator and checkpoint harness only. Neither establishes whole-game
parity.

The real-replay ratchet must run every listed recording in
`tests/fixtures/slippi/parity/recordings.json`, across Final Destination and
the tournament stages represented by the fixture set, with independent
baselines and first-divergence reports. Add traces that deliberately exercise
each Fox move family and both fighter ports. Baselines may ratchet only after
the divergence is diagnosed and the source, resource, or platform limitation
is recorded.

## Failure classification and completion rule

Every first divergence is assigned exactly one category: missing or incorrect
resource data; action entry/exit or event-order timing; controller conversion;
collision/pose/topology; combat/damage/hitlag; projectile/reflector; arithmetic
or f32/PowerPC platform behavior; runtime module/rollback behavior; or an
unimplemented source path. The report records recording hash, pack hash,
frame, fighter/port, field, expected and actual bits, reproduction command,
and independent evidence. “Blocked on pack data” is a blocker to that fixture,
not evidence that Fox is complete.

Completion requires all inventory rows implemented and resource-complete,
all accepted real-replay prefixes to run to their recording endpoints (or a
documented platform-equivalence rule that is independently justified), no
unclassified first divergence, deterministic rollback and multi-module tests,
and a published native performance baseline. Any intentionally unsupported
behavior remains an explicit failed coverage item and prevents the Fox parity
claim.

## Verified local resource baseline

The following is the state actually present on this machine (inspect with
`find`, `jq`, `tar -tzf`, and `sha256sum`; no ISO or emulator is needed by the
consumer tests):

| Resource | Verified contents | Completeness status |
| --- | --- | --- |
| `/mnt/archive/runs/melee-assets-20260909/disc/files/` | Verified USA GALE01 v1.02 extraction. `dolphin-extraction-manifest.json` records 1,209 content files, byte-for-byte ISO agreement, and the source files `PlCo.dat` (149,101 B), `PlFx.dat` (259,850 B), `PlFxAJ.dat` (1,525,984 B), `PlFxNr.dat` (362,978 B), and `GrNLa.dat` (611,125 B). | Authoritative original inputs; extraction provenance is complete. |
| `/mnt/archive/runs/skirmish-fox-fd-export-20260911/` (export v1) | Decomp `0bac93a5…`, exporter `fe7dca1…`; `fighters/fox.json` has 73 bones, 13 hurtboxes, movement attributes and a 19-frame real Attack11/jab pose/script; Final Destination has collision geometry, blast bounds and spawns. | Minimal only. Rules are partial and optional profiles (including specials, grabs, defense, ledges, damage and locomotion extensions) are explicitly unsupported. It cannot establish complete Fox parity. |
| `/mnt/archive/datasets/melee/skirmish-gameplay/v10-snapshot-20260913/` and packs `skirmish-gameplay-v10.tar.gz` through `v14` | Current repository lock is v10 (`tests/fixtures/slippi/parity/gameplay-export.lock.json`, 35,945,854 B, SHA-256 `e4189b…`). The v10 package contains per-pairing `match-data.bin` and manifests for Fox/Falco pairings plus six stage JSONs. v11–v14 also exist in the archive, with their own manifests and hashes. | Consumer gameplay packs exist and include expanded profiles, but are generated artifacts and must be validated through each manifest. They do not replace independent disc/source evidence. |
| `tests/fixtures/native-data/` | Small attributed fixtures: Fox native attribute subset with exact binary32 values and raw Attack11 events, libmelee Fox jab capture (frames 1–17), and Final Destination bounds. | Independent cross-checks only; no complete skeleton/action/resource set. |
| `tests/fixtures/slippi/parity/` | `recordings.json` names nine hashed real recordings: four Fox-vs-Fox Final Destination files, Battlefield, Yoshi's Story, Fountain of Dreams, Dream Land and Pokémon Stadium. | Independent human-played input/observation targets; current baselines stop at diagnosed first divergences and are not completion evidence. |

The smallest useful independent input-to-observation fixture is
`tests/fixtures/slippi/parity/fox-fd.slp` (1,084,706 bytes, Slippi 2.0.1,
stage 32, P1/P4 Fox, frames -123..4549, SHA-256
`87971fc4…`) paired with the `fox-fd` gameplay pack. It exercises real
controller samples and native observations through the actual replay loader;
the minimal export's 19-frame jab is the smallest independently cross-checkable
Fox action. Use the full `fox-fd` recording for whole-replay validation and a
short prefix around a selected input edge for the first Pon/native callback
integration test. Keep the expected observations external to the Pon test;
self-recording the prefix from Skirmish would only test the validator.

The authoritative source/resource command sequence is:

```text
jq . /mnt/archive/runs/melee-assets-20260909/dolphin-extraction-manifest.json
jq . /mnt/archive/runs/skirmish-fox-fd-export-20260911/manifest.json
tar -tzf /mnt/archive/datasets/melee/skirmish-gameplay/packs/skirmish-gameplay-v10.tar.gz
sha256sum tests/fixtures/slippi/parity/fox-fd.slp
```

The export manifests currently expose the key limitation clearly: even where
the expanded v10 pack reports categories as complete, a parity claim still
requires checking the exact per-pairing manifest, pack hash, source hashes,
and the real replay's first-divergence report. Missing or judgment-call fields
must remain visible in that manifest rather than being filled with defaults.
