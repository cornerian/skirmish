# Damage wall and ceiling response profile

`rules.damage.surface_response` enables ordinary headless wall and ceiling
reflection during tumbling damage. The resource explicitly supplies the strict
directional knockback threshold, reflected-velocity multiplier, repeat lockout
and synthetic wall/ceiling action durations. Omitting it retains ordinary solid
surface stopping and does not infer common-data values.
The profile requires `floor_response` because that profile defines the explicit
tumble threshold used to enter the reflected damage graph.

An eligible contact combines self velocity and knockback, mirrors the result
across the sampled stage normal, applies the configured multiplier, clears self
velocity and faces along the reflected horizontal component. Wall contacts enter
`FlyReflectWall`; ceiling contacts enter `FlyReflectCeiling`. Their configured
durations return to `DamageFall` while retaining tumble state. Collision corrects
the ECB before the transition and reports `SurfaceReflected` with the player,
surface class and stable line ID. Floor landing has priority over a wall or
ceiling reflection found in the same collision pass.

`fighter::damage::reflect_velocity` retains the float operation order from the
complete `ftCo_800C18A8` entry and its `lbVector_Add_xy`/`lbVector_Mirror`
helpers. The C adapter stubs presentation, sound, camera, skeleton-placement and
collision services; differential tests compare reflected velocity, cleared self
velocity, facing and the byte lockout. Match tests own directional eligibility,
ECB contact, floor priority, action scheduling and checkpoint replay.

`game_damage_surface` covers rightward wall and upward ceiling launches, exact
configured action durations, floor-over-wall priority, profile omission, an
unmet threshold, malformed resources and deterministic checkpoint suffixes.
These fixtures use invented stage and fighter data.

Wall and ceiling techs, tech rolls, character-specific poses, effect/audio
commands, the complete collision callback graph and independent Melee traces
remain pending. The current repeat guard records the most recent surface class;
native line-specific and connected-corner behavior needs broader collision-state
translation.
