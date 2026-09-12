# Slippi state parity: jumps and falls

`skirmish::game::locomotion` and `crates/skirmish-replay/src/observation.rs`
port the Slippi-visible identity of the ground/aerial jump direction and the
aerial-jump fall (pinned decomp rev 0bac93a5), and correct a mapping bug in
FallSpecial's own numbers left over from the air-dodge batch. Motion-state ids
come from `ftCommon_MotionState` (`src/melee/ft/kinds/ftCommon/forward.h`) and
animation (sub-motion) ids from `ftCo_Submotion`:

| motion | id | sub-motion (animation) |
|---|---|---|
| WalkSlow / WalkMiddle / WalkFast | 15 / 16 / 17 | 7 / 8 / 9 |
| JumpF / JumpB | 25 / 26 | 16 / 17 |
| JumpAerialF / JumpAerialB | 27 / 28 | 18 / 19 |
| Fall / FallF / FallB | 29 / 30 / 31 | 20 / 21 / 22 |
| FallAerial / F / B | 32 / 33 / 34 | 23 / 24 / 25 |
| FallSpecial / F / B | 35 / 36 / 37 | 26 / 27 / 28 |
| Landing / LandingFallSpecial | 42 / 43 | 35 / 36 |
| Ottotto / OttottoWait | 245 / 246 | 210 / 211 |

## FallSpecial's corrected mapping

`observation::action_state` previously mapped `FallSpecial` to motion-state
31 with animation 22; those are FallB's numbers, not FallSpecial's. The
correction maps `FallSpecial` to 35 with animation 26, matching the table
above. FallF/FallB and FallSpecialF/FallSpecialB remain unmodeled:
`ftCo_Fall_Anim_Inner` swaps the blend skeleton's figatree through
`ftAnim_8006EDD0`, which never writes `fp->anim_id`, so these directional
blends are animation-only and not Slippi-visible. `FallSpecial` always
reports its F id.

## Backward jumps

`ftCo_Jump_Enter` (`ftCo_Jump.c:157-161`) selects the launched motion from
`msid = fp->input.lstick[0].x * fp->facing_dir > -x78 ? JumpF : JumpB`
(`ftCommonData.x78`). `ftCo_JumpAerial_Enter_Basic` (`ftCo_JumpAerial.c:
169-171`, `190-192`; the other character entries at `239-241` and `261-263`
repeat the same test, and the Yoshi path at `214` is always F) uses the
identical test at the aerial jump's own launch. JumpB's callbacks are
JumpF's (`ftmotionstates.c:420-430`: Anim/IASA/Phys/Coll) and JumpAerialB's
are JumpAerialF's (`442-452`), so the two variants differ only in the
reported motion and animation id, never in behavior.

The two direction tests read `fp->input` from different points in a
fighter's per-frame dispatch, so despite being textually identical they
consult different frames' sticks. `ftCo_Jump_Enter` is called from
`ftCo_KneeBend_Anim`, an *Anim* callback, which (like the generic per-frame
animation advance `observation::observe`'s general `-1` rule already
accounts for) runs before this same frame's own controller read updates
`fp->input`, so it actually sees the *previous* frame's stick.
`ftCo_JumpAerial_CheckInput` (which calls `ftCo_JumpAerial_Enter_Basic`) is
reached through an *IASA* chain (`ftCo_800CB870`/`ftCo_800CB8E0`, called
from `ftCo_Jump_IASA`/`ftCo_JumpAerial_IASA`'s own `RETURN_IF` chains),
which sees the current frame's already-updated `fp->input`, matching every
other IASA-dispatched check this crate already reads current input for
(the Dash/Turn/Run family, `docs/validation.md`). Confirmed directly against
`fox-fd.slp`: `fighter::locomotion::jump_backward` fed the frame *before*
each of the recording's 64 KneeBend->Jump transitions (both ports, full
match) agrees with every one; fed the transition frame's own stick instead,
three disagree (P4 at frame 3, P1 at 775 and 2990).

Model: `locomotion::Parameters.jump_backward_threshold: Option<f32>` (x78;
`None` keeps every jump JumpF/JumpAerialF, matching data that never modeled
the backward variants) and `locomotion::State.jump_backward: bool`, set by
`ground_jump`'s own launch from `f.previous_input`'s stick and facing, and by
`try_aerial_jump`'s own launch from the current frame's stick and facing,
cleared for every other action by `simulation::enter` (mirroring how
`landing_allow_interrupt` is reset there and immediately re-set by its own
entry). The pure predicate `fighter::locomotion::jump_backward(stick_x,
facing, threshold) -> bool` is the source's comparison negated, not an
independent `<=`: the two agree everywhere except NaN, where the source's
ternary (and so this predicate) falls to backward. Observation: `Jump` with
the flag reports 26/17, `JumpAerial` with the flag reports 28/19; without
the flag, or with the threshold absent, the existing 25/16 and 27/18.

