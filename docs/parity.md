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

**`fox-fd-2.slp` (2026-09-13, published pack v10): moved again; a
downstream bug fixed, the root cause still open.** Re-measured directly
against pack v10 (`docs/parity.md`'s own "Published pack v10" note above;
script command variables embedded), superseding the stale `v8` baseline
(114 frames, first divergence -9, never diagnosed in this file): 118
frames now match (-123 through -6). The divergence is at -5, P1's
`last_attack_landed` (expected `0x12`/18, Melee's Blaster attack id,
actual `0x00`). `validate-replay`'s own report stops at this field (the
first mismatch in port-then-field order, P1 checked before P2), which
reads as "the hit never lands or is never recorded." A direct,
uncommitted probe stepping the native match frame-by-frame (bypassing the
CLI's stop-on-first-difference report to dump both ports' full
observations) shows the truer shape: P2's own `percent` also mismatches at
-5 (expected `3.0`, actual `0.0`), but by -4 both `last_attack_landed`
(`18`) and `percent` (`3.0`) already match the recording -- a plain
one-frame lag in when Skirmish's laser bolt registers its hit, not a
missing hit.

Two hypotheses were checked directly and ruled out. First, that the shot
is queued to fire one frame late (a script/`cmd_vars` timing bug in
`characters::fox::neutral`): the same probe shows P1's own `action_state`/
`action_age` match the recording bit-exactly on every single frame up to
and including -5 itself, so the Loop's own script-driven fire trigger
already fires on exactly the right frame. Second, that this is the same
shape as the Falco laser batch's own terrain-despawn finding (a laser that
hits terrain gets one extra frame of life before `it/item.c` actually
despawns it, `docs/falco.md`): reading the pinned decomp's own item-vs-
fighter hurtbox path directly (`ftColl_8007925C`, `ftcoll.c:1999-2270` --
the victim fighter's own priority-13 scan of every live item's hitboxes
against its own hurtboxes) shows detection and application happen on the
same GObj-priority pass, not deferred to the next frame the way the
terrain case is; that function's own hurtbox test is also not swept/
continuous at all (a single current-frame capsule check), ruling out both
an intra-frame detect-vs-apply ordering fix and a swept-vs-static test-
shape fix. Measuring the actual geometry directly at frame -5 confirms
this is not a hair's-breadth miss either: the projectile's own swept
capsule reaches only to world x = 51.2, while P2's nearest hurtbox capsule
sits around x = 57.1-57.6 -- roughly 3.4 world units short of connecting
even after accounting for both capsules' radii, not the sub-unit gap a
one-frame ordering difference alone would produce. The remaining
explanation is a genuine difference in either the victim's own hurtbox
pose/position or the projectile's own reach at this exact frame, not an
ordering or apply-timing bug; not resolved by this loop despite locating
and reading the decomp collision path, reported per this loop's own stop
condition rather than guessed at further.

A real, separate bug was found and fixed along the way
(`docs/validation.md`'s projectile-owner-hitlag entry): once the (still
one-frame-late) hit lands in Skirmish's own simulation, P1's own
`action_age` was found frozen from that frame on (expected continuing
0, 1, 2...; actual stuck at 0) instead of tracking the recording, because
`game::damage::apply_hit` gave the projectile's *owner* attacker-hitlag
from its own shot connecting -- decomp's own item-vs-fighter hit path
(`ftColl_8007925C` above) never touches the item's owner `Fighter_GObj` at
all, unlike a direct fighter-vs-fighter hitbox contact
(`ftColl_800763C0`/`Fighter_ProcessHit_8006D1EC`), which gives both
fighters hitlag from the same contact. Fixed at the source: `apply_hit`
gains an explicit `attacker_takes_hitlag: bool` parameter, `true` from the
two existing direct-contact callers (`game::simulation`'s fighter-vs-
fighter hits, `game::grab`'s throws -- unchanged, no evidence either is
wrong) and `false` from `game::projectile.rs`'s own call. Confirmed
directly: P1's own `action_age` now matches the recording exactly from the
hit onward. This does not move `checked_frames`/`first_divergent_frame`
here, since the earlier frame -4-vs--5 divergence is still open, but it is
a genuine fix in its own right, pinned by a new assertion on
`tests/game_fox_neutral_special.rs`'s existing
`the_laser_travels_before_hitting_and_despawns_on_contact`.

`fox-fd-4.slp`'s own v10 re-measurement below was previously reported as
hitting a reserved Blaster subsystem; that reservation turned out to be
broader than intended (the concurrent Falco batch owns Falco's own data
path and item-kind differences, not the shared hit pipeline or Loop
timing), and this loop's own fix above already reaches into
`game::damage.rs`/`game::projectile.rs`. See its own entry below for
whether the same root cause applies there too.

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

**Update (the real-replay parity loop, picking up the lead above): tested
directly, and falsified.** The `skirmish-f64` batch's own perturbation
sweep pointed at `ftCo_Dash_Enter`/`ftCommon_800804A0` -- the Dash-entry
velocity computation itself -- as the source of the two-ULP-lower incoming
`ground_velocity` the recording implies. This dash is entered from a dead
stop (`ground_velocity = 0.0`, confirmed exact by three static frames of
`position.x` beforehand), so `ftCo_Dash_Enter`'s entry computation
(`ftCo_Dash.c:61-66`) reduces to `mv.co.dash.x0 = facing_dir *
dash_initial_velocity` exactly (the `- gr_vel` term subtracts an exact
`0.0`, a lossless no-op) -- meaning the lead's own two-ULP-lower value
*is* `dash_initial_velocity` itself, `0x3ff33331` (`1.899999737739563`)
rather than the pack's `0x3ff33333` (`1.899999976158142`), if the lead
holds. Tested with a throwaway, uncommitted copy of both `fox-fd`'s and
`falco-fox-fd`'s `match-data.json` (deleted after use) with every
`dash_initial_velocity` field lowered by exactly those two ULP, re-measured
against both recordings with `make-initialization`/`validate-replay`.

This falsifies the hypothesis rather than confirming it. **Takeaway: the
recording's own entry-frame value confirms `dash_initial_velocity` is
already correct, and no single constant in this chain explains the
recording's one-ULP-lower result.** `fox-fd-3.slp`
diverges *earlier* against the patched pack (`checked_frames` 91 -> 90,
`first_divergent_frame` -32 -> -33) on a *new* field at the Dash entry
frame itself, `velocities.self_x_ground`, expected `0x3ff33333` (the
pack's original, unpatched value) actual `0x3ff33331` (the patch) -- the
recording's own entry-frame `ground_velocity` independently confirms the
pack's existing `dash_initial_velocity` is already exactly right, not two
ULP high; the two-ULP gap the sweep found is real but does not originate
at this specific read. Solving the *opposite* direction -- what `accel`
value (not `gr_vel`) would reproduce `0x400147ad` given the
now-confirmed-correct `gr_vel = 0x3ff33333` -- needs `accel = 0x3df5c270`
(`0.11999976634979248`) against the computed `0x3df5c290`
(`0.12000000476837158`, from `stick * dash_acceleration_mul +
dash_acceleration_base`, unchanged): a 32-ULP gap, far too large for a
single nearby constant's own export precision (`dash_acceleration_mul` or
`dash_acceleration_base` swept independently by several ULP each stays
pinned to the same `0x400147ae` result, `accel`'s own magnitude being too
coarse at this scale to move the sum's rounding by a mere ULP or two on
either input alone). Neither `dash_initial_velocity` nor an accompanying
accel constant is the explanation; the actual mechanism behind the
recording's own one-ULP-lower result remains unexplained. Not chased
further this batch: the sweep's own two-ULP figure is real (some earlier
value in the chain does need to be that much lower to reproduce the
recording) but does not localize to any single constant this loop could
find by direct substitution (`AGENTS.md`: "Host C agreement does not
establish PowerPC or whole-game equivalence").

**Update (stick-value hypothesis, tested directly, and falsified): the
recorded stick input and Skirmish's own `Controller.stick` are bit-identical;
this is not an input-conversion bug.** The remaining lead from the two
updates above is a two-ULP-lower *something* upstream of `getAccelAndTarget`'s
own evaluation that no constant substitution reproduces; one candidate never
directly tested until now is the stick value itself -- `fp->input.lstick[0].x`
after the game's own pad conversion (deadzone + division by the calibrated
max magnitude, `Fighter_8006A1BC`, `fighter.c` ~1750-1900) versus whatever
`crates/skirmish-replay`'s `observation::controllers` feeds in from the
replay. Checked both ends for `fox-fd-3.slp` (P2, frame -32) and
`falco-fox-fd.slp` (P3/Falco, frame -25):

- `uv run --with py-slippi python3` dumping `pre.joystick.x`/`raw_analog_x`
  directly from both `.slp` files: P2 holds `raw_analog_x = 80`,
  `joystick.x = 1.0` (`0x3f800000`) for frames -33 through -30; P3/Falco holds
  `raw_analog_x = 103`, `joystick.x = 0.9375` (`0x3f700000`, a clean
  power-of-two fraction consistent with the octagonal gate's notch
  quantization) for frames -26 through -24.
- `observation::controllers` (`crates/skirmish-replay/src/observation.rs:187`)
  sets `stick = [pre.joystick.x, pre.joystick.y]` verbatim from Peppi's own
  parsed `Pre` row -- no clamping, quantization, deadzone reapplication or
  `as`-cast rounding of any kind before it reaches `game::Controller`.
- Confirmed live, not just read from source: a temporary `eprintln!` in
  `game::locomotion::ground_motion` (removed before this commit), rebuilt and
  run through `make-initialization`/`validate-replay` against both
  recordings, printed `stick.x_bits=0x3f800000` at Fox's frame -32 and
  `stick.x_bits=0x3f700000` at Falco's frame -25 -- bit-for-bit identical to
  the recordings' own `pre.joystick.x`, with `ground_velocity` bits
  (`0x3ff33333`, `0x3fe8f5c2`) matching this doc's and `docs/math.md`'s
  existing figures exactly.
- Every plausible alternative processing of the recorded raw byte converges
  on the same bits anyway, so there is no room for an ULP-level stick error
  to hide in: Fox's `raw_analog_x = 80` is already the calibrated full-scale
  magnitude (any linear normalization against that same magnitude, or a
  full-deflection clamp, yields exactly `1.0`); quantizing `1.0` to the
  nearest 1/80th is still exactly `1.0`. Re-running the exact `f32` accel
  step (`stick * dash_acceleration_mul + dash_acceleration_base`, then
  `ground_velocity + accel`) against the confirmed bits reproduces
  `0x400147ae` again, not the recording's `0x400147ad` -- the same one-ULP
  gap this doc already found by other means, now with the stick input itself
  eliminated as a variable rather than merely assumed correct.

