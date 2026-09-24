# Young Link article parity specification

Pinned source revision: `0bac93a5`. Young Link is `ftCLink`, external ID
`21`; Link is external ID `6`. The fighter special rows are shared with
`ftLink` (344–359), but every article kind and every source action namespace
is distinct for CLink.

## Article identities and fighter entry

The pinned item enum in `src/melee/it/forward.h:185-192` assigns the following
IDs:

| Article | Young Link ID | Link ID | Fighter/source entry |
| --- | ---: | ---: | --- |
| Bomb | `59` (`It_Kind_CLink_Bomb`) | `58` | `ftLk_SpecialLw_Enter` / `ftLk_SpecialAirLw_Enter`, rows 358/359 |
| Boomerang | `61` (`It_Kind_CLink_Boomerang`) | `60` | `ftLk_SpecialS_Enter` / `ftLk_SpecialAirS_Enter`, rows 350/353 |
| Hookshot | `63` (`It_Kind_CLink_HShot`) | `62` | `ftLk` z-air and hookshot item path |
| Fire Arrow | `65` (`It_Kind_CLink_Arrow`) | `64` | `ftLk_SpecialN_Enter` / `ftLk_SpecialAirN_Enter`, rows 344/347 |

`ftCl_Init_OnLoad` in `src/melee/ft/kinds/ftCLink/ftclink.c:317-337`
registers CLink's item archives and calls `ftLk_Init_OnLoadForCLink`; this is
where the CLink article tables replace Link's article kinds. The CLink
appeal path also registers milk, but milk is outside the four Link-family
articles documented here.

## Fire Arrow

`ftLk_SpecialN_Enter` and `ftLk_SpecialAirN_Enter` create the held arrow and
bind it to the fighter. The source charge/release rows are 344–349 in
`ftCLink/ftclink.c:69-130`; animation and collision callbacks are the shared
`ftLk_SpecialNStart/Loop/End_*` functions in `ftlinkspecialn.c`.

The `itLinkArrowAttributes` record in `it/itCharItems.h` supplies the CLink
archive's launch, damage, shield, lifetime, and model fields (`x0`–`x20`),
plus hand joints (`x24`, `x28`). `it_802A83E0` in `itlinkarrow.c` creates the
owned item; `itLinkArrow_Logic98_DmgDealt` applies the archive's `x14` velocity
response, `itLinkArrow_Logic98_HitShield` enters the shield response using
`x18`, and `itLinkArrow_Logic98_Clanked`/`itLinkArrow_Logic98_Reflected`
perform native teardown or owner/velocity reversal. The owner is retained in
the item vars and cleared through the shared Link fighter callbacks.

## Boomerang

`it_802A013C` in `itlinkboomerang.c` creates the CLink boomerang with its
fighter owner and archive attributes. The `itLinkBoomerangAttributes` record
(`it/itCharItems.h`) contains motion/lifetime fields (`x0`–`x40`) and the two
model/animation bundles (`x44`–`x58`). `ftLk_SpecialS_Enter` selects the empty
rows 352/355 when `used_boomerang` is set; `ftLk_SpecialS1_Anim` and
`ftLk_SpecialS2_Anim` handle launch and return. `it_802A0534` updates the
return angle and velocity, while `itLinkBoomerang_Logic18_Destroyed` and
`itLinkBoomerang_Logic18_Absorbed` clear or transfer the native owner state.

The Python Young Link declaration preserves the empty-row choice and forwards
the command release. The article's trail animation, collision, absorption,
and owner transfer remain native callbacks.

## Bomb

`ftLk_SpecialLw_Enter` and `ftLk_SpecialAirLw_Enter` call `it_8029DD58` from
`itlinkbomb.c`. The CLink item kind is selected by the CLink item table, while
the `itLinkBombAttributes` archive record supplies `lifetime`, fuse threshold
`xC`, launch velocity fields `x14`/`x18`, and the post-hit/air fields `x1C`–`x30`.
At creation, `it_8029DD58` stores the fighter as owner, starts the configured
lifetime, and computes the pre-explosion animation offsets:

```text
x8 = (-2.8000002 / (lifetime - xC)) * scale
xC = (-0.15707964 / (lifetime - xC)) * scale
```

`itLinkBomb_Logic16_DmgReceived` applies the archive's damage threshold and
launch values; `itLinkBomb_Logic16_EnteredAir` enters the airborne item state;
`itLinkBomb_Logic16_Reflected`, `HitShield`, and `ShieldBounced` delegate to
the native reflection and bounce callbacks. The fighter-side script only
preserves the held-bomb reuse branch; item fuse, explosion, ownership, and
contact remain native.

## Hookshot

The CLink hookshot uses item ID `63` and the shared `itlinkhookshot.c` state
machine. `itLinkHookshotAttributes` (`it/itCharItems.h`) contains the chain
length, speed, reach, collision, damage, and joint fields (`x0`–`x60`), while
the item vars retain the chain endpoints, owner, current state callback, and
collision bookkeeping. `itLinkhookshot_UnkMotion0_Phys` through
`UnkMotion8_Phys` implement the chain phases; the collision helpers at
`it_802A5AE0`, `it_802A5E28`, and `it_802A6A78` resolve fighter/item contact.
The CLink owner is attached through the same `ftLk` z-air path but uses the
CLink item archive and hand/skeleton data.

## Exact deterministic test vector

For a CLink bomb spawned by `it_8029DD58` with `lifetime = 60`, fuse threshold
`xC = 10`, scale `1`, owner `Young Link (external ID 21)`, and facing `-1`,
the source must initialize:

```text
article kind = 59
owner        = Young Link
velocity     = (0, 0, 0)
fuse offset  = x8 = -0.056000004
spin offset  = xC = -0.0031415928
life timer   = 60
```

This vector is the direct arithmetic in `itlinkbomb.c:178-191`; the runtime
regression should additionally assert that the item owner is retained as the
fighter until native detachment.

## Current host gap

The authoring layer can preserve CLink's `Source.21` fighter phases, empty
boomerang selection, release forwarding, held-bomb reuse, and article IDs in
this specification. It does not yet expose CLink article descriptors or a
generic item owner/contact interface, so Fire Arrow spawning, boomerang trail
and absorption, bomb fuse/explosion/reflection, and hookshot chain attachment
must remain native-owned. CLink milk from `ftCl_AppealS_Anim` is an additional
native-only article path.
