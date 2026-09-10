# Grab and throw profile

`rules.grab` enables physical Z-button catch dispatch and supplies the signed
stick thresholds used by ordinary throw selection, plus explicit hold-timer,
mash, release-speed, throw-weight and capture-lift coefficients. Its optional
`shield_grab` block (`x68` dash-grab buffer frames and the `x4C` dash frame
limit) enables grabs dispatched from a raised shield; without it, A or Z while
shielding does nothing rather than guessing those common values. Each `fighters[].grab`
resource supplies separate complete standing and dash-catch poses with
bone-attached grab capsules, their explicit pull durations, two bone-local
attachment anchors, complete holder poses for pummel and four throws, separate
high/low CaptureDamage poses, and the fighter's own CatchCut and CaptureCut poses.
Pummel supplies one captured-damage frame, damage value, and native move-table
identity; each throw supplies its own identity, weight-independent flag, release
event, and hit coefficients. The identities are required when stale-move rules are enabled.
Every fighter needs parameters when the common profile is enabled.

A catch tests its current sampled grab capsules against enabled, grabbable target
hurt capsules with the matrix-aware source solver. Bone scale and shear retain
directional radius. Successful contact enters CatchPull or CatchDashPull and
selects CapturePulledLw for a grounded victim or CapturePulledHi for an airborne
victim. Pull, wait, and pummel damage retain that family in deterministic match
state. The victim's selected bone point is aligned to the holder's selected
bone point throughout
pull, wait, and throw; controller input cannot move or dispatch actions for the
captured fighter. The relationship, input history, action clocks, positions, and
bone-driven depth are included in observations and checkpoints.
If bone alignment raises a low captured fighter strictly beyond the common
threshold times its base root-bone Y scale, the family changes to high. Leaving
support does the same; high-family floor contact changes back to low. These
transitions preserve the current action frame and use the ordinary swept stage
solver before attachment resumes on the next frame.

A fresh grab request in Dash or Run enters CatchDash and uses the dedicated
dash-catch samples while retained ground momentum decays through ordinary
physics. Contact enters CatchDashPull for that resource's pull duration. Turn
uses the standing catch and applies its pending facing first, matching the
source Turn IASA ordering; Squat startup also accepts the standing transition.
Both catch animations recover normally after a miss.

The victim's timer starts at `base + percent * scale` on contact. CaptureWait
and CaptureDamage subtract the passive decrement once per active frame. The
complete `ftCommon_GrabMash` transition subtracts one additional penalty for a
fresh A/B/X/Y/processed-shoulder press and one for any main-stick axis whose
latched sign changes beyond the strict threshold. Digital and analog shoulder
pressure share one logical edge, so pressing digital L/R over held analog
pressure does not mash twice. Neutral stick does not reset a latch, and button
plus stick input can subtract twice. Hitlag freezes all of this state.
Expiry in either CaptureDamage family waits for that reaction to return to its
matching CaptureWait family; expiry
there detaches the pair, applies opposite release velocities, and enters sampled
CatchCut/CaptureCut actions before ordinary recovery.

CatchPull advances to CatchWait after its configured duration. Following
`fn_800DA4C0`, a fresh A press selects CatchAttack before throw input. Its single
scripted hit adds percent, starts the victim's sampled CaptureDamage reaction,
freezes both fighters with the shared exact hitlag calculation, and retains the
pair until the animations return independently to CatchWait and CaptureWait.
Pummel damage uses the holder's stale queue and records its CatchAttack instance
once at contact. A later pummel restarts the victim reaction. A fresh main or C-stick crossing
otherwise selects a throw using
`ftCo_800DD1E4` priority: horizontal main, horizontal C-stick, up, then down. At
the supplied release frame, the paired state clears before the victim enters the
ordinary damage, hitlag, hitstun, DI, knockback, floor, and blast-zone pipeline.
Each throw direction has a distinct stale identity; release applies staled
percent while preserving the unstaled base-damage term for knockback, then
records the holder's throw instance. Holder stock loss resets the queue. A
blast-zone exit also clears both sides of the relationship before publishing
stock loss.

Following `ftCo_800DD4B0`, each direction either advances at rate one or at
`1 / (victim weight * common scale)`. Holder and victim share the fractional
clock in checkpointed physics state. Integer pose sampling follows that clock,
and a rate above one stops at the release event instead of skipping it. Release
clears the pair clock and restores the holder's ordinary rate-one recovery.

`game_grab` covers separate standing/dash bone contact versus a miss, Dash/Run
entry, retained dash momentum, pending and completed Turn facing, Squat entry,
pummel priority,
single-hit timing, repeated and staled pummels, independent throw identities,
queue insertion and death reset, shared hitlag, sampled holder and victim
high/low reaction poses, independent reaction completion, passive and mashed escape,
button and analog-shoulder freshness, stick latches, hitlag timer freeze, cut
actions and release motion, grounded/airborne capture-family selection, all throw
directions, animation-driven lift and swept floor conversion, input suppression,
checkpoint replay, stable
simultaneous-catch order, airborne rejection, weight-dependent and independent
throw timing, fast event crossing, hurtbox pair cleanup on KO, and
malformed resources. `game_hurtbox_eligibility` adds the per-capsule
state/grabbable filters and directional bone scale. Three grab/throw conformance
scenarios now run normally. `fighter::grab` unit tests cover the exact fresh-A
predicate, mash mutation, throw-rate branch and operand order, and the strict
capture-alignment height branch.
`grab_differential` compares the three retained
throw stick-crossing predicates; `grab_mash_differential` compares the complete
mash function over arbitrary timer, input and latch state. Both select complete
functions from pinned original C. `capture_alignment_differential` compares the
complete `fn_800DAD18` position mutation and scaled-height result over arbitrary
binary32 positions, anchors, thresholds, and scales.

## Shield grabs

`ftCo_Catch_CheckInput` runs in the GuardOn, Guard and GuardReflect input
chains after the spot dodge and roll and before the jump dispatchers; GuardOff
and shield stun never grab. It requires a fresh logical A press while the
logical shoulder is held; `Fighter_procInput` folds physical Z into both bits,
so Z grabs even after the trigger was released inside the minimum hold, while
A alone does not. `ftCo_800D8B9C` precedes it in GuardOn and GuardReflect only:
when Run, or Dash past the `x4C` animation frame, raises a shield,
`ftCo_80091B9C` arms the guard `x24` buffer with `x68` frames, and a fresh A
inside that buffer starts CatchDash instead of Catch. The buffer counts down
only on callbacks that reach the predicate, so Guard neither drains nor consults
it, and an ordinary GuardOn entry (`ftCo_800923B4`) clears it. Both catch
entries use an ordinary motion change, which clears the Slippi-visible reflect
bit and powershield entry latch while the immunity window persists. The union
residue a powershield raised from Wait would inherit in `x24`, the item pickup
branch, the Link/Samus tether gates and the early-dash `x44`/`x48` shield
branches are not modeled. The buffer value survives checkpoints.
`shield_grab_differential` compares the complete `ftCo_Catch_CheckInput` and
`ftCo_800D8B9C` bodies with pinned C over arbitrary button words and buffers.

This profile does not yet implement tether catches, cargo carries, character
overrides, or multiplayer capture interference. Catch-versus-hit and
capture-clash priority also need the larger original contact scheduler.
