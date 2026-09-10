# Damage-floor response profile

`rules.damage.floor_response` enables the ordinary headless tumble landing
slice. It supplies a knockback threshold, the original buffered physical-L/R
tech window and repeat-press lockout, plus explicit synthetic durations for
Passive, DownBound, DownWait and DownStand. Omission retains the earlier damage
slice and never invents Melee constants.

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
The match samples x680/x684-style physical L/R ages during hitlag and ordinary
frames and checkpoints both timers.

`game_damage_floor` covers neutral tech recovery, exact configured state
durations, repeat lockout, non-tumble separation, DamageFall persistence,
resource rejection, checkpoint replay and reset. The two tech/knockdown
conformance scenarios run normally.

This profile does not yet provide action-specific damage/down poses,
invincibility windows, get-up choices, tech rolls, missed-tech options,
orientation selection or input-lock states. Wall/ceiling reflection and techs
are supplied separately by the [damage-surface profile](damage-surfaces.md).
The remaining paths need their own native resources and scheduler integration.
