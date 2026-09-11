# Fox/Falco up special (Fire Fox / Fire Bird) — design note

Pinned decomp rev 0bac93a5. Sources: `src/melee/ft/kinds/ftFox/
ftfoxspecialhi.c` (entries 68-102, hold 110-214, travel 215-401,
conversions 402-482, launch 483-537, landing/fall 538-665, bound 666-772),
`ftFox/types.h:109-132` (attributes), `ftFox/forward.h:64-70` (motion ids
SpecialHiHold 353, SpecialHiHoldAir 354, SpecialHi 355, SpecialAirHi 356,
SpecialHiLanding 357, SpecialHiFall 358, SpecialHiBound 359), helpers:
`ft/inlines.h:123-130` (`stickGetDir(x, 0)` = |x|), `ftCo_Pass.c:64-74`
(`ftCo_8009A134`: on a platform -> skip that floor and return true),
`lbvector.c:105+` (`lbVector_AngleXY`: angle between two XY vectors via
the clamped cosine), `ftcommon.c:283+` (`ftCommon_8007CF58`: air drift
clamp toward `air_drift_max` by `x1FC`), `ftcommon.c:650-655`
(`ftCommon_8007DB24`: clears the effect flag), `ftcommon.c:1422-1427`
(`ftCommon_8007F76C` as `x21F8`: forces `gr_vel`/`self_vel.x` to point
along the facing), `ft_084E.c:138-142` (`ft_800851C0`: `self_vel.y =
transN.y`), `forward.h:251` (`CLIFFCATCH_BOTH = 0`).

## Input
Grounded: the common up-special dispatch (`ftCo_Attack100_CheckInput`,
`ftCo_Attack100.c:~95-105`: `x686 == 0` = fresh B with `stick.y >= x21C`
this frame, per `fighter.c`'s age byte; it is the SECOND check of the
grounded chains after SpecialS) -> `ftFx_SpecialHi_Enter`. Aerial:
`ftCo_SpecialAir_CheckInput`'s up branch -> `ftFx_SpecialAirHiStart_Enter`.

## Phases
- **Hold** (`SpecialHi_Enter` ground: `gravityDelay = x54`, `gr_vel /=
  x58`, ChangeMotionState(SpecialHiHold, 0, 0, 1), script; air
  `SpecialAirHiStart_Enter`: `self_vel.x /= x58`, `self_vel.y = 0`,
  SpecialHiHoldAir). IASA none. Phys ground `ft_80084F3C` (ordinary
  friction); air: countdown then `ftCommon_Fall(x60, terminal)`, always
  `ApplyFrictionAir(x5C)`. Coll ground `ft_80082708` false -> HoldAir at
  the frame (`ftCommon_8007D60C`); air `ft_CheckGroundAndLedge(facing)`:
  landing -> Hold at the frame + `ftCommon_ClampAirDrift`; ledge catch
  possible. Anim end (both): airborne -> `SpecialAirHi_Enter`, grounded ->
  `SpecialAirHi_AirToGround` (the grounded launch).
- **Launch** (`SpecialAirHi_Enter`, 483-537): with `|stick.x| + |stick.y|
  >= x64` the direction is `atan2f(stick.y, stick.x * facing)` after
  turning around when `|stick.x| > x88`; else straight up (pi/2).
  SpecialAirHi (0, 0, 1); `x2223_b4 = 1`; `travelFrames = x68`;
  `unk = unk2 = 0`; `self_vel = (facing * x74 * cos, x74 * sin)`;
  `x21F8 = ftCommon_8007F76C`; jumps used = max. Grounded launch
  (`SpecialAirHi_AirToGround`, 418-482): with the same stick magnitude
  test, if the stick vector's angle to the floor normal is not below pi/2
  (pointing into the floor) and not on a platform (`ftCo_8009A134` false):
  turn to the stick, SpecialHi (0, 0, 1), `travelFrames = x68`, `gr_vel =
  x74 * facing`, model rotation from the floor normal; otherwise leave the
  ground (`ftCommon_8007D60C`) and take the aerial launch.
