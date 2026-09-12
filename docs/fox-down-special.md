# Fox/Falco down special (Reflector) — design note

Pinned decomp rev 0bac93a5. Sources: `src/melee/ft/kinds/ftFox/
ftfoxspeciallw.c` (SetVars/Enter 76-116, Start 125-256, Loop 257-448, Turn
450-660, Hit 662-860, End 860-981), `ftFox/types.h:141-148` (attributes:
`x98` release lag, `x9C` turn frames, `xA0` unknown, `xA4` gravity delay,
`xA8` air momentum divisor, `xAC` fall accel, `xB0` `ReflectDesc`),
`ftFox/forward.h:71-80` (motion ids: SpecialLwStart 360, SpecialLwLoop
361, SpecialLwHit 362, SpecialLwEnd 363, SpecialLwTurn 364,
SpecialAirLwStart 365, SpecialAirLwLoop 366, SpecialAirLwHit 367,
SpecialAirLwEnd 368, SpecialAirLwTurn 369), `lb/types.h:115-126`
(`ReflectDesc`), `ftcoll.c:3189-3210` (`ftColl_CreateReflectHit`: sets
`fp->reflecting`, the reflect attributes and the reflect bubble on the
bone), helpers to read before implementing: `ftCo_800C97A8` (the turn
check in `ftfoxspeciallw.c:641-660`'s caller: a reversed stick past a
common threshold), `ftCo_80099F1C` (the platform-drop predicate already
pinned by the shield-drop batch), `ftCo_8009A184` (`ftCo_Pass.c:76+`: the
pass transition keeping the frame), `ft_80081D0C` (air-to-ground check
without the ledge), `ftCommon_8007D92C` (the End exit: Wait or Fall),
`ftCommon_GroundToAirStateChange` (`ftCommon/inlines.h`), and
`ftColl_8007AEF8` (per-frame reflect collision).

## Input
Grounded: the common down-special dispatch (`ftCo_800D68C0`,
`ftCo_Attack100.c`: `x687 == 0` = fresh B with `stick.y < -x21C`,
`ftCo_800D688C`; the FOURTH check of the grounded chains after SpecialS,
SpecialHi and SpecialN) -> `ftFx_SpecialLw_Enter`. Aerial: the down branch
of `ftCo_SpecialAir_CheckInput` -> `ftFx_SpecialAirLw_Enter`.

## Phases
- **Start** (`SpecialLw_Enter`: SpecialLwStart (0, 0, 1), script;
  SetVars: `releaseLag = x98`, `isRelease = 0`, `cmd_vars[1] = 4`,
  `gravityDelay = xA4`. Air entry: `self_vel.y = 0`, `self_vel.x /= xA8`).
  Anim (both): `isRelease |= !B held`; at the end -> Loop (ground/air by
  `ground_or_air`), effect reset. IASA ground: the platform drop
  (`ftCo_80099F1C` -> `ftCo_8009A184(SpecialAirLwStart, flags, frame)` +
  reflect hit); air: none. Phys ground `ft_80084F3C`; air: gravity delay
  then `ftCommon_Fall(xAC, terminal)`, then `ftCommon_8007CF58` (drift
  clamp). Coll ground `ft_80082708` -> air Start at the frame
  (`ftCommon_GroundToAirStateChange`); air `ft_80081D0C` -> ground Start
  at the frame + `ClampAirDrift`.
- **Loop** (`Ft_MF_KeepGfx`, reflect hit created on entry): Anim:
  `isRelease |= !B held`; `releaseLag--` while > 0; when `releaseLag <= 0
  && isRelease` -> End (ground/air). IASA ground: `SpecialLwTurn_Check`
  (reversed stick -> Turn), else `ftCo_Jump_CheckInput` (jump cancel!),
  else the platform drop; air: `SpecialLwTurn_Check` else
  `ftCo_800CB870` (aerial jump: the double-jump cancel). Phys ground
  `ft_80084F3C` + `ftColl_8007AEF8`; air: the inline air physics + reflect
  collision. Coll: conversions at the frame, re-creating the reflect hit.
