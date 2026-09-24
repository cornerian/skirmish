# Mewtwo article parity specification

Pinned source: `../../External/melee/src/melee/`, primarily
`ft/kinds/ftMewtwo/ftmewtwospecialn.c`, `ftmewtwospecials.c`,
`ftmewtwospeciallw.c`, and the item implementations under
`it/kinds/itmewtwoshadowball.c` and `itmewtwodisable.c`.

## Article IDs and ownership

The `ItemKind` enum in `it/forward.h` assigns Mewtwo's two articles these
stable IDs:

| ID | Source kind | Owner | Spawn path |
|---:|---|---|---|
| `0x6E` (110) | `It_Kind_Mewtwo_Disable` | Mewtwo fighter | `ftMt_SpecialLw_CreateDisable` -> `itMewtwoDisable_Logic67_SpawnMewtwoDisable` |
| `0x70` (112) | `It_Kind_Mewtwo_ShadowBall` | Mewtwo fighter | `ftMewtwo_SpecialN_CreateHeldShadow` / `it_802C5000` |

Confusion has no article ID. Its capture is a common fighter grab object and
its reflection is a fighter collision descriptor, both created by the native
fighter code.

## Shadow Ball

### Fighter state and charge flow

The source motion states are 341–350: grounded and aerial Start, Loop,
LoopFull, Cancel, and End. `ftMewtwo_SpecialN_ChangeAction` and
`ftMewtwo_SpecialAirN_ChangeAction` reset command variables and release state;
the grounded entry clears vertical velocity and the aerial entry halves
vertical velocity. The Start animation's command variable 3 creates the held
article. Loop animation (`ftMt_SpecialNLoop_Anim` and
`ftMt_SpecialAirNLoop_Anim`) advances charge after the configured iteration
count and enters LoopFull at `x0_MEWTWO_SHADOWBALL_CHARGE_CYCLES`.

The fighter attributes are defined in `ft/kinds/ftMewtwo/types.h`:

| Field | Meaning |
|---|---|
| `x0_MEWTWO_SHADOWBALL_CHARGE_CYCLES` | maximum charge cycles |
| `x4_MEWTWO_SHADOWBALL_GROUND_RECOIL_X` | grounded release recoil |
| `x8_MEWTWO_SHADOWBALL_AIR_RECOIL_X` | aerial release recoil |
| `xC_MEWTWO_SHADOWBALL_CHARGE_ITERATIONS` | loop iterations per charge cycle |
| `x10_MEWTWO_SHADOWBALL_RELEASE_LAG` | initial release delay |
| `x14_MEWTWO_SHADOWBALL_LANDING_LAG` | aerial release landing lag |

Command variable 1 releases the held article. `ftMt_SpecialN_ReleaseShadowBall`
calls `it_802C53F0`, resets the fighter charge, clears the held pointer, and
applies ground or air recoil. Command variable 2 drives charge SFX through
`ftMt_SpecialN_PlayChargeSFX`; command variable 3 is the held-article spawn
marker. `ftMt_SpecialN_OnTakeDamage` removes an unfinished held ball.

### Article attributes and callbacks

`itMewtwoShadowball_DatAttrs` is declared in `it/itCommonItems.h`. The fields
used by the source are:

| Field | Use |
|---|---|
| `x0` | article lifetime for held, launched, and impact states |
| `x8` / `xC` | minimum and maximum launch speed, interpolated by charge |
| `x10` / `x14` | minimum and maximum hit damage, interpolated by charge |
| `x18` / `x1C` | minimum and maximum visual/hit scale, interpolated by charge |
| `x20` | wobble update interval |
| `x24` | impact hit scale multiplier |
| `x28` | impact animation lifetime |
| `x2C` | launched article speed/initial velocity |