- **Travel** (SpecialHi / SpecialAirHi): Anim: `travelFrames--`; at <= 0
  airborne -> SpecialHiFall (`SpecialHiLanding_GroundToAir`: effect clear,
  Fall state, `x21F8`), grounded -> SpecialHiLanding
  (`SpecialHiFall_AirToGround`). IASA none. Phys ground: `unk++`; at
  `unk >= x70` `ApplyFrictionGround(x78)`; ground movement. Air: `unk++`;
  at `unk >= x70` `self_vel.x += facing * x78 * cos(dir) - self_vel.x`...
  precisely `self_vel.x = -((facing * (x78 * cos(rot))) - self_vel.x)` and
  `self_vel.y = -((x78 * sin(rot)) - self_vel.y)` (an acceleration of x78
  along the direction subtracted from the velocity: reproduce the exact
  expression). Coll ground: `unk2++`; `ft_80082708` false -> SpecialAirHi
  at the frame (`Ft_MF_SkipHit`); with a floor contact rotate the model to
  the floor normal (visual). Coll air: `ft_CheckGroundAndLedge(BOTH)`;
  when it reports ground/ledge and `IsBound` (`unk2 >= x6C`, or not on a
  platform): with no floor contact or an approach angle to the floor
  normal not below `90 + x94` degrees -> SpecialHiBound; else redirect
  (`facingDir:` block: facing from `self_vel.x` sign, direction from the
  velocity). Without ground: ledge catch (`ftCliffCommon_80081298`) else
  ceiling/left/right wall contacts with an angle below `90 + x94` degrees
  -> the same redirect; otherwise nothing. (Redirect = the wall/floor
  "slide"; reproduce the branch order exactly.)
