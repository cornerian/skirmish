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

`game_damage_floor` covers neutral tech recovery, forward/backward rolls at the
inclusive threshold, sampled root motion, bone-derived ECB changes, exact state
durations, repeat lockout, non-tumble separation, DamageFall persistence,
resource rejection, serialization, checkpoint replay and reset. The two
tech/knockdown conformance scenarios run normally.

This profile does not yet provide action-specific damage/down poses,
invincibility windows, missed-tech get-up/attack choices, prone orientation or
input-lock states. Wall/ceiling reflection and techs
are supplied separately by the [damage-surface profile](damage-surfaces.md).
The remaining paths need their own native resources and scheduler integration.
