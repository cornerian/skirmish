# Damage direction

Ordinary fighter contact now uses the fighter branch of `ftColl_8007A06C`.
When the victim is strictly to the right of the attacker, the stored damage
facing is -1; every other comparison, including equal X, stores +1. The shared
damage transition then computes horizontal launch as
`-speed * cos(angle) * damage_facing`, so the victim moves away from the
attacker. This rule depends on positions rather than the attacker's facing.

Throws enter the same launch transition with the distinct assignment from
`ftCo_800DDDE4`: damage facing is the negated thrower facing. Both assignments
also reorient the victim when ordinary Damage begins. The prone DownDamage path
still launches with the newly selected damage facing, then applies its source
callback's explicit prior-facing override.

`fighter::damage::{fighter_hit_direction, throw_hit_direction}` isolate these
two exact statements. Unit tests cover strict comparison, equality, signed zero
and NaN. `hit_direction_differential` compares 512 arbitrary binary32 cases with
a host C adapter and proves both statements occur verbatim in pinned upstream
snapshots. `game_hit_direction` covers either player as attacker, both sides,
the equal-X tie, grounded tangent projection and checkpoint replay.
`game_damage_floor` covers the DownDamage override, while `game_grab` verifies
the captured-victim assignment through all four throw actions.

Item velocity/position direction, environment damage, the special 362-degree
hurtbox/contact angle and callback-provided facing overrides outside DownDamage
remain separate work.