- **Landing** (SpecialHiLanding): Anim end -> Wait. Phys
  `ApplyFrictionGround(x7C)`. Coll `ft_80082708` false ->
  `ftCo_80096900(1, 0, true, x8C, x90)` (FallSpecial). Entered from the
  fall's landing at frame 13 (`SpecialHiFall_Enter`: `Ft_MF_SkipColAnim |
  Ft_MF_UpdateCmd`, start 13) or from the travel end at frame 0.
- **Fall** (SpecialHiFall): Anim end -> FallSpecial(x8C, x90). Phys
  `ft_80084DB0` (ordinary air). Coll `ft_CheckGroundAndLedge(BOTH)` ->
  Landing at frame 13; ledge catch possible.
- **Bound** (SpecialHiBound): entry `self_vel.x *= x84`, `cmd_vars[0] =
  0`, effect. Anim: with the script's `cmd_vars[0]` set while airborne ->
  FallSpecial(x8C, x90) with jumps used = max; at the end airborne ->
  the same, grounded -> Wait. Phys air: `self_vel.y = transN.y`
  (root motion) and `ftCommon_8007CF58` (drift clamp toward
  `air_drift_max` by `x1FC` when exceeding it); ground `ft_80084F3C`.
  Coll air: `ft_CheckGroundAndLedge(facing)` -> `ftCommon_8007D7FC`
  (land, stay in Bound); ledge catch; ground `ft_80084104` (clamp).
- Falco shares the code with its own attributes.

## Resource shape
`fighter.up_special: Option<fox::UpSpecial { hold: { ground, air } pose
sets, travel: { ground, air } (hitboxes), landing, fall, bound (with
per-pose `cmd_vars[0]` flag and TransN y deltas), attributes: gravity_delay
(x54), entry_speed_div (x58), hold_air_friction (x5C), hold_fall_accel
(x60), direction_stick_min (x64), duration (x68), bounce_frames (x6C),
duration_end (x70), speed (x74), reverse_accel (x78), landing_friction
(x7C), bound_speed_mul (x84), facing_stick_min (x88), freefall_mobility
(x8C), landing_lag (x90), bound_angle_degrees (x94) } }`; common
`specials.vertical_threshold` (x21C, shared with the side special batch)
and `x1FC` (air drift clamp accel, check whether it is already a rule).
Slippi 353..359 for Fox with the character-table animation indices.

## Implemented
`src/game/characters/fox/up.rs` covers the whole phase table above against
`Action::SpecialHiHold/SpecialHiHoldAir/SpecialHi/SpecialAirHi/
SpecialHiLanding/SpecialHiFall/SpecialHiBound`: the grounded/aerial entry
dispatch (including the `x21C` age gate on the ground and the fresh-press,
no-age-gate check in the air); Hold's charge and gravity delay (ground
Phys never ticks it, matching the pinned source exactly, unlike the side
special's own Start/End); the stick-driven launch angle
(`ftFx_SpecialAirHi_Enter`) with its two thresholds and the straight-up
default; the grounded-launch-vs-decline decision
(`ftFx_SpecialAirHi_AirToGround`), including the platform check
(`ftCo_8009A134`, reused via the `pass` oracle snapshot) and the
floor-normal-derived launch angle; Travel's duration countdown
(independent of its own looping pose) and its post-`duration_end`
ground-friction-override/air-reverse-acceleration; the landing bound
decision (`unk2 >= x6C` and `!on_platform`) entering Bound, or continuing
Travel when neither holds; Landing and Fall's own Phys/Coll, including
Fall's own ground/ledge touch entering Landing at frame 13 (see the
correction below); Bound's rebound entry, its per-pose `cmd_vars[0]`/
TransN-driven exit, and its own ground/air Phys/Coll; every `FallSpecial`
exit restoring jumps and the scaled mobility; ledge catching on
`SpecialHiHoldAir`/`SpecialAirHi`/`SpecialHiFall`/`SpecialHiBound`.

## Corrections against the pinned source
This design note's own frame-13 citation was already correct (Fall's own
ground touch, not Travel's own bound decision, enters `SpecialHiLanding`
at frame 13); the gap was in the implementation, not this note: `up.rs`
had no `land()` arm for `Action::SpecialHiFall` at all, so an ordinary
ground touch during Fall fell through to the generic `Action::Landing`
instead. Fixed in this batch (`land()` now enters `SpecialHiLanding` and
forces `action_frame` to 13 for that specific transition), with a
regression test (`tests/game_fox_up_special.rs::
fall_lands_at_frame_13_via_ordinary_ground_touch`) and the oracle's own
`oracle_fall_coll`/`compare_fall_coll`.

Three further bit-exactness bugs surfaced by the C-oracle differential
suite while building it, all fixed in this batch, none of them related to
the design above:
- `angle_xy` (`lbVector_AngleXY`) guarded its zero-length case with
  `product > 0.0`; the pinned source's own `if (lena_lenb)` is a **non**-
  zero check, not a positive one. The two agree for every ordinary finite
  product (both lengths are non-negative), but diverge for a NaN product
  (a near-zero vector paired with one large enough to overflow the other
  length to infinity, `0.0 * inf = NaN`): `!= 0` is true for NaN and
  reaches `acosf(NaN)`, propagating it, while `> 0.0` is false for NaN and
  would have silently substituted 0.0.
- `enter_from_ground_hold` (`ftFx_SpecialAirHi_AirToGround`) wrote its own
  magnitude and angle gates as `>=`; the pinned source writes both as
  negated less-than (`!(magnitude < x64)`, `!(angle < HALF_PI32)`), unlike
  `ftFx_SpecialAirHi_Enter`'s own direct `>=` for the same magnitude
  check. The two forms agree for ordinary floats but diverge exactly when
  an operand is NaN (reachable through the `angle_xy` fix above), where
  `!(NaN < x)` is true but `NaN >= x` is false.
- The aerial launch velocity (`ftFx_SpecialAirHi_Enter`) and the Travel
  air reverse-acceleration (`ftFx_SpecialAirHi_Phys`) both compute
  `facing_dir * (x74_or_x78 * cosf(rotateModel))`; this port had written
  the mathematically-equal-in-real-arithmetic-but-not-in-`f32`
  `(facing * speed) * cosf(angle)` (left-to-right evaluation), which can
  differ from the source's own grouping by an ULP since `f32`
  multiplication is not associative. Both now keep the source's exact
  grouping.

## Implemented (follow-up: redirect, graze, ground rotation, landing lag)
A first pass at this move approximated four pieces of ordinary (not
exotic-edge-case) gameplay; all four are now implemented:

- **The mid-Travel wall/ceiling redirect** (`ftFx_SpecialAirHi_Coll`'s
  non-ground branch). The collision pipeline gained a new `SpecialMove`
  hook pair: `wants_redirect(action)` (action-keyed, mirrors
  `collision_mode`/`ledge_catchable`) makes a wall/ceiling contact
  response-eligible the same way `damage::can_surface_tech`/`can_reflect`
  already do for tech/reflect, instead of the ordinary auto-zero-into-wall
  response; `air_contact(fighter, data, ceiling, wall)` then decides what
  to do with it. `up.rs` returns `wants_redirect(SpecialAirHi) == true`
  and implements `air_contact` with the source's own priority (ceiling
  over either wall, not a fallback -- if a ceiling candidate exists at
  all, only its own angle is tested) and gate
  (`lbVector_AngleXY(contact_normal, self_vel) < 90 + x94` degrees). A
  shared `redirect_from_velocity` helper (facing from `self_vel.x`'s own
  sign, `rotateModel` from `atan2f(self_vel.y, self_vel.x * facing)`)
  backs both this and the landing bound's own graze below, matching the
  source's own shared "facingDir" block. `collision::resolve` calls it
  once, before its own existing tech/reflect order loop, since only one
  move wants this today and its own angle gate needs both candidates at
  once rather than a per-candidate loop.
- **The shallow-floor-angle graze** sub-case of the landing bound decision
  (`ftFox_SpecialHi_IsBound` true, but the approach angle to the floor
  normal still under the gate: `goto facingDir` instead of entering
  Bound, staying airborne). `SpecialMove::land` gained a `pre_landing:
  &Fighter` parameter: `collision::land` clones the whole fighter before
  running its own generic landing bookkeeping (grounded, velocity,
  knockback, jumps, wall-jump state, ...), so a move that declines the
  landing outright can restore that snapshot wholesale
  (`*fighter = pre_landing.clone()`) instead of hand-reverting each field
  the generic bookkeeping touched -- `locomotion::landed` alone resets
  `jumps_used` to 0, which a graze must not get to exploit as a free jump
  refresh. `up.rs`'s own `land()` computes the angle from
  `pre_landing.velocity` (the vertical component the generic bookkeeping
  has already zeroed on `fighter` itself by this point) against
  `fighter.floor_normal` (already this frame's fresh contact, read before
  the later snapshot restore), then either enters Bound or restores the
  snapshot and applies `redirect_from_velocity` on top.
- **Ground Travel's own per-frame model rotation** from the current floor
  normal (`ftFx_SpecialHi_Coll`'s floor-contact branch, not just a
  one-time launch angle). `SpecialMove` gained `update_ground_contact`,
  called once per fighter right after `collision::resolve` finishes (so
  it reads this frame's own fresh `floor_normal`/`grounded`, unlike
  `tick_ground_timers`, ticked earlier from the physics step against last
  frame's contact). `up.rs` re-derives `rotate_model` from
  `fighter.floor_normal` every grounded `SpecialHi` frame, so a Travel run
  that leaves the ground at an edge after `duration_end` carries the last
  real grounded angle into the air phase's own reverse acceleration,
  instead of a stale launch-time value.
- **`FallSpecial`'s own `landing_lag` argument** (x90), threaded like
  `mobility` already was: `aerial::State` gained a `landing_lag:
  Option<f32>` field (`None` by default, reset by every fresh
  `simulation::enter` like `mobility`), `helpers::enter_fall_special`
  gained a `landing_lag: Option<f32>` parameter storing it, and
  `escape_air::land` reads `fighter.aerial.landing_lag.unwrap_or(rules.
  landing_lag)` (captured before `simulation::enter` resets it back to
  `None`) instead of unconditionally using the shared `escape_air::Rules`'
  own rate. Every up-special call site passes
  `Some(p.attributes.landing_lag)`; the side special's own call site
  passes `None`, preserving its already-audited behavior unchanged (its
  own `FallSpecial` exit predates this resource).

## Unmodeled
- Travel's hitboxes stay empty: the pinned `ftfoxspecialhi.c` creates none
  directly, and this batch does not extract an animation-embedded hitbox
  table.
- The visual model-rotation bone itself (`ftFox_SpecialHi_RotateModel`'s
  own `ftPartSetRotX` call): rendering only, unlike the `rotateModel`
  *state* it reads, which this batch's follow-up now keeps continuously
  up to date (see above).
- The `x21F8`/`ftCommon_8007F76C` callback: only ever invoked from an
  unrelated, unmodeled post-damage facing-turnaround routine
  (`ftCo_0C35.c`'s `ftCo_800C37A0`), so assigning it has no reachable
  effect in this profile.

## Oracle
`tests/oracle/original/ftfoxspecialhi.c` pins the whole file (sha256 in
`sources.json`); `tests/oracle/ftfoxspecialhi.functions.json` selects every
non-static callback plus the three static/static-inline helpers
(`ftFox_SpecialHi_RotateModel`, `ftFox_SpecialHi_IsBound`,
`ftFox_SpecialHiBound_SetVars`) -- only the two GFX-only `_CreateChargeGFX`/
`_CreateLaunchGFX` callbacks are dropped, matching the side special's own
precedent for its one skipped GFX helper. Two dependencies live in other
already-pinned files and are reused via `adapters.json` aliases rather than
re-snapshotted: `lbVector_AngleXY` (`up_special_angle` -> the existing
`lbvector` snapshot) and `ftCo_8009A134` (`up_special_platform` -> the
existing `pass` snapshot). `tests/oracle/fox_specialhi.c`'s own header
documents exactly which dependencies are captured, scripted or faithfully
reimplemented; `ft_80084DB0` (Fall's own "ordinary air" Phys, the shared
aerial gravity/fast-fall pipeline this move does not override) and
`ftCommon_8007CF58` (Bound's own air drift clamp, in `ftcommon.c`) are
captured for dispatch only, not reproduced arithmetically, since neither
lives in `ftfoxspecialhi.c` and both are exercised by this port's own
native tests instead.

`tests/fox_up_special_differential.rs` compares every pinned function
bit-exactly, with one documented exception: values derived through a
`cosf`/`sinf`/`atan2f`/`acosf` call (directly or via `angle_xy`'s own
formula) are compared through a small ULP/absolute tolerance
(`close_bits`) instead of raw bit equality. This crate's own `libm`
dependency (used pervasively, not just here, for cross-platform replay
determinism regardless of the local system's C library) measurably
disagrees with this host's system C compiler's `atan2f`/`cosf`/`sinf`/
`acosf` by a handful of ULPs on some inputs -- confirmed directly, and the
same reason `tests/game_ground_launch.rs`'s own trig-derived expectation
already uses a tolerance rather than bit-exact equality. Near a `cosf`/
`sinf` zero crossing this small angular disagreement becomes an
arbitrarily large *relative* difference in the result (both functions
have unit-magnitude derivative at their own zero), which is why
`close_bits` also accepts a small absolute floor; both bounds are
documented in place with the concrete numbers observed, orders of
magnitude tighter than any actual translation bug this suite caught
during development (a wrong operand grouping or formula produces a
completely different value, not a fraction of a percent).

## Tests
`tests/fox_up_special_differential.rs`: Hold's Enter/Phys on the ground
and in the air with the gravity delay; the launch angle from the stick
with the two stick thresholds and the straight-up default, and the
grounded-vs-declined decision (including the platform check); Travel's
Anim/Phys/Coll on both ground and air, including the reverse acceleration
after `x70` and the wall/ceiling/bound decision (including the
NaN-reachable gates above); Landing/Fall's Anim/Phys/Coll, including the
frame-13 regression; Bound's Enter/Anim/Phys/Coll. `src/game/characters/
fox/up.rs`'s own `#[cfg(test)] mod tests` covers `angle_xy`/`face_stick`
directly: zero vectors, the NaN-product-propagates-NaN case, general
NaN-safety, and the sign/zero convention. `tests/game_fox_up_special.rs`
covers the same phases end to end through a real `Match`: grounded and
aerial entry (gravity delay, jumps left untouched at Hold's own charge
start and restored only at the actual launch, the fresh-press-only aerial
gate), the launch angle's two thresholds and the straight-up default, the
grounded launch along a floor versus declining when the stick points up
or the fighter is on a platform, Travel's duration counting down
independent of its pose and the reverse acceleration engaging, the bound
decision firing past `x6C` grounded frames, the frame-13 regression, the
`FallSpecial` exits with the scaled mobility, Bound's own exit-flag-forced
early exit, Slippi ids, a checkpoint round trip and invalid resources.
This follow-up adds: the mid-Travel wall/ceiling redirect not letting the
ordinary auto-zero-into-wall response fire against a shallow wall or
ceiling hit (`travel_air_redirect_does_not_zero_velocity_against_a_
shallow_wall`/`_ceiling` -- the redirect's own facing/rotateModel
recompute is a no-op whenever velocity itself is unchanged, since Travel
never applies gravity or drag before `duration_end`, so this is the
wiring-level effect a native `Match` test can actually observe; the
bit-exact angle/gate arithmetic for ceiling, both walls and shallow/steep
floor contacts was already covered oracle-side by `compare_travel_coll_
air`, which predates this follow-up), ground Travel's own rotation
staying live across several grounded frames and surviving into the air
phase after an edge drop
(`travel_ground_rotation_reflects_the_last_grounded_floor_normal_after_
leaving_the_edge`), and this move's own `FallSpecial` landing lag
(`fall_special_landing_uses_this_move_s_own_landing_lag_not_the_common_
one`, distinguishing the two rates numerically: `(6.1)/x90 == 1.525`
against `(6.1)/rules.landing_lag == 0.61`). `crates/cli/tests/replay_match.rs` adds a grounded (SpecialHiHold/SpecialHi
/SpecialHiLanding) and an aerial (SpecialHiHoldAir/SpecialAirHi/
SpecialHiFall/FallSpecial) self-recorded regression, framed like the side
special's own pair as self-consistency evidence, not Melee parity
(`docs/parity.md`); unlike the side special's one-shot entry press, the
grounded scenario holds its directional input for several frames past
entry, since this move's own launch decision reads the stick live at the
moment Hold's animation ends. Ledge catching during Travel/Fall is
declared via `ledge_catchable` (inspectable in `up.rs`) without its own
dedicated functional test, matching the side special batch's own
precedent (its `ledge_catchable` gain for three aerial phases has no
dedicated ledge-grab test either) -- exercising the full ledge resource
fixture is mostly testing the shared ledge system, not this move.
