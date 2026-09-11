# Walk speed variants (WalkSlow/WalkMiddle/WalkFast)

`skirmish::game::locomotion` (wiring: entry, the per-frame animation advance
and the mid-walk retype, `src/game/locomotion.rs`) and
`skirmish::fighter::locomotion` (pure arithmetic: kind selection, the
animation-rate formula and the retype start-frame remap,
`src/fighter/locomotion.rs`) port the walk-type layer this codebase's earlier,
already-audited walk physics sits under. Pinned decomp rev `0bac93a5`.
Sources: `src/melee/ft/ftwalkcommon.c` (`ftWalkCommon_GetWalkType`:16,
`..._800DFBF8_fake`:37, `ftWalkCommon_800DFC70`:58, `ftWalkCommon_800DFCA4`:71,
`ftWalkCommon_800DFDDC`:99, `ftWalkCommon_800DFEC8`:130; `getWalkAccel`:172 and
`ftWalkCommon_800E0060`:178 are the pre-existing walk physics, unchanged by
this batch), `src/melee/ft/kinds/ftCommon/ftCo_Walk.c`
(`ftCo_Walk_CheckInput`:30, `ftCo_Walk_CheckInput_Ottotto`:40,
`ftCo_Walk_Enter`:54, `ftCo_Walk_Anim`:77, `ftCo_Walk_IASA`:82,
`ftCo_Walk_Phys`:106, `ftCo_Walk_Coll`:111), `fighter.c:832-843`
(`Fighter_Create_Inline2`: `x2DC`/`x2E0`/`x2E4` are the frame counts of the
WalkSlow/WalkMiddle/WalkFast figatrees, sub-motions 7/8/9), `types.h:687-692`
(`walk_accel_mul`, `walk_accel_base`, `walk_max_vel`, `slow_walk_max`,
`mid_walk_point`, `fast_walk_min`), `kinds/ftCommon/types.h:63-65`
(`walk_stick_threshold`, `walk_middle_animation_stick_threshold`,
`walk_fast_stick_threshold`), `forward.h:313-317` (`FtWalkType` `Slow`/
`Middle`/`Fast`, in that discriminant order) and `forward.h`'s common-state
table (WalkSlow 15 / WalkMiddle 16 / WalkFast 17; sub-motions 7 / 8 / 9).

## Source behavior

- **Entry** (`ftCo_Walk_Enter` -> `ftWalkCommon_800DFCA4`): `accel_mul` is
  always 1 from this codebase's own caller (metal/scale/item multipliers
  unmodeled -- `ftCo_Walk_Enter` itself only ever varies it for those). The
  walk kind comes from `|gr_vel|` against the *fake* inline
  `ftWalkCommon_GetWalkType_800DFBF8_fake` (byte-identical to the public,
  non-inline `ftWalkCommon_GetWalkType`; the oracle pins the public one and
  wraps it as the `_fake` name, see Oracle below): Fast when `>= accel_mul *
  (walk_fast_stick_threshold * walk_max_vel)`, Middle when `>= accel_mul *
  (walk_middle_animation_stick_threshold * walk_max_vel)`, else Slow.
  `Fighter_ChangeMotionState(WalkSlow + kind, Ft_MF_None, anim_start, rate=1,
  ...)` runs with `anim_start` = the caller's own start frame -- 0 from
  `ftCo_Walk_CheckInput`/`_Ottotto` (a fresh Wait/tilt -> Walk transition),
  or the remapped frame from `ftWalkCommon_800DFEC8`'s own re-entry (below).
  `ftAnim_8006EBA4` (no script effects modeled) and the three figatree
  lengths/rates are recorded but otherwise unused by this port beyond the
  animation phase below. `fighter::locomotion::walk_kind` is the pure
  arithmetic; `game::locomotion::enter_walk` is the wiring, called both for a
  fresh entry (`start_frame = 0.0`) and from `retype_walk`.
