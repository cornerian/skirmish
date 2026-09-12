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
recording's own hardware produced a result one ULP lower. Chasing this
further would mean reproducing the original PowerPC compiler's exact
instruction selection for this expression (a known class of Gekko/Broadway
divergence: paired-single multiply-add instructions round the fused
product+sum once rather than twice, unlike separately-rounded scalar
`f32` ops), which is outside what a decomp-ported Rust function can
express -- exactly the limitation `AGENTS.md` already names ("Host C
agreement does not establish PowerPC or whole-game equivalence").

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

**Diagnosis (pending the fused-op audit, not a Skirmish logic bug --
reported per this loop's own stop condition):** P3 (Falco) enters Dash at
frame -27 and is still in Dash at -25 (`action_age` 3.0, matching); the
diverging field is `position.x` alone, moved by `ftCo_Dash_Phys`'s ordinary
friction/acceleration step (`getAccelAndTarget`/`ftCommon_8007C98C`,
`fighter::locomotion::accelerate`/`Movement::accelerate_ground`). Tracing
the exact bit patterns the native simulation itself produces (a temporary
debug trace, not committed) through frames -27..-25 and replaying each step
against `tests/physics_differential.rs`'s existing c-oracle harness (a
temporary, uncommitted probe, not a new permanent test) shows every step
already agrees with the host-compiled decomp C bit-for-bit on these exact
inputs: the entry's own initial-velocity delta, frame -26's
`accelerate_ground` call (`gr_vel=1.9` in, `0x3fe8f5c2` out, host C agrees),
and frame -25's own call (`gr_vel=0x3fe8f5c2` in, host C agrees with Rust's
result too). This is the same class of divergence as `fox-fd-3.slp`'s own
frame -32 `velocities.self_x_air` case (also a one-ULP difference on a
Dash frame's `getAccelAndTarget`/`ftCommon_8007C98C`/`accelerate_ground`
chain, also verified bit-exact against host-compiled C): a sibling batch
(`skirmish-fma`) is checking the retail binary's own disassembly for a
fused multiply-add in `ftCommon_8007C98C`/the ground-acceleration chain,
which would explain a systematic one-ULP gap between host-compiled
(separately-rounded) C and the original PowerPC Gekko/Broadway FPU
(which can round a fused product+sum once instead of twice) without any
Skirmish translation bug. Pending that audit's result, not chased further
in this batch.

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

## Practical consequence

None of these three, individually or together, is "Skirmish matches Melee."
Level 1 rules out a class of arithmetic bugs in ported functions. Level 2
guards the test harness against regressing on itself. Level 3 is the only one
that touches an independent recording, and even it is scoped to one matchup,
one stage, and a fixed field set. Treat a "matched" `real_parity` result as
"no evidence of disagreement was found in what was checked," not as a
certification.
