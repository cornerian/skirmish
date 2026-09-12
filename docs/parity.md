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
each recording listed in
[`tests/fixtures/slippi/parity/recordings.json`](../tests/fixtures/slippi/parity/recordings.json):
real, human-played Fox-vs-Fox Final Destination recordings from the CC0-1.0
`erickfm/slippi-public-dataset-v3.7` corpus. Neither the replays nor the
gameplay export's resources come from Skirmish's own simulator.

**The ratchet now spans four independent recordings**, not one:
[`fox-fd.slp`](../tests/fixtures/slippi/parity/manifest.json) (the original
recording; ports P1/P4), `fox-fd-2.slp` (ports P1/P2), `fox-fd-3.slp`
(ports P2/P4, the only one recorded on a newer Slippi client, 3.9.0, which
reports a few extra observation fields such as `velocities.self_y` that
2.0.1 does not) and `fox-fd-4.slp` (ports P2/P4, added by a concurrent
batch and folded in here; see "A second real recording" below). All four
are Fox-vs-Fox Final Destination matches, so all four currently initialize
from the same `fox-fd` pairing's `match-data.json` regardless of which two
ports played them — the pack's spawns are assigned by participant order,
not by which physical port a player sat at. `recordings.json` lists each
recording's file, sha256, ports, stage and its own baseline file
(`tests/fixtures/slippi/parity/<id>-baseline.json`); `real_parity.rs`
iterates the list and checks every recording before failing, so a
regression on one recording is reported alongside any others rather than
masking them. `fox-fd`'s own baseline file, numbers and update history are
unchanged by this generalization — the original real-replay parity loop
(the "main loop") continues to own and ratchet `fox-fd-baseline.json`
exactly as before; `fox-fd-2-baseline.json`, `fox-fd-3-baseline.json` and
`fox-fd-4-baseline.json` are independent baselines for the added
recordings, owned by a second, distinct parity loop (`docs/validation.md`)
that deliberately stays clear of whatever the main loop above is currently
chasing on `fox-fd.slp`.