- **Animation phase** (`ftCo_Walk_Anim` -> `ftWalkCommon_800DFDDC`, every
  Walk frame): with `v = gr_vel` (or the entry-recorded `mv.co.walk.x0` when
  the stage friction multiplier is below 1 -- this codebase's own caller
  always supplies 1, so `x0` is currently dead; kept as a parameter for
  oracle parity), the rate is 0 when `v * facing <= 0`, else `|v| /
  slow_walk_max` (Slow), `|v| / mid_walk_point` (Middle) or `|v| /
  fast_walk_min` (Fast). `ftAnim_SetAnimRate` takes effect on the *next*
  animation update, not this one, so `game::locomotion::WalkState` keeps
  `last_rate`: each frame the tracked float `frame` advances by the
  *previous* frame's `last_rate` (wrapping down by the current kind's
  figatree length while `frame >= length` -- the walk figatrees loop; this
  is the wrap rule this port implements, not independently reverse-verified
  against `ftAnim`/`lbAnim`'s own loop bookkeeping), and only then is a fresh
  rate computed from the current `gr_vel`/`facing`/kind and stored as the new
  `last_rate` for the following frame. At entry, `last_rate` is initialized
  to 1.0 (matching `Fighter_ChangeMotionState`'s own `rate = 1` argument);
  this is this port's own explicit modeling choice for how that initial rate
  interacts with the one-frame `SetAnimRate` delay, not something read back
  out of the pinned source. `fighter::locomotion::walk_animation_rate` is the
  pure formula; `game::locomotion::advance_walk_animation` is the wiring.
- **IASA** (`ftCo_Walk_IASA`): the existing chain order (catch, specials,
  smashes, tilts, jab, shield, taunt, jump, dash, squat, `ft_8008A244` ->
  Wait when `stick_x * facing < 0 || |stick_x| < walk_stick_threshold`,
  `ft_08A1.c:29-41`) is unchanged by this batch -- it is `game::locomotion::
  update_actions`'s shared Wait/Walk arm, audited by earlier batches. This
  batch adds only `ftWalkCommon_800DFEC8`, which that arm now runs whenever
  the walk-threshold branch is satisfied *and* the fighter was already
  walking (not a fresh Wait/tilt -> Walk entry, which instead runs the entry
  path above with `start_frame = 0` -- `ftCo_Walk_IASA` only ever runs while
  a Walk-family motion is already current, so the two paths do not overlap
  within a frame): recompute the kind from `|gr_vel|`; if it differs from
  the stored kind, re-enter Walk (`retype_walk` -> `enter_walk`, a genuine
  `game::locomotion::enter`/`ChangeMotionState`, so a new motion begins,
  clearing the smash charge and every other ordinary transition reset) with
  `start_frame = new_length * ((cur_frame mod cur_length) / cur_length)`,
  truncated to `s32` (matching `init_animFrame / len` truncated to an
  integer quotient, then `frame - len * quotient`, then `len_new *
  (adjusted / len)`, truncated again before being passed as the start
  frame). `fighter::locomotion::walk_retype_frame` reproduces this exact
  float/int arithmetic as a pure helper (`src/fighter/locomotion.rs`, unit
  tested); `game::locomotion::retype_walk` is the wiring. Since Walk and
  Dash share motion identity 102 (`fighter::action_instance::
  motion_identity`), `ft_800895E0`'s allocation rule (`fighter::
  action_instance::change`) keeps the same `action_instance.id` across a
  retype -- confirmed by `tests/game_walk.rs`.
- **Publishing**: `action_frame` (the plain per-frame integer counter every
  other action already uses) keeps advancing for Walk's own internal
  bookkeeping, but `crates/skirmish-replay/src/observation.rs`'s `observe`
  publishes Slippi's `action_age` as `WalkState.frame` while Walk is current
  and `MovementData.walk_animation` is supplied -- a real Slippi file's
  `state_age` for Walk *is* `fp->cur_anim_frame`, the float animation frame,
  not a separate integer; `match_validation.rs`'s `fighter-post-v11` policy
  already compares `state_age`/`action_age` as exact `f32` bits (`observation
  ::compare`'s `float_difference`), so no comparison-side change was needed.
  `action_state` reports `15 + kind as u16` and `animation_index` extends its
  existing `state - 8` arithmetic (already used for Turn/RunTurn/Dash/Run) to
  the 15..=17 range, giving 7/8/9.

## Resource shape

- `MovementData.walk_animation: Option<locomotion::WalkAnimation { lengths:
  [f32; 3], rates: [f32; 3] }>`: `lengths` are the WalkSlow/Middle/Fast
  figatree frame counts (`x2DC`/`x2E0`/`x2E4`), `rates` are `slow_walk_max`/
  `mid_walk_point`/`fast_walk_min`. Validated finite, `lengths[i] > 0`,
  `rates[i] > 0`, both `<= 1_000_000`.
- `Rules.walk: Option<locomotion::WalkRules { middle_threshold,
  fast_threshold }>`: `walk_middle_animation_stick_threshold`/
  `walk_fast_stick_threshold`. Validated finite, `0 <= middle_threshold <=
  fast_threshold <= 1_000_000`.
- The two are required together (`game::validation`, matching the existing
  edge/teeter and escape/escape-air pairing convention): `rules.walk`
  without a fighter's `walk_animation`, or the reverse, is rejected before a
  match exists. With both absent, Walk keeps the pre-batch behavior: a
  single state (Slippi 15, animation 7) with an integer `action_frame`
  published as `state_age`, and `game::locomotion::State.walk` never leaves
  its default (`WalkKind::Slow`, `frame = 0.0`).
- `game::locomotion::State.walk: WalkState { kind: fighter::locomotion::
  WalkKind, frame: f32, last_rate: f32 }` (`Serialize`/`Deserialize`,
  checkpoint-safe; `fighter::locomotion::WalkKind` is `Slow`/`Middle`/`Fast`
  in `FtWalkType`'s own discriminant order, shared by the pure helpers and
  the state so no separate mapping is needed).

## Tests

`tests/support/walk.rs` supplies invented `Rules.walk`/`MovementData.
walk_animation` (`middle_threshold` 0.3, `fast_threshold` 0.7, fractions of
`walk_max_velocity` chosen so a held stick reaches all three kinds; lengths
`[10.0, 12.0, 14.0]` and rates `[1.5, 1.0, 0.75]`, distinct per kind so each
one's wrap point and the retype remap are independently observable -- not
the source ISO's own WalkSlow/Middle/Fast data).

`src/fighter/locomotion.rs` unit-tests `walk_kind`'s inclusive, `accel_mul`-
scaled thresholds and negative-velocity magnitude; `walk_animation_rate`'s
zero rate against facing and its `x0`/friction-multiplier branch; and
`walk_retype_frame`'s truncating quotient/remainder/remap chain, including a
frame exactly at the length.

`tests/game_walk.rs` builds on the existing locomotion fixture (like
`tests/game_smash.rs`'s own builder) and covers: the kind at entry from Wait
is Slow with frame 0 and `last_rate` 1.0; a full-stick walk ramp (kept below
the dash-magnitude threshold throughout, so no fresh dash ever preempts it)
reaches Middle then Fast with `action_instance.id` unchanged and every
observed remapped frame bit-exact against the pure helper; the animation
rate's one-frame `SetAnimRate` delay and its zero value while `gr_vel *
facing <= 0`, checked bit-exactly against the pure helper across a whole
ramp; the frame wrapping at the Slow kind's length under a small, steady
stick; Wait-chain exit on a reversed or below-threshold stick, with Walk's
own Anim-phase advance for that exit frame (Anim precedes IASA) still
accounted for; a tilt press (aged past the smash window, so it lands as a
tilt rather than a fresh-magnitude smash) preempting the retype check on its
own frame -- `update_ground_attacks` returns before the shared Wait/Walk arm
(and its retype check) ever runs, though Walk's own Anim-phase advance for
that same frame still happened, since Anim precedes IASA; checkpoints
restoring the kind, frame and `last_rate` exactly; invalid resources (a
missing pairing, non-finite/non-positive lengths or rates, or
`middle_threshold > fast_threshold`) rejected before a match exists; and
`None` keeping the pre-batch single-Walk, integer-frame behavior.

`crates/skirmish-replay/src/observation.rs`'s existing common-action/
animation-id table test adds Walk with each kind set directly, covering
15/7, 16/8 and 17/9.

`crates/cli/tests/replay_match.rs`'s
`file_backed_walk_ramp_reports_15_16_and_17_with_float_ages_and_a_reduced_
stick_mismatch` walks a full ramp, confirms every recorded row's Slippi
state/animation matches its current kind and that the ramp reaches Middle
then Fast, matches its own generated Peppi bytes end to end (including the
float `state_age`), then reduces the first retype row's stick sample and
confirms the comparison reports a `Mismatch` starting at that exact row.

## Oracle

`tests/oracle/original/walkcommon.c` is a second, independent snapshot of
`ftwalkcommon.c` (`sources.json`; the existing `ftwalk` adapter already pins
the same upstream file's `getWalkAccel`/`ftWalkCommon_800E0060` walk
physics, unrelated to this batch's type/rate/retype layer, so this batch
selects a disjoint function set from a separate fixture name rather than
reusing that adapter). `tests/oracle/walkcommon.functions.json` selects
`ftWalkCommon_GetWalkType`, `ftWalkCommon_800DFC70`, `ftWalkCommon_800DFCA4`,
`ftWalkCommon_800DFDDC` and `ftWalkCommon_800DFEC8` verbatim.
`ftWalkCommon_GetWalkType_800DFBF8_fake` is `static inline` and not
separately extracted; `tests/oracle/walkcommon.c` forward-declares the
public, non-static `ftWalkCommon_GetWalkType` and defines the `_fake` name
as a one-line wrapper around it before including the generated selection, so
`ftWalkCommon_800DFCA4`/`800DFEC8`'s own calls to the inline resolve to the
byte-identical public body. The host adapter stubs `Fighter_ChangeMotionState`
(unused by the three oracle entry points below, since none of them exercise
`800DFCA4`'s own re-entry directly), `ftAnim_8006EBA4` (no-op),
`ftAnim_SetAnimRate` (captures the rate), `ftAnim_8006F484` (returns a
scripted current-figatree length), `ft_GetGroundFrictionMultiplier` (reads an
explicit environment field) and `OSReport`/`HSD_ASSERT` (no-ops; the
`800DFEC8` switch's default case they guard is unreachable given this
adapter's own consistent `msid`/`motion_id` setup), exposing:

- `oracle_walk_type(gr_vel, accel_mul, middle, fast, walk_max) -> kind`
  (0/1/2), calling the public `ftWalkCommon_GetWalkType` directly.
- `oracle_walk_rate(gr_vel, facing, kind, rates[3], x0, friction_mul) ->
  rate`, calling `ftWalkCommon_800DFDDC` with `fp->motion_id -
  fp->mv.co.walk.msid` set up to equal `kind` exactly.
- `oracle_walk_retype(cur_kind, cur_frame, cur_len, lengths[3], gr_vel,
  accel_mul, middle_threshold, fast_threshold, walk_max) -> { changed,
  new_kind, start_frame }`, calling `ftWalkCommon_800DFEC8` with a captured
  callback standing in for the real `ftCo_Walk_Enter` re-entry (not part of
  the pinned selection) and independently calling `ftWalkCommon_GetWalkType`
  with the same fields to report the target kind, since `800DFEC8` never
  exposes its own locally computed `walk_action_type` to its caller.

`tests/walkcommon_differential.rs` compares `fighter::locomotion::walk_kind`/
`walk_animation_rate` against `oracle_walk_type`/`oracle_walk_rate` bit-
exactly over 512 proptest cases each, including NaN and infinite inputs
(chained float comparisons and IEEE arithmetic agree on those between Rust
and this adapter's `-fno-fast-math -ffp-contract=off` build). `walk_kind`'s
own output is a discrete selection, so no NaN-safety caveat applies to it
even though its inputs may be NaN. `walk_retype_frame`'s C float-to-`s32`
cast is undefined for non-finite or out-of-range operands (Rust's `as i32`
saturates instead), so its differential is scoped to the finite, positive
frame/length domain this codebase ever calls it with (0..2000 frames,
1..2000 lengths -- validated figatree lengths and a live, non-negative
`cur_anim_frame`), with a fixed set of thresholds/velocities that
deterministically select every `(cur_kind, new_kind)` pair including the
same-kind no-op, so the full transition matrix is covered independent of the
frame/length values under test. Exact boundaries cover `gr_vel` exactly at
each threshold (scaled by `accel_mul`), negative velocity, `v * facing`
exactly zero, and the retype frame exactly at the current kind's length.

`tests/walk_differential.rs` (pre-existing, from the edge/teeter batch) pins
the unrelated `ftCo_Walk_CheckInput_Ottotto` teeter-walk predicate from a
distinct `original/walk.c` snapshot of `ftCo_Walk.c`; it is unaffected by
this batch.