## Aerial fall

`ftCo_JumpAerial_Anim` (`ftCo_JumpAerial.c:272-277`) ends into
`ftCo_FallAerial_Enter` (state 32), not `ftCo_Fall_Enter`. FallAerial's IASA
(`ftCo_Fall_IASA_Inner`), Phys (`ft_80084DB0`) and Coll (`ftswing.c:21`,
`ft_800831CC(gobj, ftCo_80096CC8, ft_80082B1C)`) are exactly Fall's, so it is
Fall with a different reported id and otherwise unchanged behavior.

Model: `locomotion::State.fall_aerial: bool`, set only where the aerial
jump's own animation end enters Fall (both the plain and `multi_jump`
branches of `locomotion::update_animation`'s `JumpAerial` arm), cleared for
every other action by `simulation::enter`. Observation: `Fall` with the flag
reports 32/23. Every later, unrelated `Action::Fall` entry — an aerial attack
ending past its last frame (`aerial::update_animation`), walking off a
platform, a ledge drop — reports the ordinary 29/20, since none of those call
sites set the flag.

## Tests

`tests/game_jump_variants.rs` (a synthetic native world, `tests/game_smash.
rs`'s style) covers: forward and backward short and full hops, including the
exact threshold boundary (`stick_x * facing == -x78` is backward, since the
source's own comparison is a strict `>`); a backward double jump reporting
its own direction independently of the ground jump that preceded it;
`jump_backward` clearing on landing, on a fresh aerial attack and on a fresh
air dodge; `fall_aerial` true only once the double jump's own animation ends,
and false both for an aerial attack's own end into Fall and immediately on
entering an air dodge from the aerial fall (the dodge itself always
continues into FallSpecial, never an ordinary Fall — `escape_air.rs`'s
`Action::EscapeAir` arm unconditionally enters FallSpecial and forces every
jump used, so a "dodge ends into an ordinary Fall" case does not exist to
test); checkpoint restoration through a backward Jump, a backward JumpAerial
and an aerial Fall; `None` keeping every jump forward; and rejecting a
non-finite or negative threshold.

`crates/cli/tests/replay_match.rs` matches a backward short hop into a
backward double jump into the aerial fall against its own Peppi bytes
(Slippi states 26/17, 28/19, 32/23), and a parallel forward short hop that
reaches the ordinary velocity-driven apex fall before its own forward double
jump (25/16, 29/20, 27/18 — in that row order, since the double jump here
follows the apex rather than preceding it), and reports a Mismatch at the
ground-jump launch row when the *previous* row's stick sample (the one
`ground_jump` actually reads) is flipped, even though that earlier row's own
JumpSquat report is unaffected. The air-dodge continuation's `FallSpecial`
row now asserts 35/26 instead of the corrected-away 31/22.

## Oracle

`ftCo_Jump_Enter` is pinned at `tests/oracle/original/jump.c`
(`tests/oracle/jump.functions.json` selects it alone; `ftCo_JumpAerial_Enter_
Basic` uses the identical test and is not separately pinned). The host
adapter `tests/oracle/jump.c` stubs `ftCommon_8007D5D4` and `ftCo_800CB110`
as no-ops and captures the requested motion from `Fighter_ChangeMotionState`,
exposing `oracle_jump_enter(stick_x, facing, x78) -> msid`.
`tests/jump_differential.rs` compares the pure `fighter::locomotion::
jump_backward` predicate against the oracle's captured direction over
proptest (512 cases, including NaN and both signs of infinity) plus the exact
equality and adjacent boundaries, `#![cfg(feature = "c-oracle")]`.

## Deferred

Walk speed variants (`ftwalkcommon.c`'s per-fighter animation-frame/rate
attributes and float `state_age`), Ottotto/OttottoWait
(`ftCo_Ottotto.c`, entered from the ground collision's edge-teeter flag) and
RunDirect (`ftCo_RunDirect.c`, state 22) remain unmodeled. This batch only
corrects FallSpecial's numbers and adds the backward jump and aerial-fall
identities above.
