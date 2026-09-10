# Hurtbox contact

Ordinary fighter hitboxes now test body hurtboxes through
`collision::shield::capsule_matrix`, the shared port of `lbColl_80006E58`.
Despite the module's historical name, this source routine is also the ordinary
fighter hit-versus-hurt narrow phase. The caller supplies the hitbox's
previous/current world centers, the hurt capsule's world endpoints, and the
hurt bone's evaluated world matrix.

The matrix inverse measures hurt radius along the actual line of contact. Bone
nonuniform scale and shear therefore affect collision directionally instead of
being guessed as one isotropic radius. The match uses the source hurtbox
broadphase multiplier of 3, retains inclusive touching, and keeps the original
closest-endpoint tie behavior. All of this remains headless bone physics.

`shield_collision_differential` already compares the shared complete C routine
over arbitrary matrices and capsule geometry. `shield_geometry` covers its
standalone nonuniform-scale, shear, sweep, broadphase and failure contracts.
`game_hurtbox_geometry` adds scheduler integration: one nonuniformly scaled
hurtbox is reachable along its long axis and unreachable at the same kind of
distance along its short axis. Existing swept-hitbox and damage-pose tests
verify that motion history and action-selected bones still feed this path.

Hurtbox state flags, invulnerability per capsule, character/model radius
multipliers and the source forced-contact branch remain separate work.
