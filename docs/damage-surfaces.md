# Damage wall and ceiling response profile

`rules.damage.surface_response` enables ordinary headless wall and ceiling
reflection during tumbling damage. The resource explicitly supplies the strict
directional knockback threshold, reflected-velocity multiplier, repeat lockout
and wall/ceiling action durations. Each paired fighter `surface_response`
resource supplies one complete sampled bone pose per reflected action frame.
Optional
`rules.damage.surface_tech` supplies wall freeze, wall and ceiling action timing,
the upward-stick threshold and the scripted ceiling-input frame. Each fighter
then supplies its wall, wall-jump and ceiling launch speeds plus one complete
sampled bone pose per action frame in `surface_tech`. Omitting both profiles
retains ordinary solid-surface stopping and does not infer common-data values.
Either profile requires `floor_response` because that profile defines the
explicit tumble threshold and physical-L/R tech window.

An eligible contact combines self velocity and knockback, mirrors the result
across the sampled stage normal, applies the configured multiplier, clears self
velocity and faces along the reflected horizontal component. Wall contacts enter
`FlyReflectWall`; ceiling contacts enter `FlyReflectCeiling`. Their configured
durations return to `DamageFall` while retaining tumble state. Their sampled
skeletons drive hurtbox and bone-based ECB geometry headlessly. Collision
corrects the ECB before the transition and reports `SurfaceReflected` with the
player, surface class and stable line ID. Floor landing has priority over a wall
or ceiling reflection found in the same collision pass. The repeat field stores
the native surface class. During `FlyReflectWall`, its wall lockout still permits
a ceiling response; `FlyReflectCeiling` likewise permits a wall response. A
second response replaces the remembered class, restarts the configured lockout
and remains deterministic across checkpoint replay.

An eligible buffered physical-L/R press turns a wall contact into `PassiveWall`
or `PassiveWallJump`. A fresh X/Y press inside the shared jump-input window, or
an upward stick at the inclusive configured threshold, selects the jump. Both
actions freeze for the configured opening frames, then launch away from the
wall using fighter attributes and recover to `Fall` after their own durations.
A neutral wall tech can queue a fresh jump or inclusive upward-stick input while
the freeze timer remains nonzero. When the timer expires, it switches to the
damage wall-jump pose track at the same action frame and uses wall-jump launch
velocity. Input first supplied on the release frame is too late to convert it.
Once released, the wall actions dispatch the implemented airborne neutral
special, aerial attack and available aerial jump branches in source priority.
Inputs during the freeze cannot start those actions, and held buttons require a
fresh edge afterward. Ceiling tech retains its empty source interrupt callback.
A ceiling contact enters `PassiveCeiling`; its configured script frame samples
horizontal input, applies the fighter's ceiling speed and then recovers to
`Fall`. Their sampled skeletons drive hurtbox and bone-based ECB geometry in
headless matches. Ordinary wall jumps and damage wall-jump techs use distinct
fighter resources despite sharing the `PassiveWallJump` action. Tech input and
pose-driven collision state survive checkpoints. Floor response wins over either
surface response, and a wall wins over a ceiling at a simultaneous corner
contact. A successful tech emits `SurfaceTeched` with the player, surface,
stable line ID and wall-jump decision.
Gravity and ordinary floor collision continue after release. Neutral wall,
wall-jump and ceiling techs all enter the shared landing action on contact,
clear both surface-tech and ordinary wall-jump transient state, and retain the
configured landing duration through checkpoint replay.
Frozen startup still runs the moving-stage collision pass. An inward-moving
wall displaces the fighter by the line's frame delta while self velocity remains
zero, retains the stable wall contact and advances the tech timer exactly once.
An outward-moving wall clears contact without dragging the fighter.

`fighter::damage::reflect_velocity` retains the float operation order from the
complete `ftCo_800C18A8` entry and its `lbVector_Add_xy`/`lbVector_Mirror`
helpers. The C adapter stubs presentation, sound, camera, skeleton-placement and
collision services; differential tests compare reflected velocity, cleared self
velocity, facing and the byte lockout. Match tests own directional eligibility,
ECB contact, floor priority, action scheduling and checkpoint replay.
`fighter::damage::wall_tech_jumps` retains complete `ftCo_800C1E0C`; its C
differential compares the strict jump-age window and inclusive stick threshold
over arbitrary binary32 inputs. The match owns contact eligibility, shoulder
lockout, launch scheduling and resource-driven speeds.

`game_damage_surface` covers rightward wall and upward ceiling reflections,
their distinct reflected pose tracks through the bone-based ECB, neutral and
jump wall techs, both wall orientations, ceiling input motion, exact configured
action durations, floor and wall priority, shoulder repeat lockout, all three
tech pose tracks through the bone-based ECB, ordinary wall-jump resource
coexistence, delayed neutral-to-jump conversion and its release-frame boundary,
post-freeze air-action timing and priority, ceiling non-interruption, profile
omission, all three tech floor-landing paths, both reflected-action knockdown
landings and cleanup, an unmet threshold,
inward/outward moving-wall startup response, both cross-surface reflection
chains during lockout, malformed resources and deterministic checkpoint suffixes.
These fixtures use invented stage and fighter data.

Missed-tech choices, invincibility, effect/audio commands, the complete collision
callback graph and independent Melee traces remain pending. Directional floor tech rolls are supplied by the
[damage-floor profile](damage-floor.md).
Connected-corner adjacency behavior still needs broader collision-state
translation.
