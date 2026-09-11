# Idle animation cycling (batch after the Run animation rate)

`skirmish::game::idle` (wiring: the resource, the state, the per-frame
animation-phase draw, `src/game/idle.rs`) and `skirmish::fighter::idle`
(pure arithmetic: the weighted pick and its re-draw gate, `src/fighter/
idle.rs`) cover `Action::Wait`'s idle-animation cycling. Pinned decomp rev
`0bac93a5`. Sources: `src/melee/ft/ftwaitanim.c` (`ftCo_8008A698`:11-17,
`ftCo_8008A6D8`:19-37, `inlineA0`:39-45, `getAnimID`:47-60,
`ftCo_8008A7A8`:62-105), `src/melee/ft/kinds/ftCommon/ftCo_Wait.c`
(`ftCo_Wait_Anim`:34-42: `ftCo_8008A7A8(gobj, ft_data->x24)`),
`src/melee/ft/ft_08A1.c` (`ft_8008A2BC`:54, `ft_8008A348`:71-109, the
ordinary Wait entry), `src/melee/ft/fighter.c` (`Fighter_ChangeMotionState`:
933-1230, `fp->anim_id = new_motion_state->anim_id;` at 1220),
`src/melee/ft/ftanim.c` (`ftAnim_IsFramesRemaining`:515-530),
`src/melee/ft/ftwaitanim.h` (`WaitStruct`:6-17), and
`src/melee/ft/kinds/ftCommon/forward.h` (`ftCo_Submotion`:633-...,
Wait1_0 = 2 at 637, Wait2 = 3, Unk004 = 4, Unk005 = 5, Wait1_1 = 6 at 641).

## Source behavior

- **Entry** (`ft_8008A348`, the ordinary Wait entry reached from
  `ft_8008A2BC`): `Fighter_ChangeMotionState(gobj, ftCo_MS_Wait, Ft_MF_None,
  0.0f, 1.0f, anim_blend, NULL)` (`ft_08A1.c:92-93`). `Fighter_
  ChangeMotionState` sets `fp->anim_id` from the destination motion state's
  own per-character table entry (`fp->anim_id = new_motion_state->anim_id`,
  `fighter.c:1220`, where `new_motion_state` is `fp->x1C_actionStateList[
  ftCo_MS_Wait]`); that per-character table is compiled character data, not
  part of the decomp source available in this checkout, so this port
  assumes every character's Wait entry is 2 (Wait1_0), matching the design
  note's own premise and this codebase's universal convention of a single
  Wait1 loop. `ft_8008A348` separately substitutes `ftCo_MS_DeadUpFall`
  when the fighter holds an item of a kind `ftCo_8008A698` reports
  (`ft_08A1.c:94-96`) -- unrelated to idle cycling and unmodeled here (see
  "Known gaps" below, item-holding).
- **Animation phase** (`ftCo_Wait_Anim` -> `ftCo_8008A7A8(gobj, ft_data->
  x24)`, every Wait frame, unless `fp->x2224_b2` routes to `ftCo_DownSpot_
  Enter` instead -- DownSpot is unmodeled and out of scope here):
  - `ftAnim_IsFramesRemaining(gobj)` (`ftanim.c:515-530`) walks the
    fighter's animated joints/parts directly; this port has no joint model,
    so it substitutes a tracked float frame against the current
    animation's own length (`IdleState.frame >= length`) -- a modeling
    choice, not independently reverse-verified against `ftAnim`/`lbAnim`'s
    own bookkeeping, the same caveat the walk/run batches recorded for
    their own frame-length wrap rules.
  - When frames remain, nothing happens this frame (`ftwaitanim.c:65-105`,
    the whole body is behind the `!ftAnim_IsFramesRemaining` gate).
  - Otherwise: with `arg1 == NULL` (no idle table) *or* holding an item of
    a kind besides Mewtwo/Fox (`ftwaitanim.c:66-69` -- unmodeled, see
    "Known gaps"), restart the current animation with no RNG draw:
    `ftCo_8008A6D8(gobj, fp->anim_id)`.
  - Otherwise, draw `max = HSD_Randi(100) + 1` and walk the table
    (`getAnimID`, `WaitStruct` rows of `{sub_motion, weight}` terminated by
    `sub_motion == -1`), accumulating weight until `max <= count`
    (inclusive: a draw landing exactly on the boundary still selects that
    row); re-draw (a fresh `HSD_Randi` call each time, `ftwaitanim.c:75-79`)
    while `inlineA0` is false (current is neither Wait1_0 (2) nor 31) *and*
    the pick repeats the current `anim_id` -- so a repeated pick from
    Wait1_0 or 31 is accepted immediately, and only a repeat from some
    other current animation forces a re-draw. A table whose accumulated
    weight never reaches `max` asserts (`HSD_ASSERTREPORT`, `ftwaitanim.c:
    59`) instead of returning -- the source relies on `__assert` never
    returning; this port rejects such tables at validation time instead
    (`game::idle::validate`; the C oracle reproduces the source's own
    control flow exactly, including the non-return, via `longjmp`).
  - Enter the picked (or restarted) idle: `fp->anim_id` is set, the idle's
    own script data (`Fighter_WaitAnimData`, hurtbox/flags commands) is
    installed, and the animation restarts at frame 0 (`ftAnim_8006EBE8(gobj,
    0.0, 1.0, blend)`, `ftwaitanim.c:32`/`97`). Script bytecode execution is
    not modeled anywhere in this codebase (a pre-existing gap, not
    introduced here); only the sub-motion id and frame are tracked.
