# Grab and throw profile

`rules.grab` enables physical Z-button catch dispatch and supplies the signed
stick thresholds used by ordinary throw selection. Each `fighters[].grab`
resource supplies complete catch poses and bone-attached grab capsules, an
explicit pull duration, two bone-local attachment anchors, and complete holder
poses for pummel and four throws. Pummel supplies one captured-damage frame and
damage value; each throw supplies a release event and hit coefficients. Every
fighter needs parameters when the common profile is enabled.

A catch tests its current sampled grab capsules against enabled, grabbable target
hurt capsules with the matrix-aware source solver. Bone scale and shear retain
directional radius. Successful contact enters CatchPull/CapturePulled and records
the bidirectional relationship in deterministic match state. The victim's
selected bone point is aligned to the holder's selected bone point throughout
pull, wait, and throw; controller input cannot move or dispatch actions for the
captured fighter. The relationship, input history, action clocks, positions, and
bone-driven depth are included in observations and checkpoints.

CatchPull advances to CatchWait after its configured duration. Following
`fn_800DA4C0`, a fresh A press selects CatchAttack before throw input. Its single
scripted hit adds percent, freezes both fighters with the shared exact hitlag
calculation, and retains the pair until the sampled animation returns to
CatchWait. A fresh main or C-stick crossing otherwise selects a throw using
`ftCo_800DD1E4` priority: horizontal main, horizontal C-stick, up, then down. At
the supplied release frame, the paired state clears before the victim enters the
ordinary damage, hitlag, hitstun, DI, knockback, floor, and blast-zone pipeline.
A blast-zone exit also clears both sides of the relationship before publishing
stock loss.

`game_grab` covers animated bone contact versus a miss, pummel priority,
single-hit timing, repeated pummels, hitlag, sampled attachment motion, all throw
directions, fresh-edge history, input suppression for held victims, checkpoint
replay, stable simultaneous-catch order, airborne rejection, hurtbox pair cleanup
on KO, and malformed resources. `game_hurtbox_eligibility` adds the per-capsule
state/grabbable filters and directional bone scale. Three grab/throw conformance
scenarios now run normally. `fighter::grab` unit tests cover the exact fresh-A
predicate; `grab_differential` compares the three retained stick-crossing
predicates with complete functions selected from the pinned original C file.

This profile does not yet implement dash/pivot/tether catches, mash and grab
escape, cargo carries, the victim's CaptureDamage reaction animation, throw or
pummel staling, weight-scaled animation rate, character overrides, or multiplayer
capture interference. Catch-versus-hit and capture-clash priority also need the
larger original contact scheduler.
