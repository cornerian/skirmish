# Fox/Falco side special (Illusion/Phantasm)

`skirmish::game::characters::fox::side` (resource, dispatch, `src/game/
characters/fox/side.rs`, implementing the shared `game::specials` framework)
and `skirmish::fighter::characters::fox` (pure arithmetic: the input gate,
the turn check, the entry ground-speed blend, `src/fighter/characters/
fox.rs`) cover `Action::SpecialSStart/SpecialS/
SpecialSEnd/SpecialAirSStart/SpecialAirS/SpecialAirSEnd`. Pinned decomp rev
`0bac93a5`. Sources: `src/melee/ft/kinds/ftFox/ftfoxspecials.c` (Start
88-246, the aerial Start entry 108-129, the dash pair 261-465, End
467-607), `src/melee/ft/kinds/ftCommon/ftCo_SpecialS.c` (`ftCo_
SpecialS_HasInput` 15-23, `ftCo_SpecialS_CheckInput` 25-39, `doEnter`
41-49), `src/melee/ft/kinds/ftCommon/ftCo_SpecialAir.c` (`ftCo_
SpecialAir_CheckInput` 11-56), `fighter.c:1725-1739` (`x686..x689`
press-age counters, `x688` is the side-special one), `ft_081B.c` (`ft_
80082708`:393-403, `ft_800827A0`:406-424, `ft_GetGroundFrictionMultiplier`
:1235-1240), `ft_084E.c` (`ft_80085004`..`ft_80085154`:91-158), `ftcommon.c`
(`ftCommon_Fall`:462-467), `ftCo_Landing.c` (`ftCo_LandingFallSpecial_
Enter`:103-113), `ftCo_FallSpecial.c` (`ftCo_80096900`/`inline0`:20-59),
`ftFox/forward.h:52-63` (motion ids), `ftFox/types.h:93-105` (attributes),
`itfoxillusion.c` (the ghost item, whole file) and `it_3F2F.c:360-390`
(the item state-table registration).

## Input (`ftCo_SpecialS.c`, `ftCo_SpecialAir.c`)

- **`ftCo_SpecialS_HasInput`**: a fresh B press (`pressed_buttons & HSD_
  PAD_B`) with `|stick.x| >= x218`. `fighter.c:1735-1739` feeds this into
  `fp->x688`: 0 on a frame `HasInput` fires, incremented (saturating at
  255) otherwise -- the exact pattern `x67C..x67D`'s own press-age
  tracking above it in the same function uses, and the one `attack_b_age`
  (x67D) already models in this port (`game::locomotion::State`). x688 is
  *not* the same counter: x67D fires on any fresh B press, x688 only on
  one with the stick already past the side threshold, so this batch adds
  a sibling field, `side_special_b_age`, updated the same way in
  `simulation::sample_input_history`, gated on `rules.specials.side_
  stick_threshold` (`f32::INFINITY` when absent, so the gate never opens).
- **`ftCo_SpecialS_CheckInput`** (grounded): fires only when the
  character has this move and `x688 == 0` -- since both `x688`'s own
  update and this check run in the same frame, in that order, this
  reduces to "`HasInput` fired this exact frame", but the age field is
  still modeled explicitly rather than inlined, for parity with `x67D`'s
  existing treatment and any future consumer. Turns around
  (`ftCommon_UpdateFacing`) when `stick.x * facing < -x220`, then calls
  `doEnter` before the character's own Start entry.
