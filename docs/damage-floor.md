# Damage-floor response profile

`rules.damage.floor_response` enables the ordinary headless tumble landing
slice. It supplies a knockback threshold, the original buffered physical-L/R
tech window and repeat-press lockout, plus explicit synthetic durations for
Passive, DownBound, DownWait and DownStand. Omission retains the earlier damage
slice and never invents Melee constants.

Optional `floor_response.tech_roll` supplies the directional-stick threshold.
Each fighter then supplies `floor_tech.forward` and `floor_tech.backward` as one
headless physics sample per action frame. Every sample contains the complete
bone pose and a signed local TransN delta. The scheduler projects that delta
through facing and the current floor normal, so rolls change world position,
ground velocity, hurtbox bones and bone-derived ECBs without a renderer.

Damage at or above the inclusive tumble threshold remains eligible after
hitstun changes the airborne action to DamageFall. Floor contact clears launch
velocity and selects neutral Passive when the buffered-tech predicate succeeds;
otherwise it enters DownBound. The native scheduler then advances the complete
DownBound -> DownWait -> DownStand -> Wait recovery chain. A non-tumbling
damage landing does not enter this graph. DamageFall uses ordinary gravity,
air drift and fresh fast-fall input while preserving tumble eligibility.

The reusable `fighter::damage::can_tech` kernel is the complete
`ftCo_800986B0` predicate. Its C differential retains the source's strict
current-press window, inclusive previous-press boundary and input-lock gate.
`fighter::damage::tech_roll_direction` retains the directional portion of
complete `ftCo_80098928`: absolute stick magnitude uses the inclusive threshold,
and the stick/facing product chooses forward or backward. Its C differential
supplies the already-tested tech result and compares every selector output.
The match samples x680/x684-style physical L/R ages during hitlag and ordinary
frames and checkpoints both timers.

Optional `floor_response.knockdown_options` supplies the two stick thresholds,
vertical angle, upward C-stick threshold and DownBound attack-buffer window.
Each fighter then supplies one full bone pose for every Passive, DownBound,
DownWait and DownStand frame, forward/backward missed-tech motions and a complete
get-up `Attack`. These poses drive hurtboxes and ECBs in headless simulation.

At the end of DownBound, separate A/B press ages are checked before a fresh
upward C-stick and then roll input. The ages reset on entry, so an attack pressed
before the missed tech cannot leak into the buffer. Upward main stick and L/R do
not stand until DownWait. DownWait preserves its own priority: a fresh A/B or
upward C-stick starts DownAttack; otherwise a fresh horizontal C-stick overrides
the main stick for DownForward/DownBack; otherwise held horizontal input can
roll, and upward main stick or fresh L/R can stand. Roll direction is relative
to facing. A C-stick held before either decision is not treated as fresh.

Optional `floor_response.recovery_invincibility` supplies separate contact-frame
durations for neutral tech, directional tech, missed-tech roll, stand and attack.
The shared hit and grab scans read the same checkpointed timer. Resource
validation requires every duration to fit its supplied motion or pose sequence.

`fighter::damage::fresh_up_cstick` and `fresh_horizontal_cstick` retain complete
`ftCo_800DF644` and `ftCo_800DF678` predicates. Original-C property tests cover
their inclusive/exclusive thresholds, comparison-based absolute value, angle
test and arbitrary binary32 values. The combined priority selector is refactored
match policy and has unit and integration coverage rather than whole-callback
parity claims.

`game_damage_floor` covers neutral tech recovery, tech and missed-tech rolls in
both directions, DownBound buffering and reset, attack/roll/stand priority,
fresh and held C-stick histories, inclusive thresholds, sampled root motion,
bone-derived ECB changes for the full grounded recovery suffix, get-up attack
contact through the shared combat pipeline, exact recovery protection and its
first vulnerable frame, state durations, repeat lockout, non-tumble separation,
DamageFall persistence, resource rejection, serialization, checkpoint replay
and reset. The two tech/knockdown conformance scenarios run normally.

This profile does not yet provide action-specific airborne damage poses, prone
orientation variants or input-lock states. Wall/ceiling reflection and techs
are supplied separately by the [damage-surface profile](damage-surfaces.md).
The remaining paths need their own native resources and scheduler integration.
