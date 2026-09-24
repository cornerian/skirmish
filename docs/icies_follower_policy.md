# Ice Climbers follower policy

`scripts/fighters/ice_climbers.py` contains the script side of Nana's source
motion policy. The state constants mirror `ftNana/ftnana.c`: side special uses
359/360, while Belay uses the 361..366 graph. `nana_state_for_leader` applies
the same Popo row and partner range checks used by `ftNn_Init_80123954` and
`ftNn_Init_801230D0`.

`nana_follow_frame` computes the values Nana copies from Popo during the
source side and Belay paths: self velocity, animation velocity, ground
velocity/acceleration, facing, animation frame, Belay startup rate, and the
already-resolved joint anchor position. `nana_lifecycle_reset` mirrors only
the source armor, parts, union fields, bilateral relation, and rotation reset
used on detach/death.
The tests in `scripts/api/tests/test_ice_climbers.py` cover those decisions
against the pinned decomp at revision `0bac93a5`.

The current generic projection is read-only and has no joint lookup. To apply
this policy in the host, the smallest character-neutral capability is a
validated secondary-entity mutation operation with these fields:

* target entity handle and owner port/ordinal validation;
* action state, animation frame, velocity fields, facing, Belay animation rate,
  and optional resolved part anchor;
* lifecycle reset for source armor, visible parts, union fields, relation, and
  rotation.

Script callbacks call `mutate_entity` or `update_entity` only when the host
provides one. Missing mutation support leaves the callback inert, preserving
the read-only projection's fail-closed behavior.

That interface belongs to the generic fighter host. It should consume the
`NanaFollowerFrame` result and must not contain Ice Climbers state numbers or
character-specific branches.
