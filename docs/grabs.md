# Grab and throw profile

`rules.grab` enables physical Z-button catch dispatch and supplies the signed
stick thresholds used by ordinary throw selection, plus explicit hold-timer,
mash and release-speed coefficients. Each `fighters[].grab`
resource supplies separate complete standing and dash-catch poses with
bone-attached grab capsules, their explicit pull durations, two bone-local
attachment anchors, complete holder poses for pummel and four throws, and the
fighter's own CaptureDamage, CatchCut and CaptureCut poses.
Pummel supplies one captured-damage frame and damage value; each throw supplies
a release event and hit coefficients. Every fighter needs parameters when the
common profile is enabled.

A catch tests its current sampled grab capsules against enabled, grabbable target
hurt capsules with the matrix-aware source solver. Bone scale and shear retain
directional radius. Successful contact enters CatchPull or CatchDashPull with
CapturePulled and records the bidirectional relationship in deterministic match
state. The victim's selected bone point is aligned to the holder's selected
bone point throughout
pull, wait, and throw; controller input cannot move or dispatch actions for the
captured fighter. The relationship, input history, action clocks, positions, and
bone-driven depth are included in observations and checkpoints.

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
Expiry in CaptureDamage waits for that reaction to return to CaptureWait; expiry
there detaches the pair, applies opposite release velocities, and enters sampled
CatchCut/CaptureCut actions before ordinary recovery.

CatchPull advances to CatchWait after its configured duration. Following
`fn_800DA4C0`, a fresh A press selects CatchAttack before throw input. Its single
scripted hit adds percent, starts the victim's sampled CaptureDamage reaction,
freezes both fighters with the shared exact hitlag calculation, and retains the
pair until the animations return independently to CatchWait and CaptureWait. A
later pummel restarts the victim reaction. A fresh main or C-stick crossing
otherwise selects a throw using
`ftCo_800DD1E4` priority: horizontal main, horizontal C-stick, up, then down. At
the supplied release frame, the paired state clears before the victim enters the
ordinary damage, hitlag, hitstun, DI, knockback, floor, and blast-zone pipeline.
A blast-zone exit also clears both sides of the relationship before publishing
stock loss.

`game_grab` covers separate standing/dash bone contact versus a miss, Dash/Run
entry, retained dash momentum, pending and completed Turn facing, Squat entry,
pummel priority,
single-hit timing, repeated pummels, shared hitlag, sampled holder and victim
reaction poses, independent reaction completion, passive and mashed escape,
button and analog-shoulder freshness, stick latches, hitlag timer freeze, cut
actions and release motion, all throw directions, input suppression, checkpoint replay, stable
simultaneous-catch order, airborne rejection, hurtbox pair cleanup on KO, and
malformed resources. `game_hurtbox_eligibility` adds the per-capsule
state/grabbable filters and directional bone scale. Three grab/throw conformance
scenarios now run normally. `fighter::grab` unit tests cover the exact fresh-A
predicate and mash mutation. `grab_differential` compares the three retained
throw stick-crossing predicates; `grab_mash_differential` compares the complete
mash function over arbitrary timer, input and latch state. Both select complete
functions from pinned original C.

This profile does not yet implement tether catches, cargo carries, separate
high/low capture reactions, throw or
pummel staling, weight-scaled animation rate, character overrides, or multiplayer
capture interference. Catch-versus-hit and capture-clash priority also need the
larger original contact scheduler.