**Takeaway: the stick input is not the source of the remaining one-ULP
mismatch.** Both the recorded joystick float and the value Skirmish's
replay stepper actually uses for the dash-acceleration computation are the
same bits, confirmed at the source (Peppi's parsed pre-frame row), in the
replay-to-`Controller` conversion (by inspection, no transform exists to
inspect), and live (instrumented and rerun). The two-ULP-lower value the
`skirmish-f64` batch's perturbation sweep implies still has no known origin;
it is not the stick, and (per the update above) not `dash_initial_velocity`
or a nearby accel constant either. Not chased further this batch: no code
or baseline change (`fox-fd-3.slp` unchanged at -32/91; `falco-fox-fd.slp`
unchanged at -25/98).

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

**Published pack v15 (2026-09-16):** sha256 c2cc0546, 36 MB, supersedes v13
(v14, sha256 ddbb21ec, was published to `cornerian/skirmish-datapacks` but
never pinned in this repo, so this entry's own "supersedes" chain skips
straight from v13 to v15 and covers both packs' content). v14 adds
`blend_frames`/`dynamics_variant` to every attack profile (jab included,
even where both are `0`) and populates a `blend` sidecar on
`movement_poses`/`damage_poses` per sub-motion (Fox `wait`/`walk_*` blend
6 frames, `dash` blends 0, matching the real runtime's own pose-blend
window). v15 adds a top-level `model_scaling: f32` per fighter (`fighters/
<name>.json` and every pairing's `match-data.json`; Fox `0.96` = `0x3F75
C28F`, Falco `1.1` = `0x3F8CCCCD`, bit-exact, `melee/ft/ftcommon.c:1407`'s
`ftCommon_GetModelScale`) -- Skirmish evaluated every Fox bone 4% too large
before the loader read this field, since the pack carried no model scale at
all. Both packs' stage data is unchanged from v13's (byte-identical,
confirmed at export time).

Landing these fields on the Skirmish side (this chain: root-facing rotation
by matrix instead of mirroring, `HSD_JObjMakeMatrix`-pinned C oracle for
`bones::Pose`, per-subaction pose blending, pre-physics laser muzzle
spawn position, and `model_scaling` applied to the root bone's scale)
corrects two compounding bugs at once: hurtboxes were oriented as if the
fighter were mirrored into depth rather than rotated by facing, and every
bone (hurtboxes included) was evaluated 4% too large. Re-measured every
`recordings.json` entry plus `falco-fox-fd` against pack v15 on this chain:
`fox-fd` 128, `fox-fd-3` 91, `fox-ys` 10, `fox-fod` 5, `fox-dl` 74 (left
alone), `fox-ps` 117, `falco-fox-fd` 98 are unchanged from their pinned
baselines. Three recordings move:

`fox-bf` and `fox-fd-4` both raise sharply (174->148 checked_frames written
as **148**, and 118->109 written as **109** -- both correctly *lower*
frame counts than their old baselines, because the old baselines were
measured with the mirrored-hurtbox/oversized-bone bugs still present and
are no longer reachable outcomes once those bugs are fixed; see each
baseline file's own 2026-09-16 note). `fox-bf`'s new first divergence
(frame 25, P1's `percent` 0.0 -> 3.0) is a real Blaster hit landing one
frame before the recording shows it, by a sub-0.4-unit hurtbox-contact
margin; `fox-fd-4`'s (frame -14, P2's own `last_attack_landed`) is the same
class of one-frame-early laser contact. Both are genuine, small residuals
in newly-correct geometry, not the old bugs recurring -- open, not chased
further here; see `docs/validation.md`'s 2026-09-16 entries for the
model-scale/muzzle investigation this residual traces to.

`fox-fd-2` improves, 118 -> **127** checked_frames, frame -5 -> **+4** (a
`percent` mismatch), from the same runtime pose-blending this pack's own
v14 content unlocked (`docs/parity.md`'s v14 gap notwithstanding, the
blend fields were already live in this chain before v14 published).

**Published pack v13 (2026-09-14):** sha256 71f1eb79, 36 MB, supersedes v12
(published to `cornerian/skirmish-datapacks` but never pinned in this repo --
its own two-line-restriction fix, `skirmish-assets` commit `820e440`, is
superseded by this pack before a pin commit for it landed here, so this
entry's own "supersedes" chain skips straight from v11 to v13). Pokemon
Stadium's export is fixed properly: all 136 of GrPs.dat's collision lines
across all 8 `MapJoint`s are exported at their original disc indices (112
after the pre-existing stage-wide dynamic-range drop) instead of being
renumbered or dropped, and each joint now carries a boolean
`enabled_at_start` (true only for joints 4 and 6, the neutral main floor,
its two side platforms, and the stadium's permanent walls/ceiling/ledges --
see `POKEMON_STADIUM_NEUTRAL_JOINTS`'s doc comment) recording which lines
the real game keeps enabled at match start, rather than compacting the
enabled set into a fresh 0-based numbering (pack v12's own approach, which
broke correspondence with the real game's `last_ground_id` values) or
stacking every form's lines together unmarked (pre-v12). The Skirmish loader
reads `enabled_at_start` as of commit `e38eac3` (default `true` when the
field is absent, so every earlier pack still loads unchanged). The other
five stages' `stages/*.json` are byte-identical to v11's once each joint's
`enabled_at_start` is stripped, and every pairing's fighter sections are
identical to v11's.

Re-measured every `recordings.json` entry plus `falco-fox-fd` against this
pack, pin worktree on Skirmish origin/main `e38eac3`. Eight of ten are
unchanged from their pinned baseline: `fox-fd` 5/128, `fox-fd-2` -5/118,
`fox-fd-3` -32/91, `fox-bf` 174/297 (already moved forward by upstream
fixes since pack v11's own pin), `fox-ys` -113/10, `fox-fod` -118/5,
`fox-dl` -49/74 (left alone, owned elsewhere), `falco-fox-fd` -25/98.

`fox-ps` improves sharply, from 85/-38 to **117/-6**: both the stacked-
collision landing gap (pack v10 and earlier) and the ground-id renumbering
artifact (pack v12) are gone. The new first divergence, frame -6 position.x
on P1 (expected `0xc288d94c`, actual `0xc288d94d`), is a single-ULP
floating-point gap -- baseline updated to match (`fox-ps-baseline.json`).

`fox-fd-4` regresses, from -5/118 to -13/110 (`last_attack_landed` mismatch,
expected `0x12` actual `0x00` -- the laser-hit-registration gap): caused by
`937486e` ("Revert the laser muzzle-bone offset's axis swap; it contradicts
decomp"), a code revert upstream of this pin, unrelated to the Pokemon
Stadium collision fix this pack carries (this pairing's own fighter/stage
data is confirmed byte-identical to v11's). Reported, not chased here (out
of this pin's own scope); per the no-lower-baseline rule,
`fox-fd-4-baseline.json` is not touched.

**Published pack v11 (2026-09-14):** sha256 aaa657fd, 36 MB, supersedes v10:
`specials.neutral.laser.{scale,muzzle_bone}` are embedded for Fox and Falco
(scale 3.0 for both; muzzle_bone 67 for Fox, 61 for Falco), the geometry the
2026-09-14 laser muzzle-bone/ray-scale fix (`docs/validation.md`) needs but
that pack v10 did not yet carry. The exporter's map_head point composition
also switched from reading a point joint's raw local translation to a full
FK ancestor-chain evaluator (`skirmish-assets` commit `0ceac40`); every one
of the six tournament stages' `stages/*.json` is byte-identical to v2's, so
this is confirmed numerically inert for the current dataset, not a data
change.

Re-measuring every `recordings.json` entry plus `falco-fox-fd` against this
pack: seven of ten are unchanged from their pinned baseline (`fox-fd` 5/128,
`fox-fd-3` -32/91, `fox-bf` 49/172, `fox-ys` -113/10, `fox-fod` -118/5,
`fox-dl` -123/0, `falco-fox-fd` -25/98). `fox-fd-2` (-5/118 baseline) and
`fox-fd-4` (-5/118 baseline) both regress, to -11/112 and -13/110
respectively -- the same Turn/Dash `action_state` mismatch (owned by the
Battlefield loop, `36a5a36`) commit `46bcb87`'s own full sweep already
found and reported for `fox-fd-2` against pack v10, upstream of where the
laser fix's own effect on these two recordings would be observed. Not
caused by this pack (the stage/laser changes above have no bearing on
Turn/Dash), not chased further here (out of this pin's own scope, already
tracked), and not masked: neither baseline is lowered, per this project's
own no-lower-baseline rule.

**Published pack v10 (2026-09-13):** sha256 e4189b85, 36 MB, supersedes v9:
the specials' per-frame script command variables (`specials.neutral.script`,
`specials.side.script`) are embedded for Fox and Falco, so the Blaster's
arming and fire timing come from the script (fox-fd-2 reaches 118 frames
with it).

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

**2026-09-14 update:** this recording's own frame-`-14` Blaster-one-
frame-early divergence (see below) was directly tested against the
working hypothesis that the victim's `LandingFallSpecial` pose-sample
index was off by one; that hypothesis is falsified (`docs/
validation.md`'s own 2026-09-14 pose-sample-index investigation entry has
the full method, including that the real triggering hurtbox on this
frame is bone 22, not bone 41, and an exhaustive index-offset sweep from
`-15` to `+5` that never avoids the frame-`-14` registration). No
baseline change; not chased further.

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

**Current measurement (2026-09-12, published pack v9, three fixes this
batch):** 106 frames match (-123 through -18) and the first divergent
frame is -17, P2's `action_age` (expected `5.0`, actual `4.0`) mid-
`SpecialNLoop`, a plain one-frame lag appearing partway through an
already-matching run rather than on any state transition -- not
diagnosed further this batch. This baseline had drifted out of sync with
this file since gameplay export v8/v9 and the concurrent GuardOn/Guard/
Guard Reflect batch; `tests/fixtures/slippi/parity/fox-fd-4-baseline.
json`'s own note now carries the authoritative current citation. In
order: the previously-recorded 93-frame/-30 divergence (labeled a
GuardOn/Guard `action_state` mismatch at capture time) was re-diagnosed
as Squat/SquatWait (states 39/40, unrelated to the shield-family
`state_age` fix a concurrent batch owns) and fixed by routing Landing's
own interruptible down-stick entry into SquatWait directly, matching
`ftCo_Landing_IASA`'s own `ftCo_SquatWait_CheckInput` call instead of the
ordinary Squat crouch-down animation (103 frames, -20); a sibling
entry-advance bug in the ordinary (non-Landing) Squat path was fixed in
the same batch without moving this baseline on its own
(`ftCo_Squat_Enter`'s own missing extra `ftAnim_8006EBA4` advance); and
the resulting -20 divergence (EscapeAir's own `action_age`, the identical
missing-entry-advance shape in `ftCo_80099A9C`) was fixed last, reaching
106 frames. `docs/validation.md`'s entry-advance table and
`fox-fd-4-baseline.json`'s own note have the full per-fix citations.

**Current measurement (2026-09-13, published pack v10): moved again, then
its own divergence fixed by the same bug as `fox-fd-2.slp`'s own.**
Re-measured directly against pack v10 (script command variables embedded,
`docs/parity.md`'s own "Published pack v10" note above): 110 frames now
match (-123 through -14). The divergence was P2's `action_age`, mid-
`SpecialNLoop` (state 342), at frame -13 (expected `9.0`, actual `8.0`) --
the same one-frame-lag shape the v9 baseline's own divergence showed.

The reservation that previously blocked this (a concurrent Falco Blaster
batch) turned out to be narrower than assumed: it covers Falco's own data
path and item-kind differences, not the shared hit pipeline or Loop
timing. Two hypotheses for the lag were checked directly and both point
away from `characters::fox::neutral`'s own script/`cmd_vars` timing.
First, whether Loop's own re-entry (`enter_loop`, reached from both
Start->Loop and a repeat Loop->Loop pass) is missing the same extra
`ftAnim_8006EBA4` advance the Start-entry family needed: reading the
pinned decomp directly (`ftFox_SpecialN_BeginLoopTransition`/
`FinishLoopTransition`, `ftfoxspecialn.c:316-333`) shows neither makes that
call (unlike `ftFox_SpecialN_InitializeState`, which does, `:246-252`), so
Loop's own entry needs no such adjustment -- `enter_loop`'s existing
default `action_frame = 0` (from `simulation::enter`'s own blanket reset)
is already correct. Second, and confirmed directly: a probe stepping the
native match frame-by-frame shows P2's own `action_state`/`action_age`
already match the recording bit-exactly on every single frame from -123
through -3 once the projectile-owner-hitlag fix below is applied -- so the
Loop's own script-driven cadence was never wrong; the v9/v10 baselines'
own `action_age` divergence was P2's own laser connecting and (like
fox-fd-2.slp's P1) freezing P2's own `action_frame` afterward via the
identical attacker-hitlag bug. Fixed at the source (`docs/validation.md`'s
projectile-owner-hitlag entry, `game::damage::apply_hit` gains an
`attacker_takes_hitlag` parameter, `false` for `game::projectile.rs`'s own
call): P2's own timing now matches throughout.

**New divergence, unrelated to either hypothesis above.** With P2's own
freeze fixed, `checked_frames`/`first_divergent_frame` stay at 110/-13 (the
fix does not move the ratchet), but the field changes: P4's `action_state`
(expected `0x002b`/43 = `LandingFallSpecial`, actual `0x004e`/78 =
`Damage`, grounded/middle/level-0 hitstun). P4 has been in
`LandingFallSpecial` continuously since -19 (its own tracked age past 18
by -13); the recording shows P4's own `percent` still rising (`3` at -13,
`6` at -5, matching P2's laser connecting again) while P4 stays in
`LandingFallSpecial` throughout, uninterrupted. Skirmish instead
transitions P4 into ordinary grounded hitstun the instant the hit lands,
same as any other hit. This looks like a genuinely unmodeled interruption
rule (a hit landing during `LandingFallSpecial`'s own lag not always
forcing a hitstun-state transition in decomp), not a shape either
hypothesis above predicted; not chased further this loop, reported per
this loop's own stop condition (`fox-fd-4-baseline.json`'s own note has
the full citation).

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

**2026-09-14 update:** this recording's own frame-`25` Blaster-one-
frame-early divergence (see below) and `fox-fd-4.slp`'s own frame-`-14`
instance were both directly tested against the working hypothesis that
the victim's `KneeBend`/`LandingFallSpecial` pose-sample index was
off by one; that hypothesis is falsified for both (`docs/validation.md`'s
own 2026-09-14 pose-sample-index investigation entry has the full method,
the real triggering hurtbox bone for each case, and the exhaustive
index-offset sweep). No baseline changes; not chased further.

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

**The exporter published gameplay-export pack v9 with `fast_fall_window`
embedded for real** (`/mnt/archive/datasets/melee/skirmish-gameplay/v2/
fox-bf`, identical to the throwaway diagnostic copy above), unblocking
continued work on the frame -27 `shield` divergence without a local patch.
It turned out to be two separate, independent root causes stacked on the
same frame, not one.

**Fixed (first of two): the passive shield-drain arithmetic read the
wrong frame's own trigger.** `ftCo_800925A4` (`Guard`/`GuardOn`'s own
`Anim` callback, `ftCo_Guard.c:394-433`) reads `fp->input.triggers[0]`,
but that field is only refreshed for the current frame by `Fighter_
Spaghetti_8006AD10` (priority 3, `fighter.c:1790-1839`), which runs
*after* `Fighter_8006A360` (priority 1, `fighter.c:898`) already
dispatched the destination action's own `Anim` callback this same frame:
the passive drain always operates on the previous frame's own processed
trigger, one frame stale, unlike the priority-3 IASA/transition checks
that see the fresh value. `game::shield::update_animation`'s own
`GuardOn`/`Guard`/`GuardReflect` arm now reads `f.previous_input.
shield_pressure()` instead of `input.shield_pressure()`. Confirmed
bit-exact against `fox-bf.slp`: P4's trigger rises `0.8928571343421936`
(frame -28) then `1.0` (frame -27, the conversion frame); reading frame
-28's own value for frame -27's drain lands on `59.76071548461914`, the
recording's own value bit-for-bit, where the conversion frame's own `1.0`
lands on `59.72000122070312` instead. `docs/validation.md` has the full
native-test breakdown (`game_shield`'s `passive_drain_reads_the_
previous_frames_trigger_not_the_current_frames`).

**Fixed (second of two): shield regeneration wrongly landed on the exact
frame `GuardOn` converts into `Pass`.** `Fighter_ProcessHit_8006D1EC`
(priority 0xE, `fighter.c:908,2821`) gates its own per-frame shield
regeneration on `fp->x221A_b7`, a flag only ever set by `GuardOn`/`Guard`/
`GuardReflect`'s own entries (`ftCo_Guard.c:267,523,708,803,984`) and
unconditionally cleared by every `Fighter_ChangeMotionState` call
(`fighter.c:1048`) -- including the same one `begin_pass`
(`ftCo_8009A184`/`ftCo_8009A228`) makes to enter `Pass`. `fox-bf.slp`
shows no regeneration lands on that conversion frame despite this: the
recording's own P4 shield health is explained in full by the passive
drain alone (`59.76071548461914`, matching the first fix above exactly),
not by that drain plus a frame of `regeneration` (`0.1`-per-frame-scale
in test fixtures; the real pack's own rate is `0.07`). `game::shield::
finish_frame` gains an explicit `was_active: bool` parameter instead of
recomputing `active(f)` internally; its caller, `simulation::advance`,
still passes plain `active(f)` ordinarily, but folds in a new
`shield_active_into_pass` flag recorded at the exact `pass_request_
after_actions`/`begin_pass` call site (whether the fighter was still
`active()` immediately before that specific conversion). This is
deliberately narrower than "skip regeneration on any exit from an active
shield state": an existing, already-passing native test (`game_escape`'s
`escape_clears_the_shield_and_lets_health_regenerate`) already pins
immediate regeneration on the frame `Guard` converts into a roll via
`ftCo_8009917C`/`ftCo_8009980C`, a different `Fighter_ChangeMotionState`
call this same unconditional `x221A_b7` clear also reaches; broadening the
fix to cover that transition too regressed that test (`49.999992` where
`50.0` was expected), so it is left alone pending its own, independently
diagnosed recording evidence rather than generalized on inference alone.

Confirmed bit-exact against `fox-bf.slp` (both fixes together): P4's
shield lands on `59.76071548461914` at frame -27, the recording's own
value. Either fix alone still diverges, by a different amount each:
without the trigger fix, `59.83071517944336` (the conversion frame's own
`1.0` trigger, loss `0.28`, regeneration `0.07` wrongly added back);
without the regeneration fix, `59.79000091552734` (frame -28's own
trigger, loss `0.2393`, regeneration `0.07` still wrongly added back).
100 frames now match (`-123` through `-23`), up from 96;
`fox-bf-baseline.json` moves to reflect this, measured against the
now-published gameplay-export pack v9 (`/mnt/archive/datasets/melee/
skirmish-gameplay/v2/fox-bf`, confirmed identical to the diagnostic copy
above) rather than a local patch.

**Fixed: the ordinary ground-lost-to-`Fall` path was missing the
edge-drop air-drift clamp.** Frame -23, `position.x` on P1 (expected
`-18.626245498657227`, actual `-17.28374481201172`) diverged one frame
after P1 runs off Battlefield's platform edge into `Fall` (Slippi action
state `29`) at frame -24 -- not the airborne fused-multiply-add
arithmetic the concurrent `bbfaf6a` batch was disassembling (that
diagnosis was left open pending this measurement), but a whole missing
call. `ftCo_Fall_Enter` (`ftCo_Fall.c:47-70`, the generic `Fall` entry
`ft_80084104` reaches whenever `ft_800827A0`'s ground check fails --
`ft_081B.c:1044-1050`, the same dispatcher every Walk/Dash/Run/Turn
`Phys` callback routes through) unconditionally calls `ftCommon_
ClampAirDrift` (`ftcommon.c:457-459`, `ftCommon_ClampSelfVelX(fp, ca->
air_drift_max)`) right after `Fighter_ChangeMotionState`, regardless of
whether `ground_or_air` was `Ground` or already `Air`: running or
dashing off an edge carries the ground speed straight into this clamp,
so the first airborne frame drifts at `air_drift_max`, not at the
(much higher) run/dash speed. `game::collision::resolve`'s own
ground-lost branch already ports `ftCo_Fall_Enter`'s ECB-lock side
effect (`ftCommon_8007D5D4`'s `ecb_lock = 10`) but had never carried
over the `ClampAirDrift` call alongside it -- unlike `begin_pass_as`
(the explicit platform-drop path, `ftCo_8009A184`/`ftCo_8009A228`),
which already inlines the same clamp for its own, narrower transition.
Confirmed directly against `fox-bf.slp`: P1's own frame-to-frame
position delta is `2.172501` units (Run ground speed) through frame -24,
then `0.81` units at frame -23 -- consistent with `air_drift_max` scaled
by that frame's own stick tilt, not the carried-over run speed.
`game::collision::resolve` now builds a scratch `Movement` from `f.
velocity` and calls `clamp_air_drift()` on it immediately after
`simulation::enter(f, Action::Fall)` in that branch, writing the
clamped `self_velocity[0]` back to `f.velocity[0]`. `docs/validation.md`
has the full native-test breakdown (`game_edges`'s existing
`dash_and_run_past_the_end_fall_off_it`, now asserting the clamp).
109 frames now match (`-123` through `-14`), up from 100;
`fox-bf-baseline.json` moves to reflect this, measured against the
same published gameplay-export pack v9. The new first divergence is
frame -14, field `action_age` on P4 (expected `1.0`, actual `0.0`), on
P4's own air dodge entering `EscapeAir` -- already fixed above (a
concurrent `fox-fd-4.slp` loop independently found and fixed the same
`ftCo_80099A9C` entry-advance bug this same day); re-measured below.

**Re-measured against the newly-published gameplay-export pack v10**
(`/mnt/archive/datasets/melee/skirmish-gameplay/v10-snapshot-20260913/
fox-bf`, identical numbers to pack v9, confirming this is a pure
pack-version bump and not a behavior change): 137 frames now match
(`-123` through `13`), up from 109; `fox-bf-baseline.json` moves to
reflect this. The new first divergence is frame 14, field `position.x`
on P4 (expected `33.07349395751953`, actual `33.836673736572266`, a
`0.76`-unit gap), on the frame P4 enters Fox's own aerial neutral
special (Blaster, Slippi action state `344`, `SpecialAirNStart`).
Not the concurrent script-driven Blaster-timing batch's own area after
all (that batch's own work is the script/`cmd_vars` timing already
landed on `main`; this divergence turned out to be unrelated, ordinary
entry-velocity handling), so continued here rather than deferred.

**Fixed: the aerial Blaster entry was zeroing velocity it should leave
alone.** `ftFx_SpecialN_Enter` (the grounded Blaster entry,
`ftfoxspecialn.c:255-269`) zeros `gr_vel` and `self_vel.{x,y,z}` right
after `Fighter_ChangeMotionState`/`ftFox_SpecialN_InitializeState`;
`ftFx_SpecialAirN_Enter` (the aerial entry, `:274-284`) does not touch
velocity at all -- only the motion-state change, the shared
`InitializeState` (extra animation advance, `cmd_vars` reset) and the
blaster-gun spawn. `game::characters::fox::neutral::update_actions`
applied the ground entry's own unconditional `self_vel = 0` to both
branches, so P4's mid-jump press hard-stopped its existing drift instead
of carrying it through untouched. Confirmed directly against
`fox-bf.slp`: P4's own frame-to-frame `position.x` delta is one smooth,
continuously-decelerating sequence straight through the frame-14
transition (`-0.8632` .. `-0.7632` .. `-0.7432`, no visible mark at the
transition itself), meaning `self_vel.x` was never reset in the
recording; before this fix the port's own frame-14 `position.x`
(`33.836673736572266`) sat within noise of frame 13's own value,
i.e. velocity had already been zeroed. Now only the ground branch
zeros `velocity`/`ground_velocity`; the air branch leaves both alone.
`docs/validation.md` has the full native-test breakdown (`game_fox_
neutral_special`'s new `aerial_entry_preserves_velocity_but_grounded_
entry_zeros_it`).

149 frames now match (`-123` through `25`), up from 137;
`fox-bf-baseline.json` moves to reflect this, measured against the same
published gameplay-export pack v10. The new first divergence is frame
26, field `action_state` on P1 (expected `KneeBend`, actual `DamageN1`)
-- P1 is hit by something Skirmish's own simulation does not expect at
that frame.

**Fixed: a zero-knockback hit was forcing a reaction it should not.**
P1 takes a Blaster hit (damage `3`) while mid-`JumpSquat` at frame 26 and
stays in `JumpSquat`, uninterrupted, continuing normally into its own
jump two frames later -- only its percent ticks up. `Fighter_ProcessHit_
8006D1EC`'s ordinary reaction dispatch (`fighter.c:2810`, case `0` of
`switch (fp->x1828)`) calls `ftCo_8008EC90` (`ftCo_Damage.c:838`), whose
own first check -- `if (fp->x2220_b3 || fp->x2220_b4 ||
!fp->dmg.kb_applied) { inlineB2(gobj); return; }` -- skips its entire
motion-state transition (`ftCo_8008E908` -> `ftCo_8008DCE0`) whenever the
hit's own final computed knockback is exactly `0.0`, applying only
cosmetic hit-effects instead. Fox/Falco's own Blaster laser has zero
growth, zero base and zero weight-independent knockback -- already
documented as "zero knockback is real: a laser flinches its target
without pushing it" (`docs/fox-neutral-special.md`) -- so it always
computes exactly this. `game::damage::apply_hit` now gates its entire
victim-side reaction (motion-state transition, velocity, hitstun,
hitlag, `last_hit_by`) on the computed knockback being nonzero, while
still applying percent and the attacker's own separate last-move-
landed/combo-count bookkeeping (`combo::record`, which is the attacker's
own accounting and stays unconditional) and gating the attacker's own
hitlag alongside the victim's. `docs/validation.md` has the full
native-test breakdown (`game_damage`'s new `zero_knockback_ticks_damage_
without_forcing_any_reaction`).

156 frames now match (`-123` through `32`), up from 149;
`fox-bf-baseline.json` moves to reflect this, measured against the same
published gameplay-export pack v10. The new first divergence is frame
33, field `action_state` on P4 (expected `Dash`, actual `Turn`). Not the
concurrent fox-fd-2/fox-fd-4 loop's own dash-entry-velocity investigation
after all -- that investigation is closed (falsified) and does not own
this frame; `dash_threshold` itself is independently verified from the
disc, so the divergence is a mechanism gap, not a pack-data one.

**Fixed: Turn's own smash-to-Dash conversion waited for the wrong
flip.** P4 enters `Turn` from `Landing` at frame 32 with a moderate
reversal (stick `0.7375`) -- an ordinary entry, `standing_turn_frames`
(`4`) far from expired -- and by frame 33 the stick strengthens to
`0.8375` (`>= dash_threshold` `0.8`, `tilt_x_age` `1 < dash_window` `2`)
and P4 is already in `Dash` with facing flipped, one frame after
entering `Turn`. `ftCo_Turn_IASA`'s own `fn_800C9C2C` conversion
(`ftCo_Turn.c:97-148,160-169`) checks the smash threshold/window against
the *current* frame's own stick independent of whether `ftCo_Turn_
Anim_Inner`'s separate `frames_to_turn` countdown has completed: it
resolves the facing flip itself the moment the check first passes,
rather than waiting on that other mechanism, since `ftCo_Dash_Enter`
(`ftCo_Dash.c:49-70`) reads `fp->facing_dir` directly and never flips it
itself. `game::locomotion::update_actions`'s own `Action::Turn` arm
previously gated this conversion on `just_turned` (the flip having
*already* happened via the separate countdown) in addition to the smash
check, so it would not have fired until several more frames later once
`standing_turn_frames` actually expired. It now checks the smash
condition fresh each frame and resolves `turn_has_turned`/`facing`
directly once it holds, without needing `just_turned` at all -- which,
having lost its only remaining consumer, was removed from the whole
per-frame pipeline. `docs/validation.md` has the full native-test
breakdown (`game_dash`'s new `a_stronger_stick_mid_turn_converts_
straight_to_dash_without_waiting_for_the_flip`).

160 frames now match (`-123` through `36`), up from 156;
`fox-bf-baseline.json` moves to reflect this, measured against the same
published gameplay-export pack v10. The new first divergence is frame
37, field `action_age` on P1 (expected `1.0`, actual `0.0`), on the
frame P1 enters `AttackAirLw` (Fox's down-air) -- the same class of
"extra entry advance" bug already fixed for Dash/Turn/Squat/EscapeAir.

**Fixed: AttackAir's own shared entry has the same extra advance.**
`ftCo_AttackAir_EnterFromMsid`/`_EnterFromCStick` (`ftCo_AttackAir.c`,
the shared entry for AttackAirN/F/B/Hi/Lw alike) call `Fighter_
ChangeMotionState` then `ftAnim_8006EBA4(gobj)` explicitly, the
identical extra advance already fixed for Dash/Turn/Squat/Fox's neutral
special. `game::aerial::update` now sets `action_frame = 1` at entry,
covering all five moves through their shared entry point. Unlike Dash/
Turn/Squat, this move's own supplied `Move.attack.frames`/`Move.flags`
sample arrays are indexed by elapsed action frames since entry (the
same situation EscapeAir's own fix already established a pattern for):
`aerial::commands`, `aerial::update_animation`'s length check and the
shared `simulation::attack_frame` hitbox/pose lookup (used by jab/tilt/
smash/dash-attack too, whose own entries make no such extra advance)
now subtract 1 through a new `aerial::attack_sample_index` helper when
serving an aerial attack specifically. `docs/validation.md` has the
full native-test breakdown (`game_aerial_actions`'s new `attack_air_
reports_the_replay_verified_state_age_of_one_on_entry`, plus the
existing hitbox/autocancel/cstick-repeat tests' own hardcoded
`action_frame` values, bumped by exactly 1 to match).

172 frames now match (`-123` through `48`), up from 160;
`fox-bf-baseline.json` moves to reflect this, measured against the same
published gameplay-export pack v10. The new first divergence is frame
49, field `shield` on P4 (expected `57.46872329711914`, actual
`57.53872299194336`, exactly one regeneration tick too high) -- on the
frame `Guard` converts into `GuardOff` (Slippi action state `180`), the
ordinary voluntary shield release.

**Fixed: shield regeneration's own frame-start snapshot generalized
past `Pass`.** Every ordinary (`Ft_MF_None`) `Fighter_ChangeMotionState`
call unconditionally clears `fp->x221A_b7` (the flag `Fighter_
ProcessHit_8006D1EC` gates regeneration on), including every transition
out of an active shield: `begin_pass` into `Pass`, the ordinary release
into `GuardOff` (`ftCo_80092F2C`), and the roll/spot-dodge escape alike.
The existing fix only modeled this for the `Pass` destination (a narrow
flag recorded at `begin_pass`'s own call site); `simulation::advance`
now snapshots `shield::active(f)` for every player up front, before
that frame's own `update_actions` runs any transition, and threads that
single `shield_active_at_frame_start` value to `shield::finish_frame`
directly -- covering `GuardOff`, `Pass`, and the roll uniformly, and any
future such transition, without a new flag per call site. A fighter who
does not transition out of an active shield this frame reads the same
value either way, so the steady-state case is unaffected.

Generalizing surfaced the same "0.07-short" discrepancy an earlier
attempt at this exact broadening had already hit and left alone
(`docs/validation.md`'s own prior entry) -- root-caused this time: the
roll's own conversion frame had been regenerating under the old,
narrower gate (checked *after* the transition, when the fighter was
already `EscapeF`) even though real Melee's own mechanism says it
should not, exactly like `Pass`/`GuardOff`. The fix correctly withholds
that tick too, so `game_escape`'s own pre-existing regeneration test
needed one more intermediate step before its final assertion (reaching
the `50.0` cap one frame later than its old, narrower-gate-shaped step
count assumed); its own `rolling.health < guarding.health` and per-frame
`health > previous` checks needed no changes. `docs/validation.md` has
the full breakdown.

273 frames now match (`-123` through `149`), up from 172;
`fox-bf-baseline.json` moves to reflect this, measured against the same
published gameplay-export pack v10. The new first divergence is frame
150, field `action_state` on P4 (expected `Dash`, actual `Turn`) -- the
same class of Turn-to-Dash conversion divergence already fixed once on
this recording (frame 33).

**Fixed (regression): the frame-33 fix's own "instant bypass" over-
corrected, regressing `fox-fd-2.slp` from 118 to 112 frames (frame -11,
P2, expected `Turn`, actual `Dash`).** `ftCo_Turn_IASA`'s own `fn_
800C9C2C` conversion gates the actual `ftCo_Dash_Enter` call on `fp->
mv.co.turn.just_turned`, a one-frame pulse true only on the exact frame
`ftCo_Turn_Anim_Inner` (the ordinary, Anim-side flip) itself flips `has_
turned`. The frame-33 fix dropped `just_turned` entirely in favor of
`!turn_has_turned` alone: correct for an ordinary Turn converting
*before* its own flip (`fox-bf.slp` P4, frame 33), but wrong two other
ways -- it blocks a *smash-entered* Turn whose ordinary flip lands
almost immediately (`frames_to_turn = 0` from entry, `fox-bf.slp` P1,
frame -4: by the time this IASA-equivalent check runs, `has_turned` is
*already* true, since Anim precedes it the same frame, even though the
flip just happened and should still convert), and it wrongly lets a
long-since-flipped Turn re-convert on any later fresh reversal
(`fox-fd-2.slp` P2, frame -11: flipped at frame -15, then holds a
smash-magnitude stick for many further frames -- including a fresh
reversal at -11 -- without the recording ever converting).
`update_actions`'s `Action::Turn` arm now gates the conversion on
`(just_turned || !turn_has_turned)`: `just_turned` is threaded back
through the per-frame pipeline exactly as it was before the frame-33
fix removed it, set by the unchanged, decomp-literal Anim-side flip.
The two conditions together cover all three recordings' own cases; see
`docs/validation.md` for the full breakdown and the new native test
(`game_dash`'s `a_fresh_reversal_after_turn_already_flipped_does_not_
convert_to_dash`). `fox-fd-2.slp` returns to its own pre-existing 118/-5
frontier (baseline unchanged); `fox-bf.slp` returns to 273/`149`,
unchanged from just above.

**Fixed: the frame 150 divergence, a second, independent bug in the same
arm.** P4 enters a smash-entered Turn (from `Dash`'s own dash-back check)
at frame 149 with `tilt_x_age` `1`, comfortably under `dash_window` (`2`),
latching `turn_smash`; the ordinary Anim-side flip lands the very next
frame (150), but by then the stick has been held continuously since the
frame before entry, so `tilt_x_age` has reached `2`, no longer under the
window. `fn_800C9C2C` (`ftCo_Turn.c:160-171`) is the *only* place the
window gates anything: it latches `x8` when the window holds, and never
clears it once latched; `ftCo_Turn_IASA`'s own final check right before
`ftCo_Dash_Enter` (`:128-135`) rechecks only the stick-vs-threshold half
of that expression, deliberately not the window. The prior fix's own
`smash_this_frame` variable bundled the window into *both* the latch step
and the final gate, so a stale window still wrongly blocked an
already-latched conversion. `update_actions`'s `Action::Turn` arm now
splits the two: a windowless `stick_past_threshold` feeds the final gate,
while the window still gates only the fresh-latch step. `docs/
validation.md` has the full native-test breakdown (`game_dash`'s new
`a_smash_entered_turn_still_converts_once_the_window_has_gone_stale`,
constructing the exact three-frame shape with the fixture's own
`dash_window` overridden to match the real pack's value).

297 frames now match (`-123` through `173`), up from 273;
`fox-bf-baseline.json` moves to reflect this, measured against the newly
published gameplay-export pack v11 (identical numbers to v10, a pure
pack-version bump). The new first divergence is frame 174, field
`action_age` on P1 (expected `1.0`, actual `0.0`), on the frame P1
enters Fox's own Down Special (Reflector/Shine) Start phase (Slippi
action state `360`, `SpecialLwStart`) -- not the already-fixed "extra
entry advance" class alone: the recording's own `state_age` holds at a
constant `1.0` for four further real frames (174-177) while B stays
held, before continuing to increment normally (178: `2.0`, 179: `3.0`)
and converting to the Loop phase at 180. This looks like a genuine
scripted animation-rate freeze -- matching the already-modeled
`RunBrake` `run_brake_marker_frame`/`run_brake_freeze_speed` pattern, a
different mechanism from the entry-advance class this whole batch has
otherwise been fixing -- rather than a simple offset, and the
held-duration does not appear tied to the release timing itself:
`ftFx_SpecialLw_Enter`/`SetVars` sets `fp->cmd_vars[1] = 4` (matching
the observed four-frame freeze), a plausible but unconfirmed script
marker value, not yet exported as pack data or independently verified.
Reported per this loop's own stop condition (unmodeled system needing
design/pack-data verification) rather than guessed at.

## The tournament-stage batch: `fox-ys.slp`, `fox-fod.slp`, `fox-dl.slp`, `fox-ps.slp`

Four more Fox-vs-Fox recordings from the same corpus, one per remaining
tournament stage not yet covered: Yoshi's Story (`16_10_01 Fox + Fox
(YS).slp`, P1/P2), Fountain of Dreams (`11_57_43 Fox + Fox (FoD).slp`,
P1/P2), Dream Land (`10_26_55 Fox + Fox (DL).slp`, P3/P4), Pokemon
Stadium (`18_54_44 Fox + Fox (PS).slp`, P1/P4), each with its own
gameplay-export pairing against pack v10 (`/mnt/archive/datasets/melee/
skirmish-gameplay/v10-snapshot-20260913`). Measured baselines: YS first
diverges at frame -113 (`position.y`, P2, 1 ULP), FoD at -118
(`position.y`, P1, 1 ULP), DL and PS both on the very first compared
frame, -123 (`checked_frames: 0`).

### Yoshi's Story / Fountain of Dreams: the Entry -> EntryStart transition frame's own 1-ULP gap is not a Skirmish bug

Both YS and FoD diverge on P2/P1's own *first EntryStart frame* -- the
exact frame `ftCo_Entry_Anim`'s transition (`entry.timer == 0`) calls
`ftCo_800C6408` (setting `entry.timer = x6BC`, `x20`/`x24` from
`1.497345 * (x34_scale.y * trophy_scale)`) and then, within that same
unconditional trailing decrement, leaves `entry.timer = x6BC - 1` for
this same frame's `ftCo_EntryStart_Phys` to use (`t = 1 / x6BC`) --
matching `game::entry::update_animation`'s `Action::Entry` arm calling
`enter_start` followed by `move_fighter`'s `Action::EntryStart` arm in
the same simulated frame (`src/game/entry.rs`).

For YS, P2 (slot 1, spawn `(42.0, 28.0)`) spawns 10 frames after the
first recorded frame (`entry_delay(1) = 10`), landing the transition
exactly on frame -113; `checked_frames: 10` in the measured baseline
confirms every field, including `position.y`, matched bit-for-bit for
all 10 preceding Entry frames (so the fixed spawn anchor `x4`/`y0` -- 28.0
exactly, `0x41e00000` -- is not in question). The recorded value at -113
is `28.044921875` (`0x41e05c00`); Skirmish computes `28.044919967651367`
(`0x41e05bff`), one ULP low.

Three independent checks agree Skirmish's arithmetic is exactly right
here, given these inputs (`trophy_scale = 0.9` exactly -- re-confirmed
against `fox-fd.slp`'s own already-solved `EntryEnd`-entry value below --
`x34_scale.y = 1.0` for a standard, non-giant/tiny Fox, `y0 = 28.0`, `t =
1 / 30` via `fdivs`):

1. **The retail `main.dol` disassembly itself** (`tools/
   ppc_precision_audit.py`, then a full Capstone dump of
   `ftCo_EntryStart_Phys` at `0x800c6740`): the position write is two
   separately single-rounded instructions, `fmuls f0, f0, f31` (`x28 =
   x20 * t`) then, after a store/reload round-trip through memory,
   `fadds f0, f1, f0` (`cur_pos.y = x4 + x28`) -- no `fmadds`, no hidden
   double-precision retention. Replaying this exact instruction sequence
   in `f32` (`y0 + (amplitude * t)`, each op rounded once) reproduces
   Skirmish's own `0x41e05bff`, not the recording's `0x41e05c00`.
2. **A new C-oracle differential**, `tests/entry_differential.rs`'s
   `entry_transition_frame_matches_the_oracle_bit_exactly` (a proptest)
   and `entry_transition_frame_reproduces_the_fox_ys_p2_one_ulp_gap` (the
   exact case), calling a new `oracle_entry_transition_frame`
   (`tests/oracle/entry.c`) that chains a fresh-transition
   `ftCo_Entry_Anim` directly into `ftCo_EntryStart_Phys` within the same
   call -- exactly the transition frame's own per-frame order, which
   neither existing oracle harness covered (`oracle_entry_start_frame`
   models a *steady-state* EntryStart frame, re-running
   `ftCo_EntryStart_Anim`'s own decrement first, which is the wrong
   frame's math for a fresh transition). The recompiled decomp source
   (`ft_0C31.c` verbatim, via `entry_original.inc`) also lands on
   `0x41e05bff`.
3. **All later EntryStart frames match Skirmish's own formula exactly**:
   frames -112 through -105 (`t = 2/30 .. 9/30`), checked directly
   against `fox-ys.slp` via `py-slippi`, reproduce bit-for-bit under the
   identical `y0 + amplitude * t` computation -- ruling out a general
   `trophy_scale`/`x34_scale`/formula bug, since only the transition
   frame itself (`t = 1/30`) disagrees.
4. `trophy_scale = 0.9` was re-verified, not assumed: solving
   `fox-fd.slp`'s own `EntryEnd`-entry value (`y = x4 + x20` at full
   amplitude, no `t` multiply -- `docs/match-start.md`'s original
   derivation) is sensitive enough to distinguish `0.9` from a
   0.00005-away neighbor (`0.9` reproduces `0x41358fd0` exactly;
   `0.9000452` lands on `0x41359017` instead), so the transition frame's
   gap is not explained by a coarser derivation missing a small
   `trophy_scale` error either.

This is reported, not fixed: a genuine one-ULP difference with the
function (`ftCo_EntryStart_Phys`, specifically the Entry -> EntryStart
transition frame) and every input identified, per this loop's own stop
condition. FoD's own divergence (frame -118, P1, spawn `(-42.0, 21.0)`,
`entry_delay(0) = 5`) is the same class of gap on the same transition
frame shape and was not independently re-derived beyond confirming the
measured baseline. `fox-ys-baseline.json`/`fox-fod-baseline.json` are
unchanged (still -113/-118); the two new oracle tests stand as
permanent regression coverage proving Skirmish already agrees with the
compiled decomp source here, so a future change to this arithmetic that
accidentally "fixes" the ULP by drifting from the decomp would be
caught.

### Dream Land / Pokemon Stadium: both diverge on frame -123 itself, in the exported spawn coordinates, not the simulation

Both DL and PS mismatch on the very first frame compared
(`checked_frames: 0`), which rules out any Skirmish simulation step:
nothing has run yet. Both trace to the exported `stage.spawns` values in
their own `match-data.json`, checked directly against each recording's
own first frame via `py-slippi`:

- **Dream Land**: the pack's `stage.spawns` are `[[-46.599998474121094,
  37.221500396728516], [47.38909912109375, 37.321502685546875]]`. The
  recording shows both P3 and P4 sitting at `position.y = 37.0` exactly
  for frames -123 through at least -121 (x already matches the pack
  exactly for both ports). Both spawn Y values are wrong by a few tenths
  of a unit -- not a rounding-scale gap -- and are wrong by *different*
  amounts (`0.2215` and `0.3215`, suspiciously exactly `0.1` apart).
- **Pokemon Stadium**: the pack's `stage.spawns` are `[[-39.999996185302734,
  31.99999237060547], [40.0, 32.0]]`. The recording shows both P1 and P4
  at exactly `(-40.0, 32.0)` and `(40.0, 32.0)`. Spawn index 1 is exact;
  spawn index 0 is off by a few ULPs in *both* x and y.

Root cause, found by the exporter owner: the retail game places a spawn
point by composing the spawn marker joint's *full parent chain* --
`lb_8000B1CC` (the map's spawn-point resolver) calling
`HSD_JObjSetupMatrix` up the joint hierarchy to get the marker's world
transform, not just its own local translation. The `skirmish-assets`
exporter reads the marker joint's local translation directly and skips
that composition, which is silently correct only when every ancestor
joint between the marker and the map root is an identity transform.
Dream Land's *unequal* per-port offsets (`0.2215` vs `0.3215`) match two
spawn markers sitting under different, non-identity parent joints in the
map's scene graph; Pokemon Stadium's *few-ULP* gap on spawn index 0 only
(index 1 exact) matches one spawn marker's parent chain carrying a
near-identity transform with just enough floating-point residue to move
the last few bits, while the other marker's own chain composes to
exactly identity. Both are the same one bug (composing world transforms
the exporter currently skips), not two unrelated stage-specific issues.

This is exporter code, not exporter *data*: reported per this loop's own
instructions to the exporter owner rather than patched locally --
`skirmish-assets`'s stage export (`stage.rs`) needs to compose the spawn
marker's parent chain via the same `HSD_JObjSetupMatrix`-equivalent walk
used for bone poses elsewhere in the exporter, not read the marker's
local translation alone. A dedicated exporter-side loop is fixing the
composition in `stage.rs` directly; once it republishes corrected
`stage.spawns` for these two stages (`fox-dl`: both `.y` should read
`37.0`; `fox-ps`: spawn index 0 should read `(-40.0, 32.0)`),
`fox-dl-baseline.json`/`fox-ps-baseline.json`'s `-123`/`0` will move
without any further Skirmish-side change.

## 2026-09-14 re-measurement of every ratchet recording against pack v10

A full sweep of every recording in `recordings.json` plus the standalone
`falco-fox-fd.slp`, all against the published gameplay-export pack v10
(`/mnt/archive/datasets/melee/skirmish-gameplay/v10-snapshot-20260913`),
run directly via `make-initialization`/`validate-replay` (not through
`cargo test`, to see every recording's own number in one pass rather than
stopping at the first regression). No baseline file is lowered by this
sweep; `fox-fd-4-baseline.json` moves forward (see below) and every other
baseline is confirmed unchanged.

| Recording | Matched frames | First divergence | Field | Status |
|---|---|---|---|---|
| `fox-fd` | 128 | 5 | `position.x`, 1 ULP | unchanged; open (`docs/math.md`) |
| `fox-fd-2` | 112 | -11 | `action_state` (Turn vs Dash) | unchanged; **known regression, owned by the Battlefield loop (`36a5a36`)** |
| `fox-fd-3` | 91 | -32 | `velocities.self_x_air`, 1 ULP | unchanged; open (`docs/math.md`) |
| `fox-fd-4` | 118 | -5 | `percent` (P4, expected `6.0`/`0x40c00000`, actual `3.0`/`0x40400000`) | **moved forward from 110/-13**, upstream zero-knockback fix; new divergence below |
| `fox-bf` | 172 | 49 | `shield` (P4) | unchanged; open, not yet diagnosed |
| `fox-ys` | 10 | -113 | `position.y`, 1 ULP | unchanged; genuine ULP, reported (above) |
| `fox-fod` | 5 | -118 | `position.y`, 1 ULP | unchanged; genuine ULP, reported (above) |
| `fox-dl` | 0 | -123 | `position.y` | unchanged; blocked on exporter spawn-composition fix (above) |
| `fox-ps` | 0 | -123 | `position.x` | unchanged; blocked on exporter spawn-composition fix (above) |
| `falco-fox-fd` | 98 | -25 | `position.x`, 1 ULP | unchanged; genuine ULP, reported (`docs/validation.md`) |

`fox-fd-4.slp`'s own improvement (110/-13 -> 118/-5) is not this loop's own
fix: it lands purely from the Battlefield loop's zero-knockback fix
(`f196f0e`/`36a5a36`, `docs/validation.md`), which independently resolves
the exact `LandingFallSpecial`-interruption shape this loop had reported
open at -13 (`fox-fd-4-baseline.json`'s own prior note). Re-diagnosed
below as the next divergence in this loop's own queue.

**Update (2026-09-14): the Pokemon Stadium half of this diagnosis was
wrong.** `fox-ps.slp`'s "few-ULP gap on spawn index 0" was not the
exporter's parent-chain composition bug after all -- the fixture itself was
recorded on modified stage data. Checked directly with `py-slippi` against
every Fox-involved Pokemon Stadium replay in `slippi-public-dataset-v3.7/
data/FOX` (18 files): the split is exact and follows the recording's own
Slippi client version, not measurement noise. Every Slippi 2.0.1 replay's P1
spawn (14 files, including `12_35_58 Fox + Fox (PS).slp`) reads
`(-39.999996185302734, 31.99999237060547)`, bit-for-bit the pack's own
`stage.spawns[0]`; every Slippi 3.9.0 replay's P1 spawn (4 files, including
the original `18_54_44 Fox + Fox (PS).slp` fixture) reads exactly
`(-40.0, 32.0)` instead -- a clean half-unit shift in `x` and an exact
integer `y`, the signature of a netplay build's own modified stage table
entry, not floating-point residue from a missing joint-chain composition.
Fourteen independent 2.0.1 recordings agreeing with the pack bit-for-bit
rule out both "the pack is slightly wrong" and "this is coincidence": the
pack's `stage.spawns[0]` was already correct, and the original `fox-ps.slp`
fixture was the wrong file to measure it against.

`fox-ps.slp` was replaced 2026-09-14 with `12_35_58 Fox + Fox (PS).slp`
(same CC0-1.0 corpus, Slippi 2.0.1, Fox vs Fox, P1/P4) -- see
`tests/fixtures/slippi/parity/manifest.json`'s updated entry and its own
`note` field. Re-measured against gameplay-export pack v10: the frame -123
spawn divergence is gone (both spawns now match bit-for-bit); 85 frames
match (-123 through -39). The new first divergence is frame -38, P4's
`action_state` (expected `0x001d`/`Fall`, actual `0x002a`/`Landing`, three
frames before the recording's own landing) -- a genuine, previously-hidden
divergence now visible for the first time because the spawn-frame bug no
longer masks it; `fox-ps-baseline.json`'s own `note` has the frame-by-frame
trace and points at a likely stage-geometry export gap near Pokemon
Stadium's side platforms (`x=40`), not chased further in this batch.

Dream Land's own diagnosis above is not re-examined by this update and
remains an open, separately-tracked exporter-code item (`fox-dl.slp`'s
unequal per-port `y` offsets, `0.2215` vs `0.3215`); this batch does not
touch `fox-dl.slp` or its baseline.

**Update (2026-09-14, later the same day): the Dream Land half of the
original diagnosis was also wrong, for a different reason than Pokemon
Stadium's.** It is not the exporter's spawn-marker parent-chain composition
bug either, and, unlike Pokemon Stadium, it is not a fixture-selection
mistake (a recording captured on a netplay build) -- `fox-dl.slp` is a
console-era Slippi 2.0.1 recording, the same class of file the corpus's
other fixtures are. The actual root cause is that a real recording's spawn
position is *match-start state*, not purely a function of stage data: the
retail game resolves it once per match from `VsSceneController`'s per-slot
`spawn_point` through `gm/gmvs.c`'s `getSpawnPoint`/spawn-assignment loop
into `gr/stage.c`'s `Stage_80224E64` and `gr/ground.c`'s `Ground_801C2D24`,
and at least two different environments are now confirmed to produce
different results there. Checked directly (`skirmish-cli inspect-replay`
plus `py-slippi` cross-checks against every stage-legal recording available):
Slippi *netplay* builds (confirmed for 3.9.0) inject UnclePunch's "Neutral
Spawns" table (`External/NeutralSpawn/NeutralSpawn.asm`, injection
`8016e510`) instead of the vanilla points -- this is what actually explains
the original Pokemon Stadium gap, not exporter data. Every Fox-vs-Fox Dream
Land recording checked, however, is console 2.0.1 and starts at `(-46.6,
37.0)`/`(47.3891, 37.0)`: `x` is bit-exact vanilla (`data.stage.spawns`,
confirmed against the pack), but `y` (`37.0`) matches neither the pack's own
vanilla `y` (`37.2215..`/`37.3215..`) nor the Neutral Spawns table's `y`
(`37.2`) -- a third, still-unidentified console-era codeset, not a bug in
the exporter and not fixed by this batch (`crates/skirmish-replay/src/
spawn_policy.rs`'s `classify_spawn_provenance` reports it `unknown_codeset`,
one of the three possible outcomes, not a failure).
`skirmish-assets`'s `stage.rs` composition question from the original
diagnosis may still be worth checking independently some day, but it is not
what explains either stage's actual gap, and no exporter fix is pending on
it anymore.

The practical fix does not require identifying that codeset: `SpawnPolicy`
(below) makes match initialization take the replay's *own* recorded spawn
position (`SpawnPolicy::Explicit`, filled by `make-initialization` from
frame -123's own post-frame position) rather than assume the resource
pack's vanilla `stage.spawns` describes every real recording. `fox-dl.slp`'s
baseline moves from `checked_frames: 0`/`first_divergent_frame: -123` (the
spawn-frame mismatch, now understood as this batch's own real, if
unidentified, codeset difference rather than an exporter defect) to
`checked_frames: 74`/`first_divergent_frame: -49`, a genuine one-ULP
`position.y` gap (`0x4213427c` vs `0x4213427b`, the same class of gap
already reported open elsewhere in this document and in `docs/math.md`),
found because Skirmish now starts the simulation from the same point the
real console did instead of a few tenths of a unit away. See "2026-09-14:
SpawnPolicy" below for the full mechanism and every current fixture's
classification.

### Fixture-selection rule: a recording's first frame must be a real, understood recording -- not necessarily vanilla

`fox-ps.slp`'s history above is a genuine lesson, but its original framing
(**"a real recording is only a valid parity fixture if its first frame's
spawn positions bit-match the pack's `stage.spawns`"**) turned out to be too
strong once Dream Land's own gap was correctly diagnosed (above): a fixture
whose first frame does *not* bit-match vanilla is not automatically a bad
fixture -- `fox-dl.slp` never was one, even though it fails that literal
test. The real distinguishing question is not "does this bit-match
vanilla", it is "did this recording start somewhere `make-initialization`
can reproduce exactly", which is a strictly weaker and more accurate
requirement now that `SpawnPolicy::Explicit` (below) takes each recording's
own frame -123 position rather than assuming vanilla. `fox-ps.slp`'s
original defect was real, but it was specifically that the *previous*
`18_54_44 Fox + Fox (PS).slp` fixture was recorded on a Slippi *netplay*
build (confirmed for 3.9.0) shipping modified Pokemon Stadium stage data
(UnclePunch's Neutral Spawns table, not a stage-data difference at all) --
not that its spawn merely differed from vanilla.

Practical rule for picking or auditing a fixture: prefer the oldest
available Slippi version for a given stage/matchup unless there is a
specific reason to want a newer one (older, console-era recordings are the
population `SpawnPolicy`'s two known tables and this project's other
parity work are calibrated against), and, before trusting a new baseline,
classify the recording's own first-frame position with
`classify_spawn_provenance` (`skirmish_replay::spawn_policy`; run `make-
initialization` and read `spawn_provenance` in its printed summary, or
`crates/cli/tests/parity_fixture_spawns.rs` for the whole corpus at once).
`vanilla` or `slippi_neutral` both mean the recording started on a fully
understood codeset; `unknown_codeset` (`fox-dl.slp`'s own case) does not
disqualify the fixture, but it does mean nobody has yet explained *why* the
recording started there, which is worth chasing down when it recurs, not a
one-time embarrassment to except.

`crates/cli/tests/parity_fixture_spawns.rs` automates the classification
step for every recording in `recordings.json`: for each pairing whose pack
has landed under `SKIRMISH_GAMEPLAY_DATA`, it reads the fixture's own first
selected frame, assigns spawn points by participant order (ports sorted
ascending, the same convention `initialization::build` uses -- the pack's
spawns are assigned by participant order, not by which physical port a
player sat at), classifies the pair with `classify_spawn_provenance`, and
prints/records the class -- it no longer asserts a bit-match and carries no
exception list (the previous `KNOWN_PACK_DATA_SPAWN_GAPS = ["fox-dl"]` is
gone): `unknown_codeset` is an expected, valid outcome for a real
recording, not a failure to except. It still skips (rather than fails) a
recording whose pack has not been published yet, exactly like
`real_parity.rs`. As of this batch, every currently published recording
classifies `vanilla` except `fox-dl` (`unknown_codeset`); see "2026-09-14:
SpawnPolicy" below for the full table.

## 2026-09-14: SpawnPolicy -- match initialization from the replay's own recorded spawn

A fighter's spawn position at match start is *recording-environment* state,
not purely stage data. The retail game resolves it once per match from
`VsSceneController`'s per-slot `spawn_point` (defaulting to the slot index
itself, `gm/gmvs.c`'s `getSpawnPoint`, ~1754) through the per-slot
spawn-assignment loop (~1905-1935) into `gr/stage.c`'s `Stage_80224E64` and
`gr/ground.c`'s `Ground_801C2D24`. Two environments besides vanilla NTSC
1.02 are now confirmed to produce different starting positions there, and a
third is confirmed present but not yet explained (all measured directly
against real recordings, not assumed):

- **Vanilla**: the resource pack's own `stage.spawns`
  (`skirmish-assets`, verified against the retail ISO, md5
  `0e63d4223b01d9aba596259dc155a174`). Every Slippi 2.0.1 console recording
  checked across FD/BF/YS/FoD/PS starts bit-for-bit here.
- **Slippi netplay's "Neutral Spawns"** (`External/NeutralSpawn/
  NeutralSpawn.asm`, injection `8016e510`, confirmed present in public
  Slippi 3.9.0 replays): singles rows indexed by participant order among
  active slots (row index 0/1, the same convention `data.stage.spawns[player]`
  already uses). As exact `f32` literals:

  | Stage | id | P0 | P1 | P2 | P3 |
  |---|---|---|---|---|---|
  | Final Destination | `0x20` | `(-60, 10)` | `(60, 10)` | `(-20, 10)` | `(20, 10)` |
  | Battlefield | `0x1F` | `(-38.8, 35.2)` | `(38.8, 35.2)` | `(0, 8)` | `(0, 62.4)` |
  | Yoshi's Story | `0x08` | `(-42, 26.6)` | `(42, 28)` | `(0, 46.9)` | `(0, 4.9)` |
  | Dream Land | `0x1C` | `(-46.6, 37.2)` | `(47.4, 37.3)` | `(0, 7)` | `(0, 58.5)` |
  | Fountain of Dreams | `0x02` | `(-41.25, 21)` | `(41.25, 27)` | `(0, 5.25)` | `(0, 48)` |
  | Pokemon Stadium | `0x03` | `(-40, 32)` | `(40, 32)` | `(70, 7)` | `(-70, 7)` |

  Teams rows also exist in the same source and are documented here for
  completeness, but are unused (team matches are not implemented,
  `ensure!(!start.is_teams, ...)`): FD `(-60,10),(-20,10),(60,10),(20,10)`;
  BF `(-38.8,35.2),(-38.8,5),(38.8,35.2),(38.8,5)`; YS
  `(-42,26.6),(-42,5),(42,28),(42,5)`; DL
  `(-46.6,37.2),(-46.6,5),(47.4,37.3),(47.4,5)`; FoD
  `(-41.25,21),(-41.25,5),(41.25,27),(41.25,5)`; PS
  `(-40,32),(-40,5),(40,32),(40,5)`. All 22 netplay Dream Land replays
  checked match the singles table exactly.
- **An unidentified console-era codeset**: all 192 checked console 2.0.1
  Dream Land replays start at `(-46.6, 37.0)`/`(47.3891, 37.0)` -- vanilla
  `x`, but a `y` neither table above produces. Not explained by this batch;
  see the corrected Dream Land diagnosis above.

`skirmish_replay::spawn_policy::SpawnPolicy` (`crates/skirmish-replay/src/
spawn_policy.rs`) makes the choice part of `match_validation::Initialization`
explicitly, as a plain, required, serialized field (so a checkpoint's own
resource identity names its spawn policy; two initializations cannot
silently disagree about it while otherwise looking equivalent):

- `Vanilla` -- the pack's own `stage.spawns`, unchanged. The default for
  every non-replay match; behaviour predates this type.
- `SlippiNeutral` -- resolves the table above from `data.stage.name` at
  construction time.
- `Explicit { spawns }` -- caller-supplied positions, in the same
  participant-order convention as `data.stage.spawns`.

Facing is unaffected by any of this: `fighter::entry::spawn_facing` (sign of
the resolved spawn `x`, matching `gmvs.c`'s `fn_8016DEEC`/`direction`) reads
whatever `data.stage.spawns` ends up holding after
`SpawnPolicy::resolve`, so a policy only ever changes positions, never the
facing rule.

`make-initialization` (`crates/cli/src/initialization.rs::build`) always
fills `SpawnPolicy::Explicit` from each port's own frame -123 *post*-frame
position (participant order), not the pack's `stage.spawns`: `post`, not
`pre`, because that is the field the whole validation harness compares
(`observation::expected`'s `post.position`, the `"fighter-post-v11"`
policy), and because the match-start warp-in does not move a fighter within
a frame, so a genuinely-vanilla recording's `post.position` at frame -123
already equals `data.stage.spawns` bit-exactly -- `Explicit` reproduces that
for every codeset, identified or not, rather than assuming vanilla and
failing to align when a recording (like `fox-dl.slp`) turns out not to be.
`build` also returns a read-only `spawn_provenance` diagnostic
(`classify_spawn_provenance`, printed in `make-initialization`'s stdout
summary) naming which known codeset, if either, the replay's own spawn
happens to match -- purely informational, it never changes what
`Explicit` resolves to.

**Every currently published fixture's classification** (pack v10,
`crates/cli/tests/parity_fixture_spawns.rs`, `SKIRMISH_GAMEPLAY_DATA`
pointed at `v10-snapshot-20260913`):

| Fixture | Classification |
|---|---|
| `fox-fd` | vanilla |
| `fox-fd-2` | vanilla |
| `fox-fd-3` | vanilla |
| `fox-fd-4` | vanilla |
| `fox-bf` | vanilla |
| `fox-ys` | vanilla |
| `fox-fod` | vanilla |
| `fox-dl` | unknown_codeset |
| `fox-ps` | vanilla |

(`falco-fox-fd.slp`, the standalone fixture outside `recordings.json`, is
not covered by this table; it is not part of the corpus this test iterates.)

**Re-measurement**, both against pack v10, via `make-initialization`/
`validate-replay` directly (matching the 2026-09-14 sweep's own convention
above): every recording except `fox-dl` is numerically unchanged from the
sweep table above, since `SpawnPolicy::Explicit` is bit-identical to
`Vanilla` whenever a recording's own frame -123 already matched the pack
(true for all of them except `fox-dl`).

| Recording | Matched frames | First divergence | Field | Status |
|---|---|---|---|---|
| `fox-dl` | 74 (was 0) | -49 (was -123) | `position.y` (P3), 1 ULP (`0x4213427c` vs `0x4213427b`) | **moved forward**; genuine one-ULP gap, same class as elsewhere in this document; not diagnosed further |
| `fox-ps` | 85 (unchanged) | -38 (unchanged) | `action_state` (P4) | unchanged; see `fox-ps-baseline.json`'s own note |

`fox-dl-baseline.json` is updated to `74`/`-49`; `fox-ps-baseline.json`'s
numbers are unchanged (its note records the re-measurement). No other
recording's baseline moves in this batch.

## 2026-09-14: Pokemon Stadium's collision joints and `last_ground_id`'s -50 offset

Skirmish's stage loader never modeled a stage's `on_init`: `mpLibLoad`
(`melee/mp/mplib.c:878-936`) sets every collision joint's `CollJoint_Enabled`
bit at load, and Pokemon Stadium's `grStadium_OnInit` (`melee/gr/
grpstadium.c:163-184`) then disables joints 0, 1, 2, 3, 5 and 7 via
`mpLib_80057BC0`, leaving only joints 4 and 6 enabled at match start (the two
belonging to whichever transformation is active; the other six joints' lines
belong to the stage's four other transformations). Every pack through v12
left all eight joints permanently enabled instead, mirroring only
`mpLibLoad`'s pre-`on_init` state -- and v12's own exporter additionally
dropped/renumbered Pokemon Stadium's disabled-transformation lines rather
than keeping the full mpcoll table (the "24 lines from the stage-wide
`dynamic` surface range... dropped as transformation-swap scratch space"
noted against v10 in the tournament-stage batch section above; v12 carries
the same gap). `fox-ps`'s own `last_ground_id` observation against pack v12
diverges from the recording by a constant -50 across every frame it's
checked, rather than scattered per-frame noise -- exactly the signature of a
shortened/renumbered line table: every ground id past the cut lines up 50
short of the recording's real, original-order index, not a physics
disagreement.

This batch adds `collision::stage::Joint::enabled_at_start: Option<bool>`
(absent, i.e. every pack through v12, means enabled, so those packs load and
behave byte-identically) and a precomputed per-line active mask
(`active_lines`) that `Stage::line_active` -- the single choke point every
line-enablement check in `collision::stage` goes through (`neighbor`, used
by `extended_endpoints`/`project`'s adjacency walk, and `sweep_filtered`'s
per-line candidate check) -- folds in alongside each line's own `ENABLED`
bit. A line confined to a joint disabled at stage start (including that
joint's `dynamic` range, which `mpJointListAdd`/`mpLib_80057BC0`,
`melee/mp/mplib.c:5415-5481`/`5508-5566`, toggle identically to the four
static ranges) takes no part in any query -- no landing, no wall/ceiling
contact, not eligible for ground snapping or ledge grabs -- while keeping
its original table index, since `last_ground_id` and any other line-id-
bearing observation must stay comparable to the recording's.

This closes the *modeling* gap, not `fox-ps`'s own baseline: the -50 offset
is a pack-content problem (the dropped/renumbered lines), and
`enabled_at_start` only lets a complete, correctly indexed table encode
which joints start disabled. v13 is expected to re-export Pokemon Stadium's
full, unrenumbered line table alongside this flag; only then does
`fox-ps`'s `last_ground_id` divergence become a fair test of this batch's
change. `fox-ps-baseline.json` is unchanged by this batch.

## 2026-09-14: post-rotation/post-muzzle-fix Blaster hits land one frame *early*, not late -- swept-vs-point falsified, root cause still open

The root-facing rotation (`13f675d`) and the pre-physics muzzle fix
(`0111600`, "Fire the Blaster's muzzle from this frame's pre-physics
fighter position") together narrow the Blaster-hit geometry considerably
from the coarse, ~3.4-unit-short miss this document's own `fox-fd-2.slp`
"2026-09-13, published pack v10" entry above measured, and change its
*shape*: against pack v13, three recordings now diverge on their own
`percent`/`last_attack_landed` transition landing one Skirmish-internal
frame **before** the recording, not after -- the opposite direction from
every laser-timing gap this document previously recorded (the "2026-09-13"
entry above, and `fox-fd-4-baseline.json`'s/`fox-bf-baseline.json`'s own
committed notes, both describe a one-frame-*late* hit, fixed at the
source by the muzzle-bone/ray-scale work those notes cite). Measured
directly (`make-initialization` + `game::Match::step` driven frame-by-frame
with each recording's own real controller inputs, bypassing
`validate-replay`'s stop-on-first-mismatch the way this document's own
prior "direct, uncommitted probe" entries do):

- `fox-fd-2.slp`: recording's `last_attack_landed`/`percent` transition
  lands at frame -5 (P1/P2 respectively); Skirmish's own transition lands
  at -6 (`checked_frames: 117`, matching the current
  `fox-fd-2-baseline.json` regression this loop leaves unresolved).
- `fox-fd-4.slp`: recording's transition (the *first* of two Blaster hits
  in this match) lands at -13; Skirmish's lands at -14
  (`checked_frames: 109`).
- `fox-bf.slp`: recording's transition lands at frame 26; Skirmish's lands
  at 25 (`checked_frames: 148`; this recording's own committed baseline,
  297/174, predates the rotation and records an unrelated Down-Special
  `action_age` freeze at frame 174 -- see that baseline's own note -- not
  reachable again until this Blaster-timing gap is fixed, so `fox-bf`
  stays red against its own committed baseline regardless of this
  investigation's outcome).

**Decomp ordering confirmed to match Skirmish's own, ruling out a
data-staleness/ordering bug.** By GObj priority within one frame, item
position integration (prio 4) and item commit (prio 5, `it/item.c`) run
before fighter physics (prio 6, sets root JObj to the post-move `cur_pos`)
and fighter skeleton/hurtbox matrix setup (prio 7), both of which run
before the item-vs-fighter hit test itself (`Fighter_8006CB94`, prio
`0xD`/13, calling `ftColl_8007925C`, `ft/fighter.c:2625-2643`,
`ft/ftcoll.c:1999-2270`) -- so by hit-test time, both the item and the
fighter are already post-move for the current frame. `simulation::advance`
matches this exactly: `move_fighter`/`collision::resolve` run first, the
resulting post-move `poses` are built next, and only then does
`projectile::advance` run (which itself advances the projectile's own
position before its own hurtbox test). No reordering fix applies here.

**Swept-vs-point hitbox-shape hypothesis, falsified across the full
hitbox grid.** The projectile hitbox capsule `projectile.rs`'s Hurtbox
block tests against each victim hurtbox is built as a *swept* segment
(`previous_offset` -> `offset`, i.e. last frame's position to this
frame's), while decomp's own single-current-frame damage-hit path
(`ftColl_8007925C` -> `lbColl_8000805C` -> `lbColl_80006E58`) reads as a
plausible point-only test by contrast -- a shape mismatch that would
produce exactly a one-frame-early bias. Directly measured (a temporary,
env-gated instrumentation pass computing, for every hitbox x every victim
hurtbox pair on the frame each recording's own Skirmish transition lands,
both the real swept-segment separation and a hypothetical point-only
(this-frame-endpoint-only) separation; not committed, removed after
measurement):

- `fox-fd-2.slp` at its own wrong-early frame -6: exactly one negative
  (overlapping) pair in the whole grid, hitbox 0 (the leading capsule)
  against hurtbox 7, separation -0.372 -- already overlapping using only
  this frame's own endpoint, no sweep required.
- `fox-fd-4.slp` at its own wrong-early frame -14: three simultaneous
  negative pairs, all on hitbox 0, against hurtboxes 1 (-1.262), 2
  (-2.497) and 5 (-0.270) -- more decisively already-overlapping than
  `fox-fd-2.slp`.
- `fox-bf.slp` at its own wrong-early frame 25: the pair the real (swept)
  code actually fires on, hitbox 0 against hurtbox 2, does miss under a
  point-only test (its swept segment, `prev_offset=5.055 -> offset=
  -1.459`, crosses hurtbox 2's span `[1.89, 2.71]`, but the endpoint alone
  falls short). However, hitbox 1 (the second capsule) against the *same*
  hurtbox 2 is independently at separation -0.116 -- also a genuine,
  non-swept, point-time overlap, on the identical frame, one frame clean
  (+6.48) the frame before. Because the real loop breaks on the first
  overlapping hitbox in index order, it never reaches hitbox 1 today; a
  point-only change would only make it reach hitbox 1, which fires on the
  same wrong-early frame anyway.

So a point-only fix would not move any of the three recordings' hit
frames: two show the wrong-early overlap on the exact pair the current
code already uses, and the third (`fox-bf.slp`) would just fire from a
different capsule index on the identical frame. Not implemented, per this
document's own convention of reporting a falsified hypothesis rather than
applying it anyway.

**`fox-fd-3.slp` checked as a fourth candidate and ruled out as a control,
not a bug case.** This recording's own second Blaster shot (spawned -14,
a trailing capsule -- hitbox 2, offset -6.51, not hitbox 3/-14.06 as
first suspected -- catching P4's head, bone 41, mid-jump) was measured
the same way: at frame -4, the recording's own `last_attack_landed`/
`percent` already agree bit-for-bit with Skirmish's (both `18`/`3.0`);
the only residual is a ~1e-5-magnitude position delta (e.g. `-17.047989`
vs. the recording's `-17.047997`), consistent with ordinary float-
accumulation drift, not a timing bug. This recording does not exhibit the
one-frame-early symptom and is not counted among the three above.

**Victim side confirmed bit-exact throughout; projectile-side ground
truth unavailable for the three recordings that do show the bug.** A
wide-window probe (roughly 20-30 frames before each shot's own spawn
through its hit frame + 1) found the victim fighter's own position
matching the recording exactly (delta `0.000000` to six decimal places)
at *every* frame in that window, for all three recordings -- ruling the
victim/hurtbox side out entirely. The equivalent check on the projectile
side is not possible for `fox-fd-2.slp`, `fox-fd-4.slp` or `fox-bf.slp`:
all three are recorded on Slippi client `2.0.1`
(`tests/fixtures/slippi/parity/manifest.json`), and
`crates/peppi-adapter/src/envelope.rs`'s own `Event::Item` handling
requires `version.gte(3, 0)` ("Item events are unavailable before Slippi
3.0") -- these files genuinely never recorded item/projectile position
data, not a parsing gap. Only `fox-fd-3.slp` (Slippi `3.9.0`, this
document's own "the only one recorded on a newer Slippi client" note
above) carries it, which is exactly why `real_parity_laser_muzzle.rs`
could hardcode real recorded laser positions only for that file -- and
that recording is the control case above, not one of the three with the
bug.

**Conclusion, most likely explanation, not confirmed by direct
measurement.** With the ordering match, the swept-vs-point falsification
and the victim-side/no-compounding-mechanism findings above, the
remaining candidate consistent with everything measured is the same
already-documented, already out-of-scope bone-animation-sample muzzle
residual `real_parity_laser_muzzle.rs`/`docs/validation.md`'s own
2026-09-14 entry already track (`RESIDUAL = [0.19955, 0.35594]`, a
spawn-time-only offset that -- since neither the recording's nor
Skirmish's own laser re-accelerates after spawn, `projectile.rs`'s own
module doc, confirmed here by an exactly-constant `±7.0`/frame position
delta in Skirmish's own trajectory for all three shots -- would stay
constant rather than grow across the shot's flight, the same algebra that
document's own entry already uses). This is inference from the absence of
any contrary evidence, not a direct measurement: no ground truth exists to
confirm it for these three recordings, per the Slippi-version gap above.
Reported per this loop's own stop condition (the original task's own
explicit fallback for "decomp order already matches Skirmish's") rather
than guessed at further; no baseline file moves as a result of this
entry.

## Practical consequence

None of these three, individually or together, is "Skirmish matches Melee."
Level 1 rules out a class of arithmetic bugs in ported functions. Level 2
guards the test harness against regressing on itself. Level 3 is the only one
that touches an independent recording, and even it is scoped to one matchup,
one stage, and a fixed field set. Treat a "matched" `real_parity` result as
"no evidence of disagreement was found in what was checked," not as a
certification.
