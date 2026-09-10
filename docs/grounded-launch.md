# Grounded launch profile

`rules.damage.ground_launch` enables the grounded branch of
`ftCo_8008DCE0`. It is paired with `rules.damage.damage_motion`, whose selected
knockback level determines whether the hit uses ordinary grounded response or
the fly branch. The profile supplies common-data x1E8, x1EC and x200: the fly
bounce angle, its vertical multiplier and the grounded knockback friction
multiplier.

The transition compares the incoming launch vector with the current supporting
floor normal. An angle below pi/2 leaves the floor. At pi/2 and above, levels
one through three remain grounded and project horizontal launch onto the floor
tangent. A fly hit always leaves the floor; when its angle is strictly greater
than pi/2 plus x1E8, its vertical component reverses and is scaled by x1EC.
Prone DownDamage supplies an explicit motion ID in the source, so it forces this
fly branch even when its ordinary knockback level is low.

Grounded response retains source xF0 as `Fighter::ground_knockback`. Active
physics frames decay that scalar by fighter ground friction times x200, then
reproject it using the latest floor normal. Hitlag freezes the scalar. Landing,
capture, ledge, death, rebirth and surface-tech transitions clear it. The field
is serialized in privileged state and preserved by checkpoints.

`fighter::damage::ground_launch` and `vector_angle` isolate the pointer-free
arithmetic. Unit tests cover the pi/2 branch boundary, fly bounce, tiny-vector
fallthrough, slopes and signed zero. `ground_launch_differential` runs 512
arbitrary binary32 cases through Rust and a host C adapter containing the
complete source branch and extracted `lbVector_Angle`. The branch outputs match
bitwise; angle results allow two ULPs for the host math implementation.
`game_ground_launch` covers flat and sloped support, ground retention, departure,
bounce, hitlag freezing, scalar friction, state serialization, invalid resources
and exact checkpoint replay. `game_damage_floor` also covers forced fly response
for prone DownDamage.

The profile composes with the source-backed fighter/throw
[damage-direction rules](hit-direction.md) and supplied stage normals. Ice
launch-angle changes, material-dependent ground-friction
multipliers, authentic common data and the PowerPC reciprocal-root estimate
remain separate work.