- **Turn** (`Ft_MF_KeepGfx`; SetVarAll: `reflecting`, `turnFrames = x9C`,
  `cmd_vars[0] = 0`, then one `Turn` step): Anim: `isRelease |= !B held`,
  `releaseLag--`, turn step (`turnFrames--`; when `cmd_vars[0] == 0 &&
  turnFrames <= x9C` flip facing and set the flag: on the FIRST step, so
  the facing flips immediately; the model rotation is visual); when
  `turnFrames <= 0` -> `SpecialLwHit_Check`: with `releaseLag <= 0 &&
  isRelease` -> End, else -> Loop with a new reflect hit. IASA none.
  Phys/Coll as Loop.
- **Hit** (entered by the reflect callback when a projectile is reflected:
  `facing = reflectHitDirection`, Hit state): Anim: release bookkeeping;
  at the end `SpecialLwHit_Check` (End or Loop). Reflection of projectiles
  needs items, which Skirmish does not have: model the state machine and
  the `reflecting` flag (Slippi's reflect bit, already reported for the
  powershield) and leave the Hit phase reachable only through a test hook
  or unmodeled (say which).
- **End**: Anim end -> `ftCommon_8007D92C` (Wait/Fall); Phys ground
  `ft_80084F3C`, air gravity delay + `ftCommon_Fall(xAC)` + drift clamp;
  Coll conversions.
- Falco shares the code with its own attributes.

## Resource shape
`fighter.down_special: Option<fox::DownSpecial { start/loop/turn/hit/end
pose sets (ground/air; the loop's script owns the reflect timing only),
attributes: release_lag (x98), turn_frames (x9C), gravity_delay (xA4),
air_momentum_div (xA8), fall_accel (xAC), reflect: { bone, max_damage,
offset, size, damage_mul, speed_mul, behavior } }`; Slippi 360..369 with
the character-table animation indices; the reflect bubble geometry is
reported only through the `reflecting` bit.

## Oracle
Pin `ftfoxspeciallw.c` with scripted B-held sequences, turn-check answers
and collision answers; compare the release/turn/end state machine and the
air physics bit-exactly per frame against a Rust mirror.

## Tests
Hold B: Loop persists; release before `x98` frames: End only after the
lag; jump cancel from the ground loop and aerial jump from the air loop;
turn on a reversed stick with the immediate facing flip and the `x9C`
countdown back to Loop; platform drop from Start/Loop; conversions; the
`reflecting` bit; checkpoints; invalid resources; Slippi ids.

## Verified callbacks (per-phase, read on 2026-09-11)
- End: Anim end -> `ftCommon_8007DB24` then `ftCommon_8007D92C`
  (`ftcommon.c:596-604`: airborne -> `ftCo_Fall_Enter`, grounded ->
  `ft_8008A2BC` Wait). Phys ground `ft_80084F3C`; air gravity delay ->
  `ftCommon_Fall(xAC)` then `ftCommon_8007CF58`. Coll ground `ft_80082708`
  false -> `ftCommon_GroundToAirStateChange(SpecialAirLwEnd)`; air
  `ft_80081D0C` -> `ftCommon_AirToGroundStateChange(SpecialLwEnd)` +
  `ClampAirDrift`. Entries: plain ChangeMotionState (0, 0, 1).
- Turn predicate `ftCo_800C97A8` (`ftCo_Turn.c:28-36`): `stick.x * facing
  <= x34` (the standing-turn threshold, already `turn_threshold` in the
  locomotion parameters).
- Every air phase's landing uses `ft_80081D0C` (no ledge catch), unlike the
  side special's `ft_CheckGroundAndLedge`: cite its body.

## Implementation (2026-09-11 batch)

