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

**Current measurement (2026-09-11, gameplay export v2, private dataset
`cornerian/skirmish-datapacks`, pinned by
`tests/fixtures/slippi/parity/gameplay-export.lock.json`):** 64 frames
match (-123 through -60: the Entry warp-in of both ports, the input lock
and the dead flag), and the first divergent frame is -59, where P1 leaves
EntryEnd for Fall. Skirmish reports the new action's age as 1 on that
frame while the recording reports 0. The v2 pack carries Fox's
locomotion, idle, escapes, air dodge, grab, dash attack, tilts, smashes,
jab combo, aerials, shield, ledge and nudge profiles plus the match rules;
the remaining profiles are being exported. The baseline file records this
number and must move forward as divergences are fixed.

## Practical consequence

None of these three, individually or together, is "Skirmish matches Melee."
Level 1 rules out a class of arithmetic bugs in ported functions. Level 2
guards the test harness against regressing on itself. Level 3 is the only one
that touches an independent recording, and even it is scoped to one matchup,
one stage, and a fixed field set. Treat a "matched" `real_parity` result as
"no evidence of disagreement was found in what was checked," not as a
certification.
