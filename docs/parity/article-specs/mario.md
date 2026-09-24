# Mario article and effect parity specification

Pinned source: `../../External/melee/src/melee/` (the checked out
`doldecomp/melee` snapshot).

## Fireball

Mario's neutral special uses fighter motion states 343 and 344:

| State | Source symbol | Role |
| --- | --- | --- |
| 343 | `ftMr_MS_SpecialN` | grounded fireball start |
| 344 | `ftMr_MS_SpecialAirN` | aerial fireball start |

`ftMr_SpecialN_Enter` and `ftMr_SpecialAirN_Enter` in
`ft/kinds/ftMario/ftmariospecialn.c` clear command variable 0 and
`throw_flags`. The animation callback ends in Wait (ground) or Fall (air).
The accessory callback `ftMr_SpecialN_ItemFireSpawn` consumes the B0 cue from
`throw_flags_b0`, samples the `L1stNb` joint, and calls
`it_8029B6F8(..., It_Kind_Mario_Fire, facing_dir)`. The authoring API exposes
this article as `ArticleId.MARIO_FIRE` (48) and its B0 event as the spawn
boundary.

`it/kinds/itmariofireball.c` initializes velocity from the item attributes as
`facing * speed * cos(angle)` and `speed * sin(angle)`, applies the lifetime,
and uses `itMariofireball_UnkMotion0_Coll`: after `it_8027781C` reports a
terrain collision, it compares the current 2D velocity magnitude with the
stop threshold and emits effect 1147 without an incoming-normal gate. Damage, shield,
reflection, absorption, clank, and wall effects are item-owned callbacks:
`itMarioFireball_Logic87_DmgDealt`, `...Reflected`, `...HitShield`,
`...Absorbed`, and `...Clanked`.

The typed host currently leaves Mario fireball reflection unsupported. Tests
using `reflection: "none"` are partial resource fixtures and do not claim
parity with `...Reflected`.

The ordinary `...HitShield` callback destroys the fireball. The separate
`...ShieldBounced` callback can preserve and reflect it in the source, but
the typed host has no distinct callback event yet and therefore does not
claim that bounce path.

## Cape article and reflection

Side special uses states 345 and 346 (`ftMr_MS_SpecialS` and
`ftMr_MS_SpecialAirS`). `ftMr_SpecialS_Enter`/`ftMr_SpecialAirS_Enter` call
`changeAction`, which clears command variables 0, 1, and 2 and schedules
`ftMr_SpecialS_CreateCape`. That callback attaches `It_Kind_Mario_Cape` to
the right-hand part and installs the pre/post hitlag callbacks
`ftMr_SpecialS_EnterHitlag` and `ftMr_SpecialS_ExitHitlag`.

`ftMr_SpecialS_Phys` and `ftMr_SpecialAirS_Phys` call `reflect`. When command
variable 1 becomes 1, `reflect` enables the cape reflection descriptor through
`ftColl_CreateReflectHit`; when it returns to 0, the reflecting flag is
cleared. `ftMr_SpecialS_Coll` and `ftMr_SpecialAirS_Coll` preserve the
ground/air phase and `collUpdateVars` republishes the reflection callbacks.
The Python `Cape.projectile_contact` gate mirrors the observable projectile
eligibility; cape object creation, hitlag pausing, and projectile velocity
handoff remain native host responsibilities.

## Tornado effects and command cues

Down special uses states 349 and 350 (`ftMr_MS_SpecialLw` and
`ftMr_MS_SpecialAirLw`). `ftMr_SpecialLw_Enter` and
`ftMr_SpecialAirLw_Enter` call `setCmdVar2` and `doStartMotion`, clearing
command variables 0 and 1, initializing grounded momentum, and spawning
effect ID `0x47C` through `efSync_Spawn`. The effect is paused and resumed by
`efLib_PauseAll`/`efLib_ResumeAll` around hitlag.

`ftMr_SpecialAirLw_Anim` consumes command variable 1 once, clears it, and
sets `x2234_tornadoCharge`. While the charge is not set, the aerial physics
callback checks command variable 2 and held B to apply the tap ascent. Ground
and air collision callbacks use the source ECB, convert between states while
preserving the animation frame, and maintain the rotation callback state.
The script layer can reset and consume the command cues; Tornado momentum,
effect lifetime, hitboxes, rotation, and landing lag require native physics
and effect hosts.

## Test vector

For a grounded Mario with a fresh B press, a complete neutral resource, and a
B0 animation event:

1. Enter state 343 and clear command slot 0 before the event.
2. Spawn one `ArticleId.MARIO_FIRE` at the fighter's L1stNb position with the
   current facing direction.
3. With item speed `2.0` and angle `30°`, the item starts at approximately
   `(1.732, 1.000)` in facing-relative XY velocity.
4. For Cape state 345, command slot 1 equal to 1 reflects an eligible
   projectile contact; command slot 1 equal to 0 clears reflection.
5. For aerial Tornado state 350, command slot 1 equal to 1 is consumed once
   and becomes 0 while arming the native tornado-charge branch.

The current declaration exports fighter states 343–350, the Mario fire
article ID, B0 launch, Cape reflection gate, and Tornado command consumption.
The native resource boundary now exposes `kind: "mario_fireball"` for article
48. Its launch, lifetime, gravity, terrain stop threshold, hitbox, and contact
values must be supplied by an authored resource export; the port does not
invent values that are absent from the pinned source or a verified data pack.
The immutable cache links that descriptor to the native projectile step and
the B0 queue drains it after fighter callbacks.

The source callback coverage still has explicit limits. `DmgDealt`,
`Clanked`, `HitShield`, and `Absorbed` retain the shared projectile result
path; `Reflected` transfers ownership and `ShieldBounced` mirrors velocity.
Terrain contact now uses Mario's source rule: below the authored stop speed it
despawns, otherwise it emits native effect 1147 and continues without the
generic gravity bounce. Cape remains native-host owned, as do
`efSync_Spawn(0x47C)`, hitlag effect pause/resume, Tornado physics, and all
native article/effect cleanup callbacks.