Implemented as `src/game/characters/fox/down.rs`: the five-phase Start/
Loop/Turn/Hit/End state machine, the ground-fourth/air-directional fresh
entry, the mid-move turn (`should_turn_mid_move` in `fighter::characters::
fox`, reusing `locomotion::Parameters::turn_threshold`, not the side
special's own `x220`), the ground jump cancel and aerial jump (reusing
`locomotion::jump_input`/`try_aerial_jump`), the platform drop (a new
`collision::begin_pass_as`, generalizing the existing `begin_pass` to a
caller-supplied destination and frame-preserving entry, called from
`simulation::advance` alongside the generic `locomotion::
pass_request_after_actions` since both need the geometry-aware
`on_platform` query `SpecialMove::update_actions` does not receive), every
phase's ground/air conversion (`transfer_ground_air`), and the `reflecting`
bit (piggybacked on the existing `fighter.shield.reflecting` field, since
the source itself stores both the powershield's and this move's reflect
state in the same single `fp->reflecting` bit).

**A decomp fact that contradicted the side-special-derived assumption**:
none of this move's five *grounded* Phys callbacks touch `gravityDelay`
(only the air ones do), unlike the side special where the ground Start/End
Phys explicitly tick it down even though gravity is never applied on the
ground. Confirmed by reading all five ground Phys bodies in the pinned
source. `down.rs` therefore has no `tick_ground_timers` override at all.

**Anim vs IASA, and the "just entered" ambiguity**: the trait's
`update_animation` hook only fires clip-length-driven transitions with no
input (Start's Loop entry, End's Wait/Fall exit); everything else --
isRelease/releaseLag bookkeeping, Loop's exit and mid-move IASA, Turn's
countdown, Hit's clip-end-through-`hit_check` -- lives in `update_actions`,
since only that hook receives `input`. Because `update_animation` runs
before `update_actions` within the same engine frame, a phase entered by
an `update_animation` transition this frame reaches `update_actions` with
the *new* action already current; matching the source's own same-frame
Anim-then-IASA-then-Phys-then-Coll cascade (already the established
pattern from the side special's Start-to-Dash transition) requires
suppressing a freshly-entered phase's own per-frame "Anim" bookkeeping on
its first `update_actions` call (`fighter.action_frame != 0` guards the
Loop and Hit arms) while still running IASA-style checks (Loop's turn/
jump-cancel) unconditionally, since IASA genuinely does cascade same-frame
after a transition in the source. Turn's own first step is not gated this
way: it is a *synchronous* call within `ftFox_SpecialLwTurn_SetVarAll`
itself, not a subsequent Anim callback, so `enter_turn` performs it
directly as part of entry.

**Engine limitation surfaced by Loop's indefinite duration**: unlike every
other modeled action, Loop's `Ft_MF_KeepGfx` animation has no fixed length
(the player can hold B indefinitely), but the engine's generic per-frame
`action_frame` increment has no wrap/hold mechanism for a resource with a
finite `frames` array, and out-of-bounds access is a hard `Error::Physics`.
Fixed by extending the existing `locomotion::hold_action_frame` (already
used by RunTurn/RunBrake's own wait states) with a `down_special.looping`
flag, set once `action_frame` reaches Loop's last sample and preserved
across ground/air conversions like every other whole-move field.

**`ftCommon_8007CF58` (the common aerial drift function every air phase's
Phys calls, cross-file, not itself part of `ftfoxspeciallw.c`)** is fully
modeled: `game::specials::helpers::drift_or_friction_air` (used through
`gravity_delayed_fall_with_drift`, called by every Reflector air phase)
reproduces both branches verbatim -- the ordinary under-`air_drift_max`
friction toward zero, and the over-maximum branch that decelerates
`self_vel.x` back toward the maximum using the common over-drift step
(`ftCommonData.x1FC`, `ftcommon.c:283-306`), with the source's own sign
handling. This branch is genuinely reachable: Fox's `air_drift_max` sits
well below his run speed, so a jump out of a run, or `Fall` from the side
special's own air End (`self_vel.x = x3C * facing`), can carry an aerial
Reflector's entry velocity above the maximum even after the `xA8`
division. `x1FC` is exposed as `characters::fox::side::Rules::
air_drift_recovery_step` (the same shared `ftCommonData` struct as
`x218`/`x21C`/`x220`), read by the down special even though the side
special's own air phases never touch it (their fixed custom friction
attributes bypass `ftCommon_8007CF58` entirely). A new small adapter,
`tests/oracle/air_drift_recovery.c` (aliased to "ftcommon"), pins the real
`ftCommon_8007CF58` by extracting it verbatim from the pinned `ftcommon.c`
snapshot -- not a stub -- with differential cases exercising both
branches; `ftfoxspeciallw.c`'s own adapter cannot link against that
extraction directly (different, minimal struct layouts; see its own
header), so it duplicates the identical both-branch body, verified
faithful by the standalone adapter's own tests.

**Animation indices for 360..369**: the motion (Slippi action-state) ids
are exact, not extrapolated (`ftFx_MS_SpecialLwStart = ftCo_MS_Count + 6 +
4`, directly following the side special's own five states in `forward.h`'s
declaration order). Their animation indices reuse the same unverified -46
state-id offset the side special's own two confirmed data points
(341->295, 344->298) established -- still not a confirmed figatree table.
`forward.h`'s `ftFx_SM_*` list has no entry of its own for
Turn/SpecialAirLwTurn (only Start/Loop/Hit/End do); Turn's animation index
reuses Loop's, consistent with Turn being a brief interruption of the Loop
cycle rather than a separate asset.

**Unmodeled / reachable only through tests**:
- The Hit phase's *entry* (`ftFx_SpecialLwHit_Enter`, fired only by the
  projectile-reflect callback `fp->reflect_hit_cb`) is unreachable in
  play -- Skirmish has no projectiles. Its own per-frame bookkeeping and
  clip-end exit (shared `hit_check` with Turn's countdown exit) are
  implemented and exercised by the C-oracle differential tests
  (`oracle_down_anim` phase 3, `oracle_down_hit_check`) and confirmed wired
  into the observation layer by a native test; nothing in `down.rs` ever
  transitions a fighter into `Action::SpecialLwHit`/`SpecialAirLwHit`.
- The reflect bubble's geometry (`Reflect`'s bone/offset/size/damage_mul/
  speed_mul/behavior) has no effect in this engine; only the `reflecting`
  bit is observable. Kept in the resource shape for completeness/
  validation per this note's own original brief.
