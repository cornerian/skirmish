# Ordinary damage-motion profile

`rules.damage.damage_motion` enables source-shaped ordinary Damage motion
selection. Its three ascending thresholds are common data x158, x15C and x160;
the selector first multiplies applied post-armor knockback by the shared
`rules.hitstun_scale` (x154). Comparisons are strict, so equality advances to
the next level. Each fighter supplies `damage_poses` with all 15 selected motion
families and one low/middle/high selector for every hurtbox.

The pre-hit ground state chooses the source table row. Levels one through three
select nine grounded motions by hurt height or three airborne motions that
ignore hurt height. The fourth level selects low/middle/high DamageFly in both
rows. Throws use the source's middle-height default. The chosen `DamageMotion`
is checkpointed and visible in privileged state.

Every motion contains one complete bone pose per physics frame. These evaluated
bones drive environmental collision boxes, hurtboxes and later contacts without
a renderer. Damage exits only after both hitstun and the supplied motion end. If
hitstun is longer, the final pose remains active. A repeated hit immediately
reselects the motion from its new post-armor knockback and contact height. If a
longer motion remains after hitstun, airborne Damage switches from locked damage
physics to ordinary gravity, drift and fresh fast-fall input and can dispatch
the implemented neutral special, aerial attack and double jump branches.

Resources and rules are paired for every fighter. Validation rejects missing or
extra profiles, incomplete hurtbox-height maps, empty or oversized motions,
changed skeleton topology, nonfinite thresholds and thresholds that are not
strictly ascending. Omitting both fields retains the earlier synthetic static
pose and hitstun-only completion policy.

`fighter::damage::damage_motion` isolates the pointer-free arithmetic and table
choice from `ftCo_8008DCE0`. Its unit tests cover every table cell, equality and
NaN fallthrough. `damage_motion_differential` runs arbitrary binary32 values
through Rust and a host C adapter containing the byte-for-byte upstream level
selection and motion table. `game_damage_motion` covers all ground/air levels
and hurt heights, exact threshold equality, repeated-hit reselection,
bone-derived hurtbox and ECB changes, animation/hitstun completion, final-pose
holding, airborne physics and air-action dispatch across the hitstun boundary,
checkpoint replay, serialization and malformed resources.

This profile does not supply authentic character poses or the rest of the
damage callback graph. Floor-relative launch is supplied separately by the
[grounded launch profile](grounded-launch.md). Elemental and character damage
states and dynamic armor modifiers remain separate work.
