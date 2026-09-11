# Taunt and the Wait-chain spot dodge

`skirmish::game::taunt` (wiring: the resource, the entry dispatch across
every chain that lists it, the animation/physics/collision callbacks,
`src/game/taunt.rs`) and `skirmish::fighter::taunt` (pure arithmetic: the
D-pad-up press check and the facing/availability side selection,
`src/fighter/taunt.rs`) cover `Action::AppealSR`/`AppealSL`. The same batch
adds the D-pad input bits and corrects the Wait-chain spot dodge
(`ftCo_80099794`, covered in its own section below). Pinned decomp rev
`0bac93a5`. Sources: `src/melee/ft/kinds/ftCommon/ftCo_AppealS.c`
(`ftCo_800DE9B8`:35-41, `ftCo_800DE9D8`:43-50, `ftCo_800DEA28`:52-73,
`ftCo_800DEAE8`:75-87, `ftCo_800DEBD0`:89-104, `ftCo_AppealS_Anim`:106-111,
`ftCo_AppealS_IASA`:113-132, `ftCo_AppealS_Phys`:134-137,
`ftCo_AppealS_Coll`:139-142), `ftCo_Escape.c` (`inlineB0`:198-206,
`ftCo_80099794`:208-216, `ftCo_8009980C`:218-226, `ftCo_80099894`:228-240),
`ftCo_Wait.c` (`ftCo_Wait_IASA`:44-67, the chain itself: line 58 =
`ftCo_80099794`, line 59 = `ftCo_80091A4C`, line 60 =
`ftFx_AppealS_CheckInput`, line 61 = `ftCo_800DE9D8`), the twelve other
`ftCo_800DE9D8` call sites (below), `sysdolphin/baselib/controller.h:13-16`
(`HSD_PAD_DPADLEFT/RIGHT/DOWN/UP = 1 << 0..3`), `src/melee/ft/kinds/
ftCommon/forward.h` (`ftCommon_MotionState`: `ftCo_MS_AppealSR` = 264,
`ftCo_MS_AppealSL` = 265; `ftCo_Submotion`: `ftCo_SM_AppealSR` = 239,
`ftCo_SM_AppealSL` = 240, both computed by enum position from their own
`= -1` base), `ft_081B.c` (`ft_80084104`:1043-1050) and `ft_084E.c`
(`ft_80084FA8`:55-68, `ft_80085030`:79-89).

## D-pad input