- `xA0_FOX_REFLECTOR_UNK1` is never read by any function this batch cites
  and is not modeled at all (no resource field).
- `ftColl_8007AEF8` (the per-frame reflect-vs-projectile collision check,
  called from every Loop/Turn/Hit Phys callback) is an unconditional no-op:
  there is nothing to collide against without projectiles.

## Hitboxes (script-embedded, Start's own hit)

Like the up special's Hold/Travel hitboxes, Reflector's own hit belongs to
the animation script (the `ftaction.c` hitbox opcode this codebase already
processes generically, `game::hitboxes::update_tracks`), not to a
`SetAllHitboxes`-style call inside `ftfoxspeciallw.c` itself -- reading the
whole pinned file confirms no such call exists there either, exactly like
the up special. The schedule comes from the same gameplay export pack:
`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911/fox-fd/
match-data.json`, `fighters[0].specials.down.start.{ground,air}`: a single
hitbox (bone 3, centered on the bone, no offset, radius `7.9995`, damage 5,
angle 0 degrees, growth 100, base/fixed/shield damage 0, clank `true`,
rebound `false`) active on frames 0 and 1 of the 5-pose Start set, clear on
frames 2 through 4. `move_id` 21. Loop/Turn/Hit/End have none in the pack
(this move's real damage is entirely Start's own reflector spin-up hit).

