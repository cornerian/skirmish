# Local validation provenance

The 2026-09-11 idle-animation coverage batch is recorded at:

`/mnt/archive/runs/skirmish-idle-animations-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and
release modes. `game::idle` (wiring: the resource, the state, the per-frame
animation-phase draw, `src/game/idle.rs`) and `fighter::idle` (the pure
weighted pick and its re-draw gate, `src/fighter/idle.rs`) cover
`Action::Wait`'s idle-animation cycling (`docs/idle.md`). Two new resources:
`FighterData.idle: Option<idle::IdleAnimations { wait1_length: f32, entries:
Vec<IdleEntry { animation: u32, weight: i32, length: f32 }> }>` and
`Fighter.idle: idle::State { animation: u32, frame: f32 }` (reset to `{ 2,
0.0 }` on every action transition, unconditionally like `dash::State`/
`smash::State`, since both fields are read only while `Action::Wait` is
current). Every Wait frame, once the current sub-motion's own tracked frame
reaches its length: with an empty (or absent) table, restart with no RNG
draw; otherwise draw from the match's shared `HsdRng` and walk the weighted
table (`fighter::idle::pick`), re-drawing while a repeated pick is neither
Wait1_0 (2) nor a fresh pick from Wait1_0/31. The draw happens in
`simulation::advance`'s per-player animation-phase loop, in player order,
which reassigns `state.rng_seed` before the same frame's blast-zone death
draw reconstructs its own `HsdRng` from it -- so an idle draw always shifts
a later death draw's position in the shared sequence, exactly as Melee's
single per-frame RNG stream would. `crates/skirmish-replay/src/
observation.rs`'s `animation_index` gains a dedicated `14 => fighter.idle.
animation` arm (previously `12..=14 => 2` covered Rebirth/RebirthWait/Wait
alike with the same constant; Rebirth/RebirthWait are out of this batch's
scope and keep the constant). `action_age` needed no change, since Wait was
already covered by `observe`'s catch-all `action_frame` branch and this
batch's own `action_frame` reset on every restart/pick keeps that branch
correct. With the resource absent, `Action::Wait` keeps its pre-batch
behavior unchanged: `action_frame` grows without bound and the reported
animation index stays 2.

No contradiction between this batch's implementation and the pinned source
was found; the pre-existing test suite remains green unchanged. This
batch's own explicit modeling choices, documented in `docs/idle.md` rather
than treated as verified source behavior: `ftAnim_IsFramesRemaining`'s real
joint-based check (`ftanim.c:515-530`) is replaced by a tracked-frame/
length comparison, not independently reverse-verified against `ftAnim`/
`lbAnim`'s own bookkeeping (the same caveat the walk/run batches recorded
for their own frame-length wrap rules); every character's Wait entry is
assumed to set `fp->anim_id` to 2 (Wait1_0) via `Fighter_ChangeMotionState`'s
own per-character table lookup (`fighter.c:1220`), since that table itself
is compiled character data, not part of the decomp source available here.
Item-holding, the Mewtwo/Fox exception (`ftwaitanim.c:66-69`) and Wait
script bytecode execution are pre-existing gaps (no `Fighter` item-holding
state or script-bytecode execution exists anywhere in this codebase) left
unmodeled, not introduced or silently changed by this batch.

`src/fighter/idle.rs` adds 5 unit tests for `pick`'s boundary selection,
the Wait1_0/31 exemption, the non-exempt re-draw and the "table falls short
of `max`" panic. `tests/game_idle.rs` (7 tests, `tests/support/idle.rs`
supplies an invented two-entry table) covers: a table-less restart at
`wait1_length` leaving the animation at 2, resetting the tracked frame and
`action_frame`, and drawing no RNG; a two-entry table's pick matching
`pick` called directly against a fresh `HsdRng` from the same match seed;
a repeated pick from a non-Wait1 current animation re-drawing, with the
seed advancing by two draws for that event; the idle draw shifting a
same-frame blast-death roll's `Kind` between resource-present and
resource-absent runs of an otherwise-identical real hit-driven scenario (a
grounded fighter can never itself exceed a valid `blast.top`, since stage
validation requires `floor.y < blast.top` strictly, so this reuses `tests/
support/death.rs`'s own proven hit/knockback shape rather than a synthetic
position); checkpoints restoring the idle state and seed exactly across a
pick; invalid resources (non-finite/non-positive `wait1_length` or entry
fields, weights summing below 100) rejected before a match exists; and
`None` keeping the pre-batch unbounded integer `action_frame` with the
animation index fixed at 2. `tests/idle_differential.rs` (8 tests, 512
proptest cases for the pick comparison) pins `ftCo_8008A698`/
`ftCo_8008A6D8`/`inlineA0`/`getAnimID`/`ftCo_8008A7A8` from a new
`waitanim` snapshot of `ftwaitanim.c`; `fighter::idle::pick` is compared
against the C oracle over equal-share-weight tables of 1..=5 entries,
current animations in `{2, 3, 6, 31, 40}` and 64-value scripted `HSD_Randi`
sequences, plus exact boundaries (`max == count` inclusive selection, the
repeat re-draw in both implementations, Wait1_0/31 never re-drawing,
weights summing below 100 asserting in the oracle and panicking in `pick`,
frames still remaining, and an empty table restarting without drawing).
The oracle adapter's `HSD_ASSERTREPORT` override `longjmp`s back to the
call site rather than falling through a failed table walk, reproducing the
pinned source's own "never returns" contract (the real `__assert` it calls
is `ATTRIBUTE_NORETURN`, and `getAnimID` has no `return` after its own
`HSD_ASSERTREPORT` call) -- an earlier version of the adapter let execution
fall through instead, reading an undefined return value that the caller
then used as an out-of-bounds `fp->x24`/`x28` array index and crashed the
test binary; this was found and fixed during this batch's own
implementation, not left as a discrepancy. `crates/cli/tests/
replay_match.rs`'s `file_backed_idle_fighter_reports_the_picked_animation_
index_with_a_restarting_age` (1 test) drives an idle fighter with the
two-entry table, locates the restart/pick row from a trace of the tracked
idle frame, confirms the reported animation index and restarted
`action_frame` there, matches its own generated Peppi bytes end to end,
then corrupts that row's animation-index field and confirms a `Mismatch`
starting there.

The 2026-09-11 run-corrections batch is recorded at:

`/mnt/archive/runs/skirmish-run-corrections-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. This batch fixes three behaviors the run-animation-rate batch below
audited and reported as discrepancies/gaps rather than silently changing,
out of that batch's stated scope; no further contradiction against the
pinned source was found while fixing them (`docs/run.md`'s "Corrections"
section has the full detail with source line citations).