`itMewtwoshadowball_UnkMotion0_Anim` keeps a held article attached and asks
`ftMt_SpecialN_CheckShadowBallRemove` and
`ftMt_SpecialN_CheckShadowBallCancel` whether fighter state requires removal.
`it_802C53F0` converts charge into speed, damage, scale, and launch velocity.
Launched motion callbacks (`UnkMotion8`/`UnkMotion9`) update movement, stop on
wall/floor/ceiling collision through `it_802C5E5C`, and preserve charge-scaled
impact data. `it_2725_Logic101_Destroyed` clears the fighter's article pointer
when the owner still matches. Damage, clank, absorb, shield-hit, and shield
bounce callbacks are the `itMewtwoShadowball_Logic101_*` functions in
`itmewtwoshadowball.c`; reflection reverses the launch angle in
`it_2725_Logic101_Reflected`.

## Confusion reflect and capture

Ground and aerial Confusion are states 351 and 352. Entry resets command
variables 0 and 1 and the one-shot aerial boost. Command variable 0 is
consumed by `ftMewtwo_SetGrabVictim`: if a victim exists, the common grab
attachment `ftCo_800DE2A8` runs and the victim is released from its prior
state with `ftCo_80090780`.

Command variable 1 is consumed by `ftMt_SpecialS_ReflectThink`:

* value 1 calls `ftColl_CreateReflectHit` with
  `x1C_MEWTWO_CONFUSION_REFLECTION`, sets the fighter reflecting flag, and
  installs `ftMt_SpecialS_OnReflect`;
* value 2 removes reflection and clears the callback;
* value 0 does nothing.

The current script can expose command markers and a portable reflecting flag,
but capture attachment, victim state, reflect hitbox lifetime, and the native
`ReflectDesc` data remain host-owned.

## Disable

Ground and aerial Disable are states 359 and 360. Command variable 0 invokes
`ftMt_SpecialLw_CreateDisable`, which computes the spawn point from the
`L3rdNb` bone plus facing-scaled `x80_MEWTWO_DISABLE_OFFSET_X` and
`x84_MEWTWO_DISABLE_OFFSET_Y`. The item spawn path stores the owner, applies
the fighter scale to `x_vel`, starts its `lifetime`, and attaches the article
to the owner relationship.

Disable article attributes are defined by `itMDisableAttributes` and consumed
in `it_802C4B38`: `x_vel` is horizontal launch speed and `lifetime` is the
article lifetime. `itMewtwodisable_UnkMotion0_Coll` reports wall and ceiling
collisions after common item collision processing. Damage dealt, clank,
shield hit, absorb, and shield bounce callbacks all return true; reflection
delegates to `it_80273030`. `itMewtwoDisable_Logic67_Destroyed` clears the
fighter's owner pointer, while `itMewtwoDisable_Logic67_EvtUnk` removes owner
interaction references. Fighter damage/death paths call
`ftMt_SpecialLw_RemoveDisable`.

## Conformance test vector

Use Shadow Ball article attributes `max_charge = 4`, `x8 = 2`, `xC = 6`,
`x10 = 8`, `x14 = 20`, `x18 = 0.5`, and `x1C = 1.5`:

1. Start with command variable 3 on grounded state 341; article `0x7F` is
   created and remains owner-attached.
2. At charge 2, release with command variable 1. `it_802C53F0` must derive
   speed `4`, damage `14`, and scale `1.0` by linear interpolation.
3. The fighter's held pointer is cleared, charge resets to zero, and ground
   recoil is applied once.
4. On a wall collision, the article enters its impact state and retains the
   charge-scaled hit data until its impact lifetime expires.

## Host gap

The authoring layer currently models Mewtwo's motion graph, command markers,
reflecting flag, capture marker, and article-fired flags. Full parity still
requires native article resources and an object bridge for bone attachment,
hitboxes, interpolation attributes, owner cleanup, collision callbacks,
victim capture, and Confusion's `ReflectDesc`. Those are intentionally not
reimplemented as Python-only effects.
