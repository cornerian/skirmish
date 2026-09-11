# Run animation rate (batch after walk speeds)

`skirmish::game::locomotion` (wiring: entry, the per-frame animation advance,
`src/game/locomotion.rs`) and `skirmish::fighter::locomotion` (pure
arithmetic: the animation-rate formula, `src/fighter/locomotion.rs`) extend
the walk batch's float animation-frame model (`docs/walk.md`) to Run. Pinned
decomp rev `0bac93a5`. Sources: `src/melee/ft/kinds/ftCommon/ftCo_Run.c`
(`fn_800CA5F0`:22-33, `fn_800CA644`:35-46, `fn_800CA698`:48-59,
`ftCo_Run_Enter`:61-64, `ftCo_Run_Enter_Full`:66-74, `ftCo_Run_Anim`:76-99),
`ftCo_RunBrake.c` (`ftCo_RunBrake_Anim`:49-77), `ftCo_TurnRun.c`
(`ftCo_TurnRun_Enter`:44-55, `ftCo_TurnRun_Anim`:57-77), `types.h:698`
(`run_animation_scaling`, `co_attrs+0x2C`), `types.h:312-313` (`x42C`/`x430`,
unnamed common-data floats), `kinds/ftCommon/forward.h:310`
(`ftCo_MS_Run`, common motion-state table) and `kinds/ftCommon/forward.h:648`
(`ftCo_SM_Run`, the `ftCo_Submotion` table -- `None=-1` then 0-indexed,
`Run` is the 14th entry, giving sub-motion 13).

## Source behavior