RunTurn's flip check (`game::locomotion::update_animation`'s `Action::
RunTurn` arm) now reads `f.locomotion.run_turn_facing` -- the per-entry
facing `start_run_turn` already captured for the physics branch -- instead
of the removed `Parameters::run_turn_velocity_scale` fixed resource
constant, matching `ftCo_TurnRun.c:67`'s `facing_at_entry * gr_vel <=
0.01F` exactly. `run_turn_velocity_scale` is removed from `Parameters`
(`deny_unknown_fields` required dropping it from every fixture).

RunBrake's velocity-gated marker freeze (`ftCo_RunBrake.c:49-77`) is now
modeled: `Parameters` gains `run_brake_marker_frame: Option<u32>` and
`run_brake_freeze_speed: Option<f32>` (paired; `None` keeps the pre-batch
unfrozen behavior), `locomotion::State` gains `run_brake_frozen: bool`, and
`hold_action_frame` now also freezes `action_frame` while RunBrake is
frozen, alongside its existing RunTurn case. The `frames` countdown keeps
ticking regardless and can still end the brake into `Wait` while frozen.

`run.x0`, the turn-run lockout gating `ftCo_Run_IASA`'s RunTurn/RunBrake
entry (`ftCo_Run.c:125-126`), is now modeled: `Parameters` gains
`run_turn_lockout_frames: Option<f32>` (`x430`), `locomotion::State` gains
`run_lockout: f32`, set by `enter_run`'s new `lockout` parameter (`0.0`
from both Dash-to-Run call sites, `run_turn_lockout_frames.unwrap_or(0.0)`
from a RunTurn-to-Run re-entry) and counted down every Run animation frame
independent of `MovementData.run_animation`. Both `game::locomotion::
update_actions`'s `Action::Run` arm and `game::dash::update_dash_or_run`'s
Run arm now gate their RunTurn/RunBrake entry checks behind `run_lockout <=
0.0`, matching `ftCo_Run_IASA`'s own gate order.

`tests/game_run_corrections.rs` (9 tests) covers: a right-facing and a
left-facing run-turn reversal each flipping only once ground velocity has
crossed past the *entry* facing's own sign, with the frozen frames holding
`action_frame`; the RunBrake freeze holding frames while fast and resuming
once slow with the countdown still ticking; the freeze speed never reached
still ending the brake on the countdown while frozen; the marker absent
keeping the pre-batch unfrozen behavior; the lockout blocking both RunTurn
and RunBrake entry from Run for exactly its frame count (a neutral and a
reversed stick, branched from the same pre-lockout checkpoint via a cloned
`Match`); an ordinary Dash-to-Run entry never setting the lockout;
checkpoints mid-lockout; and every new field's invalid/unpaired/
out-of-range values rejected before a `Match` exists.
`tests/turn_run_anim_differential.rs` (7 tests, 512+512 proptest cases)
extends the pinned `turn_run.c` snapshot (already used for
`ftCo_TurnRun_Phys`) with `ftCo_TurnRun_Enter`/`_Anim`/`fn_800C9CEC`/
`fn_800C9D40`, comparing a Rust mirror of the freeze/resume/flip/
animation-end decision against the C oracle bit-exactly, including a
generated 8-frame velocity sequence per fixed facing threaded frame to
frame. `tests/runbrake_differential.rs` (5 tests, 512+512 proptest cases)
snapshots `ftCo_RunBrake.c` for the first time
(`tests/oracle/original/runbrake.c`, `ftCo_RunBrake_Anim`/`_IASA`),
comparing a Rust mirror of the freeze/resume/end decision against the C
oracle, including a generated 12-frame, physically realistic
(monotonically non-increasing magnitude) velocity sequence, plus every
boolean combination of the IASA gate order. `tests/run_differential.rs`
gains `run_iasa_lockout_gate_order_matches_c` (extending `ftCo_Run_IASA`
into the pinned `run.c` snapshot), proving the jump check always runs
first and the RunTurn/RunBrake checks only run while `run.x0 <= 0.0`, over
every scripted boolean combination and `run_x0` in `{-1, 0, 1, 5, NaN}`.

The 2026-09-11 run-animation-rate coverage batch is recorded at:

`/mnt/archive/runs/skirmish-run-animation-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::locomotion`/`fighter::locomotion` extend the walk batch's float
animation-frame model (`docs/walk.md`) to `Action::Run` (`docs/run.md`) with
one new resource, `MovementData.run_animation: Option<RunAnimation { length,
scaling }>` (the Run figatree's frame count and `run_animation_scaling`,
`types.h:698`) -- unlike Walk, not paired with any `Rules` entry, since Run
has no kind-selection thresholds. `game::locomotion::State.run: RunState
{ frame, last_rate }` tracks a float animation frame that advances one frame
behind `ftAnim_SetAnimRate`'s own delay and wraps at the Run figatree's
length; both of this codebase's reachable Run entries (Dash-to-Run and
RunTurn-to-Run, via the new `game::locomotion::enter_run`) always start it at
frame 0.0 with `last_rate` 1.0, matching the pinned source's fixed
`ChangeMotionState` rate argument. Slippi's `action_state`/`animation_index`
for Run were already correct (21/13, unchanged); `action_age` now publishes
the tracked float frame instead of the integer `action_frame` while the
resource is present. With the resource absent, Run keeps its pre-batch
behavior unchanged: state 21 with an integer `action_frame` published as
`state_age`, rate 1 always.

Two discrepancies against the pinned source were found and reported rather
than silently fixed (both out of this batch's scope, the Run animation-
frame/rate layer only; documented in full in `docs/run.md`): RunBrake's own
velocity-gated marker freeze (`cmd_vars[1]`/`x42C`, `ftCo_RunBrake.c:49-77`)
is not modeled anywhere in this codebase, contrary to an earlier design
draft's assumption that it was -- only the unrelated `mv.co.runbrake.frames`
countdown exists; and `game::locomotion`'s existing RunTurn flip check
(`update_animation`'s `Action::RunTurn` arm) multiplies by a fixed
`Parameters::run_turn_velocity_scale` resource constant instead of the
per-entry facing the pinned source actually uses (`ftCo_TurnRun.c:67`,
`facing_at_entry * gr_vel <= 0.01F`, sourced from `ftCo_TurnRun_Enter`'s own
`turnrun.accel_mul = facing_dir`) -- `game::locomotion::start_run_turn`
already captures that exact per-entry value as `run_turn_facing` for the
physics branch, but the flip check does not use it. [Corrected by the
2026-09-11 run-corrections batch above: the flip check now reads
`run_turn_facing` and `run_turn_velocity_scale` is removed.] Both are
pre-existing gaps in already-audited code, left unchanged per this batch's
"verify, don't silently change" instruction. [RunBrake's own freeze is also
corrected by the run-corrections batch above.] Separately, `run.x0` (the
turn-run lockout countdown gating `ftCo_Run_IASA`'s RunTurn/RunBrake entry,
`ftCo_Run.c:125-126`) remains unmodeled, as it was before this batch --
outside this batch's resource/state shape, which covers only the
animation-frame/rate layer. [Also corrected by the run-corrections batch
above.]

This batch's own explicit modeling choices, documented in `docs/run.md`
rather than treated as verified source behavior: `last_rate` is initialized
to 1.0 at Run entry (mirroring `Fighter_ChangeMotionState`'s own `rate = 1`
argument, the same choice the walk batch made for Walk), and the `frame >=
length` wrap rule (not independently reverse-verified against `ftAnim`/
`lbAnim`'s own loop bookkeeping, the same caveat the walk batch recorded).
`run_animation_rate` is not a call into the existing `walk_animation_rate`
(identical branch shape/order, but Run has a single figatree/scaling
constant rather than three kind-indexed ones, so a shared call would need a
fabricated `rates` array and an unused kind axis); this is noted rather than
treated as an unexplained duplication.

`src/fighter/locomotion.rs` adds 1 unit test for the pure `run_animation_rate`
zero-rate/facing/`x4`-branch selection, mirroring `walk_animation_rate`'s own
test. `tests/game_run.rs` (7 tests) covers: a Dash-to-Run transition entering
at frame 0.0 with `last_rate` 1.0; the first Run frame afterward advancing by
exactly 1.0 and every later frame matching the pure helper bit-exactly
against the previous frame's velocity/facing; wrapping at the figatree
length; checkpoints; invalid resources; `None` keeping the pre-batch integer
frame through a real Dash-to-Run transition; and a Run-to-RunTurn-to-Run
reversal confirming `ground_velocity` already agrees with the flipped facing
on every observed Run frame, documenting (not asserting) that the zero-rate-
against-facing branch is not independently reachable through an actual Run
frame in this codebase. `tests/run_differential.rs` (4 tests, 512 proptest
cases for the rate comparison) pins `ftCo_Run_Enter`/`_Full`/`_Anim` from a
new `run` snapshot of `ftCo_Run.c`; `run_animation_rate` is compared over the
full binary32 domain including NaN, plus exact boundaries (velocity exactly
0, negative velocity, velocity negative with facing also negative, scaling
as small as `f32::MIN_POSITIVE`/`1e-30`, the friction-multiplier branch, and
the `run.x0` countdown at several values); the oracle's own `run.x0`
countdown and `ftCo_Run_Enter`'s literal field assignments are checked
directly against the source lines, since no Rust helper models them.
`crates/cli/tests/replay_match.rs`'s
`file_backed_dash_into_a_run_reports_state_21_with_float_ages_and_a_reduced_
stick_mismatch` (1 test) drives a dash into a run, confirms every Run row
reports state 21/animation 13 with the tracked float frame, matches its own
generated Peppi bytes end to end, then reduces a stick sample well into the
ramp and confirms a `Mismatch` starting at that row -- and separately
confirms, by re-simulating with the same edited input directly, that the
reported age itself only starts to differ two rows later (Anim precedes the
ground-movement physics that reacts to the edited stick).

The 2026-09-11 walk-speed variants coverage batch is recorded at:

`/mnt/archive/runs/skirmish-walk-speeds-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::locomotion`/`fighter::locomotion` subdivide `Action::Walk` into
WalkSlow/WalkMiddle/WalkFast (`docs/walk.md`) by `|ground_velocity|` against
two new resources: `MovementData.walk_animation` (the three figatree lengths
and animation-rate divisors) and `Rules.walk` (the middle/fast selection
thresholds), paired the same way as the edge/teeter and escape/escape-air
resources. `game::locomotion::State.walk: WalkState { kind, frame, last_rate
}` tracks the current kind and a float animation frame that advances one
frame behind `ftAnim_SetAnimRate`'s own delay and wraps at the current kind's
figatree length; a velocity crossing re-enters Walk (a genuine
`ChangeMotionState`, so `action_instance.id` is unaffected since Walk and
Dash already share motion identity 102) with the frame remapped
proportionally into the new kind's length, reproducing the source's
truncating quotient/remainder/remap chain exactly
(`fighter::locomotion::walk_retype_frame`). Slippi's `action_state` now
reports 15/16/17 by kind with animation indices 7/8/9, and `action_age`
publishes the tracked float frame instead of the integer `action_frame`
while the resource is present; `action_frame` itself is kept for Walk's own
internal bookkeeping. With either resource absent, Walk keeps its pre-batch
behavior unchanged: a single state (15/7) with an integer `action_frame`
published as `state_age`.

No discrepancy against the pinned source was found; the existing walk
physics (`getWalkAccel`/`ftWalkCommon_800E0060`, already audited by an
earlier batch) is unchanged. This batch's own explicit modeling choices,
documented in `docs/walk.md` rather than treated as verified source
behavior: `last_rate` is initialized to 1.0 at Walk entry (mirroring
`Fighter_ChangeMotionState`'s own `rate = 1` argument, not independently
confirmed against the animation system's own sub-frame scheduling), and the
`frame >= length` wrap rule (not independently reverse-verified against
`ftAnim`/`lbAnim`'s own loop bookkeeping).

`src/fighter/locomotion.rs` adds 3 unit tests for the pure `walk_kind`
threshold/sign arithmetic, `walk_animation_rate`'s zero-rate/`x0`-branch
selection and `walk_retype_frame`'s truncating remap chain.
`tests/game_walk.rs` (9 tests) covers: Slow/frame-0 entry from Wait;
a full-stick ramp (kept below the dash-magnitude threshold throughout)
reaching Middle then Fast with a stable `action_instance.id` and every
remapped frame bit-exact against the pure helper; the animation rate's
one-frame delay and its zero value while moving against facing, checked
bit-exactly across a whole ramp; wrapping at the Slow kind's length; Wait-
chain exit on a reversed or below-threshold stick, with Walk's own Anim-
phase advance for that exit frame still accounted for; a tilt press
preempting the retype check on its own frame; checkpoints; invalid
resources; and `None` keeping the pre-batch behavior.
`tests/walkcommon_differential.rs` (5 tests, 512 proptest cases each for the
type/rate/retype helpers) pins `ftWalkCommon_GetWalkType`,
`ftWalkCommon_800DFC70`, `ftWalkCommon_800DFCA4`, `ftWalkCommon_800DFDDC` and
`ftWalkCommon_800DFEC8` from a new `walkcommon` snapshot of `ftwalkcommon.c`
(distinct from the existing `ftwalk` adapter, which pins the same upstream
file's unrelated walk-physics functions), mapping the `static inline`
`..._800DFBF8_fake` duplicate to the pinned, non-static
`ftWalkCommon_GetWalkType` in the adapter. `walk_kind`/`walk_animation_rate`
are compared over the full binary32 domain including NaN; `walk_retype_frame`
is scoped to the finite, positive frame/length domain this codebase ever
calls it with (C's float-to-`s32` truncation is undefined outside it, unlike
Rust's saturating cast), with a fixed threshold/velocity table that
deterministically reaches every `(cur_kind, new_kind)` pair including the
same-kind no-op. `crates/skirmish-replay/src/observation.rs`'s existing
common-action/animation-id table test adds direct Middle/Fast kind coverage
(16/8, 17/9). `crates/cli/tests/replay_match.rs`'s
`file_backed_walk_ramp_reports_15_16_and_17_with_float_ages_and_a_reduced_
stick_mismatch` walks a full ramp, confirms every recorded row's reported
state/animation matches its current kind and that the ramp reaches Middle
then Fast, matches its own generated Peppi bytes (including the float
`state_age`), then reduces the first retype row's stick sample and confirms
a `Mismatch` starting at that exact row.

The 2026-09-11 floor-end collision modes and edge-teeter coverage batch is
recorded at:

`/mnt/archive/runs/skirmish-floor-ends-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::collision` now derives one of three floor-end rules
(`CollisionMode::{Plain,Clamp,Teeter}`, `docs/edges.md`'s per-action table,
including `Catch`/`CatchDash` and `RunTurn` alongside every other mode-2
action) from the fighter's action wherever the ordinary grounded floor
projection fails past a line end: Plain keeps the previous fall-off
behavior; Clamp holds the position at the end (`Fighter.edge_contact`, no
facing/stick condition, `gr_vel` untouched); Teeter does the same clamp only
when facing and stick admit it (mpcoll's own 0.75 literal, `rules.edge.
teeter_stick_limit`), then enters the new `Action::Ottotto`/`OttottoWait`
(Slippi 245/246, animation 210/211) -- a zero-velocity, no-physics teeter
that exposes the full Wait input chain (catch, smash, tilt, jab, shield,
jump, dash, squat, turn all work unmodified through the existing
`tilt::interrupt_chain` mechanism) plus its own walk-away threshold
(`rules.edge.teeter_walk_threshold`) and exit-to-Wait distance check
(`rules.edge.teeter_exit_distance`/`teeter_exit_tolerance`). This corrects
the 2026-09-10 shield-escape batch's "falls off a floor edge" expectation
for rolls and the spot dodge (mode 2, always clamp); that entry below is
annotated with a bracketed correction rather than rewritten.

Two bugs were found and fixed as part of this batch. `locomotion::
update_actions` computed `interruptible_tilt` once, before dispatching the
jump-squat entry; entering `Action::JumpSquat` did not return, so the shared
Wait/Walk arm below (whose guard still matched on the stale
`interruptible_tilt`) ran again the same frame and could overwrite the fresh
JumpSquat with Walk whenever the jump press carried an admissible walk
stick -- reachable from Ottotto/OttottoWait but not unique to them. Every
Wait-chain IASA is a `RETURN_IF` chain that returns immediately once a jump
fires (`ftCo_Wait_IASA`: `RETURN_IF(ftCo_Jump_CheckInput(gobj))`), so
`update_actions` now does too; see `docs/edges.md` for why no other action
entry in that function shares the exposure. `tests/game_edges.rs`'s
`jump_squat_overwrites_walk_only_after_it_wins_not_before` covers an
interruptible smash pose, an interruptible jab pose and Ottotto itself.
Separately, `passive_wall_launch_differential.rs`'s bit-exact proptest
compared two independently computed NaN results by raw bits; NaN payload
propagation through arithmetic is unspecified, so a generated case landing
on two distinct quiet-NaN payloads (0x7FC00001 vs 0x7FC00002) was a
test-design flake, not a divergence. Hardened to assert only "both NaN" when
either side is NaN, otherwise the exact bits as before, matching the
existing `same_float`-style idiom already used by several other differential
tests (e.g. `ground_launch_differential.rs`). The same audit found three
more differential tests feeding arbitrary `any::<u32>()` bits into real
floating-point arithmetic (not a pure copy/selection, which cannot diverge)
without that guard, and applied it there too: `dash_differential.rs`
(`apply_friction`'s addition), `escape_air_differential.rs` (`decay`'s
multiplication) and `grab_mash_differential.rs` (the mash timer's `-=
penalty`); `shield_grab_differential.rs` (the dash-grab buffer's `-= 1.0`)
already had an equivalent flake risk and is hardened the same way.
`clank_differential.rs`, `hit_direction_differential.rs`,
`jab_differential.rs`, `landing_differential.rs`, `smash_differential.rs`
and `wall_jump_differential.rs` were checked and found not to need it: each
either rejects non-finite inputs before any arithmetic, constrains its
proptest generator to a finite range, or only copies/negates/selects an
input's exact bits without combining it arithmetically with another value.

`tests/game_edges.rs` (14 tests) covers attacks (a forward smash's root
motion, a supplied root-motion dash attack) and a roll reaching a floor end
clamping and staying grounded; walking into an admissible teeter entering
Ottotto with zero velocity then OttottoWait; an outward stick at exactly the
0.75 limit and a fighter facing away (pushed by an overlap nudge) both
falling instead; Dash and Run past an end falling as before; every
Wait-chain dispatcher reachable from Ottotto, including a same-direction
dash correctly carrying the fighter off the cliff it was teetering at rather
than staying frozen, and jump correctly reaching JumpSquat instead of being
overwritten by Walk; the teeter walk-away threshold; checkpoints; invalid
`rules.edge`/`fighter.teeter` combinations; and `rules.edge = None` keeping
Wait/Walk falling while a smash still clamps. `src/fighter/edge.rs` adds 5
unit tests for the pure floor-end decision, teeter gate, clamped position
and exit-distance arithmetic. `tests/edge_differential.rs` (3 tests, 512
proptest cases) compares that same pure decision against pinned
`mpColl_8004A45C_Floor`/`mpColl_8004A678_Floor` (a new `edge_floor` alias
adapter reusing the existing `mpcoll` snapshot) over generated positions,
facings, sticks and wall answers plus exact boundaries; the search for the
real invariant behind an early spurious mismatch (`ecb.bottom.x` is always
exactly 0.0, enforced by every ECB loader) is recorded in `docs/edges.md`.
`tests/ottotto_differential.rs` (6 tests, 512 proptest cases) pins
`ftCo_Ottotto.c`'s complete `ftCo_Ottotto_IASA` call order and the shared
Coll fall/exit decision against a Rust mirror. `tests/walk_differential.rs`
(3 tests, 512 proptest cases) pins `ftCo_Walk_CheckInput_Ottotto` from a new
snapshot of `ftCo_Walk.c` (distinct from the existing `ftwalk` adapter,
which pins the unrelated `ftwalkcommon.c`). Peppi-written replays match a
walk into a floor end (states 245/246, animations 210/211) that teeters and
then jumps out, and report a Mismatch at the exact row the walk crosses the
edge when that row's stick sample is pushed out to precisely 0.75 instead of
an admissible value.

The 2026-09-11 state-parity coverage batch is recorded at:

`/mnt/archive/runs/skirmish-state-parity-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `observation::action_state` and `animation_index` now map FallSpecial
to 35/26 instead of 31/22, which were FallB's numbers; FallF/FallB and
FallSpecialF/FallSpecialB stay unmodeled, since `ftCo_Fall_Anim_Inner` swaps
their blend skeleton through `ftAnim_8006EDD0` without writing `fp->anim_id`.
An optional `locomotion::Parameters.jump_backward_threshold` (`ftCommonData.
x78`) reports a backward ground or aerial jump through its own Slippi id
(26/17, 28/19 instead of the ordinary 25/16, 27/18), selected by the exact
stick/facing test `ftCo_Jump_Enter` and `ftCo_JumpAerial_Enter_Basic` run at
their own launch frame, with equality on the threshold selecting backward;
`None` keeps every jump forward. An aerial jump's own animation end now
reports the aerial variant of Fall (32/23) instead of the ordinary 29/20;
every other Fall entry (an aerial attack's own end, walking off a platform, a
ledge drop) is unaffected. `tests/game_jump_variants.rs` covers forward and
backward short/full hops including the exact threshold boundary, a backward
double jump independent of its preceding ground jump, both new flags clearing
on every other transition, checkpoint replay through all three flagged
phases, `None` and invalid-threshold rejection. `tests/jump_differential.rs`
compares the pure `fighter::locomotion::jump_backward` predicate against
`ftCo_Jump_Enter` pinned in `tests/oracle/original/jump.c` over proptest (512
cases, including NaN and infinities) plus the exact equality and adjacent
boundaries. Peppi-written replays match a backward short hop into a backward
double jump into the aerial fall (26/17, 28/19, 32/23) and a parallel forward
run reaching the ordinary apex fall before its own forward double jump
(25/16, 29/20, 27/18), and report a Mismatch at the ground-jump launch row
where a flipped stick sample first flips the reported direction; the
air-dodge replay's FallSpecial row now asserts 35/26.