- **`doEnter`**: `gr_vel += -(gr_vel * (1 - specials_ground_speed_
  retention)) * ft_GetGroundFrictionMultiplier(fp)`. The multiplier is
  the *current floor material's* friction multiplier (`ft_081B.c:
  1235-1240`: 1.0 for Ice Climbers always, else `mpColl_8004CA6C`'s
  per-surface lookup) -- **not** a fixed `1.0`. This port has no modeled
  per-surface friction-material table (only a single `rules.clank.
  surface_friction_multiplier` scalar exists, unrelated to floor
  material), so `fighter::characters::fox::entry_ground_velocity`
  implements the ordinary-terrain case (multiplier `1.0`) only. This is a
  known simplification, not a silent rewrite of the source: it is exact
  on every ordinary floor and wrong only on a floor whose material
  friction multiplier differs from 1.0, which this codebase does not
  currently model at all.
- **`ftCo_SpecialAir_CheckInput`** (aerial): on a fresh B press, `stick.y
  >= x21C` selects SpecialAirHi and `stick.y <= -x21C` selects
  SpecialAirLw -- Fox has both moves, but neither is modeled in this
  codebase; the dispatcher still checks the threshold first and declines
  to fire the side branch when it is met, rather than falling through to
  it (`rules.specials.vertical_threshold`). Otherwise, `|stick.x| >= x218`
  selects the side branch with the same turn rule as the grounded check
  (no `x688`-style age gate exists for the aerial dispatcher in the
  source); else the existing neutral branch (unchanged by this batch).
- **Chain position**: `ftCo_SpecialS_CheckInput` is the first `RETURN_IF`
  in every grounded chain that also reaches the neutral special (Wait,
  Walk, both Dash/Run phases, Squat/SquatWait/SquatRv, Turn, Landing, the
  interruptible attack chains, the taunt's own interruptible frames).
  This port computes that exact `ground`/`air` eligibility once, in
  `game::specials`'s shared dispatch, and hands it to every registered move
  in priority order; Fox's registry (`characters::fox::MOVES`) lists this
  move's own `update_actions` ahead of the shared neutral shell, matching
  the source's own before-this-special-after ordering. The `specials::
  update_actions` -> `dash::apply_transition_friction` interaction
  documented for the neutral shell (`docs/specials.md`) is unchanged: it
  still fires whenever *either*
  special enters from Dash.

## Fox/Falco phases (`ftfoxspecials.c`)

- **`SpecialSStart`/`SpecialAirSStart`** (`ftFx_SpecialSStart_Enter`/
  `ftFx_SpecialAirSStart_Enter`): `cmd_vars[2] = 0` (ghost-item
  bookkeeping, unmodeled -- see "Ghost item" below), `gravityDelay = x24`,
  ground `gr_vel /= x28`; air `self_vel.y = 0`, `self_vel.x /= x28`,
  `jumps_used = max`. **Anim**: end of poses enters the Dash phase.
  **IASA**: none. **Phys**: both variants count `gravityDelay` down every
  frame it is nonzero, *even on the ground*, where the value is never
  read again (`ftFx_SpecialSStart_Phys` ticks it unconditionally, exactly
  like the air variant -- this is not a no-op skipped by this port;
  this move's own `tick_ground_timers` (calling the shared `specials::
  helpers::tick_ground_delay`) mirrors it, since a
  mid-Start ground<->air conversion must see the same countdown the air
  variant would have reached). Ground: ordinary ground friction
  (`ft_80084F3C`, the fighter's plain `ground_friction` attribute -- no
  dedicated Start-ground attribute exists in `ftFox_DatAttrs`). Air: after
  the delay, `ftCommon_Fall(x30, terminal_velocity)`; always `ftCommon_
  ApplyFrictionAir(x2C)`. **Coll**: ground `ft_80082708` false -> ground-
  to-air at the current animation frame (`ftCommon_8007D60C`,
  `Fighter_ChangeMotionState` with `ftFx_MF_SpecialS_Coll`); air `ft_
  CheckGroundAndLedge` true -> air-to-ground at the current frame
  (`ftCommon_AirToGroundStateChange`), else `ftCliffCommon_80081298`
  (ledge catch, reusing the ledge module's own check like the aerial
  attacks). This move's `transfer_ground_air` (the `SpecialMove` phase hook
  `game::specials` dispatches through) handles the Start pair; it snapshots
  and restores `fox_side_special.gravity_delay` around the shared
  `specials::helpers::transfer_frame` call the same way that helper already
  preserves `action_frame`, since `enter()` otherwise resets all per-move
  state including this one.
- **`SpecialS`/`SpecialAirS`** (the dash): `ftFx_SpecialS_Enter`/`ftFx_
  SpecialAirS_Enter` -> `ftFox_SpecialS_SetVars` (ghost bookkeeping only,
  unmodeled). **Anim**: end of poses enters the End phase; the ghost item
  is (re)spawned every frame `cmd_vars[2] == 1` (unmodeled -- see "Ghost
  item"). **IASA**: a fresh B press shortens the dash into the End phase
  immediately, on the ground or in the air *by `ground_or_air` at that
  moment*, not by which Dash variant currently owns dispatch (`ftFx_
  SpecialS_IASA`/`ftFx_SpecialAirS_IASA` are identical: both check
  `ground_or_air` themselves). **Phys**: ground `ft_80085088` ->
  `ft_800850E0`: when the current pose's root-motion flag (`x594_b0`) is
  set, `gr_vel = transN.z * facing` (a direct set, not an accumulation);
  else ordinary ground friction (`co_attrs.ground_friction` -- the same
  attribute the default grounded branch already uses, so a pose with no
  TransN sample falls through to this port's existing plain-friction
  branch with no dedicated Dash attribute). Air `ft_80085134`:
  `self_vel = (transN.z * facing, transN.y)`, unconditionally -- the air
  variant has no root-motion gate at all, unlike every ground-side TransN
  consumer in this codebase. `Dash.ground_trans_n: Vec<Option<f32>>` and
  `Dash.air_trans_n: Vec<[f32; 2]>` model this per pose. **Coll**: same
  shape as Start, with `ftFx_MF_SpecialSDash_Coll` and an explicit
  `cmd_vars[2] = 0` reset on either conversion direction (unmodeled, ghost
  bookkeeping only). This move's `transfer_ground_air` also handles this pair.
- **`SpecialSEnd`/`SpecialAirSEnd`**: `ftFx_SpecialSEnd_Enter`/`ftFx_
  SpecialAirSEnd_Enter` -> `ftFox_SpecialSEnd_SetVars`: ground `gr_vel =
  x34 * facing`; air `self_vel = (x3C * facing, 0)`; both set
  `gravityDelay = x44` (named `FOX_ILLUSION_FALL_ACCEL` in the source --
  a misleading field name; it is assigned directly into `mv.fx.SpecialS.
  gravityDelay`, i.e. functionally the End phase's gravity delay, exactly
  as modeled here). **Anim**: ground end of poses -> Wait; air end of
  poses -> `ftCo_80096900(1, 0, true, x4C, x50)` = FallSpecial with
  `allow_interrupt = true`, mobility `air_drift_max * x4C`, landing lag
  `x50`, every jump restored (the source's `unk` argument is `true`).
  **IASA**: none. **Phys**: ground countdown then `ftCommon_
  ApplyFrictionGround(x38)` (**not** the fighter's ordinary `ground_
  friction` -- a dedicated End-ground attribute) then `ftCommon_
  ApplyGroundMovement`; air after the delay `ftCommon_Fall(x48, terminal_
  velocity)` (`x48` is named `FOX_ILLUSION_TERMINAL_VELOCITY` in the
  source but passed as `ftCommon_Fall`'s *gravity* argument, exactly like
  Start's `x30` -- another misleading field name, modeled functionally as
  `end_fall_accel`), always `ftCommon_ApplyFrictionAir(x40)`. **Coll**:
  this phase's conversions are **not** symmetric with Start/Dash, and this
  is the one place this batch's own design note (superseded by this
  document) got it wrong:
  - Ground `ft_800827A0` (mode-2 clamp, `edge::mode_for_action`) false ->
    **`ftCo_Fall_Enter`** (ordinary Fall), not SpecialAirSEnd. This
    port's existing generic ground-to-air fallback (`collision::land`'s
    sibling site: `!specials::transfer_ground_air(f, false) && ... {
    simulation::enter(f, Action::Fall) }`) already produces this for
    free, since `SpecialSEnd` deliberately has no arm in this move's own
    `transfer_ground_air`.
  - Air `ft_CheckGroundAndLedge` true -> **`ftCo_LandingFallSpecial_
    Enter(gobj, false, x50)`** directly (`ftCo_Landing.c:103-113`: enters
    `LandingFallSpecial` with rate `(0.1 + x2EC) / x50`, `x2EC` being the
    same fighter-wide special-landing end frame `escape_air::Parameters.
    landing_animation_end` already models), **not** SpecialSEnd.
    This move's own `land` phase hook implements this, called through
    `collision::land`'s shared `specials::land` alongside (and before)
    `escape_air::land`, via the shared `specials::helpers::
    enter_landing_fall_special` (reusing `fighter::aerial::
    landing_animation_rate` -- the exact formula `ftCo_Landing.c` itself
    uses, confirmed by reading it, not assumed from the EscapeAir comment
    alone). Else `ftCliffCommon_80081298` (ledge catch).

## Ghost item (`itfoxillusion.c`, `it_3F2F.c:360-390`)

Resolved, not left open: the Fox Illusion/Falco Phantasm ghost **does
not** spawn hitboxes. `it_803F6818`'s three `Coll` callbacks
(`itFoxillusion_UnkMotion{0,1,2}_Coll`) all unconditionally `return
false`, and `itFoxIllusion_Logic14_DmgDealt` (the item's `DmgDealt`
table slot) only clears `item->xCA8` and returns `false` -- there is no
hitbox table, `SetAllHitboxes` call or damage-dealing path anywhere in
the 243-line file. The ghost is a purely visual trailing echo (`ftFx_
SpecialS_CopyGhostPosIndexed`/`ReturnFloatVarIndexed` feed its position
and X-rotation from `ghostEffectPos`/`blendFrames`, a 4-deep ring buffer
`ftFox_SpecialS_SetPhys` shifts every Dash-phase physics frame). It, its
ring buffer, `cmd_vars[2]`'s creation bookkeeping and `Ft_MF_SkipRumble`
stay entirely unmodeled.

A later batch (script-driven Blaster timing) confirmed the ghost's own
spawn frame directly, from the exporter's newly-decoded script trace:
`CreateGhostItem` (`ftfoxspecials.c:61-64, 247-266`) spawns it when the
Dash subaction's own `SetCmdVar` sets `cmd_vars[2] == 1`, at frame 2 for
both ground and air. `SideSpecial::script: Option<Box<SideScript>>`
(`SideScript { dash: characters::fox::side::ScriptPhase }`, the same
`ScriptPhase`/`ScriptFrames` shape `neutral::NeutralScript` uses) carries
this per-fighter, validated against `dash.ground`/`dash.air`'s own pose
counts -- recorded for citation/testing completeness only, since the
ghost stays confirmed hitbox-free and unmodeled above: no gameplay logic
in `side.rs` reads it.

## Resource and state shape

- `Rules.specials: Option<characters::fox::side::Rules { side_stick_threshold
  (x218), turn_threshold (x220), vertical_threshold (x21C) }>` -- shared
  match rules, paired with each fighter's own resource, like `rules.dash`/
  `rules.tilt`.
- `FighterData.specials: Option<characters::Specials>`, an enum tagged by
  character; Fox's variant is `Specials::Fox { neutral: Option<specials::
  neutral::Parameters>, side: Option<characters::fox::side::SideSpecial {
  ground_speed_retention (co_attrs.specials_ground_speed_retention),
  start: { ground: Attack, air: Attack }, dash: { ground: Attack,
  ground_trans_n: Vec<Option<f32>>, air: Attack, air_trans_n: Vec<[f32;
  2]> }, end: { ground: Attack, air: Attack }, attributes: {
  gravity_delay, entry_speed_div, start_air_friction, start_fall_accel,
  ground_end_speed, end_ground_friction, air_end_speed, end_air_friction,
  end_gravity_delay, end_fall_accel, freefall_mobility, landing_lag },
  script: Option<Box<SideScript>> }> }` (`script`, added later -- see
  "Ghost item" above).
  `Attack`/`AttackFrame` is the same sampled pose-plus-hitboxes shape jabs
  and aerials use; every phase here supplies an empty hitbox list per
  frame (Start and End have none in the source, and the Dash's own hit is
  the ghost's, which is unmodeled). Landing shares `escape_air::
  Parameters.landing_animation_end`; supplying the `side` entry without
  `escape_air` is rejected.
- `Fighter.fox_side_special: characters::fox::side::State { gravity_delay:
  f32 }` -- the only persistent per-fighter state needed (`mv.fx.SpecialS.
  gravityDelay`; every other `mv.fx.SpecialS` field is ghost bookkeeping
  and stays unmodeled). Reset by `simulation::enter` like every other
  per-move state, and explicitly preserved by `specials::transfer_ground_
  air` around a mid-Start conversion.
- `aerial::State` gained `mobility: f32` (default `1.0`): `mv.co.
  fallspecial.mobility` is `air_drift_max * mobility`; every previously
  modeled FallSpecial entry point in this codebase already passes the
  source's literal `mobility == 1`, so the default recovers their exact
  prior behavior unchanged, and only this batch's own End-air exit sets
  it explicitly (`x4C`). `move_fighter`'s default air-drift branch now
  scales by it whenever `Action::FallSpecial` is current
  (`fighter::movement::Movement::drift_air_scaled`).
- `locomotion::State` gained `side_special_b_age: u8` (`x688`, default
  255), updated in `simulation::sample_input_history` alongside the
  existing `attack_b_age` (x67D).
- `edge::mode_for_action` gained `SpecialSEnd => Clamp` (`ft_800827A0`);
  `SpecialSStart`/`SpecialS` are unlisted and already fall to the default
  `Plain` (`ft_80082708` is plain, matching the source).
- `ledge::catchable` gained `SpecialAirSStart | SpecialAirS |
  SpecialAirSEnd`, reusing the existing per-frame ledge scan unchanged
  (each phase's own `Coll` calls `ft_CheckGroundAndLedge` then
  `ftCliffCommon_80081298`, exactly like the aerial attacks already do).
- Slippi ids 347..352 (`ftFx_MS_SpecialSStart = ftCo_MS_Count + 6`,
  confirmed against `ftFox/forward.h`'s own enum order) are mapped in
  `crates/skirmish-replay/src/observation.rs::action_state`, gated on
  `character == Some(2)` only -- the same precedent the neutral-special
  shell's own 341/344 mapping already set (Falco 22 shares `ftfoxspecials.
  c` with its own attributes, but this port does not gate the observation
  layer for it, matching that existing precedent rather than expanding
  scope). **The matching `animation_index` entries (301..306) are an
  unverified extrapolation**: the pinned C decomp contains no figatree/
  animation-index table for Fox's character-specific motion states (only
  DAT-resource data, owned by the separate `skirmish-assets` project,
  would confirm it). The extrapolation assumes the neutral shell's own two
  confirmed data points (341 -> 295, 344 -> 298, both offset by -46)
  continue as a constant -46 offset across the six new states; this is a
  reasonable inference from the same per-character contiguous block, not
  a confirmed fact, and is flagged in code and here for a follow-up audit
  once asset-exported data is available.

## Tests

`src/fighter/characters/fox.rs` unit-tests `has_input`'s press-and-
threshold conjunction, `should_turn`'s strict comparison and `entry_
ground_velocity`'s blend arithmetic.

`tests/game_fox_side_special.rs` covers: grounded entry from Wait with the
ground-speed-retention blend and the `x28` division, the gravity delay
(including its own same-frame ground tick), jumps left untouched; aerial
entry with the `x28` division and every jump restored; the strict turn-
around boundary; the `x688` age gate rejecting a stale B held from before
the stick moved past the threshold; a vertical stick suppressing the
aerial side branch (the unmodeled SpecialAirHi/Lw priority); the ground
and air TransN-driven dash, including the no-TransN-pose friction
fallback; B-press shortening into the End phase on the ground and in the
air; a ground<->air conversion mid-Start preserving both the frame and
the gravity delay; the End phase's fixed speeds and dedicated frictions;
the FallSpecial exit with the scaled freefall mobility, `allow_interrupt`
and every jump restored; the Slippi 347..352 ids and their (extrapolated)
animation indices; a full-phase checkpoint round trip; invalid resources
(an out-of-range retention, a mismatched TransN length, a zero speed
divisor, a missing shared landing resource); and B+side staying inert
without the resource. Exact per-frame numeric expectations (friction
results, gravity-delay countdowns, TransN outputs) were captured
empirically against this module's own implementation of the cited source
functions, cross-checked against the pipeline's existing generic-
increment timing (confirmed directly against a debug trace, not assumed)
-- not against the pinned C oracle bit-for-bit.

## Known gaps and deviations from the batch's own design note

- **`doEnter`'s friction multiplier is not `1.0`** in the source (`ft_
  GetGroundFrictionMultiplier`, a per-floor-material lookup this codebase
  does not model); this port fixes it at the ordinary-terrain value.
  Flagged above and in `fighter::characters::fox::entry_ground_velocity`.
- **The End phase's ground/air conversions are not symmetric** with
  Start/Dash (ground leaving the floor enters ordinary Fall; air landing
  enters `LandingFallSpecial` directly, bypassing SpecialSEnd/SpecialAirSEnd
  entirely) -- the design note this document replaces assumed a uniform
  GroundToAir/AirToGround pattern across all three phases and was wrong
  about End; corrected here with the exact source citations.
- **The Start/End grounded phases tick `gravityDelay` down every frame**
  even though gravity is never read on the ground; an earlier draft of
  this implementation only ticked it in the air, which would have desynced
  the counter across a mid-Start ground<->air conversion. Fixed
  (this move's `tick_ground_timers`, via the shared `specials::helpers::
  tick_ground_delay`) and covered by the
  ground<->air conversion test above.
- The ghost item, its ring buffer, `cmd_vars[2]`, rumble suppression
  (`Ft_MF_SkipRumble`) and every GFX/effect call are unmodeled (see
  "Ghost item" above; confirmed hitbox-free, not merely deferred).
- **Animation indices 301..306 are extrapolated, not confirmed** (see
  "Resource and state shape" above). This is the one open question the
  C oracle cannot resolve either: the pinned decomp has no figatree table
  for character-specific motion states, only DAT-resource data owned by
  the separate `skirmish-assets` project.

## Oracle

`tests/oracle/original/fox_specials.c` snapshots `ftfoxspecials.c`;
`fox_specials.functions.json` selects 42 of its 43 functions verbatim
(every non-static callback plus all four `static inline` helpers -- the
source has four, not three -- except `ftFx_SpecialS_CreateGFX`, hand-
written as a no-op instead: its real body needs the full `ftParts`/joint
system, and it is never invoked by anything this oracle calls, only
installed as `accessory4_cb`). `tests/oracle/original/special_s.c`/
`special_air.c` snapshot `ftCo_SpecialS.c`/`ftCo_SpecialAir.c` in full,
with `ftData_SpecialS[]`/`ftData_SpecialAirHi/Lw/S/N[]` stubbed to
logging functions at Fox's own table index, recording which one fired.

`tests/oracle/fox_specials.c` treats dependencies as briefed: `Fighter_
ChangeMotionState`/`ftCo_80096900`/`ftCo_LandingFallSpecial_Enter`/
`ftCo_Fall_Enter`/`ft_8008A2BC` (Wait re-entry, an undocumented fifth
capture the End-ground Anim path needed) are captured; `ftAnim_
IsFramesRemaining`/`ft_80082708`/`ft_800827A0`/`ft_CheckGroundAndLedge`/
`ftCliffCommon_80081298`/`ft_GetGroundFrictionMultiplier` are scripted;
`ftCommon_Fall`/`ApplyFrictionAir`/`ApplyFrictionGround`/`8007D60C`/
`8007D6A4`/`8007D7FC`/`AirToGroundStateChange`/`UseAllJumps`/`ft_
80084F3C`/`ft_800850E0`/`ft_80085088`/`ft_80085134` are faithful verbatim
reimplementations against this file's own `Fighter` struct, deliberately
*not* linked against `tests/oracle/physics.c`/`locomotion.c`'s own
extractions of the same source functions -- nothing in this oracle build
guarantees two independently hand-written adapter structs share field
offsets, so duplicating the bodies against one owned struct is the safe
way to get faithful arithmetic without an unchecked cross-adapter ABI
assumption. `ftCommon_ApplyGroundMovement` is a documented no-op (its real
effect is position-only, through the full map-collision system this
oracle does not model); every ghost/GFX call is a no-op.

`tests/fox_side_special_differential.rs` compares, over 512 NaN-safe
proptest cases plus boundaries, in both debug and release: the common
grounded/aerial entry dispatch (`ftCo_SpecialS_HasInput`/`CheckInput`/
`doEnter`, `ftCo_SpecialAir_CheckInput`'s four-way table priority) against
`fighter::characters::fox`'s pure functions; every phase's Enter/Phys
speed and gravity-delay arithmetic against `fighter::movement::Movement`'s
existing `friction_ground`/`friction_air`/`fall`; the Dash phase's TransN
velocities; the B-press IASA shortening; and the Start/Dash/End Coll
conversion/exit decisions. `entry_multiplier_gap_is_the_documented_one`
exercises the `doEnter` friction-multiplier gap directly rather than
excluding it: agreement at the ordinary-terrain value 1.0, and a
deliberate, asserted divergence away from it.

## Replays

`crates/cli/tests/replay_match.rs` covers a grounded Fox Illusion (a
physical B press from Wait through SpecialSStart/SpecialS/SpecialSEnd,
Slippi 347/348/349) and an aerial one that free-falls all the way into
`LandingFallSpecial` (Slippi 43, an already-verified mapping) each
matching their own generated Peppi bytes, then reporting a `Mismatch` at
frame 0 (`checked_frames: 0`) once the entry press is removed -- the
earliest row the removed press's effect can appear, since without it the
fighter never leaves Wait/Fall at all.
