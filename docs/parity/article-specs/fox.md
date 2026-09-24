# Fox article parity specification

Pinned source revision: `0bac93a5`. This page records the article-facing
parts of Fox's neutral, side, and down specials. Fighter phase behavior is
specified in [`docs/fox-neutral-special.md`](../../fox-neutral-special.md),
[`docs/fox-side-special.md`](../../fox-side-special.md), and
[`docs/fox-down-special.md`](../../fox-down-special.md).

## Blaster laser

The fired article is `It_Kind_Fox_Laser = 54`. The source path is
`ftFx_SpecialN_CreateBlasterShot` in `src/melee/ft/kinds/ftFox/ftfoxspecialn.c`
(`tests/oracle/original/ftfoxspecialn.c:194-217`), which calls
`it_8029C6A4` in `itfoxlaser.c`. Fox's exported Blaster attributes are:

| field | value | source use |
| --- | ---: | --- |
| launch angle | `0.0` radians | `ftFox_SpecialN_PrepareBlasterShot` |
| speed | `7.0` | same |
| landing lag | `0.0` | natural End-air exit |
| shot item kind | `54` | `x1C_FOX_BLASTER_SHOT_ITKIND` |
| article lifetime | `35` frames | `FoxLaserAttr[0]` |
| article max visual scale | `3.0` | `FoxLaserAttr[1]`, visual only |

`it_8029C504` normalizes the angle into `[0, 2π]`, computes the spawn point
from the owner's ECB midpoint, sets `scale = 0`, stores angle and speed, and
starts the article lifetime (`itfoxlaser.c:44-71`). Each animation callback
recomputes velocity as `speed * (cos(angle), sin(angle))`, sets facing from
the sign of horizontal velocity, and grows the visual scale
(`itfoxlaser.c:83-90`; `Item_UpdateRayAnimation`). Physics copies the item
position into the ray state (`itfoxlaser.c:92-96`). Stage contact arms a
one-frame expiry timer (`itfoxlaser.c:98-107`), while shield contact mirrors
velocity and recomputes the normalized angle (`itfoxlaser.c:132-136`).

Reflector contact uses `itFoxLaser_Logic94_Reflected` (`itfoxlaser.c:115-125`):
the laser facing snaps to the reflector's direction, its visual scale resets,
and its angle advances by π. `Item_80269F14` then transfers ownership and
scales each active hitbox as `trunc(hit.damage * xC6C + 0.99)`, capped by the
native global damage limit (`item.c:1613-1619`). Fox's laser callback does not
apply `Reflector.speed_mul`.

## Illusion ghost

`ftFox_SpecialS_CreateGhostItem` (`ftfoxspecials.c:247-266`) consumes
`cmd_vars[2] == 1`, clears that register, and calls `it_8029CEB4` with the
fighter position and facing. The exact article kinds are:

| fighter | article kind |
| --- | --- |
| Fox | `It_Kind_Fox_Illusion = 1` |
| Falco | `It_Kind_Falco_Phantasm = 2` |

When creation succeeds, the fighter stores the ghost object and sets the
ghost-present flag. `ftFox_SpecialS_SetVars` (`ftfoxspecials.c:420-438`)
initializes all four position and blend slots to the current fighter position
and installs the accessory GFX callback. The ghost is visual only:
`itfoxillusion.c`'s three collision callbacks return `false`, and its damage
callback does not create or deal a hit. It therefore has no gameplay hitbox,
damage, owner transfer, or projectile reflection behavior.

## Reflector contact

Reflector is a fighter reflect state, not a spawned Fox article. The source
sets `fp->reflecting` and `fp->reflect_hit_cb` through
`ftFox_SpecialLw_SetReflectVars` (`ftfoxspeciallw.c:595-600`), and creates the
reflect bubble with `ftFox_SpecialLwHit_CreateReflectInline`
(`ftfoxspeciallw.c:662-668`), which calls `ftColl_CreateReflectHit` using the
fighter's `ReflectDesc` attributes. The resource fields are `bone`, `offset`,
`size`, `max_damage`, `damage_mul`, `speed_mul`, and `behavior`.

If a Fox laser overlaps that bubble, the item callback performs the owner
transfer and π angle turn described above. The Rust host models the laser's
owner, angle, velocity reversal, hitbox damage scaling, and reflected event.
The global native damage cap is not sourced from the pinned runtime data and
remains a documented host gap.

## One source-backed test vector

For `owner = (10, 20)`, ECB top `4`, ECB bottom `-2`, angle `0`, speed `7`,
and lifetime attribute `35`, `it_8029C6A4` must produce:

```text
spawn position = (10, 21)
stored angle    = 0
stored speed    = 7
facing          = +1
lifetime        = 35
```

After a Reflector callback with reflector direction `-1`, the same article
must have facing `-1`, angle `π` (normalized), unchanged speed magnitude `7`,
and ownership transferred to the reflecting fighter. This vector is exercised
by `tests/fox_laser_differential.rs` (`compare_spawn` and
`compare_reflected`) against `tests/oracle/original/itfoxlaser.c`.

## Fire Fox Bound exit

Fire Fox's native Bound exit restores all jumps before entering FallSpecial,
both when the bound exit command marker fires and when the bound animation ends
normally (`ftFx_SpecialHiBound_Anim` in
`src/melee/ft/kinds/ftFox/ftfoxspecialhi.c`). The fighter hook mirrors that
restoration on both exits.

## Current host gaps

- The Illusion/Phantasm article's model, trailing four-slot GFX history, and
  accessory callback are intentionally not represented; its confirmed lack of
  hitboxes makes this gameplay-neutral.
- Blaster's cosmetic hand gun article and its model/SFX callbacks are omitted.
- Laser visual rotation and scale growth are not gameplay state; the host
  keeps collision-relevant position, angle, velocity, facing, lifetime, and
  hitboxes.
- The native global `it_804D6D28->xD8` reflected-damage cap is unavailable in
  the pinned runtime data, so the host applies the source arithmetic and
  documents the cap as unresolved.
- Reflector's native item collision helper is represented through the host
  projectile contact path; no separate generic item runtime exists.