No code change was needed in `down.rs` for this, for the identical reason
`up.rs` needed none: `attack()` already returns `&parameters.start.*` for
`Action::SpecialLwStart`/`SpecialAirLwStart`, and the shared dispatch chain
(`FighterData::attack` -> `specials::attack` -> this move's own `attack()`)
already routes into the ordinary hit-scan/damage pipeline. Validation
gained the same `specials::helpers::validate_hitboxes` call `up::validate`
now also uses (see `docs/fox-up-special.md`'s own "Hitboxes" section for
what it checks, including the `rules.clank` gate and the `move_id`-under-
staling requirement the 2026-09-12 hit-record-refresh batch wired up for
both moves; `docs/validation.md`). Neither `ftfoxspeciallw.c` nor anything
it calls touches a hitlag field, so the attacker's own hitlag on a
connecting hit is the ordinary, already-generic engine rule -- no
special-casing needed or added.

**Native test**
(`tests/game_fox_down_special.rs::reflector_start_hits_a_nearby_opponent_on_
the_pack_documented_frames`) builds the pack's schedule locally (like the up
special's own regressions, not edited into the shared `fixtures/game/
fox-down-special.json`, whose short poses back every other test's own
step counts -- `"Start is 4 frames"`) and places a stationary second fighter
in reach. The hit connects on Start's own entry frame (frame 0 is already
sampled the instant `Action::SpecialLwStart`/`SpecialAirLwStart` is
entered, the identical same-frame cascade `docs/fox-up-special.md`'s Travel
regression documents) and deals 5 damage as the pack reports; frame 1's own
copy of the identical hitbox does not independently connect a second time,
since it never leaves `frame.hitboxes` for even one frame -- the shared
per-attacker `hit_groups` bitmask has nothing to refresh
(`hitboxes::refreshed_groups`, `docs/validation.md`'s 2026-09-12 entry) and
stays set from frame 0's own connect, exactly like Travel's own continuous
hit. No C-oracle differential was added: hit resolution arithmetic is
already oracle-pinned and generic over `Hitbox` fields, and this batch
supplies new data through that existing pipeline, not new arithmetic.

## C-oracle coverage

`tests/oracle/original/ftfoxspeciallw.c` pins the whole file (sha256 in
`sources.json`); `tests/oracle/ftfoxspeciallw.functions.json` extracts all
72 of its own function definitions (every non-static callback plus every
`static`/`static inline` helper -- effectively the whole file minus its
three GFX-only `Create*GFX` accessory callbacks, hand-written no-ops
instead since nothing this oracle calls ever invokes them).
`tests/fox_down_special_differential.rs` compares, bit-exactly against
inline Rust formulas (the same style the side special's own differential
suite established): Enter's velocity math and initial release/gravity
state; every phase's air Phys arithmetic, including `ftCommon_8007CF58`'s
both branches (proptest, 256 cases each across a ground pass, a bounded-
`self_vel.x` air pass exercising the over-drift branch at a realistic
rate, and a full-`f32`-range air pass, plus an explicit boundary-crossing
test on both signs); the standalone `air_drift_recovery` adapter's own
extraction of the same function (256 cases bounded and full-range);
Start's release-only Anim bookkeeping; Loop's
release-lag countdown and End exit; Turn's flip-once-on-the-first-step
step and its `turnFrames<=0` exit through `hit_check`; Hit's clip-end
exit through the same `hit_check`; End's Wait/Fall dispatch; Loop's IASA
short-circuit order (turn-check before jump-cancel/aerial-jump, with call
counters proving the RETURN_IF-style short circuit, not just its net
effect); `hit_check`'s own End-vs-Loop decision across all four combinations
of release-lag/isRelease; the platform drop's reflect-hit side effect; and
every phase's ground/air conversion, including which ones re-create the
real reflect hit (`ftColl_CreateReflectHit`, Loop only) versus which just
set the flag directly (`ftFox_SpecialLw_SetReflectVars`/`ftFx_
SpecialLwHit_SetCall`, Turn/Hit). These pin the extracted callback graph's
own behaviour, not a full `Match`; `tests/game_fox_down_special.rs` and the
two self-recorded replay regressions in `crates/cli/tests/replay_match.rs`
(ground and air, Start through Loop through End) separately exercise the
Rust engine's own mirror end to end -- regression evidence against this
codebase's own prior behaviour, not independent Melee parity evidence.