- **Entry** (`ftCo_Run_Enter_Full`): `Fighter_ChangeMotionState(gobj,
  ftCo_MS_Run, Ft_MF_None, anim_start, anim_speed, 0.0F, NULL)`;
  `run.x0 = arg0` (the turn-run lockout countdown consulted by
  `ftCo_Run_IASA`, `ftCo_Run.c:125-126` -- see "Known gap" below),
  `run.x4 = gr_vel` (the velocity remembered for low-friction stages, dead in
  this codebase -- see Animation phase below). Both of this codebase's
  reachable Run entries go through the two-argument `ftCo_Run_Enter` wrapper,
  which always calls `Enter_Full` with `anim_start = 0.0F`, `anim_speed =
  1.0F`: `fn_800CA5F0` (`arg0 = 0.0`, called from `ftCo_Dash.c:131`, Dash's
  own IASA transition) and `fn_800CA644` (`arg0 = x430`, called from
  `ftCo_TurnRun.c:74`, RunTurn's own Anim-phase re-entry once its flip has
  finished). `fn_800CA698` (`ftCo_RunDirect.c`, a distinct motion state not
  modeled in this codebase) is the only caller of `Enter_Full` with a
  different `anim_start`/`anim_speed` (`fp->cur_anim_frame`/
  `fp->frame_speed_mul`) and is not reachable here. `game::locomotion::
  enter_run` is the wiring: it always enters with the tracked frame at 0.0
  and `last_rate` at 1.0 (`Fighter_ChangeMotionState`'s own `rate = 1`
  argument, applied on the first animation update exactly as the walk batch
  modeled Walk's own entry rate), called from both `game::dash`'s and
  `game::locomotion`'s own Dash-to-Run transitions and from `game::
  locomotion`'s RunTurn-to-Run transition.
- **Animation phase** (`ftCo_Run_Anim`, every Run frame): `vel = gr_vel` (or
  `run.x4` when the stage friction multiplier is below 1 -- this codebase's
  own caller always supplies 1, so `run.x4` is never read back; kept as a
  parameter for oracle parity, the same modeling choice `walk_animation_rate`
  already makes for Walk's `x0`). `rate = 0` when `vel * facing <= 0.0`, else
  `ABS(vel) / run_animation_scaling`; `ftAnim_SetAnimRate(rate)` applies from
  the *next* animation update, not this one, so `game::locomotion::RunState`
  keeps `last_rate` exactly as `WalkState` does: each frame the tracked
  `frame` advances by the *previous* frame's `last_rate` (wrapping down by
  the Run figatree's length while `frame >= length` -- the Run figatree
  loops; this is the wrap rule this port implements, not independently
  reverse-verified against `ftAnim`/`lbAnim`'s own loop bookkeeping, the same
  caveat the walk batch recorded), and only then is a fresh rate computed
  from the current `gr_vel`/`facing` and stored as the new `last_rate` for
  the following frame. `fighter::locomotion::run_animation_rate` is the pure
  formula; `game::locomotion::advance_run_animation` is the wiring. Then
  `run.x0` counts down to 0 by 1.0 per frame (never otherwise clamped) --
  unmodeled here, see "Known gap" below. Unlike Walk (three kind-indexed
  figatree lengths cached at `Fighter_Create_Inline2`, `fighter.c:838-840`),
  the source has no dedicated `Fighter` field caching the single Run
  figatree's length (`Fighter_Create_Inline2`, `fighter.c:829-841`, caches
  only sub-motions 7/8/9/0x23/0x25 -- WalkSlow/Middle/Fast, Landing,
  GuardOn -- none of which is Run), so `RunAnimation.length` here is supplied
  resource data standing in for a runtime animation-length query, the same
  convention this codebase already uses for other per-motion frame counts.
- **`run_animation_rate` versus `walk_animation_rate`**: identical branch
  shape and order (the same friction-multiplier gate, the same `v * facing
  <= 0.0` zero-rate test, the same `v.abs() / scaling` otherwise) -- Run just
  has a single figatree/scaling constant rather than three kind-indexed
  ones, so it is its own function (`src/fighter/locomotion.rs`) instead of a
  call into `walk_animation_rate` with a fabricated `rates` array and an
  unused kind axis.
- **Publishing**: Run already reports Slippi state 21 (`ftCo_MS_Run`,
  `forward.h:310`) and animation 13 (`ftCo_SM_Run`, `forward.h:648` --
  `ftCo_Submotion`'s 14th entry, 0-indexed after `None = -1`), unchanged by
  this batch (`crates/skirmish-replay/src/observation::action_state`/
  `animation_index`). `observe` now publishes Slippi's `action_age` as
  `RunState.frame` while Run is current and `MovementData.run_animation` is
  supplied, mirroring exactly how the walk batch published `WalkState.frame`
  -- a real Slippi file's `state_age` for Run *is* `fp->cur_anim_frame`, the
  float animation frame, not a separate integer.
- **RunBrake and RunTurn's existing freeze/resume model**: both are left
  exactly as they were before this batch (rates 0 and 1 only, integer
  frames). `ftCo_RunBrake_Anim` (`ftCo_RunBrake.c:49-77`): while the script's
  `cmd_vars[1]` marker is set, freeze (rate 0) once `|gr_vel| >= x42C`, then
  resume at rate 1 and clear the marker once `|gr_vel| <= x42C`; the brake
  `frames` timer separately counts down to 0 (clamped there) every frame
  regardless of the marker. `ftCo_TurnRun_Anim` (`ftCo_TurnRun.c:57-77`):
  while its own marker is set, first freeze (rate 0), then resume at rate 1
  and flip facing once `facing_at_entry * gr_vel <= 0.01` --
  `ftCo_TurnRun_Enter` (`ftCo_TurnRun.c:44-55`) stores `turnrun.accel_mul =
  facing_dir` at entry, and the check reads that same value back through the
  union alias `mv.co.walk.middle_anim_frame` (`ftCo_TurnRun.c:67`). Verifying
  these against the source surfaced two discrepancies from an earlier design
  draft's assumptions, reported below rather than silently fixed (out of
  this batch's scope, which is the Run animation-frame/rate layer only).

### Discrepancy: RunBrake's velocity-gated marker freeze is not modeled

An earlier draft of this note assumed RunBrake's `cmd_vars[1]`/`x42C`
velocity-gated animation freeze was "already modeled ('velocity-gated marker
freeze' in the locomotion batch)". Auditing the actual code
(`game::locomotion::update_animation`'s `Action::RunBrake` arm and
`game::locomotion::hold_action_frame`) shows only `mv.co.runbrake.frames`'s
plain countdown is modeled (`f.locomotion.run_brake_frames`, decremented and
clamped at 0, driving the exit to Wait); no field or check corresponds to
`cmd_vars[1]`/`x42C`/`runbrake.x0`, and `hold_action_frame` -- the only
frame-freeze mechanism in this codebase -- only ever returns true for
RunTurn (`f.action == Action::RunTurn && f.locomotion.run_turn_waiting`).
So unlike RunTurn, RunBrake's own reported `action_frame`/Slippi age never
freezes here, contrary to that earlier assumption. This batch does not add
it (out of scope; RunBrake's own animation layer, not Run's); it is reported
as a pre-existing gap.

### Discrepancy: RunTurn's flip check uses a fixed scale instead of the per-entry facing

`game::locomotion`'s existing RunTurn model gates the flip on `f.action_frame
>= p.run_turn_flip_frame` (standing in for the unmodeled script event that
sets `cmd_vars[1]`) and then checks `p.run_turn_velocity_scale *
f.ground_velocity <= 0.01`. Comparing that against `ftCo_TurnRun.c:67`'s
`facing_at_entry * gr_vel <= 0.01F`: the source multiplies by the *per-entry*
facing captured at `ftCo_TurnRun_Enter` (`turnrun.accel_mul`, read back
through the `middle_anim_frame` union alias), not a fixed resource constant.
`game::locomotion::start_run_turn` already captures that same per-entry value
as `f.locomotion.run_turn_facing` (used correctly by `fighter::locomotion::
turn_run`'s own physics branch), but the flip check in `game::locomotion::
update_animation`'s `Action::RunTurn` arm uses the unrelated `Parameters::
run_turn_velocity_scale` field instead. A fixed per-match resource constant
cannot reproduce a per-entry sign that flips with whichever direction the
fighter was facing when RunTurn began, so this is a discrepancy from the
pinned source. Reported here rather than silently changed; the existing
behavior (already covered by earlier-batch tests) is left as is.

### Known gap: `run.x0` (the turn-run lockout) is not modeled

`ftCo_Run_IASA` (`ftCo_Run.c:125-126`) gates RunTurn/RunBrake entry behind
`run.x0 <= 0.0F`: while `run.x0 > 0` (freshly set by a RunTurn-to-Run
re-entry, `fn_800CA644`'s `arg0 = x430`; always 0 from the ordinary
Dash-to-Run path, `fn_800CA5F0`'s `arg0 = 0.0`), the whole rest of Run's IASA
chain -- including the RunTurn and RunBrake checks -- is skipped. No existing
Rust code models this countdown or its IASA gate: `game::locomotion::
update_actions`'s `Action::Run` arm allows RunTurn/RunBrake entry
unconditionally, with no lockout field on `locomotion::State`. This batch's
resource/state shape (`RunAnimation { length, scaling }`, `RunState { frame,
last_rate }`) does not add one either -- it is out of scope here (the
animation-frame/rate layer only) -- so this is reported as a pre-existing gap
rather than introduced or silently fixed by this batch. The C oracle
(`tests/oracle/run.c`) still exposes `run.x0`'s own countdown
(`oracle_run_anim`'s `run_x0_after`) for a future batch that models it.

## Resource shape

- `MovementData.run_animation: Option<locomotion::RunAnimation { length: f32,
  scaling: f32 }>`: `length` is the Run figatree's frame count (supplied
  resource data, see above), `scaling` is `run_animation_scaling`
  (`types.h:698`). Validated finite, `0.0 < length <= 1_000_000.0`,
  `0.0 < scaling <= 1_000_000.0`. Unlike Walk, not paired with any `Rules`
  entry -- Run has no kind-selection thresholds. Absent keeps Run's
  pre-batch behavior: a single integer `action_frame` published as
  `state_age`, rate 1 always, and `game::locomotion::State.run` never leaves
  its default.
- `game::locomotion::State.run: locomotion::RunState { frame: f32, last_rate:
  f32 }` (`Serialize`/`Deserialize`, checkpoint-safe).

## Tests

`tests/support/run.rs` supplies an invented `MovementData.run_animation`
(`length` 20.0, `scaling` equal to `tests/fixtures/game/locomotion.json`'s
`dash_max_velocity` 2.5, so a fighter at top ground speed gets an animation
rate of exactly 1.0 -- not the source ISO's own Run figatree/
`run_animation_scaling` data).

`src/fighter/locomotion.rs` unit-tests `run_animation_rate`'s zero rate
against facing and its `x4`/friction-multiplier branch, mirroring
`walk_animation_rate`'s own test.

`tests/game_run.rs` builds on the existing locomotion fixture (like
`tests/game_walk.rs`'s own builder) and covers: a Dash-to-Run transition
enters Run with the tracked frame at 0.0 and `last_rate` 1.0; the first Run
frame afterward advances the tracked frame by exactly 1.0, and every later
frame matches `run_animation_rate` bit-exactly against the previous frame's
`ground_velocity`/`facing`; the frame wraps at the figatree length under a
sustained ramp; checkpoints restore the frame and `last_rate` exactly;
invalid resources (non-finite/non-positive length or scaling) are rejected
before a match exists; `None` keeps the pre-batch integer `action_frame` and
rate 1 through a real Dash-to-Run transition. A dedicated test drives a
Run-to-RunTurn-to-Run reversal (ramp to Run, then hold a hard reverse stick
through a RunTurn flip back into Run) and confirms `ground_velocity` already
agrees with the flipped `facing` on every observed Run frame -- under this
codebase's own `Action::Run` gating (stay in Run only while `stick * facing >
turn_threshold` and `|stick| >= run_threshold`, else RunTurn/RunBrake,
combined with `ftCo_Run_Phys`'s acceleration toward `stick *
dash_max_velocity`), the zero-rate branch (`vel * facing <= 0.0`) is not
independently reachable through an actual Run frame here; it is exercised
directly by `run_animation_rate`'s own unit test instead, and this test
documents rather than asserts that absence.

`crates/skirmish-replay/src/observation.rs`'s `observe` gains the matching
`Action::Run` branch for `action_age` (already covered indirectly by the
replay test below); `action_state`/`animation_index` needed no change (Run
already reported 21/13).

`crates/cli/tests/replay_match.rs`'s
`file_backed_dash_into_a_run_reports_state_21_with_float_ages_and_a_reduced_
stick_mismatch` drives a dash into a run, confirms every Run row reports
state 21/animation 13 with the tracked float frame starting at 0.0, matches
its own generated Peppi bytes end to end (including the float `state_age`),
then reduces a stick sample well into the ramp and confirms the comparison
reports a `Mismatch` starting at that exact row (`ground_velocity`/position
already diverge there) -- and separately confirms, by re-simulating with the
same edited input directly (bypassing the Peppi round trip), that the
reported *age* itself only starts to differ two rows later: Anim precedes
the ground-movement physics that reacts to the edited stick, so the changed
velocity is not read into a new `last_rate` until the following frame's Anim
call, and that rate is not read into the tracked frame until the frame after
that.

## Oracle

`tests/oracle/original/run.c` is a snapshot of `ftCo_Run.c` (`sources.json`).
`tests/oracle/run.functions.json` selects `ftCo_Run_Enter`,
`ftCo_Run_Enter_Full` and `ftCo_Run_Anim` verbatim (in that order, matching
the pinned source's own definition order; the host adapter forward-declares
`ftCo_Run_Enter_Full` since `ftCo_Run_Enter` calls it before its own
definition). The host adapter (`tests/oracle/run.c`) stubs
`Fighter_ChangeMotionState` (captures the motion id, start frame and rate
argument), `ftAnim_SetAnimRate` (captures the rate) and
`ft_GetGroundFrictionMultiplier` (reads an explicit environment field, so
both the `run.x4` and `gr_vel` branches compile and are exercised), exposing:

- `oracle_run_anim(gr_vel, run_x4, facing, scaling, friction_mul, run_x0) ->
  (rate, run_x0_after)`, calling `ftCo_Run_Anim` with those fields set up on
  a minimal host `Fighter` and returning the captured `SetAnimRate` rate plus
  `run.x0` after its own countdown.
- `oracle_run_enter(x0, gr_vel) -> (msid, start, speed, x0, x4)`, calling the
  two-argument `ftCo_Run_Enter` (the reachable entry path in this codebase)
  and returning the captured `ChangeMotionState` arguments plus `run.x0`/
  `run.x4` after the call. `oracle_run_enter_full` additionally exposes the
  three-argument `Enter_Full` entry point directly, for completeness/
  documentation of the unreached `fn_800CA698` path rather than a Rust-side
  comparison.

`tests/run_differential.rs` compares `run_animation_rate` against
`oracle_run_anim`'s rate bit-exactly over 512 proptest cases (arbitrary
binary32 `gr_vel`/`run_x4`/`scaling`/`friction_mul`, `facing` in `{-1, 1}`,
`run_x0` covering zero, a bounded positive range and arbitrary binary32,
NaN-safe the same way `walkcommon_differential` claims for
`walk_animation_rate`), plus exact boundaries (velocity exactly 0, negative
velocity, velocity negative with facing also negative, scaling as small as
`f32::MIN_POSITIVE`/`1e-30`, the friction-multiplier branch, and the
`run.x0` countdown at a positive value, exactly 1.0, exactly 0.0 and
negative). It also checks `oracle_run_anim`'s `run_x0_after` against the
source's own literal decrement (`ftCo_Run.c:96-98`, not modeled by any Rust
helper -- see "Known gap" above) and `oracle_run_enter`'s literal
`anim_start = 0.0`/`anim_speed = 1.0`/`run.x0 = arg0`/`run.x4 = gr_vel`
field assignments, which `game::locomotion::enter_run` assumes.