- **Publishing**: `anim_id` is 2 while in Wait1, otherwise the picked
  sub-motion; `state_age` (`fp->cur_anim_frame`-equivalent in this port,
  `action_frame`) restarts at 0 on every restart/pick, since `ftAnim_
  8006EBE8` restarts the figatree at frame 0 and Slippi's `state_age` is
  read from that same frame counter. The motion state stays Wait (14)
  throughout.

## Known gaps (unmodeled, pre-existing or explicitly out of scope)

- **Item-holding and the Mewtwo/Fox exception** (`ftwaitanim.c:66-69`):
  holding an item (of a kind besides Mewtwo's/Fox's own item-hold
  exception) forces the restart-only path regardless of whether a table is
  supplied. This codebase has no item-holding state on `Fighter` at all, so
  it is not modeled; the resource/state shape added here does not
  distinguish it either. `ftCo_8008A698` (`ftwaitanim.c:11-17`) and its own
  substitution use in `ft_8008A348` (`ft_08A1.c:94-96`, the `DeadUpFall`
  swap) are likewise unmodeled, for the same reason.
- **`ftAnim_IsFramesRemaining`'s real joint-based check** is replaced by a
  tracked-frame/length comparison, as described above.
- **Wait script bytecode** (hurtbox/flags commands per idle animation) is
  not executed by this codebase at all; only the sub-motion id and frame
  are tracked, matching every other action in this port.
- **The per-character Wait1_0-is-2 assumption** (`fighter.c:1220`'s table
  lookup): this port cannot verify the per-character `x1C_actionStateList`
  table itself from the pinned decomp source (it is compiled character
  data), so it assumes 2 universally, per this codebase's existing
  single-Wait1-loop convention.

No contradiction between this batch's implementation and the pinned source
was found; the existing test suite (pre-dating this batch) remains green
unchanged.

## Resource and state shape