`sysdolphin/baselib/controller.h:13-16`: `HSD_PAD_DPADLEFT = 1 << 0`,
`HSD_PAD_DPADRIGHT = 1 << 1`, `HSD_PAD_DPADDOWN = 1 << 2`, `HSD_PAD_DPADUP
= 1 << 3` -- the low nibble of the native pad word, shared by Slippi's
physical and processed button words (`crates/skirmish-replay/src/
observation.rs`'s `controllers`: `game::Controller.buttons` is `pre.
buttons_physical` directly, and the processed mask is `u32::from(BUTTONS)`
over the same constant). `game::mod` adds `BUTTON_DPAD_LEFT = 0x1`,
`BUTTON_DPAD_RIGHT = 0x2`, `BUTTON_DPAD_DOWN = 0x4`, `BUTTON_DPAD_UP =
0x8`; the importer's `BUTTONS` mask accepts all four (previously it
rejected every one), so a real replay containing a D-pad left/right/down
press now imports -- only D-pad up (the taunt press) has an observable
effect anywhere in this codebase. `game::validation::inputs` (the
`Match::step` input mask, independent of the replay importer) accepts the
same four bits.

## Taunt

- **Input**: `ftCo_800DE9B8` (`fp->input.pressed_buttons & HSD_PAD_DPADUP`)
  is a fresh D-pad-up press; `ftCo_800DE9D8` checks it and, on success,
  dispatches into the entry chain and reports "handled" to its caller's own
  `RETURN_IF`.
- **Chain position**: `ftCo_800DE9D8` is called from exactly thirteen
  places in `ftCommon` (confirmed by an exhaustive grep of the pinned
  source directory): `ftCo_Wait.c:61`, `ftCo_Walk.c:98`, `ftCo_Squat.c:111`,
  `ftCo_SquatWait.c:106`, `ftCo_SquatRv.c:67`, `ftCo_Turn.c:125`, `ftCo_
  Landing.c:143`, `ftCo_Run.c:122`, `ftCo_RunDirect.c:43` (this port has no
  separate RunDirect state; its own real chain is otherwise identical to
  Run's, and is reached the same way here), `ftCo_Ottotto.c:79`, `ftCo_
  AttackS4.c:227` (inside its `if (fp->allow_interrupt)` interruptible
  block), and `ftCo_Dash.c:128` (`block_42`, inside `ftCo_Dash_IASA`). Every
  one of these reaches it immediately after its own ordinary shield check
  (`ftCo_80091A4C`/`ftCo_80091AD8`, or -- for the down tilt's own
  interruptible block, which has no shield check of its own -- at the same
  relative position after its attack checks) and immediately before its own
  jump check (`ftCo_Jump_CheckInput`/`fn_800CAF78`). This port implements
  the check once, in `simulation::update_actions`, called right after
  `shield::update_actions` returns false, gated to `f.grounded &&
  (matches!(Wait | Walk | Squat | SquatWait | SquatRv | Turn) ||
  edge::owns_action(f.action) || matches!(tilt::interrupt_chain(f, data),
  Some(Chain::Wait) | Some(Chain::DownTilt)))` -- covering Wait, Walk,
  Squat, SquatWait, SquatRv and Turn directly; Landing and AttackS4's
  interruptible frames via `Chain::Wait` (`tilt::interrupt_chain` already
  folds `landing::interruptible`/`smash::interruptible` into it);
  Ottotto/OttottoWait via `edge::owns_action`; and the down tilt's
  interruptible frames via the dedicated `Chain::DownTilt` (whose own real
  chain has no shield check, unlike every other member of this list, but
  still reaches `ftCo_800DE9D8`). Dash and Run instead reach it from
  `dash::update_dash_or_run`'s own `block_42`, before the shared jump
  dispatch there -- see "Dash and Run" below; this keeps them from being
  checked twice, since `dash::update_actions` (called earlier in `simulation
  ::update_actions`) already fully owns their frame.
- **Entry** (`ftCo_800DEA28` -> `ftCo_800DEBD0` -> `ftCo_800DEAE8`):
  `ftCo_800DEA28` is a per-character-kind pre-hook (Young Link `ftCl_Init_
  80149318`, Dr. Mario `ftDr_Init_80149910`, Ganondorf a captain-sword
  particle spawn, every other kind falls straight to `ftCo_800DEBD0`), then
  always calls `ftCo_800DEBD0` and a bonus-stat tracking call
  (`pl_80040120`); `ftCo_800DEBD0` is debug-rom-only Peach/Zelda hooks and a
  Kirby copy-ability re-init, then unconditionally `ftCo_800DEAE8(gobj,
  ftCo_MS_AppealSR, ftCo_MS_AppealSL)`; `ftCo_800DEAE8` sets `fp->
  allow_interrupt = false`, then picks `msid1` (AppealSL) when `fp->
  facing_dir == -1.0f` AND the AppealSL motion's own animation data reports
  its figatree available (`ftData_80085FD4(fp, ms->anim_id)->x8 != 0`,
  modeled here as "a left motion is supplied"), else `msid0` (AppealSR),
  and enters it (`Fighter_ChangeMotionState`, frame 0, rate 1.0). `fighter::
  taunt::select_side`/`pressed` are the pure mirrors of the selection and
  press check; `game::taunt::try_taunt` calls them directly. Young Link,
  Dr. Mario, Ganondorf, the debug-only Peach/Zelda hooks, Kirby's copy-
  ability re-init and the bonus-stat call are not modeled.
- **Animation** (`ftCo_AppealS_Anim`): end of poses returns to Wait (`ft_
  8008A2BC`); taunts are grounded-only, so there is no airborne `Fall`
  destination to consider, unlike most other actions in this port.
- **IASA** (`ftCo_AppealS_IASA`): gated on `fp->allow_interrupt`, then --
  in this exact order -- `ftCo_SpecialS_CheckInput`, `ftCo_
  Attack100_CheckInput`, `ftCo_800D6824`, `ftCo_800D68C0`, `ftCo_
  Catch_CheckInput`, `ftCo_AttackS4_CheckInput`, `ftCo_AttackHi4_
  CheckInput`, `ftCo_AttackLw4_CheckInput`, `ftCo_AttackS3_CheckInput`,
  `ftCo_AttackHi3_CheckInput`, `ftCo_AttackLw3_CheckInput`, `ftCo_
  Attack1_CheckInput`, `ftCo_80099794` (the Wait-chain spot dodge, below)
  and `ftCo_80091A4C` (the ordinary shield check) -- exactly `ftCo_
  Wait_IASA`'s own attack/catch/spot-dodge/shield segment (lines 46-59),
  without its Fox-appeal check, its own `ftCo_800DE9D8` self-recheck, or
  its jump/dash/squat/turn/walk tail. `tilt::Chain::Taunt` models this: it
  is returned by `taunt::interrupt_chain` whenever the current pose's
  `allow_interrupt` flag is set, and is treated as equivalent to `Chain::
  Wait` everywhere shield/catch/special dispatch already check for it
  (`shield::update_actions`, `grab::update_actions`, `special::
  update_actions`), while `locomotion::update_actions` explicitly excludes
  it (`taunt_chain` there) from the jump/dash/squat/turn/walk match arm and
  the jump-input dispatch, even though it still reaches `tilt::
  update_ground_attacks` (smashes, tilts, jab) through the same
  `interruptible_tilt` gate every other interruptible chain uses.
- **Phys** (`ftCo_AppealS_Phys` -> `ft_80084FA8` -> `ft_80085030`): when
  the animation's own `x594_b0` root-motion flag is set, ground
  acceleration is derived directly from the animation's local Z
  translation (TransN) scaled by facing, exactly like the escape rolls;
  otherwise ordinary ground friction applies (scaled by the shared
  above-walk-speed multiplier past `walk_max_vel`, same as Wait's own
  `ft_80084F3C`). `TauntAnimation.root_translations: Option<Vec<f32>>`
  models the flag: `Some` supplies a per-pose delta (`game::taunt::
  ground_target_velocity`, consulted in `move_fighter`'s target-velocity
  chain the same way escape/smash/jab/dash-attack root motion already is);
  `None` falls through to ordinary friction.
- **Coll** (`ftCo_AppealS_Coll` -> `ft_80084104`): the same generic
  ground-collision-with-ledge-fall check every other grounded attack-like
  state in this port uses (`edge::mode_for_action`'s `Mode::Clamp`, already
  covering EscapeF/EscapeB/EscapeN, the tilts, the smashes, the dash
  attack, Catch/CatchDash/CatchCut, DownAttack, PassiveStandF/B, CliffClimb,
  RunTurn and Ottotto/OttottoWait, all of which call `ft_80084104` too):
  `AppealSR`/`AppealSL` clamp at a floor end instead of sliding through.
- **Resource identity**: no explicit action-instance queue call on entry,
  matching Wait's own default (`simulation::enter`'s ordinary `Ft_MF_None`
  path).
- **Dash and Run**: `ftCo_Dash_IASA`'s `block_42` reads `if (!ftCo_
  800DE9D8(gobj)) { ...jump/run-transition checks, each an early return...
  } ` -- a fired taunt is the pinned source's only way to fall out of
  `block_42` into the shared `x54` friction tail instead of returning
  (`docs/dash-attack.md`); `ftCo_Run_IASA` reaches `ftCo_800DE9D8` at the
  exact same relative position (right before its own shared jump check,
  `fn_800CAF78`) but has no friction tail of its own. This port's `dash::
  update_dash_or_run` already reaches a single shared `block_42` tail for
  both Dash and Run (the jump dispatch, then Dash's run transition or
  Run's turn/brake logic), so the taunt check is implemented once there,
  right before the jump dispatch, with the friction application
  conditioned on `f.action == Action::Dash` at the moment it fires.

## Wait-chain spot dodge (`ftCo_80099794`)

`ftCo_Wait.c:58` and `ftCo_AppealS_IASA` (at the equivalent position, ahead
of the ordinary shield check in both) are the *only* two callers of `ftCo_
80099794` in the entire pinned `ftCommon` directory (confirmed by an
exhaustive grep; Walk's own chain, and every other `ftCo_800DE9D8` caller
above, has no such call). Unlike the escape module's existing `ftCo_
8009980C` (checked from every guard IASA chain: `inlineB0` OR a held
downward C-stick, `ftCo_800DF8E8`), `ftCo_80099794` requires a held
*logical shoulder* (`fp->input.held_buttons[0] & HSD_PAD_LR`) **AND**
`inlineB0` (a fresh downward main stick: `stick.y <= x314` with the shared
stick-Y age below `x318`) -- the C-stick alternative is not part of this
check at all. On success it dispatches straight into `ftCo_80099894`
(EscapeN entry, with a Yoshi-specific variant this port does not model)
and reports handled, exactly like `ftCo_8009980C` does when it fires.

Before this batch, Skirmish modeled only the ordinary guard-IASA spot
dodge: pressing a shoulder with the stick already down from Wait raised
GuardOn on that press frame (Wait's own neutral shield-entry check had no
idea a dodge was also available), and only dodged a frame later, once
already inside GuardOn, via the pre-existing `ftCo_8009980C`-equivalent
check. This batch adds `game::escape::try_wait_chain_spot_dodge` (the
main-stick-only predicate, reusing `fighter::escape::main_stick_spot_
dodge` and the same `Rules.spot_dodge_stick_threshold`/`spot_dodge_
window`) and calls it from `simulation::update_actions`, gated to
`f.action == Action::Wait || tilt::interrupt_chain(f, data) ==
Some(Chain::Taunt)`, immediately *before* `shield::update_actions`. So a
held shoulder with the stick already down (fresh, inside the window) now
enters EscapeN on that exact frame from Wait or an interruptible AppealS
pose, without a GuardOn frame in between; the same input from Walk, or
with a stale stick, or with only the C-stick held down, still raises
GuardOn (or does nothing) as before.

## Known gaps (unmodeled, pre-existing or explicitly out of scope)

- The per-character `ftCo_800DEA28` pre-hooks (Young Link, Dr. Mario,
  Ganondorf) and `ftCo_800DEBD0`'s debug-rom-only Peach/Zelda hooks and
  Kirby copy-ability re-init: this port has no per-character kind
  dispatch at all (a pre-existing gap, not introduced here).
- `ftFx_AppealS_CheckInput` (checked in `ftCo_Wait_IASA` immediately
  *before* `ftCo_800DE9D8`, line 60 vs. 61): Fox/Falco's own Corneria
  taunt-input override. Unmodeled; this port's D-pad-up check is
  unconditional wherever the chain reaches it.
- The Yoshi-specific branch of `ftCo_80099894` (`ftCo_80099954`, an extra
  `x5F4_arr` check before the ordinary EscapeN entry): unmodeled, matching
  the existing escape module's own Yoshi-branch gaps.
- `pl_80040120`'s bonus-stat tracking call: not modeled (no bonus-stat
  system exists in this codebase).
- This port has no separate `RunDirect` action (`ftCo_RunDirect.c`, one of
  the thirteen real `ftCo_800DE9D8` callers); its own chain position is
  otherwise identical to Run's and is reached the same way here.

No contradiction between this batch's implementation and the pinned source
was found; the pre-existing test suite remains green unchanged.

## Resource and state shape

- `FighterData.taunt: Option<taunt::Taunt { right: TauntAnimation, left:
  Option<TauntAnimation> }>`. `TauntAnimation { frames: Vec<TauntFrame {
  bones: Vec<Bone>, body_state: BodyState }>, flags: Vec<tilt::
  GroundFrameFlags> (only `allow_interrupt` is meaningful; `repeat_ready`
  is rejected), root_translations: Option<Vec<f32>> }` -- the same
  attack-like pose-without-hitboxes shape `escape::SpotDodgeFrame` already
  uses, reused directly rather than duplicated. Validated: 1..=4096 poses,
  one flags entry per pose, frame 0's `allow_interrupt` must be `false`
  (matching `ftCo_800DEAE8`'s own entry assignment, ahead of any script
  command that could raise it), and (when supplied) one finite,
  magnitude-bounded root translation per pose.
- No new per-fighter runtime state: which side is currently playing is
  `fighter.action` itself (`Action::AppealSR`/`AppealSL`), and the sampled
  pose/flags/root-motion lookups key off `fighter.action` and `fighter.
  action_frame` directly, the same pattern `escape`/`edge` already use.

## Tests

`src/fighter/taunt.rs` unit-tests `pressed`'s exact-bit check and `select_
side`'s exact `facing == -1.0` comparison (a near-miss like `-0.5` still
selects AppealSR).

`tests/game_taunt.rs` (`tests/support/taunt.rs`'s shared `profile` fixture
installs an invented right/left motion pair, the right with root motion
and the left without; the facing-left-without-a-left-motion case builds
its own resource directly, cloning the right motion into a `left: None`
`Taunt`) covers: taunt firing after the shield check
and before the jump from Wait, Walk, Squat, SquatWait, Turn (confirming
the *post*-turn facing selects the side), Run, Landing's interruptible
frame, an interruptible smash pose and Ottotto; Dash falling through into
its own friction tail while the identical check from Run does not;
facing left picking AppealSL when supplied and AppealSR otherwise; every
D-pad bit besides up staying inert; the taunt's own interruptible chain
admitting catch, a fresh-stick smash (outranking a moderate tilt-only
stick), a moderate-stick tilt, a plain jab press, the Wait-chain spot
dodge and shield, while a jump press, a dash-magnitude stick alone and a
downward stick without the shoulder all stay in the taunt; exact sampled
root motion matching the supplied deltas and ending into Wait; clamping at
a floor end instead of sliding through; the Wait-chain spot dodge's own
cases (a fresh downward stick with a held shoulder dodging immediately
from Wait and from an interruptible AppealS pose; a stale stick or a held
C-stick alone still raising GuardOn instead; the same fresh combination
from Walk still raising GuardOn, since Walk's chain has no `ftCo_
80099794` call); checkpoint replay; invalid resources (empty poses, a
flags/poses length mismatch, a `true` `allow_interrupt` on the entry pose,
a `repeat_ready` flag, a wrong-length or non-finite root-motion sample);
and `None` keeping D-pad up inert.

## Oracle

`tests/oracle/original/appeal.c` is a snapshot of `ftCo_AppealS.c`
(`sources.json`). `tests/oracle/appeal.functions.json` selects `ftCo_
800DE9B8`, `ftCo_800DE9D8`, `ftCo_800DEAE8`, `ftCo_800DEBD0` and `ftCo_
AppealS_IASA`, in the pinned source's own definition order; `ftCo_
800DEA28` is deliberately not extracted (outside this batch's selection)
and is instead hand-written in the host adapter (`tests/oracle/appeal.c`),
reproducing its real per-character switch with every side effect stubbed
to a no-op -- every oracle call uses `kind == 0`, matching none of `FTKIND_
CLINK`/`DRMARIO`/`GANON`, so it always takes the real function's own
`default` branch and only the extracted `ftCo_800DEBD0` -> `ftCo_800DEAE8`
chain is ever exercised. `ftData_80085FD4` is a scripted per-animation-id
availability table (>= 512 entries); `Fighter_ChangeMotionState` captures
only the selected motion id and `fp->allow_interrupt` at the moment of the
call; `DbLevel` is fixed at 0, so the debug-rom-only Peach/Zelda hooks
never fire. Every `ftCo_AppealS_IASA` `CheckInput` dependency is stubbed to
log its own call, in the source's own `RETURN_IF` order, and answer from a
caller-supplied bitmask. The adapter exposes `oracle_taunt_enter(pressed,
facing, left_available) -> (fired, msid, allow_interrupt)` and `oracle_
taunt_iasa(allow_interrupt, answers) -> (fired, calls[])`.

`tests/oracle/escape.functions.json` and `tests/oracle/escape.c` (the
pinned `ftCo_Escape.c` snapshot from the earlier escape batch) gain `ftCo_
80099794` and `oracle_wait_spot_dodge(held, stick_y, tilt_y_age, threshold,
window) -> (fired, motion)`, reusing the adapter's existing `ftCo_
80099894` stub and adding `held_buttons`/`HSD_PAD_LR` to its `Fighter`/
`FighterInput` model.

`tests/taunt_differential.rs` compares: `fighter::taunt::select_side`/
`pressed` against `oracle_taunt_enter` over 512 proptest cases (arbitrary
press/facing/availability, including NaN facing) plus exact boundaries (no
press, every facing/availability combination, NaN facing); a pure Rust
mirror of `ftCo_AppealS_IASA`'s 14-check `RETURN_IF` chain against `oracle_
taunt_iasa` over 512 proptest cases (arbitrary `allow_interrupt`/answer
bitmask) plus exact boundaries (the chain locked entirely, each of the 14
checks as the sole winner, none winning, every check winning); and `held
&& fighter::escape::main_stick_spot_dodge` against `oracle_wait_spot_dodge`
over 512 proptest cases (arbitrary held/stick/age/threshold/window, NaN-
safe) plus the same threshold/window boundaries `escape_differential`
already uses for the sibling `ftCo_8009980C` predicate.
