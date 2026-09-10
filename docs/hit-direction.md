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

Hitbox angle 362 replaces the ordinary position rule with the contact-relative
branch later in `ftColl_8007A06C`. It subtracts the matrix narrow phase's hurt
surface contact from the contacted hurt capsule's midpoint. The horizontal sign
selects damage facing; `atan(dy / abs(dx))` selects a signed launch angle and is
converted to degrees before truncation toward zero. Horizontal separations below
1e-5 select zero degrees. This uses evaluated bones and remains headless physics.

`fighter::damage::{fighter_hit_direction, throw_hit_direction}` isolate these
two exact statements. Unit tests cover strict comparison, equality, signed zero
and NaN. `hit_direction_differential` compares 512 arbitrary binary32 cases with
a host C adapter and proves both statements occur verbatim in pinned upstream
snapshots. `game_hit_direction` covers either player as attacker, both sides,
the equal-X tie, grounded tangent projection and checkpoint replay.
`game_damage_floor` covers the DownDamage override, while `game_grab` verifies
the captured-victim assignment through all four throw actions.
`fighter::damage::positional_launch` isolates the complete angle-362 arithmetic;
its unit tests cover quadrants, truncation and the strict vertical threshold.
`positional_angle_differential` compares 512 arbitrary finite geometries with a
verbatim C branch and its pinned SDK conversion macro. `game_positional_angle`
covers three contact quadrants, the vertical tie and checkpoint replay through
matrix-aware body contact and the shared damage transition. Angle 362 is only
valid on body-contact hitboxes because scripted throws do not produce that
geometry.

Item velocity/position direction, environment damage and callback-provided
facing overrides outside DownDamage remain separate work.