- `FighterData.idle: Option<idle::IdleAnimations { wait1_length: f32,
  entries: Vec<IdleEntry { animation: u32, weight: i32, length: f32 }> }>`.
  `wait1_length` is Wait1_0's own figatree length, consulted whenever the
  current animation is 2 regardless of whether a table row also lists
  animation 2 (mirroring the source: `wait1_length` is not itself a
  `WaitStruct` row). `entries` may be empty -- then only the restart-at-
  `wait1_length` rule applies, matching the source's own `arg1 == NULL`
  behavior even though the resource itself is present. Validated: `0 <
  wait1_length <= 1_000_000`, every entry's `weight > 0` and `0 < length <=
  1_000_000`, and (unless `entries` is empty) the entries' weights sum to
  at least 100 -- rejecting tables `getAnimID`'s own walk could fall off
  the end of (`ftwaitanim.c:50-59`). Absent entirely keeps Skirmish's
  pre-batch behavior: `Action::Wait` never advances or restarts (`action_
  frame` grows without bound) and the reported animation index stays 2.
- `Fighter.idle: idle::State { animation: u32, frame: f32 }` (`Serialize`/
  `Deserialize`, checkpoint-safe: an ordinary field of the checkpointed
  `Fighter`). Reset to `{ animation: 2, frame: 0.0 }` on every action
  transition (`simulation::enter`), unconditionally like `dash::State`/
  `smash::State` -- harmless outside Wait, since both fields are read only
  while `Action::Wait` is current.

## RNG

`game::idle::update_animation` draws from the match's shared `HsdRng`
during the per-player animation-phase loop in `simulation::advance`, in
player order, matching `ftCo_Wait_Anim`'s own place in Melee's per-fighter
callback order. That loop reassigns `state.rng_seed` from the loop's own
advanced `HsdRng` immediately after both players are processed, and the
same frame's blast-zone death draw (further down in `simulation::advance`)
reconstructs its own `HsdRng` from that already-advanced `state.rng_seed`
-- so an idle draw earlier in the frame always shifts a later death draw's
position in the shared sequence, exactly as Melee's single per-frame RNG
stream would. `tests/game_idle.rs`'s `the_idle_draw_shifts_the_same_frames_
death_draw` exercises this directly with a real hit-driven blast-death
(see "Tests" below).

## Tests

`src/fighter/idle.rs` unit-tests `pick`'s boundary selection, the Wait1_0/
31 exemption, the non-exempt re-draw and the "table falls short of `max`"
panic.

`tests/game_idle.rs` (7 tests, `tests/support/idle.rs` supplies an invented
two-entry table and a shared `wait1_length`) covers: a table-less restart
at `wait1_length` leaves the animation at 2, resets the tracked frame and
`action_frame`, and draws no RNG; a two-entry table's pick matches `pick`
called directly against a fresh `HsdRng` from the same match seed; a
repeated pick from a non-Wait1 current animation re-draws, with the seed
advancing by two draws for that event (constructed so the first scripted
draw deterministically repeats the current animation and the second
differs); the idle draw shifting a same-frame death roll's `Kind` between
"resource present" and "resource absent" runs of an otherwise-identical
real hit-driven blast-death scenario (a grounded fighter can never itself
exceed a valid `blast.top`, since stage validation requires `floor.y <
blast.top` strictly, so this uses `tests/support/death.rs`'s own proven
hit/knockback shape rather than a synthetic position); checkpoints restore
the idle state and seed exactly across a pick; invalid resources (non-
finite/non-positive `wait1_length` or entry fields, weights summing below
100) are rejected before a match exists; and `None` keeps the pre-batch
unbounded integer `action_frame` with the animation index fixed at 2.

`crates/skirmish-replay/src/observation.rs`'s `animation_index` gains a
dedicated `14 => fighter.idle.animation` arm (previously `12..=14 => 2`
covered Rebirth/RebirthWait/Wait alike with the same constant; Rebirth/
RebirthWait keep the constant, since respawn animations are out of this
batch's scope). `action_age` needed no change: Wait was already covered by
`observe`'s catch-all `fighter.action_frame as f32` branch, and `idle::
update_animation`'s own `action_frame` reset on every restart/pick keeps
that branch correct.

`crates/cli/tests/replay_match.rs`'s `file_backed_idle_fighter_reports_the_
picked_animation_index_with_a_restarting_age` drives an idle fighter with
the two-entry table (a seed chosen so the first pick visibly lands on the
second entry, animation 3), locates the restart/pick row from a trace of
the tracked idle frame (rather than assuming it), confirms the reported
animation index and the restarted `action_frame` there, matches its own
generated Peppi bytes end to end, then corrupts that exact row's animation-
index field and confirms the comparison reports a `Mismatch` starting
there.

## Oracle

`tests/oracle/original/waitanim.c` is a snapshot of `ftwaitanim.c`
(`sources.json`). `tests/oracle/waitanim.functions.json` selects
`ftCo_8008A698`, `ftCo_8008A6D8`, `inlineA0`, `getAnimID` and
`ftCo_8008A7A8`, in the pinned source's own definition order. `inlineA0`/
`getAnimID` are `static inline` in the original file, but `build.rs`'s
`extract_function` matches definitions purely by name/position (a column-
zero header containing no `;`/`=`/`(` before the name, with no `;` before
the opening brace) regardless of `static`/`inline` qualifiers, so both are
extracted verbatim like the non-static functions; they still had to be
listed explicitly, since extracting only `ftCo_8008A7A8` would not have
pulled in the separate top-level definitions it calls. `ftCo_8008A698` is
not actually called by any of the other four (`ftCo_8008A7A8` checks `fp->
item_gobj` inline rather than through it) but is included per this batch's
selection regardless, for completeness against the pinned file.

The host adapter (`tests/oracle/waitanim.c`) stubs `ftAnim_
IsFramesRemaining` (scripted bool) and `HSD_Randi` (a scripted sequence
recording how many draws were consumed); `ftData_80085CD8`, `ftCo_
8009E7B4`, `ftAnim_8006EBE8`, `ftAnim_8006EBA4` and `itGetHoldKind` as no-
ops; `HSD_ASSERTREPORT` recording the assert and `longjmp`-ing back to the
call site -- reproducing the source's own "never returns" contract (the
real `__assert` it calls is `ATTRIBUTE_NORETURN`, and `getAnimID` has no
`return` statement after its own `HSD_ASSERTREPORT` call, so letting
execution fall through instead reads an undefined return value that the
caller then uses as an `fp->x24`/`x28` array index -- this crashed the
adapter with a real out-of-bounds access before the `longjmp` fix); a
`WaitStruct` table built from the caller's entries with the `-1`
terminator; `fp->x24`/`x28` arrays sized 64; `kind` 0; `item_gobj` NULL.
It exposes `oracle_wait_anim(current_anim, frames_remaining, table[],
table_len, rng_sequence[], rng_len, out_draws, out_asserted) -> new_anim`.

`tests/idle_differential.rs` compares `fighter::idle::pick` against `oracle_
wait_anim` over 512 proptest cases (equal-share-weight tables of 1..=5
entries using animation ids 2..=6, summing to exactly 100; current
animations in `{2, 3, 6, 31, 40}`; 64-value scripted `HSD_Randi` sequences
-- long enough that a worst-case run of consecutive re-draws against a
<=50%-weighted colliding entry is astronomically unlikely to exhaust it),
plus exact boundaries: `max == count` selects inclusively rather than
falling through; a repeated pick from a non-exempt current re-draws in
both implementations; a repeated pick from Wait1_0/31 never re-draws;
weights summing below 100 assert in the oracle and panic in `pick`; frames
still remaining draws nothing and leaves `anim_id` unchanged; and an empty
table restarts the current animation without drawing.
