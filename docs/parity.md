# Three levels of "does this match Melee", and what each does not prove

Skirmish's test suite makes three distinct claims about agreement with the
original game. They are easy to conflate because all three involve comparing
Rust output against something external; keeping them separate matters because
each one only rules out a specific kind of bug.

## 1. Function-level C-oracle equivalence

`tests/*_differential.rs` (run with `cargo test --features c-oracle`) compile
small, content-hashed snapshots of the pinned decomp's own C (`tests/oracle`)
and compare a Rust function's output against that compiled C, bit for bit,
over generated and boundary inputs.

**Proves:** a specific Rust function reproduces a specific original C
function's arithmetic, including edge cases (NaN, signed zero, boundary
thresholds), given the same inputs.

**Does not prove:** that the function is called correctly, in the right order,
with the right inputs, as part of a whole action or frame; that unported
neighboring logic doesn't change the outcome; or anything about PowerPC/GameCube
floating-point behavior beyond what host C shares with it. See `AGENTS.md`:
"Host C agreement does not establish PowerPC or whole-game equivalence."

## 2. Self-recorded replay regression

`crates/cli/tests/replay_match.rs`, `crates/peppi-adapter/tests/replays.rs`
and `crates/cli/tests/slippi_corpus.rs` write synthetic `.slp` files (or use
[ten archived real files](../tests/fixtures/slippi/README.md) for import-only
checks), drive the actual native `Match::step` to produce expected
observations, embed those same observations back into the file, and then run
`validate-replay` against it. Corrupting any recorded field or controller
input is asserted to produce a first-divergence failure at that exact frame.

**Proves:** the file-backed comparison harness itself works — timeline
selection, checkpoint restoration, input conversion, the observation policy's
field-by-field comparison and first-divergence reporting all function
correctly, including their many optional-profile and version-gated branches.

**Does not prove:** anything about Melee. The "expected" observations came
from the same native implementation being tested; a bug shared between the
recording step and the comparison step is invisible to this harness by
construction. This is why `replay_match.rs`'s own doc comment calls these
"self-recorded" rather than "parity" regressions, and why `make-initialization`
and `validate-replay`'s docs describe them the same way.

## 3. Real-replay comparison with the ratchet

`crates/cli/tests/real_parity.rs` compares the native match — initialized from
an independently produced gameplay export via `make-initialization` — against
[`tests/fixtures/slippi/parity/fox-fd.slp`](../tests/fixtures/slippi/parity/manifest.json),
a real, human-played Fox-vs-Fox Final Destination recording from the CC0-1.0
`erickfm/slippi-public-dataset-v3.7` corpus. Neither the replay nor the
gameplay export's resources come from Skirmish's own simulator.

**Proves:** for however many frames the report's `first_divergent_frame`
reaches (or fully, if `matched`), the native simulation's observable fields
agree with an authentic recording, for this one matchup and stage. The test
ratchets that frame against a recorded baseline
(`tests/fixtures/slippi/parity/fox-fd-baseline.json`), so a code change that
makes agreement *worse* is a failure, not just a number that quietly
regresses.

**Does not prove:** agreement for any other matchup, stage, or input pattern
than what this one recording happens to exercise; agreement beyond the
selected observation fields (`docs/replays.md`'s `fighter-post-v11` policy
excludes RNG, collision-line geometry, items and more); or agreement once the
first divergence is reached — the report's `checked_frames` is a matched
*prefix*, not a summary of the whole file. It is also gated on real data:
without `SKIRMISH_GAMEPLAY_DATA` (see `docs/gameplay-export.md`) this test
skips, and a skip is not evidence of anything.

**Current measurement (2026-09-11, gameplay export v1, private dataset
`cornerian/skirmish-datapacks`, pinned by
`tests/fixtures/slippi/parity/gameplay-export.lock.json`):** the first
divergent frame is -123, the recording's first post-frame. Port P1's action
state there is 0x0142 (Entry, the match-start warp-in) while Skirmish
begins in 0x001d (Fall): match start is not modeled, so no frame has been
matched yet. The export also models only Fox's jab as an attack; every other
sub-action is absent from the data. The baseline file records this number
and must move forward as batches land.