**`fox-fd-2.slp` (2026-09-11, gameplay export v6): blocked on a pack-data
gap, not chased further.** The first real measurement matched 92 frames
(-123 through -32) and diverged at -31, P1's `action_state` (expected
`0x0155`/`SpecialN` i.e. Fox's neutral special "Blaster", actual
`0x002a`/`Landing`). Diagnosis: P1 presses B (a fresh press, confirmed via
the recording's own pre-frame input samples) while grounded in an
interruptible Landing (`action_frame` 19, well past `normal_landing_lag`,
`landing_allow_interrupt` set) — every eligibility check `game::specials::
grounded_chain_open`/`tilt::interrupt_chain`/`landing::interruptible` and
`fighter::special::neutral_input`'s own fresh-press/stick-neutral test
already agree the special should start. It does not, because `game::
specials::neutral::Move::update_actions` bails out immediately when
`data.specials.as_ref().and_then(|s| s.neutral())` is `None` — and pack v6's
`fox-fd/match-data.json` fighter entry's `specials` object has only
`character`, `down`, `side` and `up` keys; `neutral` (Fox's Blaster ground/
air poses and `neutral_thresholds`) is entirely absent. This is a pack-data
gap (the gameplay-export pipeline, not Skirmish code), reported per this
loop's own stop condition and left at this baseline rather than chased
further; the next divergence past it is undiagnosed. `fox-fd-4.slp` (added
by a concurrent batch, see "A second real recording" below) hits the exact
same pack-data gap on its own equivalent divergence.

**`fox-fd-2.slp` (2026-09-12, live gameplay export pack, `/mnt/archive/
datasets/melee/skirmish-gameplay/v2/fox-fd/match-data.json`): moved twice
by the entry-advance batch.** The pack gap above is now filled (`specials.
neutral` is present), so Fox's Blaster dispatches correctly; the next
measurement matched 93 frames (-123 through -31) and diverged at -31
itself, P1's `action_age` (expected `1.0`, actual `0.0`) on the very
`SpecialNStart` entry frame -- `docs/validation.md`'s entry-advance table
fixes this (`ftFx_SpecialN_Enter`'s own extra, undocumented `ftAnim_
8006EBA4` advance) along with five sibling instances of the identical bug
across Fox's other specials. Current measurement: 98 frames match (-123
through -26); the next divergence is -25, `action_state` (expected
`0x0156`/`SpecialNLoop`, actual `0x0155`/`SpecialNStart`), traced to a
pack-export mismatch, not a Skirmish bug: `fighters[0].specials.neutral.
start.ground.frames` has length `8` in the live pack, but the recording's
own `state_age` sequence shows the real grounded Start phase lasting
exactly 6 frames, matching the pack's own `start.air.frames` length (`6`)
instead. Reported per this loop's own stop condition (`tests/fixtures/
slippi/parity/fox-fd-2-baseline.json`'s own note has the full citation),
not chased further.

**`fox-fd-3.slp` (2026-09-11, gameplay export v6): moved by the
landing-velocity fix.** The first real measurement matched 79 frames
(-123 through -45) and diverged at -44, P2's `velocities.self_y` (expected
`-2.53`, actual `0.0`) — fixed by `docs/validation.md`'s landing-velocity
entry (`game::collision::land` no longer zeroes vertical self-velocity on
landing, matching `ftCommon_8007D6A4` leaving `self_vel.y` unassigned).
Current measurement: 91 frames match (-123 through -33); the next
divergence is -32, field `velocities.self_x_air` on P2 (expected
`0x400147ad` = `2.0199997`, actual `0x400147ae` = `2.0199999`, a one-ULP
rounding difference).

**Diagnosis (a suspected cross-platform floating-point limitation, not a
Skirmish bug -- reported per this loop's own stop condition, not
chased):** P2 enters `Dash` at -33 (`ground_velocity` set to the pack's
`dash_initial_velocity`, `1.899999976`, with `velocity` itself still `[0,
0]` that frame -- decomp's own `getAccelAndTarget`/`ftCommon_8007C98C`
chain is not projected into `self_vel` until the following frame, exactly
like the already-cited "Dash Enter writes gr_accel2... not projected...
this first frame" note). At -32, `game::locomotion::ground_motion`'s Dash
branch computes `accel = stick(1.0) * dash_acceleration_mul(0.1) +
dash_acceleration_base(0.02)`, `target = stick(1.0) * dash_max_velocity
(2.2)`; since `ground_velocity(1.9) + accel(0.12)` (`2.02`) does not yet
exceed `target(2.2)`, `accelerate_ground`'s clamp never engages, and
`ground_velocity = 1.9 + 0.12` projects into `velocity[0]` unchanged
(`Movement::project_ground` multiplies by the flat floor's exact `1.0`
normal, losslessly). Every step here is confirmed byte-for-byte identical
to the pinned decomp (`getAccelAndTarget`, `ftCommon_8007C98C`/
`accelerate_ground`, `ftCommon_ApplyGroundMovementNoSlide`/
`project_ground`) in both structure and operand order, and
`tests/physics_differential.rs`'s existing `accelerate_ground` C-oracle
case already passes bit-exact against the compiled original for
proptest-generated and boundary inputs -- so the Rust port is a verified,
faithful transcription of the source expression, not an approximation.
Evaluating that exact expression chain in ordinary IEEE 754 `f32`
arithmetic, using the pack's own full-precision constants (`1.899999976 +
(1.0 * 0.100000001 + 0.019999999)`), independently reproduces Skirmish's
own `0x400147ae` bit-for-bit -- confirming the Rust port computes the
mathematically correct IEEE 754 result for this expression, while the
recording's own hardware produced a result one ULP lower. The `skirmish-fma`
batch's `tools/ppc_fma_audit.py` has since checked the retail binary's own
disassembly for exactly the suspected mechanism (a fused multiply-add
rounding the product+sum once instead of twice) and ruled it out for this
chain: `ftCo_Dash_Phys` (the function that inlines `getAccelAndTarget` and
calls `ftCommon_8007C98C` and `ftCommon_ApplyGroundMovementNoSlide`)
disassembles to a plain `fmuls` followed by a plain `fadds` for `accel`,
not a single `fmadds`, and none of `ftCommon_8007C98C`,
`ftCommon_ApplyGroundMovement` or `ftCommon_ApplyGroundMovementNoSlide`
contain any fused op either (`docs/math.md`'s per-function findings). So
the retail PowerPC binary computes this exact chain with the same
separately-rounded multiply-then-add IEEE 754 `f32` arithmetic this Rust
port does; reproducing the original PowerPC compiler's exact instruction
selection is not the explanation here after all. The actual mechanism
behind the recording's own one-ULP-lower result remains unexplained, which
is outside what a decomp-ported Rust function can express -- exactly the
limitation `AGENTS.md` already names ("Host C
agreement does not establish PowerPC or whole-game equivalence").

**Update (`skirmish-f64` batch, `tools/ppc_precision_audit.py`,
`docs/math.md`):** the remaining candidate mechanism -- a genuine
double-precision intermediate (PowerPC's *single-precision* mnemonics
still compute at double precision internally and round once to `f32`; a
plain, non-fused double-precision op feeding a single-rounded one would
still differ from two separately-rounded `f32` steps, and the FMA audit's
own tool never actually checked for that) -- is now also ruled out, and
not merely by absence of evidence: `ftCo_Dash_Phys`, `ftCommon_8007C98C`,
`ftCommon_ApplyGroundMovement(NoSlide)`, `ftCommon_ApplyFrictionGround`,
`ftCo_Dash_Enter` and `ftCommon_800804A0` all disassemble with zero
double-precision arithmetic instructions (every `lfd` found is a
callee-saved FPR stack spill, not a constant load). Running Fox's own
exact recorded pack constants (`f32` bits `0x3ff33333`/`0x3dcccccd`/
`0x3ca3d70a`/`0x400ccccd`) through every rounding model this expression
could plausibly use -- single-precision step by step, fully
double-precision with one final round, and every mix in between -- always
produces `0x400147ae`, never the recording's `0x400147ad`: no instruction
selection reaches the recorded value from these inputs at this expression,
full stop. A perturbation sweep shows the incoming `ground_velocity` would
need to be about two ULP lower than modeled to reproduce `0x400147ad`,
pointing at the Dash-entry velocity computation (`ftCo_Dash_Enter`/
`ftCommon_800804A0`, also confirmed double-precision-free) or an earlier
frame, not this expression's own evaluation order -- a concrete lead for
whichever future batch picks this up, not chased further here. No baseline
change (`fox-fd-3.slp` unchanged at -32/91).

**Proves:** for however many frames each recording's own
`first_divergent_frame` reaches (or fully, if `matched`), the native
simulation's observable fields agree with an authentic recording, for that
one matchup, stage and set of ports. Each recording's ratchet independently
guards against a code change that makes *that* recording's agreement worse.

**Does not prove:** agreement for any other matchup, stage or input pattern
than what these recordings happen to exercise (all three are Fox-vs-Fox on
Final Destination; no other matchup or stage is covered); agreement beyond
the selected observation fields (`docs/replays.md`'s `fighter-post-v11`
policy excludes RNG, collision-line geometry, items and more); or agreement
once a recording's first divergence is reached — each report's
`checked_frames` is a matched *prefix*, not a summary of the whole file. It
is also gated on real data: without `SKIRMISH_GAMEPLAY_DATA` (see
`docs/gameplay-export.md`) this test skips entirely, and a skip is not
evidence of anything. Since 2026-09-12 the data discovery under
`SKIRMISH_GAMEPLAY_DATA/<pairing>/` prefers a compact `match-data.bin` over
`match-data.json` when both are present (`docs/gameplay-export.md`'s
"Compact binary pack" section); either file decodes to the identical
`MatchData`, so this changes CI download/load time only, never any of the
measurements below.

**Published pack v9 (2026-09-12):** sha256 2481ab51, 36 MB, supersedes v8:
`rules.fast_fall_window` (4 frames, `ftCommonData` +0x8C) is embedded in
every pairing, so the exact fast-fall check applies; on Battlefield the
first divergence moves from position at -27 to the shield health on the
same frame.

**Published pack v8 (2026-09-12):** sha256 a01a9101, 36 MB, supersedes v7:
the special-move phase pose counts now equal the figatree frame counts (the
earlier packs carried one spare hold pose per phase, a one-frame timing
error for every phase whose length is its duration). With it fox-fd-2
reaches 114 frames and fox-fd-4 93.

**Published pack (2026-09-12):** gameplay export v7 (sha256 5ebf4a2d, 36 MB)
is the complete export in the compact binary format: every Fox and Falco
pairing (`fox-fd`, `falco-fd`, `fox-falco-fd`, `falco-fox-fd`, `fox-bf`,
`fox-dl`, `fox-ys`, `fox-fod`, `fox-ps`) as `match-data.bin`, with the
neutral special, per-orientation knockdown and ledge snap data embedded.
The recordings list's baselines were measured against the live export that
v7 snapshots, so CI's ratchet applies to all of them.

**Current measurement (2026-09-12, gameplay export v6, after the msl-trig
batch, `docs/math.md`):** unchanged -- 128 frames match (-123 through 4) and
the first divergent frame is still 5, field `position.x` (expected
`-29.740234375`, actual `-29.740236282348633`), identical to the
previous measurement's own values below. `fighter::escape_air::
launch_velocity` and every other fighter/common trigonometry call site now
use ported copies of the game's own `sinf`/`cosf`/`tanf`/`atan2f`/`atanf`/
`acosf`/`asinf` (`src/math.rs`) instead of the portable `libm` crate the
previous measurement's own diagnosis suspected -- but the divergence
persists at the exact same frame with the exact same bits, so that
diagnosis is not confirmed by this measurement. `docs/math.md`'s
fused-multiply-add finding has the full trail, including which operations
are fused: not guessed from this measurement (an earlier revision's
approach, and documented there as a cautionary finding in its own right),
but read directly from a Capstone disassembly of the retail `main.dol`,
cross-checked against a sibling batch's independent tool
(`skirmish-fma`'s `tools/ppc_fma_audit.py`). Porting every fused operation
exactly as the retail binary computes it still ties, not improves on, this
measurement. The true cause of the frame-5 divergence remains open; not
chased further in this batch, per this loop's own stop condition.

**Update (`skirmish-f64` batch):** the other leading candidate, a genuine
double-precision intermediate rather than a fused op, is also ruled out
for this chain by direct disassembly (`tools/ppc_precision_audit.py`,
`docs/math.md`): `ftCo_80099A9C` (the launch itself), `inlineA0`,
`ftCommon_8007D9D4` (the stick-angle helper feeding `atan2f`), and
`ftCo_EscapeAir_Phys`/`IASA` all disassemble with zero double-precision
arithmetic instructions. This is consistent with, not a new explanation
for, the `skirmish-msl-trig` batch's own finding above that a fully
disassembly-verified `cosf`/`sinf` port still ties this exact frame rather
than fixing it -- the divergence is real and still open. No baseline
change (`fox-fd.slp` unchanged at 5/128).

Previously (2026-09-11, gameplay export v6, after this loop's
ground-jump-direction fix): 128 frames match (-123 through 4) and the
first divergent frame is 5, field `position.x` (expected `-29.740234375`,
actual `-29.740236282348633`, on P1's own continuing ground slide inside a
`LandingFallSpecial` entered from an earlier air dodge).

**Diagnosis (a suspected cross-platform floating-point limitation, not a
Skirmish logic bug -- reported per this loop's own stop condition):**
tracing `game::simulation::move_fighter`'s generic grounded fallback frame by
frame from this `LandingFallSpecial` entry shows every intervening frame's
`position.x` matching the recording bit-for-bit through frame 4, using a
ground velocity that decays by exactly the expected friction each frame
(`ft_80084F3C`'s `friction = 0.08`, doubled above `walk_max_velocity`); the
frame-5 addition itself (`-1.1955388 + 0.08 = -1.1155387`, IEEE-754 exact,
reproduced identically in Python) is not in question, but solving for the
velocity decomp's own recording would need to land on the *expected* frame-5
position gives `-1.1155376` instead -- nine ULPs away at the velocity's own
magnitude, small enough that adding it to the much larger `position.x` had
rounded to the *same* representable bit pattern on every earlier frame,
until frame 5's own rounding boundary finally exposes it. This points
upstream, to the air dodge's own launch velocity: `fighter::escape_air::
launch_velocity` computes `force * libm::cosf(angle)`/`sinf(angle)`
(`ftCo_EscapeAir.c`'s `inlineA0`), and `libm`'s portable, from-scratch
`cosf`/`sinf` are not guaranteed bit-identical to the original GameCube SDK's
own trigonometric routines -- unlike the addition/multiplication chain that
follows, which IEEE 754 does guarantee reproduces exactly given exact
inputs. A several-ULP error at launch, decayed and carried through the dodge
and its landing's friction, exactly matches a discrepancy too small to
appear until it crosses a rounding boundary many frames later. Not chased
further in this batch: fixing it would mean matching PowerPC's own
transcendental-function implementation bit-for-bit, not a Skirmish
behavior or sequencing bug.

Previously (2026-09-11, gameplay export v6, before this loop's ground-jump-
direction fix): 126 frames matched (-123 through 2) and the first divergent
frame was 3, field `action_state` (expected `0x0019`/Jump-forward, actual
`0x001a`/Jump-backward, on P4's own KneeBend->Jump transition). Fixed:
`ftCo_Jump_Enter`'s direction test (`ftCo_Jump.c:157-161`) and `ftCo_
800CB110`'s launch-velocity computation are both dispatched from `ftCo_
KneeBend_Anim`, an Anim callback, which (like the generic per-frame animation
advance `observation::observe`'s general `-1` rule already accounts for)
runs before this same frame's own controller read updates `fp->input` --
unlike this crate's other input reads, which are IASA-dispatched
(`ftCo_Wait_IASA`'s jump-request chain, `ftCo_JumpAerial_CheckInput`'s own
identical-looking direction test) and so already see the current frame's
fresh controller. `game::locomotion::ground_jump` now reads `f.
previous_input.stick[0]` for both the direction test and the launch
velocity, confirmed directly against `fox-fd.slp`: `fighter::locomotion::
jump_backward` fed the frame *before* each of the recording's 64 KneeBend->
Jump transitions (both ports, full match) agrees with every one, while fed
the transition frame's own stick instead, three disagree (P4 at frame 3, P1
at 775 and 2990). The new divergence at frame 5 is a separate, unrelated
matter (above), reported rather than chased in this batch.

Previously (2026-09-11, gameplay export v6, before this loop's `action_age`
tracked-rate fix): the pack fix alone (below) reached frame -3, field
`action_age` (expected a non-integer `3.01`, actual `1.0`) on P1's
KneeBend->LandingFallSpecial transition (an air-dodge landing immediately out
of a short hop). Diagnosis: `ftCo_LandingFallSpecial_Enter`'s own anim-speed
argument to `Fighter_ChangeMotionState` is `(0.1F + fp->x2EC) / landing_lag`
(`ftCo_Landing.c:111`), not `1.0` -- `fp->x2EC` is the character's own cached
FallSpecial animation-frame count (`fighter.c:836`) and `landing_lag` is
`ftCommonData`'s `x344` (`escape_air::Rules::landing_lag`), so Melee's
`cur_anim_frame` advances at that computed rate, not one frame per game
frame. The ordinary aerial landings (`LandingAirN`/`F`/`B`/`Hi`/`Lw`) scale
the same way through the L-cancel divisor (`game::aerial::land`). Skirmish
already tracks this rate at the source (`fighter.aerial.landing_elapsed`/
`landing_rate`, `game::aerial.rs`, `game::escape_air.rs`) but
`observation::observe`'s `action_age` fell through to the generic
`action_frame`-based rule (the same one Walk/Run needed their own tracked-
float branch for) instead of reading it. Fixed: `observation::observe` now
reads `fighter.aerial.landing_elapsed` directly for those six actions,
confirmed directly against `fox-fd.slp`: P1's air-dodge landing enters
`LandingFallSpecial` at frame -4 already reporting `state_age = 0.0`, then
`3.01`, `6.02`, `9.03` on -3, -2 and -1, a constant rate of `3.01` rather
than the generic rule's `1.0`. The new divergence at frame 3 is a separate,
unrelated subsystem (P4's own jump-direction selection), reported rather
than chased in this batch.

Previously (2026-09-11, gameplay export v5,
`/mnt/archive/datasets/melee/skirmish-gameplay/v5-snapshot-20260911`, after
the real-replay parity loop's Dash/Turn `action_frame` batches): 116
frames match (-123 through -8) and the first divergent frame is -7, field
`position.x` (expected `-17.7748`, actual `-17.6948`, on P1's Run->KneeBend
transition, with `action_state` itself already matching).

**Diagnosis (not a Skirmish bug -- a pack data gap, reported per this loop's
own stop condition):** frame-by-frame position deltas through this
transition (`-14`..`-6`: `2.1175, 2.1175, 2.1175, 2.1175, 2.09, 2.0625,
2.035, 1.875, 1.715`) show a *constant* per-frame reduction of `0.0275`
through Run (frames `-11`..`-8`), then a *constant* reduction of `0.16`
starting exactly at the KneeBend/JumpSquat entry (`-8`->`-7` and `-7`->`-6`
alike) -- `0.16` is exactly `0.08 * 2.0`, where `0.08` is Fox's own
`movement.ground_friction` in this pack. `game::simulation::move_fighter`'s
generic grounded fallback (used by JumpSquat, Turn, Wait, Squat and every
other action `game::locomotion::ground_motion` doesn't own) already ports
`ft_80084F3C` exactly: `friction = ground_friction; if |gr_vel| >
walk_max_velocity { friction *= rules.friction_above_walk }`. Fox's
`ground_velocity` here (~1.9-2.0) is well above `walk_max_velocity` (`1.6`
in this pack), so the boost should apply -- but pack v5's (and v4's)
`rules.friction_above_walk` is `1.0`, a no-op, so Skirmish applies the
unboosted `0.08` instead of the needed `0.16`. Confirmed directly: patching
a local copy of the pack's `match-data.json` to `friction_above_walk = 2.0`
(not committed -- a throwaway diagnostic copy, deleted after use) moves
`checked_frames` from 116 to 120 with no other change, isolating this as
the sole cause of the -7 divergence. `rules.friction_above_walk` is a single
match-wide constant (`src/game/data.rs`), matching `ftCommonData` being
shared across every character, so a single corrected value should apply
universally once re-exported; this needed a live-pack data fix (the
gameplay-export pipeline, not this repository) rather than a Skirmish code
change. Gameplay export v6 republished with `rules.friction_above_walk =
2.0` (confirmed byte-identical to this diagnostic patch), superseding this
entry.

Previously (2026-09-11, gameplay export v5, after the real-replay parity
loop's Dash->Run `action_frame` timing fix): 116 frames matched
(-123 through -8) and the first divergent frame was -7, `position.x`, as
above -- unchanged by the subsequent entry-time consolidation batch
(`docs/validation.md`), confirmed by re-measuring rather than assumed.

Previously (2026-09-11, gameplay export v5, the real-replay parity loop's
Dash->Run `action_frame` timing fix): 116 frames matched (-123 through -8),
up from 110. Pack v5 publishes `move_id` for the specials that pack v4 was
missing (`docs/ecb-load-flags.md`'s "Known gap" no longer blocks stepping
this match through its own recorded inputs past frame ~71); it is otherwise
identical to v4 for this fox-fd data through the range measured here. The
previous divergence (-13, `action_state`
on a Dash->Run transition, described below) is fixed: `game::dash::
update_dash_or_run`'s and `game::locomotion`'s Dash-to-Run check compared
`action_frame >= dash_run_frame`, one frame later than `fn_800CA5F0`'s own
`cur_anim_frame >= dash_run_frame` (the animation's scripted run flag,
`ftaction.c:462`) -- the generic per-frame animation advance
(`Fighter_Spaghetti_8006AD10`'s `ftAnim_8006EBA4`, `fighter.c:1684`) already
bumps decomp's `cur_anim_frame` for the current frame before `ftCo_Dash_IASA`
reads it, while `simulation::advance`'s shared end-of-frame `action_frame +=
1` has not yet run at the point this check reads `action_frame`. Both copies
of the check now compare `action_frame + 1 >= dash_run_frame`, confirmed
directly against `fox-fd.slp`: P1 holds forward through Dash frames -24..-14
(`action_age` 1..11) and is already in Run at -13, one frame before the
unadjusted comparison produced. The new divergence at -7 is a separate,
unrelated subsystem (Run's ground movement or the KneeBend/JumpSquat entry
itself, not the Dash-to-Run transition), reported rather than chased in this
batch.

Previously (2026-09-11, gameplay export v4,
`/mnt/archive/datasets/melee/skirmish-gameplay/v4-snapshot-20260911`, the
real-replay parity loop's Dash->Turn `action_age` fix): 110 frames match
(-123 through -14) and the first divergent frame is -13, field
`action_state` (expected `0x0015`/Run, actual `0x0014`/Dash, on P1's
Dash->Run transition). The previous divergence (-30, `action_age` on a
Dash->Turn transition, described below) is fixed: `ftCo_Turn_Enter`/
`ftCo_Turn_Enter_Smash` (`ftCo_Turn.c:49-62`, `:173-188`) call `ftAnim_
8006EBA4(gobj)` immediately after `Fighter_ChangeMotionState`, the same
extra animation advance `ftCo_Dash_Enter` already made -- so `Action::Turn`
needed the same `action_frame`-unadjusted treatment `observation::observe`
already gave `Action::Dash`, confirmed directly against `fox-fd.slp`'s
dash-dance rally (P1 re-enters Turn at both -30 and -25, each already
reporting `state_age = 1.0`). `ftCo_TurnRun_Enter` (`ftCo_TurnRun.c:44-51`)
changes motion state but does not make this extra call, so `Action::RunTurn`
is unaffected. The new divergence at -13 is a separate, unrelated subsystem
(the Dash-to-Run transition frame itself), reported rather than chased in
this batch. A missing `move_id` in the v4 pack (unrelated to either fix)
blocks stepping this same match past frame ~71 through its own recorded
inputs, which is why the jump landing at frames 418-421 could only be
confirmed against the recording's own ground truth, not against Skirmish's
simulated value there (`docs/ecb-load-flags.md`'s "Known gap").

Previously (2026-09-11, gameplay export v4,
`/mnt/archive/datasets/melee/skirmish-gameplay/v4-snapshot-20260911`, the
ECB-load-flags batch, `docs/ecb-load-flags.md`): 93 frames match
(-123 through -31) and the first divergent frame is -30, field
`action_age` (expected 1, actual 0, on P1's Dash -> Turn transition). This
measurement used the v4 pack directly rather than `SKIRMISH_GAMEPLAY_DATA`
(unset in ordinary CI; the ratchet below still applies once v4 or later is
published there). The previous divergence (-50/-49, `action_state` Fall
vs. Landing, described below) is fixed: `game::collision::sample` now
passes `load_joints` the flags the decomp's own collision entry point uses
per path (6 airborne, 5 grounded) instead of one static, always-0 resource
field, and `game::collision::resolve`'s floor-contact branch now rests
`position` itself on the floor (instead of `position + ecb.current.bottom`)
whenever the raw airborne ECB bottom samples above position, matching
`mpColl_80046904`'s `ecb_unlocked`/`ignore_bottom`. The entry fall
(-52..-49) now matches the recording bit-for-bit, including the landing
position (`0.0001`). The new divergence at -30 is a separate, unrelated
subsystem (action/turn-state age tracking) and is reported rather than
chased in this batch. A missing `move_id` in the v4 pack (unrelated to
collision) blocks stepping this same match past frame ~71 through its own
recorded inputs, which is why the jump landing at frames 418-421 could only
be confirmed against the recording's own ground truth, not against
Skirmish's simulated value there (`docs/ecb-load-flags.md`'s "Known gap").

Previously (2026-09-11, gameplay export v4, private dataset
`cornerian/skirmish-datapacks`, pinned by
`tests/fixtures/slippi/parity/gameplay-export.lock.json`): 73 frames
match (-123 through -51) and the first divergent frame is -50, field
`action_state` (expected Fall, actual Landing). Pack v4 corrects the
collision-box and hurtbox bones: the game indexes its joint array
directly for those (`ft_081B.c:52-57`, `ftcoll.c:3231-3285`) and only
routes hitbox bones through the parts table (`ftaction.c:315`), so Fox's
collision box samples joints `[41, 55, 25, 13, 7, 4]`, not the root. The
remaining frame was the loader's two-unit padding: the game's ordinary
airborne and grounded loads pass flag bit 4 and skip it
(`mpcoll.c:392-397`, entry points at 2741-2835 and 3999-4034), while
Skirmish applied one static flag of zero; fixed above.

Previously (2026-09-11, gameplay export v3, private dataset
`cornerian/skirmish-datapacks`, pinned by
`tests/fixtures/slippi/parity/gameplay-export.lock.json`): 72 frames
match (-123 through -52) and the first divergent frame is -51, field
`action_state` (expected Fall, actual Landing). The v3 pack is the complete
Fox on Final Destination export: every fighter profile, the three Fox
specials, per-frame poses for every movement state, and all rule sets.
The divergence is not a data limitation: every landing in the recording
(the entry fall at -52..-49, jump landings at 418-421, 739-742, 1344-1347,
the Illusion landing at 654-657) shows the position 2.7 to 3.6 units
below the floor for one airborne frame before Landing at 0.0001, so the
game detects the floor one frame after the position crosses it, while
Skirmish lands on the crossing frame. That ordering inside the airborne
collision solver is the next batch (`docs/landing-order.md` when it
lands). The ECB-timing batch's earlier conclusion that Fox's collision
bottom equalling the position makes this unfixable is superseded by that
evidence; its ten-frame ECB lock fix stands (`docs/ecb-timing.md`).

Previously (2026-09-11, gameplay export v2, private dataset
`cornerian/skirmish-datapacks`, pinned by
`tests/fixtures/slippi/parity/gameplay-export.lock.json`):** 72 frames
match (-123 through -52: the Entry warp-in of both ports, the input lock,
the dead flag, and P1's EntryEnd->Fall handoff at -59 with the corrected
0-based `state_age`), and the first divergent frame is still -51, field
`action_state` (expected `0x001d`/Fall, actual `0x002a`/Landing) --
unchanged by the movement-poses batch (`docs/movement-poses.md`).

That batch spliced the v2 pack's `movement_poses` into a local copy of
`fox-fd/match-data.json` (`/mnt/shared/tmp/skirmish-gameplay-v2-poses/`,
not committed; `fox-fd-baseline.json` is untouched, since the published
snapshot lacks the poses) and re-ran `make-initialization`/
`validate-replay`: the frame and field are identical to the prior
measurement below. `docs/movement-poses.md` explains why: Fox's exported
`collision_box.indices` includes the skeleton's root joint (translation
`[0.0; 3]` in every pose by this dataset's own TransN-stripping
convention), which pins his ECB bottom to exactly `position.y` regardless
of animation, so no per-frame pose -- however it tucks the other five
joints -- can move his landing frame. This is a limitation of that
dataset's `collision_box.indices` (from the earlier, separately-pinned ECB
batch), not of the movement-poses wiring itself, which `tests/
game_movement_poses.rs`'s synthetic (non-root-sampling) fixture confirms
works as designed. The recording's own state-age reset at -51 (Fall's
9-sample pose looping over an 8-frame period, not a discrete
`Fighter_ChangeMotionState` re-entry) is diagnosed and reproduced
(`game::movement::loop_period`, `crates/skirmish-replay/src/
observation.rs`'s matching `action_age` branch) but does not itself affect
ground-contact timing. The v2 pack carries Fox's locomotion, idle,
escapes, air dodge, grab, dash attack, tilts, smashes, jab combo, aerials,
shield, ledge, nudge and movement-pose profiles plus the match rules; the
remaining profiles are being exported. The baseline file records the
matched-frame count and must move forward as divergences are fixed; this
one needs a `collision_box.indices` revisit, not a poses change, to move.

Previously (2026-09-11, before the movement-poses batch): the same 72
frames matched and the same frame/field diverged (-51, `action_state`,
Fall vs. Landing) -- recorded here as "a new, separate divergence that
this batch did not diagnose"; the movement-poses batch above is the
diagnosis.

Previously (before the `state_age`/`action_age` transition-frame fix,
`docs/validation.md`): 64 frames matched (-123 through -60), and the first
divergent frame was -59, where P1 leaves EntryEnd for Fall: Skirmish
reported the new action's age as 1 on that frame while the recording
reported 0.

## A second real recording: `fox-fd-4.slp`

A second, independently recorded Fox-vs-Fox Final Destination match,
`tests/fixtures/slippi/parity/fox-fd-4.slp` (`14_56_00 [C2] Fox + Fox
(FD).slp`, same CC0-1.0 `erickfm/slippi-public-dataset-v3.7` corpus,
`batch_00`, Slippi 2.0.1, ports P2/P4). It has since been folded into the
shared `tests/fixtures/slippi/parity/recordings.json` ratchet described
above (its standalone `real_parity_fox_fd_4.rs` harness is removed;
`crates/cli/tests/real_parity.rs` now covers it against the same
`fox-fd-4-baseline.json`), rather than staying a fifth separate test file.
It reuses the same `fox-fd/match-data.json` export `fox-fd.slp` does,
unchanged: the pack is Fox-vs-Fox-on-FD data and spawns follow participant
order, not recorded port numbers, so any two-Fox FD recording works
against it regardless of physical ports.

**Current measurement (2026-09-11, gameplay export v6,
`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911`, after
the real-replay parity loop's input-lock `previous_input` fix):** 85 frames
match (-123 through -39) and the first divergent frame is -38, field
`action_state` (expected `0x0155`/341 = `Action::SpecialN`, actual
`0x002a`/42 = `Action::Landing`, for P2).

**Diagnosis (not fixed in this batch -- a pack-data gap, reported per this
loop's own stop condition):** the recording shows P2 pressing B out of
`Landing` once its interrupt window opens and entering Fox's neutral
special (Blaster, `Action::SpecialN`, already mapped to Slippi state 341 by
`game::characters::fox`). Skirmish instead keeps P2 in `Landing`. Traced
directly (the second real-replay parity loop, working the same finding
independently on `fox-fd-2.slp`'s equivalent divergence): every eligibility
check the dispatch actually runs is satisfied on this frame --
`game::specials::grounded_chain_open` reaches `Action::Landing` through
`tilt::interrupt_chain`'s `landing::interruptible` arm (not through its own
literal `Wait`/`Walk`/`Dash`/`Run`/`RunBrake`/`Turn`/`Squat` family list, as
an earlier pass through this diagnosis guessed; `landing::interruptible`
itself is confirmed satisfied here: grounded, `Action::Landing`,
`landing_allow_interrupt` set, `action_frame` well past
`normal_landing_lag`), and `fighter::special::neutral_input`'s fresh-B-
press/neutral-stick test also passes (confirmed via direct instrumentation
of the recorded input samples). The special still does not start because
`game::specials::neutral::Move::update_actions` bails out immediately when
`data.specials.as_ref().and_then(|s| s.neutral())` is `None` -- and pack
v6's `fox-fd/match-data.json` fighter entry's `specials` object has only
`character`, `down`, `side` and `up` keys; `neutral` (Fox's Blaster ground/
air poses and `neutral_thresholds`) is entirely absent from the export.
This is a gameplay-export pack-data gap, not a Skirmish eligibility or
dispatch bug: no code change here can start a special whose own resource
parameters were never exported. A live concurrent worktree
(`skirmish-blaster`) is already reserved for Fox's neutral special, so the
actual pack/implementation work is left to it; both `fox-fd-2.slp` and
`fox-fd-4.slp` are blocked on the same gap and will move together once it
is filled.

**Current measurement (2026-09-12, live gameplay export pack, `/mnt/
archive/datasets/melee/skirmish-gameplay/v2/fox-fd/match-data.json`, the
entry-advance batch):** the pack gap above is now filled (`specials.
neutral` is present); P2 dispatches into Fox's neutral special correctly.
91 frames match (-123 through -33); the next divergence is -32,
`action_state` (expected `0x0156`/`SpecialNLoop`, actual `0x0155`/
`SpecialNStart`) -- the identical pack-export mismatch `fox-fd-2.slp`'s own
current measurement above hits at its equivalent frame: `fighters[0].
specials.neutral.start.ground.frames` has length `8` in the live pack, but
both recordings' own `state_age` sequences show the real grounded Start
phase lasting exactly 6 frames, matching the pack's own `start.air.frames`
length (`6`) instead. Between the pack gap closing and this new divergence,
this batch also fixed `action_age` (expected `1.0`, actual `0.0`) on the
`SpecialAirNStart` entry frame (-38, the same shape `fox-fd-2.slp` hit on
its own grounded entry): `docs/validation.md`'s entry-advance table.
Reported per this loop's own stop condition rather than chased further
(`tests/fixtures/slippi/parity/fox-fd-4-baseline.json`'s own note has the
full citation).

**Previously (2026-09-11, gameplay export v6, before the input-lock
`previous_input` fix):** 84 frames matched (-123 through -40) and the first
divergent frame was -39, field `position.y` (expected `5.2149`, actual
`3.1949`, for P4). Fixed: `docs/input-lock.md`'s "open question" -- P4 in
this recording holds its stick down continuously from before the pre-"GO"
input lock through its unlock frame (-39), and the lock's original
neutral-`previous_input` implementation made that continuously-held input
read as a fresh press exactly at unlock, wrongly edge-triggering fast-fall
one frame before the recording's own ordinary gravity-only fall.
`game::simulation::advance` now keeps `previous_input` tracking the real,
un-neutralized samples throughout the lock (only dispatch sees the neutral
controller); see `docs/input-lock.md` for the full diagnosis and citation.

## A first Falco recording: `falco-fox-fd.slp`

The first recording to carry a Falco fighter,
`tests/fixtures/slippi/parity/falco-fox-fd.slp` (`12_45_21 Falco + [HAMB]
Fox (FD).slp`, same CC0-1.0 `erickfm/slippi-public-dataset-v3.7` corpus,
`batch_00`, Slippi 2.0.1, ports P3/P4, Falco first), is ratcheted by
`crates/cli/tests/real_parity_falco_fox_fd.rs` against its own baseline,
`tests/fixtures/slippi/parity/falco-fox-fd-baseline.json`, following
`fox-fd-4.slp`'s own precedent above (a standalone file, not folded into
`real_parity.rs`, until the shared recordings list lands). It uses the
`falco-fox-fd` pairing's own export (`/mnt/archive/datasets/melee/
skirmish-gameplay/v2`), the first to carry a `fighters/falco.json` pack, and
exercises Falco's own registration (`game::characters::Specials::Falco`,
`docs/falco.md`) end to end: `make-initialization` accepting that pack, and
Falco's external Slippi id (20) resolving through the shared Fox move table.

**Current measurement (2026-09-12, gameplay export v2, the real-replay
parity loop's Walk entry-time fix, measured directly against the live pack
-- this pairing has no `SKIRMISH_GAMEPLAY_DATA` snapshot yet):** 98 frames
match (-123 through -26) and the first divergent frame is -25, field
`position.x` (expected `0xc26121ec` = `-56.28312683105469`, actual
`0xc26121eb` = `-56.28312301635742`, a 1-ULP difference, for P3/Falco).

**Diagnosis (the fused-op hypothesis is now checked and ruled out; still a
suspected cross-platform floating-point limitation, not a Skirmish logic
bug -- reported per this loop's own stop condition):** P3 (Falco) enters
Dash at frame -27 and is still in Dash at -25 (`action_age` 3.0, matching);
the diverging field is `position.x` alone, moved by `ftCo_Dash_Phys`'s
ordinary friction/acceleration step (`getAccelAndTarget`/
`ftCommon_8007C98C`, `fighter::locomotion::accelerate`/
`Movement::accelerate_ground`). Tracing the exact bit patterns the native
simulation itself produces (a temporary debug trace, not committed) through
frames -27..-25 and replaying each step against
`tests/physics_differential.rs`'s existing c-oracle harness (a temporary,
uncommitted probe, not a new permanent test) shows every step already
agrees with the host-compiled decomp C bit-for-bit on these exact inputs:
the entry's own initial-velocity delta, frame -26's `accelerate_ground`
call (`gr_vel=1.9` in, `0x3fe8f5c2` out, host C agrees), and frame -25's own
call (`gr_vel=0x3fe8f5c2` in, host C agrees with Rust's result too). This is
the same class of divergence as `fox-fd-3.slp`'s own frame -32
`velocities.self_x_air` case below, and both were flagged for the
`skirmish-fma` batch's fused-multiply-add audit
(`tools/ppc_fma_audit.py`) to check against the retail binary's own
disassembly. That audit is now complete for this exact chain, and finds no
fused op: `ftCo_Dash_Phys` (`.text:0x800CA53C`, which inlines
`getAccelAndTarget` and calls `ftCommon_8007C98C` and
`ftCommon_ApplyGroundMovement`) disassembles to a plain `fmuls` (`stick *
dash_accel_mul`) followed by a plain `fadds` (`+= dash_accel_base`), not a
single `fmadds`; `ftCommon_8007C98C`, `ftCommon_ApplyGroundMovement` and
`ftCommon_ApplyGroundMovementNoSlide` themselves also disassemble with zero
fused ops (`docs/math.md`'s per-function findings). The one-ULP gap against
the real recording is confirmed *not* explained by a fused product+sum
rounding once instead of twice on this chain -- both the retail PowerPC
binary and this Rust port compute the same separately-rounded multiply
then add. The actual mechanism behind the recording's own one-ULP-lower
result remains unexplained; not chased further in this batch (`AGENTS.md`:
"Host C agreement does not establish PowerPC or whole-game equivalence").

**Update (`skirmish-f64` batch):** the double-precision-intermediate
hypothesis is also ruled out for this identical, fighter-generic Dash
chain by direct disassembly (`tools/ppc_precision_audit.py`,
`docs/math.md`): zero double-precision arithmetic instructions in any of
`ftCo_Dash_Phys`, `ftCommon_8007C98C`, `ftCommon_ApplyGroundMovement`
(`NoSlide`), or `ftCommon_ApplyFrictionGround`. Falco's own lower
`dash_max_velocity` puts frame -25 through the clamp branch (unlike Fox's
frame -32 above), so the exhaustive per-input rounding-model sweep run for
Fox's case was not independently repeated here; the same instructions
compute it regardless of which branch is taken. No baseline change
(`falco-fox-fd.slp` unchanged at -25/98).

Previously (2026-09-12, gameplay export v2, the Falco registration batch):
93 frames matched (-123 through -31, the pre-game Entry warp-in) and the
first divergent frame was -30, field `action_age` (expected `1.0`, actual
`0.0`, for P4/Fox). Fixed: `game::locomotion::enter_walk` (`ftCo_
Walk_Enter`/`ftWalkCommon_800DFCA4`, `ftwalkcommon.c:71-92`) makes the same
extra, explicit `ftAnim_8006EBA4` call `ftCo_Dash_Enter`/`ftCo_Turn_Enter`
already modeled (`game::locomotion::start_dash`/`start_turn`, the Dash/Turn
entry-time consolidation above), so Walk's own tracked animation frame is
`start_frame + 1.0`, not `start_frame`, on every entry -- both a fresh
Wait/tilt -> Walk transition and a mid-walk kind retype, since
`ftWalkCommon_800DFEC8`'s own re-entry routes through the identical
`ftCo_Walk_Enter` call. Confirmed directly against this recording (P4/Fox's
Landing->Walk transition at frame -30, the actual divergence here, now
reporting the recording's own `state_age = 1.0`) and against `fox-fd.slp`
(11 further Walk entries from Wait/Turn/RunBrake/Landing/Squat, all already
`1.0` on their own entry frame). The new divergence at -25 is a separate,
unrelated matter (above), reported rather than chased in this batch.

## A first Battlefield recording: `fox-bf.slp`

The first recording on a stage other than Final Destination,
`tests/fixtures/slippi/parity/fox-bf.slp` (`18_21_03 Fox + Fox (BF).slp`,
same CC0-1.0 `erickfm/slippi-public-dataset-v3.7` corpus, `batch_00`,
Slippi 2.0.1, ports P1/P4), is folded into the shared
`tests/fixtures/slippi/parity/recordings.json` ratchet (its own
`fox-bf-baseline.json`) under a new `fox-bf` pairing (`/mnt/archive/
datasets/melee/skirmish-gameplay/v2/fox-bf`), the first Battlefield
`match-data.json` export: platforms at world y 27.2 (side) and 54.4 (top),
main platform edges at +/-68.4, and both players' two-player spawns at
(+/-38.8, 35.2). This pairing exercises platform landing/drop-through
(`ftCo_Pass`) and platform-adjacent ledge/teeter logic Final Destination
never did.

**Initial measurement (2026-09-12, gameplay export v2, measured directly
against the live pack -- this pairing has no `SKIRMISH_GAMEPLAY_DATA`
snapshot yet):** already benefiting from origin/main's own Walk entry-time
fix (`game::locomotion::enter_walk`'s `start_frame + 1.0`, the same batch
this loop independently rediscovered on this exact recording -- P4's own
Landing->Walk transition at frame -31 -- before finding it already fixed
upstream), 95 frames matched (-123 through -29) and the first divergent
frame was -28, field `action_age` on P4 (expected `0xbf800000` = `-1.0`,
actual `0x00000000` = `0.0`), on P4's own one-frame `GuardOn` shield-drop
entry (immediately into `Pass` the next frame).

**Fixed: GuardOn/Guard/GuardReflect's own constant `-1` `state_age`.**
`ftCo_800923B4`/`ftCo_80092C54` (GuardOn/Guard's own entries,
`ftCo_Guard.c:386,509,790,1012`) and `ftCo_8009388C` (GuardReflect, `:901`)
all call `Fighter_ChangeMotionState` with `Ft_MF_SkipAnim`.
`Fighter_ChangeMotionState` unconditionally lands `cur_anim_frame` on
`anim_start - anim_speed` (`fighter.c:1224`) for every transition --
`0 - 1 = -1` here -- but `Ft_MF_SkipAnim` additionally skips the generic
per-frame animation advance (`Fighter_Spaghetti_8006AD10`'s unconditional
`ftAnim_8006EBA4(gobj)`, `fighter.c:1684`) that brings every *other*
action's `cur_anim_frame` back to `0` by the end of its own entry frame:
the shield family owns no scripted animation figatree for that advance to
move, so `state_age` stays a constant `-1` for the entire state, not just
its entry frame -- unlike `GuardSetOff` (`Ft_MF_None`, no `SkipAnim`),
which keeps the ordinary rule. `crates/skirmish-replay/src/observation.rs`'s
`action_age` now reports a constant `-1.0` for `GuardOn`/`Guard`/
`GuardReflect`, joining the existing `Entry`/`EntryEnd` constant-age
branch as its own arm. Confirmed directly against `fox-bf.slp`: P1 holds
`GuardOn` then `Guard` for 21 frames (1430-1450) with `state_age = -1.0`
throughout, then `GuardSetOff` at 1451 already reports the ordinary `0.0`;
P4's own one-frame shield-drop at -28 (the divergence this fixes) reports
the same `-1.0` on its only frame. 96 frames now match (-123 through -28).

**Current measurement:** the new first divergent frame is -27, field
`position.y` on P4 (expected `0x41d3c2c4` = `26.47010040283203`, actual
`0x41be669b` = `23.800100326538086`), on the same frame P4's `GuardOn`
converts into `Pass` (falling through the platform).

**Diagnosis (blocked on a gameplay-export pack-data gap, reported per this
loop's own stop condition): `ftCommonData`'s fast-fall stick-timer window
constant is not exported.** `ftCo_Pass_Phys`/`ft_80084DB0` calls
`ftCommon_CheckFallFast` (`ftcommon.c:492-503`) every frame while airborne,
the same generic check every other falling action's own Phys callback
makes; its exact condition is `!fp->fall_fast && fp->self_vel.y < 0 &&
fp->input.lstick[0].y <= -p_ftCommonData->x88 (fast_fall_threshold,
already exported and modeled) && fp->x671_timer_lstick_tilt_y <
p_ftCommonData->x8C` -- an `int` field immediately after
`fast_fall_threshold` in `ftCommonData` (`types.h:88-89`) that is not yet
given a name in the pinned decomp and is not present anywhere in
`fox-bf/match-data.json`'s `rules` object (confirmed: `fast_fall_threshold`
is exported; no sibling window field is). `fp->x671_timer_lstick_tilt_y`
(`fighter.c:1976-2019`, already modeled at the source as
`fighter.locomotion.tilt_y_age`, reset to the same `0xFE`/`254` sentinel by
`game::locomotion::pass_request_after_actions` on every platform pass,
`wall_jump::enter` and elsewhere) counts consecutive frames the stick has
held past the vertical smash deadzone in one direction, restarting at `0`
on a fresh press; `ftCo_8009A184`/`ftCo_8009A228` (`begin_pass`'s own
source) reset it to `0xFE` specifically so the same down-hold that
triggered a platform pass cannot also immediately re-trigger fast-fall.
Traced directly against `fox-bf.slp`: P4's stick crosses the down deadzone
at frame -28 (neutral the frame before) and is still down at -27, when
`Pass` begins and `game::locomotion::pass_request_after_actions` already
resets `tilt_y_age` to `254`; `game::simulation::move_fighter`'s existing
fast-fall trigger, though, does not consult `tilt_y_age` at all --
it approximates decomp's timer-window check with `f.previous_input.
stick[1] > -rules.fast_fall_threshold` (a heuristic already known to be an
approximation, `docs/input-lock.md`/this file's own `fox-fd-4.slp` entry
above), which reads the *previous* frame's stick, sees it already past the
threshold too (-28's own `-0.637` past `-0.6625`), and fires anyway --
producing an extra frame of `fast_fall_velocity` (`-3.4`) instead of the
recording's own `pass_velocity + gravity` (`-0.5 + -0.23 = -0.73`).
Replacing the heuristic with the real mechanism only needs `tilt_y_age`
(already tracked) to be compared against the true window constant, which
is not exported: guessing a value is not safe here, since a too-small or
too-large window could silently change fast-fall timing on any other
continuously-held-stick transition into an airborne action elsewhere in
the corpus, not just this one. Reported per this loop's own stop condition
(pack data needed: `ftCommonData+0x8C`, adjacent to the already-exported
`fast_fall_threshold` at `+0x88`) rather than fixed with an unverified
constant.

**Fixed: the fast-fall window is now modeled exactly, gated on an optional
pack field.** `ftCommonData+0x8C` was read directly from the retail disc
(`PlCo.dat`, the same `ftLoadCommonData[0]` pointer `skirmish-assets`'s own
`gameplay::common::decode` already resolves `fast_fall_threshold` through):
a plain `int`, value `4`, matching its already-named integer neighbors
`dash_smash_window` (`+0x40` = `2`) and `tap_jump_window` (`+0x74` = `4`) in
both type and magnitude -- not the denormalized garbage a naive blanket
`f32` reinterpretation of that word would show. `game::data::Rules` gains
`fast_fall_window: Option<u32>`; `fighter::damage::fast_fall_trigger` now
ports `ftCommon_CheckFallFast` exactly (`!fast_fall && velocity_y < 0.0 &&
stick_y <= -threshold && tilt_y_age < window`) and `game::simulation::
move_fighter` calls it when the pack supplies `fast_fall_window`, falling
back to the previous `previous_input`-edge heuristic when it is absent (so
every existing fixture, none of which export this field yet, is
unaffected). Confirmed against `fox-bf.slp` directly: frame -21 (P1) is an
ordinary fresh down-press one frame after crossing the smash deadzone
(`tilt_y_age = 1`), and the recording shows fast-fall triggering exactly
there -- `1 < 4` -- agreeing with both the new check and the old heuristic
alike (which is why the old approximation had never visibly failed before
this recording's own platform-pass case); frame -27 (P4, the divergence
above) is the case they disagree on, `tilt_y_age = 254 !< 4` correctly
blocking it.

Confirmed end-to-end with a throwaway diagnostic copy of `fox-bf/
match-data.json` patched to add `"fast_fall_window": 4` (not committed,
deleted after use, the same kind of diagnostic patch `friction_above_walk`
used above): `position.y` now matches at frame -27 (96 frames measured
against the *unpatched* live pack still stop at -27 on `position.y`,
since `fast_fall_window` isn't exported there yet and the fallback
heuristic is unchanged -- confirmed by re-measuring against the unpatched
pack after this fix, byte-identical to before). Against the diagnostic
copy, the new first divergence is frame -27 itself, a different field:
`shield` on P4 (expected `59.76071548461914`, actual `59.790000915527344`).
Ground truth shows P4's shield health already at `60.0` through frame -28
(`GuardOn`'s only frame) and already reduced to `59.7607` at -27, the same
frame `Guard` converts into `Pass` -- decomp's own shield-decay Phys
callback, like the already-diagnosed Dash/Turn/Walk/jump-direction class
of bugs above, evidently still runs once more for the *outgoing* Guard
action on its own last frame before this frame's `IASA` converts it to
`Pass`, and by a slightly different amount (`0.2393` observed here) than
whatever rate Skirmish's own shield decay currently applies (`0.21`). Not
diagnosed further in this batch: this is a new, separate root cause (per-
frame shield-health decay timing/rate, not fast-fall), reported rather
than chased, and does not move `fox-bf-baseline.json` (which tracks the
*unpatched*, currently-published pack and is therefore still `96`/`-27`,
unchanged by this fix) -- it will move once the exporter publishes
`fast_fall_window` for real, without any further Skirmish code change.

## Practical consequence

None of these three, individually or together, is "Skirmish matches Melee."
Level 1 rules out a class of arithmetic bugs in ported functions. Level 2
guards the test harness against regressing on itself. Level 3 is the only one
that touches an independent recording, and even it is scoped to one matchup,
one stage, and a fixed field set. Treat a "matched" `real_parity` result as
"no evidence of disagreement was found in what was checked," not as a
certification.