The 2026-09-11 landing-window coverage batch is recorded at:

`/mnt/archive/runs/skirmish-landing-window-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `Fighter.landing_allow_interrupt` is set only by the ordinary Landing
entry and reset to `false` by every other transition, including
`LandingFallSpecial`, which keeps its own separate `aerial.allow_interrupt`
full lockout unaffected. Once `MovementData.normal_landing_lag` is supplied
and `cur_anim_frame` reaches it, `tilt::interrupt_chain` opens the same
complete Wait chain an interruptible tilt or dash attack pose already
exposes (grab, shield, special, smash, tilt, jab, jump, dash, turn, walk),
narrowed only by the crouch entry, which `locomotion::update_actions` gates
to the single first interruptible frame. Match integration coverage steps a
short hop into Landing with the fixture's `landing_frames` widened to 6 and
`normal_landing_lag` set to 3.0: every one of A, L, X, Z, stick down, a fresh
dash stick and a fresh C-stick is ignored on both frames before the lag; the
first interruptible frame accepts a jab, a moderate-stick tilt, a catch, a
fresh shield press, a jump, a fresh dash stick, a fresh C-stick smash and the
crouch; the second accepts everything again except the crouch; Turn and Walk
open from an interruptible frame; the animation still ends in Wait without
input; checkpoints restore every interrupt phase; invalid resources
(non-finite, negative, above `landing_frames`) are rejected; and omitting the
resource keeps a chainless Landing where a fresh A press well past where the
lag would otherwise open the chain still does nothing. `landing_differential`
traces `ftCo_Landing_IASA`'s complete dispatch order against a Rust mirror of
the exact pinned control flow (the lag and allow_interrupt gates, every
attack/movement callee in source order, the frame-gated squat check, turn and
walk) over generated frame/rate/lag/allow/answer combinations, plus boundary
cases, and separately compares the entry family's motion/allow/rate
arithmetic, including `ftCo_LandingFallSpecial_Enter`'s `(0.1 + x2EC) / lag`
bit-exact. `tests/game_landing.rs` covers the full per-phase dispatch order
end to end against the Rust implementation. A Peppi-written replay matches a
short-hop landing followed by a C-stick smash on the first interruptible
frame and reports a Mismatch at the row where the removed C-stick sample's
effect first appears.

The 2026-09-11 dash-attack coverage batch is recorded at:

`/mnt/archive/runs/skirmish-dash-attack-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage dashes into the early phase and, with the
entry stick already stale (`ftCo_Dash_Enter` resets the age to 254), confirms
the dash-specific forward smash needs no age window and keeps facing, while
the C-stick crossing can still flip it; the held-shoulder forward roll fires
through its own limit and does nothing once past it while still inside the
early phase. A fresh A enters the dash attack from the middle phase and from
Run, arming the shared shield-grab buffer, but never from the early phase
(a neutral stick) or the late phase. An opposite fresh stick in the middle
phase enters a smash Turn without restarting the dash; the same direction in
the late phase restarts Dash with `dash_from_input` set and `action_frame`
back at 1. Middle- and late-phase shield entry differ only by the buffer
(unarmed before the limit, armed after), and a fresh press opens GuardReflect
with the same buffer in both phases. The `x54` transition-friction tail
applies on the same frame as every fall-through transition (the early
forward smash/roll, the middle dash-back Turn/shield entry, the late
re-dash/Turn/shield entry, and a neutral special entered from Dash) but
never after the catch, the AttackDash entry, the jump-squat entry, the run
transition, "nothing fired", or ever for Run: a late-phase re-dash's first
observed `ground_velocity` matches `dash_initial_velocity - gr_vel_before *
x54` by hand computation (`ftCo_Dash_Enter` reads `gr_vel` before the tail
reduces it), a late-phase GuardOn entry's `ground_velocity` matches the tail
composed with GuardOn's own ordinary ground friction, and an idle Dash frame
(nothing fired) leaves `ground_velocity` exactly equal to the same frame
with `rules.dash = None` — since the tail is otherwise gated behind the
unmodeled taunt (`ftCo_800DE9D8`/`ftCo_800DE9B8`, D-pad up entering AppealS,
`ftCo_AppealS.c:35,43`) in the pinned source's own `block_42`. AttackDash's
catch buffer fires on a held shoulder without A, counts down and expires;
its Wait chain opens only on the flagged pose; its physics apply friction
absent supplied root motion; a close victim is hit exactly once; checkpoints
and invalid resources (non-finite/negative rules, a repeat flag, mismatched
root length, a missing grab shield-grab pairing, missing locomotion, and
dash rules without a per-fighter attack or vice versa) are covered.
`dash_differential` traces `ftCo_Dash_IASA`'s complete dispatch order against
a Rust mirror of the exact pinned control flow (catch, forward smash, roll,
AttackDash entry, the real `ftCo_Dash_CheckInput` predicate, both shield
entries, the guard buffer arm, the taunt, jump and run checks, and the
friction tail itself) over generated phases, sticks, ages, run flags and
answer combinations, plus boundary cases, and separately compares
`ftCo_Dash_CheckInput`, `ftCo_800D8AE0`'s catch buffer and
`ftCommon_ApplyFrictionGround` (already pinned by the `locomotion_common`/
`physics` adapters, which `apply_friction` mirrors, exercised here for
AttackDash's `x50` deceleration) with pinned C. `tests/game_dash.rs` covers
the full per-branch dispatch order, including the transition-friction
arithmetic, end to end against the Rust implementation. Peppi-written
replays match a dash attack entered from Run and report Slippi state 50 with
animation 52, match a late-phase re-dash, and detect a removed press at its
first affected frame in both.

The 2026-09-11 jab-combo coverage batch is recorded at:

`/mnt/archive/runs/skirmish-jab-combos-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage starts the first jab from Wait (a held
press is not fresh), latches a buffered press on an uninterruptible pose and
fires the second jab on the first pose whose follow-up flag is raised, and
chains the same pattern into the third jab, which has no follow-up of its
own. The follow-up window survives into Wait and decays there without a
press; any other motion resets it immediately rather than gradually. The
first and second jabs' interruptible poses try smashes, then tilts, then the
ordinary jump/dash/squat/turn/walk dispatch, and never reach shield or catch;
the third jab's interruptible pose reaches the complete Wait chain, including
catch, shield and a fresh first jab. The rapid jab counts fresh presses and
releases (a held button does not add to the count) into Attack100Start,
which plays into Attack100Loop keeping the action instance
(`Ft_MF_SkipAttackCount`) before the loop's own frame zero restarts the stale
identity and allocates a fresh instance id; tapping through the loop's
continuation check keeps it going and re-hits an in-reach victim each cycle,
and no input at the check ends it into Attack100End and Wait. A later pose's
explicit `rapid: Some(false)` turns the flag back off, a script's clear_hits
command lets one hitbox group hit the same victim twice within a jab, the
rapid press count persists into the second jab and resets on a fresh first
jab, the second jab's TransN root motion is applied in both facings,
checkpoints restore every phase (first jab both before and after
`allow_interrupt`, second, third, rapid start/loop/end and Wait with the
window still open), and invalid resources (mismatched flag/root-translation
lengths, a loop check outside the rapid cycle, a follow-up flag without its
next jab, a rapid flag without the rapid jab, a cycle without any
continuation check, a negative rapid window, a non-finite window, staled
rapid animations with differing move identities and missing locomotion) are
rejected before a match is constructed; `jab_combo = None` keeps the
original chainless jab. `ftCo_Attack1_CheckInput`, `checkAttack11`,
`doAttack12`, `checkAttack12`, `doAttack13`, `checkAttack13`,
`ftCo_Attack11_IASA`, `ftCo_Attack12_IASA`, `ftCo_Attack13_IASA`,
`ftCo_Attack_800D6A50`, `ftCo_800D6B00`, `ftCo_Attack100Loop_Anim` and
`ftCo_Attack100Loop_IASA` are compared with pinned C over generated
press/release and script-flag sequences. Peppi-written replays match a
three-jab combo and a rapid jab, report Slippi states 44..46 with animation
indices 46..48 and states 47..49 with animation indices 49..51, and detect a
removed press at its first affected frame.

The 2026-09-10 smash coverage batch is recorded at:

`/mnt/archive/runs/skirmish-smashes-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage selects every forward-smash angle variant
with the stick-sign facing and availability fallbacks, enforces the strict
dash-stick window, orders up and down smashes before tilts with inclusive
thresholds and float windows, enters from fresh C-stick crossings without a
button (also over a same-frame shield press), keeps Turn's post-turn facing,
reaches the smashes from SquatWait and Walk, jump-cancels the windowless up
smash and the grab from JumpSquat, leaves a released press unarmed, freezes the
charge pose while A is held until release or the hold limit, scales released
damage (including a fractional value with its truncated stale count), scales a
charging victim's knockback before armor, applies TransN root motion in both
facings with a frozen charge pose, opens the Wait chain only on flagged
samples, re-enters from a released and re-crossed C-stick, reads Z as the
logical A press from the down tilt, restores checkpoints in every charge phase
and rejects invalid resources. The complete input predicates, the forward
entry with `decideFighter`/`doEnter`, the windowless KneeBend variant, the
fresh C-stick predicates and the charge lifecycle are compared with pinned C.
Peppi-written replays match a charged forward smash, a C-stick down smash and
an up smash, report Slippi states 58..64 with animation indices 60..66 and the
frozen action frame, and detect a removed press, an early release or a removed
C-stick sample at its first affected frame. The tilt batch's SquatWait/SquatRv
catch was corrected: `ftCo_Catch_CheckInput` is not in those chains.

The 2026-09-10 tilt coverage batch is recorded at:

`/mnt/archive/runs/skirmish-tilts-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage selects every forward-tilt angle variant and
its availability fallbacks, orders forward, up, down and jab from each grounded
state with Turn's post-turn facing, opens the Wait chain or the down tilt's
narrower chain only on flagged samples, ends in Wait or SquatWait, buffers and
repeats the down tilt while keeping the entry action instance and restarting
the stale instance on exit, accepts A with a held shoulder as a catch, restores
checkpoints and rejects invalid resources. The complete input predicates,
`decideAngle` and `checkPadA` are compared with pinned C. Peppi-written replays
match straight, high, up and repeated down tilts, report Slippi states 51..57
with animation indices 53..59, and detect a removed attack press at its first
affected frame.

The 2026-09-10 air-dodge coverage batch is recorded at:

`/mnt/archive/runs/skirmish-air-dodge-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage enters EscapeAir from Jump, Fall and
JumpAerial on a fresh physical L or R press only, launches along the stick
angle outside the strict deadzone, decays both axes without gravity until the
scripted resume, re-arms fast fall afterwards, blocks hits on scripted
intangible samples, continues into FallSpecial with every jump used and no
aerial attack or jump, lands into the special landing at the source rate with
input locked out, wavedashes straight from the dodge with ground friction,
falls through one-way platforms only while holding down, orders the dodge
before aerial attacks and double jumps, restores checkpoints in every phase and
rejects invalid resources. The complete trigger, launch, decay and
platform-landing bodies are compared with pinned C. Peppi-written replays
match a landing dodge and a FallSpecial continuation, report Slippi states
236/31/43 with animation indices 44/22/36 [FallSpecial's 31/22 were FallB's
numbers; corrected to 35/26 by the 2026-09-11 state-parity batch] and the
scripted hurtbox byte, and detect a removed trigger press at its first
affected frame.

The 2026-09-10 shield-grab coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-grab-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage grabs from GuardOn, Guard and GuardReflect
with A plus a held shoulder or with physical Z, rejects A without a shoulder,
orders the grab after the spot dodge and roll and before the jump dispatchers,
arms the dash-grab buffer from Run and late Dash shields (including a
powershield), counts it down only on GuardOn/GuardReflect callbacks, expires
it, excludes Guard, GuardOff and shield stun, clears it on a fresh shield,
restores checkpoint branches, requires explicit rules and rejects invalid ones.
The complete `ftCo_Catch_CheckInput` and `ftCo_800D8B9C` bodies are compared
with pinned C over arbitrary button words and binary32 buffers. Peppi-written
replays match the A, Z and buffered late-dash suffixes and detect a removed
grab button at its first affected frame. Every ordinary exit from a guard
state (escape, grab or jump) now clears the Slippi-visible reflect bit and the
powershield entry latch as `Fighter_ChangeMotionState` does.

The 2026-09-10 grounded shield-evasion coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-escape-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage enters EscapeF, EscapeB and EscapeN from
GuardOn, Guard, GuardReflect and (spot dodge only) GuardOff through fresh
main-stick and held C-stick input, checks main-stick priority and
facing-relative direction, applies exact sampled root motion and bone poses,
clears the shield, blocks hits and grabs only on scripted intangible samples,
exempts the evading fighter from overlap nudges, falls off a floor edge
[corrected 2026-09-11: rolls and the spot dodge use mode 2, always clamp;
they stop at a floor end instead of falling off it -- see the floor-ends
batch below],
returns to Wait, restores checkpoints inside every escape action and rejects
invalid motions before a match exists. The complete `ftCo_8009917C` and
`ftCo_8009980C` dispatchers plus `ftCo_800DF8B0` and `ftCo_800DF8E8` are
compared with pinned C over arbitrary bit patterns and boundaries. Peppi-written
replays match each escape suffix, report Slippi states 233..235, animation
indices 42/43/41 and the scripted hurtbox byte, and detect a changed roll
stick, C-stick X or C-stick Y sample at its first affected frame.

The 2026-09-10 C-stick shield-jump coverage batch is recorded at:

`/mnt/archive/runs/skirmish-cstick-shield-jump-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage holds an upward C-stick before shield entry,
uses it on the next Guard callback, checks main-stick/X/Y priority, the inclusive
threshold, release-based short hops, ordinary-state exclusion and checkpoint
restoration. The complete `ftCo_800DF910` predicate is compared with pinned C
over arbitrary binary32 values. A Peppi-written replay matches the
GuardOn-to-JumpSquat suffix and detects a changed C-stick at its first affected
frame.

The 2026-09-10 shield-drop coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-drop-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Integration coverage drives digital and processed analog shields through
an immediate Pass transition on a one-way platform, rejects the same request on
a solid floor, preserves the single-transition scheduler boundary and reuses
the existing collision-line skip and ECB lock. The exact `ftCo_8009A080` and
`ftCo_80099F1C` predicates are compared with pinned C over arbitrary input bit
patterns and boundary values. A Peppi-written replay matches the resulting
GuardOn-to-Pass suffix and detects a changed down-stick on its first affected
frame.

The 2026-09-10 inert shield-touch batch is recorded at:

`/mnt/archive/runs/skirmish-shield-touch-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration tests prove that an inert hitbox can overlap a shield
without shield/body damage, hitlag, stun or hit-history consumption, that the
one-frame signal survives checkpoints, and that inert body overlap is ignored.
The `fighter-post-v11` policy compares the recorder's `x221C_b5` bit. A
Peppi-written GuardReflect replay contains a positive shield-touch frame and
requires an independent corruption to report `state_flags.shield_touch`.

The 2026-09-10 powershield batch is recorded at:

`/mnt/archive/runs/skirmish-powershield-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and the selected original-C functions in debug and release
modes. GuardReflect integration tests cover fresh digital entry, conversion
from analog GuardOn, expiration boundaries, shield-damage immunity, hitlag,
shield stun, defender push, attacker recoil and checkpoint replay. The exact
`ftCo_80093BC0` timer routine and both ordinary/powershield push branches are
also differential-tested against the pinned C source. The `fighter-post-v10`
policy maps state 182 to the GuardOn animation and compares `reflecting` and
`x221C_b2`; a Peppi-written file requires independent corruptions to report
`state_flags.reflect` and `state_flags.powershield`.

The 2026-09-10 Slippi sleep-state batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-sleep-state-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, 567 native
workspace tests, and 761 original-C differential tests in both debug and release
modes. The `fighter-post-v9` policy maps Skirmish's inactive respawn interval to
Melee's common `Sleep` state 11, its absent animation index, and
`Fighter::x221F_b3`. Peppi-written ordinary, star and screen death replays now
continue through sleep and return to active play. They require an independently
corrupted positive sleep bit to report `state_flags.sleep`. The 19 ignored tests
require external references, extracted assets, or a GPU/window and are
unchanged.

The 2026-09-10 Slippi death-flag batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-death-flag-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, 567 native
workspace tests, and 761 original-C differential tests in both debug and release
modes. The `fighter-post-v8` policy adds the recorder's `Fighter::x221F_b1`
bit. Native lifecycle tests cover immediate ordinary deaths, delayed star/screen
disappearance, inactive respawn retention and rebirth clearing. Peppi-written
replay tests exercise all three death paths with a positive flag value and
require an independently corrupted fifth-byte bit to report
`state_flags.dead`. The 19 ignored tests require external references, extracted
assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi animation-index batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-animation-index-20260910`

It validates formatting, strict all-target/all-feature Clippy, 566 native
workspace tests, and 760 original-C differential tests in both debug and release
modes. The `fighter-post-v7` replay policy now compares Slippi 3.11's exact
32-bit `Fighter::anim_id`, separately from the motion-state ID. The pinned
common motion table supplies shared animation indices and preserves `-1` as
`0xffffffff`; mapped Fox neutral-special startup states use their character
table indices. Focused unit coverage checks ordinary, shared, damage, prone,
ledge, no-figatree and Fox-specific entries. Peppi-written file tests enforce
the 3.11 version boundary and independently corrupt the new field in the full
reported-field loop. The 19 ignored tests require external references, extracted
assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi action-instance batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-action-instance-20260910`

It validates formatting, strict all-target/all-feature Clippy, 566 native
workspace tests, and 760 original-C differential tests in both debug and release
modes. The `fighter-post-v6` replay policy now compares Slippi 3.16's current
action-instance ID and retained last-hit source instance. Independent nonzero
16-bit match counters reproduce identity changes, shared motion-family
retention, explicit restarts, wrap and zero skipping. The selected complete
`ft_800895E0`, `ft_80089824` and `plAttack_80037B08` bodies are compared over
1,024 arbitrary states. Match coverage checks hit attribution,
aerial-to-landing retention, zero-identity actions, stock cleanup, checkpointed
state and independent resets; Peppi-written replay files version-gate and corrupt
each new field independently. The 19 ignored tests require external references,
extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 retained-combo push batch is recorded at:

`/mnt/archive/runs/skirmish-combo-push-20260910`

It validates formatting, strict all-target/all-feature Clippy, 563 native
workspace tests, and 755 original-C differential tests in both debug and release
modes. Repeated same-move hits now start the fighter-owned separation timer and
apply the original grounded floor-tangent displacement during ordinary frames
and hitlag. The exact `ftcoll.c` bodies are compared over arbitrary retained
pointer identities, signed thresholds, 32-bit timer values, native counter
wraps, eligibility states and binary32 bit patterns. Match coverage checks both
timer frames, position changes, checkpoints and stock-loss reset. The 19 ignored
tests require external references, extracted assets, or a GPU/window and are
unchanged.

The 2026-09-10 Slippi combat-provenance batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-combat-provenance-20260910`

It validates formatting, strict all-target/all-feature Clippy, 562 native
workspace tests, and 752 original-C differential tests in both debug and release
modes. The `fighter-post-v5` replay policy now compares internal character ID,
last landed attack, retained combo count and last-hitting physical port. The
native scheduler retains the source combo pointer through hitstun and the
configured post-hitstun grace period. Exact unit tests cover every translated
counter branch and the complete character-ID mapping; match and Peppi-written
file tests cover source attribution, repeated hits, checkpoint replay and an
independently corrupted value for every new field. The 19 ignored tests require
external references, extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi state-flags batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-state-flags-20260910`

It validates formatting, strict all-target/all-feature Clippy, 558 native
workspace tests, and 748 original-C differential tests in both debug and release
modes. The `fighter-post-v4` replay policy now compares protection, fast-fall,
hitlag, active-shield and hitstun bits, plus the action-state union as a hitstun
counter only while Slippi marks that interpretation valid. File-backed tests
drive digital-L shield entry, fast fall, hitlag and hitstun and corrupt every new
reported field independently. The 19 ignored tests require external references,
extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi post-frame v3 batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-post-v3-20260910`

It validates formatting, strict all-target/all-feature Clippy, 557 native
workspace tests, and 747 original-C differential tests in both debug and release
modes. File-backed replay coverage now compares retained ground-line identity,
successful and unsuccessful l-cancel results, and version-gated hurtbox collision
state. Native physics keeps ordinary invincibility and intangibility separate;
ledge intangibility blocks hits and grabs and serializes with the recorder's
priority. Debug and release traces are identical. The 19 ignored tests require
external references, extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 airborne Damage callback batch is recorded at:

`/mnt/archive/runs/skirmish-damage-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. A long sampled airborne Damage motion proves the last hitstun
frame retains locked physics and input, then checks ordinary drift, fast fall,
neutral special, aerial attack and double jump on a fresh legal frame with
checkpoint replay.

The 2026-09-10 DamageFall air-callback batch is recorded at:

`/mnt/archive/runs/skirmish-damage-fall-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Integration coverage holds DamageFall fast fall, neutral special,
aerial attack and double jump through hitstun, accepts fresh input after the
exact boundary and replays every branch from checkpoints.

The 2026-09-10 reflected damage-air callback batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Both reflected actions now run ordinary aerial gravity and stick
drift. Match coverage holds fast fall, neutral special, aerial attack and double
jump through hitstun, accepts fresh input after the exact boundary and replays
every branch from checkpoints.

The 2026-09-10 moving-wall reflection batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-moving-wall-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. A zero-speed `FlyReflectWall` integration fixture isolates the
moving collision plane: inward translation pushes the ECB by the exact line
delta, outward translation separates without dragging, and both paths replay
from checkpoints.

The 2026-09-10 reflected-surface landing batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-landings-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Match-level coverage drives both `FlyReflectWall` and
`FlyReflectCeiling` through ordinary floor collision, missed-tech `DownBound`
entry, velocity and knockback cancellation, response-history cleanup and
deterministic checkpoint replay.

The 2026-09-10 reflected-surface chaining batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-chains-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Integration coverage drives wall-to-ceiling and
ceiling-to-opposite-wall reflections while the configured repeat lockout remains
active, checks response ordering and remembered surface-class replacement, and
replays each cross-surface suffix from a checkpoint.

The 2026-09-10 damage-surface pose and delayed-jump batches are recorded at:

`/mnt/archive/runs/skirmish-surface-tech-poses-20260910`

`/mnt/archive/runs/skirmish-surface-tech-jump-queue-20260910`

Each isolated batch runs formatting, strict all-target/all-feature Clippy,
native workspace tests and original-C differential tests in debug and release
modes. The pose batch supplies complete wall, wall-jump and ceiling physics
skeletons, validates every sample and distinguishes ordinary wall-jump ownership
through the headless bone ECB. The delayed-jump batch covers a neutral wall
tech's queued jump transition, same-frame pose handoff, release-frame boundary
and deterministic checkpoint suffix. Exact jump selection and launch arithmetic
remain compared with their complete pinned C bodies over arbitrary binary32
inputs.

The 2026-09-10 wall-tech air-interrupt batch is recorded at:

`/mnt/archive/runs/skirmish-wall-tech-air-interrupts-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage holds actions through frozen wall
tech startup, rearms fresh input after release, dispatches neutral special,
aerial attack and double jump in source priority, applies the same branches to
ordinary wall jumps and preserves the ceiling tech's empty interrupt callback.

The 2026-09-10 surface-tech landing coverage batch is recorded at:

`/mnt/archive/runs/skirmish-surface-tech-landings-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. One match-level integration test drives neutral wall tech,
wall-jump tech and ceiling tech through gravity, floor contact, shared Landing
timing, transient surface/wall-jump cleanup and checkpoint replay.

The 2026-09-10 moving-wall tech coverage batch is recorded at:

`/mnt/archive/runs/skirmish-surface-tech-moving-wall-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. The integration scenario schedules a collision-line translation
immediately after wall-tech entry and checks inward displacement, outward
separation, contact state, frozen self velocity, one timer callback and
deterministic checkpoint replay.

The 2026-09-10 reflected-surface pose batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-poses-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Per-fighter complete `FlyReflectWall` and `FlyReflectCeiling`
pose tracks now drive the headless bone ECB. Integration coverage checks both
selectors and checkpoint replay; resource tests cover pairing, exact duration,
skeleton topology, finite transforms and serialization.

The 2026-09-09 grounded-launch coverage batch is recorded at:

`/mnt/archive/runs/skirmish-ground-launch-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises flat and sloped tangent
projection, exact floor departure, fly bounce, hitlag freezing, scalar friction,
prone DownDamage's explicit fly override, malformed resources and checkpoint
replay. The complete retained launch branch and extracted vector-angle helper
are compared over arbitrary binary32 inputs.

The 2026-09-09 prone DownDamage coverage batch is recorded at:

`/mnt/archive/runs/skirmish-down-damage-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises both prone orientations,
strict threshold equality, the source face-down selector quirk, launch and
landing, shared countdown recovery, malformed resources and checkpoint replay.
The complete retained eligibility/selector callback is compared with pinned C.

The 2026-09-09 prone-orientation coverage batch is recorded at:

`/mnt/archive/runs/skirmish-prone-orientation-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage selects face-up and face-down from
the evaluated hip matrix, preserves the choice through checkpoints and drives
distinct bound/wait poses, both roll directions, stand poses and get-up attacks.
The strict matrix-axis/inversion selector has direct unit boundary coverage.

The 2026-09-09 floor-recovery coverage batch is recorded at:

`/mnt/archive/runs/skirmish-floor-recovery-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises the full supplied
Passive/DownBound/DownWait pose suffix, reset and checkpoint behavior for the
source A/B attack buffer, DownBound attack/roll priority, every recovery
invincibility category and combat on the first vulnerable frame. Exact selector
boundaries remain covered by Rust unit tests and the existing complete C-stick
predicate differentials.

The 2026-09-09 missed-tech recovery batch is recorded at:

`/mnt/archive/runs/skirmish-knockdown-options-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises attack/roll/stand input
priority, both roll directions, fresh and held C-stick history, exact sampled
root motion and bones, get-up attack contact, resource rejection and checkpoint
replay. The two exact C-stick predicates are compared against pinned original C
over arbitrary binary32 values.

The 2026-09-09 floor-tech roll batch is recorded at:

`/mnt/archive/runs/skirmish-floor-tech-roll-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral, forward and backward
floor techs, inclusive input selection, separate sampled action durations,
TransN root motion, bone-derived ECB changes, serialization, malformed resources
and exact checkpoint replay. Direction selection compares against complete
pinned original C across arbitrary binary32 inputs. Independent scheduler traces
remain pending.

The 2026-09-09 damage-surface tech batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-tech-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral and jump wall techs,
both wall orientations, ceiling input motion, exact action timing, floor/wall
priority, repeat lockout, invalid resources and exact checkpoint replay. Wall
tech jump selection compares against complete pinned original C across timer
boundaries and arbitrary binary32 inputs. Independent scheduler traces remain
pending.

The 2026-09-09 damage-surface batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs wall and ceiling reflection,
configured action timing, floor-first collision priority, omission and unmet
threshold behavior, malformed resources and exact checkpoint replay. The
retained reflection arithmetic compares against complete pinned original C and
vector-helper bodies. Independent scheduler traces remain pending.

The 2026-09-09 ECB-response batch is recorded at:

`/mnt/archive/runs/skirmish-ecb-response-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs four-sided moving compression,
horizontal and vertical squeeze, next-frame ECB restoration, moving-floor
landing, one-way direction filtering, tangential-motion rejection and exact
checkpoint replay. The reusable squeeze functions continue to compare against
their complete pinned original C bodies. Independent corner/squeeze scheduler
traces remain blocked pending a separate producer.

The 2026-09-09 stage-motion batch is recorded at:

`/mnt/archive/runs/skirmish-stage-motion-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs affine/cyclic collision-line
motion, current geometry, grounded carry after self movement and during hitlag,
airborne detachment/relanding, countdown timing, checkpoint replay and invalid
resources. Generated cases compare the complete original moving-line remap,
including exceptional binary32 inputs. The independent moving-platform trace
remains blocked pending a separate producer.

The 2026-09-09 blast-death batch is recorded at:

`/mnt/archive/runs/skirmish-deaths-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs every normal blast direction,
forced top death, star and screen phases, delayed/final stock loss, input
suppression, checkpoint replay and invalid resources. The complete original
blast selector is compared across generated states, including exact HSD RNG
consumption. Thirteen fidelity cases remain blocked on independent reference
fixtures, including the separate star/screen trace comparison.

The 2026-09-09 neutral-special batch is recorded at:

`/mnt/archive/runs/skirmish-specials-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Both formerly ignored neutral-special scenarios now pass, with
focused integration coverage for input priority and rearming, both player slots,
ground/air physics and terrain conversion, bone-attached combat, checkpoints and
invalid resources. The exact neutral input predicate is compared with complete
pinned C. The conformance audit now has no executable unimplemented scenarios;
13 cases remain blocked on independent reference fixtures.

The 2026-09-09 rebirth-platform batch is recorded at:

`/mnt/archive/runs/skirmish-rebirth-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. The stock-loss platform scenario now passes, with focused
integration coverage for both player slots, exact target travel, wait/release,
invulnerability, checkpoint restoration and invalid resources. Two complete
original leader physics callbacks are compared bit for bit. The conformance
audit now has 2 executable unimplemented scenarios and 13 cases blocked on
independent reference fixtures.

The 2026-09-09 static ledge-action batch is recorded at:

`/mnt/archive/runs/skirmish-ledge-20260909`

It validates the composed tree after the native asset-import commit with
formatting, strict all-target/all-feature Clippy, native workspace tests and
original-C differential tests in debug and release modes. Six formerly ignored
ledge scenarios now pass, with focused integration coverage for endpoint
eligibility and ownership, bone attachment, every ledge option, combat contact,
damage/KO release, cooldown/regrab and checkpoint restoration. The current
conformance audit has 3 executable unimplemented scenarios and 13 cases blocked
on independent reference fixtures.

The 2026-09-09 movement/combat coverage milestone is recorded at:

`/mnt/archive/runs/skirmish-gameplay-20260909-v6b`

It checks the isolated gameplay snapshot with formatting, strict Clippy, native
workspace tests and original-C differential tests in debug and release modes.
Native trace comparisons exercise both the original demo and explicit movement,
combat/displacement/armor and stacked-platform profiles. The run preserves its
commands, input resources, scripts, output traces and source hashes. Synthetic
debug/release agreement checks determinism, not original-game equivalence.

Eleven previously ignored scenarios are now active, accompanied by focused
integration regressions for input history, movement/action timing, displacement,
armor, platform collision and blast boundaries. The remaining audit contains
21 executable unimplemented scenarios and 13 independent-reference-blocked
cases; all 34 are invoked explicitly and remain failures rather than passing
coverage. Authentic movement poses, special-character callbacks and the full
game scheduling order are still outside this milestone.
The earlier `v6` run caught inconsistent synthetic movement/displacement
thresholds after input-history consolidation; `v6b` uses corrected explicit data.

The 2026-09-09 Peppi Slippi importer validation run is stored outside Git at:

`/mnt/archive/runs/skirmish-slippi-20260909-v5c`

It contains command/exit-code metadata and debug, optimized, lint and Rust-only
test logs, plus debug/optimized synthetic match traces and their comparison.
Peppi 2.1.2 writes wholly synthetic `.slp` fixtures covering supported versions,
ports, follower presence/absence, exact input bits, rollback, finalization,
malformed events and executable error handling. A synthetic position protocol
checks the generic transition interface. Separate file-backed scenarios drive
the real native match across walking, jumping, landing and combat, detect late
corruption and changed inputs, and ensure reference observations never reset
simulated state. Their expectations originate from the experimental native
match and do not establish independent Melee equivalence. Explicit conformance
audits report missing implementation and independent-reference fixtures as
failures, separately from the normal passing regressions; see [testing.md](testing.md).
That archived audit had 32 executable missing-behavior scenarios and 13 cases blocked on
independent reference fixtures. It invoked all 45 explicitly and recorded
their failures; default test discovery reports them as ignored. Three real
Slippi 2.0.1 corpus files are also parsed with hashes and frame counts recorded.
This smoke check covers parsing, without compatible native-match initialization
or a gameplay equivalence assertion. The earlier `v5` attempt stopped at
formatting while another task was editing menu tests. `v5b` passed in the shared
workspace; `v5c` validates the fixed commit snapshot in a temporary worktree,
separately from concurrent menu/presentation additions.
The earlier `skirmish-physics-20260909-v4` contains stage, ECB, swept-contact and
damage physics validation. `skirmish-headless-match-20260909-v3` contains the first native match
and skeletal physics milestone. `skirmish-rust-port-20260909-v2` contains the runtime/physics/input/
replay milestone. `skirmish-rust-port-20260909-v1` also contains a
hash inventory of all 2,441 upstream C/C++/header/assembly files. The input source
revision and toolchain version are recorded in `validation.json`. The upstream
revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`. Original test snapshots are
tracked separately in `tests/oracle/sources.json` and checked for exact coverage
and SHA-256 equality by the test suite.

Build products remain in `/mnt/shared/tmp/skirmish-target`. Local absolute paths
are provenance, not build requirements; CI builds in its own temporary directory.
No Melee game image or full-game behavior comparison participated in this run.
The synthetic native match reaches stock-based termination. Original-C checks
cover selected functions, while debug/optimized match agreement validates the
experimental scheduler's determinism, not Melee compatibility.