**Match-start measurement (2026-09-11, same v1 export, patched locally, not
committed):** `docs/match-start.md`'s `game::entry` batch was measured by
copying the export to `/mnt/shared/tmp/skirmish-gameplay-v1-entry/` and
adding `rules.entry {30, 30, 0.0, 0}`, `trophy_scale: 0.9` (solved
bit-exactly from the replay's own recorded EntryEnd Y bits, `0x41358fd0` at
frame -89, against `y0 (10.0, also read directly from the replay) + 1.497345
* trophy_scale`) and `entry {11}` (the EntryStart figatree length that
caps the reported `state_age`, also confirmed directly from the replay's
own recorded values) to both fighters in that copy's
`fox-fd/match-data.json`. `make-initialization` + `validate-replay
--report` against this patched copy still report the first divergent
frame as **-123**, unmoved from the
unpatched measurement above, but for a different, pre-existing reason
unrelated to match-start: the differing field is **`shield`** (expected
`0x42700000` = 60.0, the real replay's starting shield health; Skirmish
reports `0x00000000`), because export v1's `match-data.json` has no
`rules.shield`/per-fighter `shield` resource at all (confirmed directly:
absent from both the original and the patched copy), so every fighter
spawns with `shield.health == 0.0` regardless of match-start modeling
(`game::simulation::spawn`'s `data.rules.shield.as_ref().map_or(0.0, ...)`).
This pre-existing gap already blocked frame -123 before this batch and
still does after it; the Entry sequence's own correctness is not visible
through this specific real-file comparison at all yet, since the shield
field diverges on the very same frame Entry's own fields would first need
to agree. It is independently verified instead by `tests/game_entry.rs`'s
`the_replay_verified_frame_table_is_reproduced_for_slots_zero_and_three`
(the replay-verified Y-curve table above, bit-checked against a synthetic
fixture with the same `trophy_scale`/frame counts) and `tests/
entry_differential.rs`'s C-oracle comparison, not by this measurement.
`fox-fd-baseline.json` is left at -123, matching the note's own instruction
(pack v1 has no `entry` rules; CI is unaffected). A future batch modeling
`rules.shield` (or a v2 export that includes it) is a prerequisite for this
measurement to ever move past -123 at all, independent of match-start.

**Input-lock measurement (2026-09-11, gameplay export v2, uncommitted local
copy, `docs/input-lock.md`):** gameplay export v2 already ships
`rules.entry`/`countdown_frames: 123` and per-fighter `trophy_scale`/
`entry.start_frames` baked in (no local patch needed, unlike v1's match-start
measurement above). Copying
`/mnt/archive/datasets/melee/skirmish-gameplay/v2/` to
`/mnt/shared/tmp/skirmish-gameplay-v2-lock/` and running
`make-initialization` + `validate-replay --report` against the pinned
`fox-fd.slp` first reported the first divergent frame as -123, unmoved, for
the same pre-existing reason recorded above: the differing field was
`shield` (expected `0x42700000` = 60.0, Skirmish reported `0x00000000`),
since that copy of v2 still carried no `rules.shield`/per-fighter `shield`
resource. The published export gained `rules.shield` data mid-batch
(unrelated to this work); re-copying it and re-running moved the divergence
past `shield` to **`state_flags.dead`** (Slippi flags byte 4, bit `0x40`,
`Fighter::x221F_b1`) -- the match-start sequence's own dead-flag bit, set
through Entry and cleared at EntryStart, which this batch had not yet
modeled. Fixed (`docs/input-lock.md`'s own entry has the full citation and
implementation): `game::simulation::enter` now unconditionally clears
`fighter.death.hidden` (mirroring `Fighter_ChangeMotionState`'s own
unconditional `x221F_b1 = 0`, `fighter.c:1066`), and `game::entry::enter`
sets it back to `true` immediately after, mirroring `ftCo_800C61B0`
(`ft_0C31.c:46`) exactly. Re-running after that fix moves the divergence
past `state_flags.dead` too, landing on **`position.x`** for **P4**
(expected `0x42700000` = 60.0, Skirmish reports `0x41a00000` = 20.0) -- a
stage-spawn coordinate mismatch in the pack's own `stage.spawns` data,
unrelated to either this batch or the match-start batch (neither reads
spawn coordinates from anywhere else), left for whoever owns that data
next. Both of this batch's own fields (and the match-start batch's) now
agree with the recording on frame -123; verified independently by `tests/
game_entry.rs`'s native tests (held-stick-produces-no-drift during the
lock, the first controlled frame acting, the legacy `rules.entry.is_none()`
freeze unaffected, and the dead-flag bit's Entry/EntryStart transition).
`fox-fd-baseline.json` is left unmoved (still -123); moving it is a future
batch's call once `position.x` (or whatever the next-found field is) is
fixed.

## Practical consequence

None of these three, individually or together, is "Skirmish matches Melee."
Level 1 rules out a class of arithmetic bugs in ported functions. Level 2
guards the test harness against regressing on itself. Level 3 is the only one
that touches an independent recording, and even it is scoped to one matchup,
one stage, and a fixed field set. Treat a "matched" `real_parity` result as
"no evidence of disagreement was found in what was checked," not as a
certification.
