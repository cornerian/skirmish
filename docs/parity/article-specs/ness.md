# Ness article parity specification

Pinned source: `../../External/melee/src/melee/` at the workspace's locked
decomp revision.

Ness's fighter motion declarations expose the source states, but the
observable special behavior is split between `ftNess` callbacks and item
articles. The article IDs below are the `ItemKind` ordinals from
`it/forward.h` (Fox laser is 54, so Ness's sequence begins at 66).

## PK Fire and pillar

| Article | ID | Native role |
| --- | ---: | --- |
| `It_Kind_Ness_PKFire` | 66 | Moving PK Fire projectile |
| `It_Kind_Ness_PKFire_Flame` | 67 | Hit or clank spawned pillar |

`ftNs_SpecialS_ItemPKFireSpawn` in `ft/kinds/ftNess/ftnessspecials.c:21-75`
places the projectile at the right-hand bone plus
`x30_PKFIRE_SPAWN_X * facing` and `x34_PKFIRE_SPAWN_Y`. Ground and air use
separate launch angles and speeds (`x20`/`x24` and `x28`/`x2C`). The article
uses its own lifetime attribute and native ground collision
(`it/kinds/itnesspkfire.c:24-75`).

Damage and clank callbacks spawn the pillar one article-height offset above
the projectile (`itnesspkfire.c:77-97`). The pillar grows from its initial
scale to its final scale, has the source's accidental always-air physics and
collision assignment, and extends its life when damaged
(`itnesspkfirepillar.c:67-145`). Reflection transfers ownership; shield
bounce mirrors the visual rotation and velocity
(`itnesspkfire.c:99-124`). Grounded PK Fire falls when it loses the floor;
aerial PK Fire enters fall-special on contact with
`x38_PKFIRE_LANDING_LAG` (`ftnessspecials.c:146-165`).

## PK Flash and explosion

| Article | ID | Native role |
| --- | ---: | --- |
| `It_Kind_Ness_PKFlush` | 68 | Owner-linked charging article |
| `It_Kind_Ness_PKFlush_Explode` | 78 | One-shot explosion |

`it_802AA8C0` initializes ownership, charge, command variables, and the
article lifetime (`itnesspkflash.c:84-118`). During charge, the article grows
its hitbox and visual scale, polls whether Ness is still in either hold state,
and enters the explosion phase when B is released or the item command signals
release (`itnesspkflash.c:182-233`). While held, owner stick X accelerates
horizontal velocity with a cap; gravity and terminal fall speed use
`x18_FLASH_CONTROL`, `x20_FLASH_UNK2`, `x1C_FLASH_GRAVITY`, and
`x24_FLASH_UNK3` (`itnesspkflash.c:283-328`). Terrain contact also enters
explosion (`itnesspkflash.c:342-356`).

Near the end of the explosion delay, the charge article spawns article 78 and
passes its charge as state (`itnesspkflash.c:235-271`). The explosion computes
damage as `charge * x10_FLASH_EXPL_DAMAGE_MUL + xC_FLASH_EXPL_BASE_DAMAGE`,
scales its one hit capsule by charge, and has no movement
(`itnesspkflashexplode.c:68-145`). Reflection changes phase and facing
(`itnesspkflash.c:369-375`). Fighter attributes also include minimum charge,
gravity delay, falling acceleration, and aerial landing lag
(`ftNess/types.h:108-115`).

## PK Thunder ball and trails

| Article | IDs | Native role |
| --- | ---: | --- |
| `It_Kind_Ness_PKThunder` | 69 | Owner-linked controllable ball |
| `It_Kind_Ness_PKThunder1..4` | 70–73 | History-following trail segments |

`it_802AB58C` initializes a 16-sample position and angle history, lifetime,
owner pointer, and six trail slots (`itnesspkthunderball.c:110-161`). While
Ness remains in either hold state, stick input changes the ball angle using the
authored threshold and turn radius, with separate behavior above and below 45°
from the current velocity (`itnesspkthunderball.c:301-365`). Every animation
cycle emits up to six trail articles from the history
(`itnesspkthunderball.c:232-280`).

The ball is destroyed by terrain contact (`itnesspkthunderball.c:367-377`).
Damage, clank, absorption, reflection, and shield callbacks clear owner and
trail links; reflection reverses the angle and rebuilds the history
(`itnesspkthunderball.c:379-431`). Trail segments read historical positions
and radius from the ball and self-destruct when their link is cleared
(`itnesspkthundertrail.c:77-155`). Fighter-side distance checks and PK
Thunder 2 launch are in `ft/kinds/ftNess/ftnessspecialhi.c:116-249` and
`ftnessspecialhi.c:687-758`.

## PSI Magnet absorption

PSI Magnet has no separate projectile article ID. Its absorb capsule is
created in `ftNs_SpecialLwStart_Enter`, hold, turn, and hit transitions
(`ftnessspeciallw.c:37-82`, `416-445`, `625-726`, `855-889`). The absorb
callback `ftNs_AbsorbThink_DecideAction` subtracts
`int(damage_taken * x94_PSI_MAGNET_HEAL_MUL)` from Ness's percent, clamps at
zero, updates player stock HP, faces Ness toward the absorbed source, and
enters or retains the grounded/aerial hit state (`ftnessspeciallw.c:894-937`).
Release lag (`x74`), turnaround frames (`x78`), hit continuation window
(`x7C`), gravity delay (`x84`), momentum preservation (`x88`), fall
acceleration (`x8C`), and the absorb descriptor (`x98`) are fighter
attributes (`ftNess/types.h:152-167`).

## Exact test vector

Use a grounded PK Fire with `facing = -1`, bone position `[10, 20, 0]`,
`x30_PKFIRE_SPAWN_X = 1.5`, `x34_PKFIRE_SPAWN_Y = 2.0`, grounded launch angle
`30°`, and grounded velocity `4.0`:

1. Spawn position is `[8.5, 22.0, 0]`.
2. Initial velocity is `[-4*cos(30°), 4*sin(30°), 0]`, approximately
   `[-3.4641016, 2.0, 0]`.
3. On a damage callback with pillar offset `x4 = 0.75`, spawn the pillar at
   `[8.5, 22.75, 0]` with the same facing.

For PSI Magnet, with `damage_taken = 12` and `x94_PSI_MAGNET_HEAL_MUL = 0.5`,
the source subtracts `int(6.0) = 6` percent and clamps the result at zero.

## Current host gap

The host currently supports fixed ray/gravity projectiles, generic collision,
reflection, shield policy, and lifetime. Ness requires owner-linked state,
typed per-frame steering, article-child spawn commands carrying charge or
history state, absorb callbacks, and explicit detach/cleanup of linked
articles. A generic article VM is unnecessary; a small typed native behavior
set for PK Fire, PK Flash, PK Thunder, their children, and PSI Magnet absorb
would preserve the hot path and source ownership boundaries.
