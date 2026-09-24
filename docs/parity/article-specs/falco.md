# Falco article contract

This is the source contract for Falco's two character-owned articles.  The
source reference is the pinned melee decomp revision `0bac93a5` under
`External/melee`.  Fox and Falco use the same fighter callbacks for these
moves, but their article kinds and exported laser data remain distinct.

## Article IDs and registration

The adjacent entries in `src/melee/it/forward.h:181-184` are the authoritative
kind order and numeric IDs:

| ID | Kind | Owner/use |
|---:|---|---|
| 54 | `It_Kind_Fox_Laser` | Fox neutral special |
| 55 | `It_Kind_Falco_Laser` | Falco neutral special |
| 56 | `It_Kind_Fox_Illusion` | Fox grounded/aerial side special |
| 57 | `It_Kind_Falco_Phantasm` | Falco grounded/aerial side special |

`ftFc_Init_OnLoad` in `src/melee/ft/kinds/ftFalco/ftfalco.c:468-484`
registers Falco's two laser kinds from `dat_attrs[7]` and `dat_attrs[8]`, and
registers kind 57 in item slot 3.  The shared
`ftFx_Init_OnLoadForFalco` (`src/melee/ft/kinds/ftFox/ftfox.c:481-483`)
loads the Fox/Falco special-attribute layout.  Fox's own `ftFx_Init_OnLoad`
registers Illusion in slot 2 (`ftfox.c:485-500`); Falco's explicit slot-3
registration is why Phantasm must not be aliased to Fox Illusion.

## Laser data and callbacks

The exported article values used by the native script adapter are:

| Field | Fox laser | Falco laser |
|---|---:|---:|
| launch speed | 7.0 | 5.0 |
| lifetime | 35 frames | 100 frames |
| damage | 3 | 3 |
| hitbox growth | 0 | 100 |
| hitbox fixed | 0 | 5 |
| hitbox base | 0 | 0 |
| angle | 0 | 0 |

Falco's four hitbox rows preserve the shared laser shape, with centers
`(-0.7812, -3.6442978, -6.5073957, -9.3744)` and radii
`(1.1718, 1.1718, 1.1718, 1.1718)`.  Fox's final center is `-14.0616` and
radius `1.5624`; this is the meaningful shape difference in addition to the
speed/lifetime/knockback fields above.  The source item logic table in
`src/melee/it/it_3F2F.c:330-363` gives Fox and Falco laser the same state and
callback functions.  Those callbacks are implemented in
`src/melee/it/kinds/itfoxlaser.c` (`itFoxLaser_Logic94_*`): clank, reflect,
absorb, shield-bounce, shield-hit, and event handling are shared.

The fighter-side fire path is also shared.  `ftFx_SpecialN_CreateBlasterShot`
and `ftFox_SpecialN_FireBlasterShot` in
`src/melee/ft/kinds/ftFox/ftfoxspecialn.c:172-230` consume command variable 2,
compute the launch position/angle from the loaded special attributes, create
the registered article kind, and only branch by fighter kind for the sound
effect.  The neutral motion table in `ftFalco/ftfalco.c:23-120` and its aerial
continuation at `:120-165` routes through the shared `ftFx_SpecialN*`
callbacks (states 341-346).

## Phantasm state and owner lifecycle

The Falco motion table uses the shared side-special callbacks in states
347-352 (`ftFalco/ftfalco.c:90-160`): grounded start/dash/end, then aerial
start/dash/end.  The exact article branch is in
`ftFox_SpecialS_CreateGhostItem` (`ftfoxspecials.c:247-265`): when command
variable 2 is 1 it is cleared, `it_8029CEB4` creates kind 57 at the fighter's
current position and facing, and a successful object is stored in
`fp->mv.fx.SpecialS.ghostGObj` while `fp->x2222_b2` is set.  Fox takes the
same branch with kind 56.  `ftFx_SpecialS_Anim` and
`ftFx_SpecialAirS_Anim` (`ftfoxspecials.c:271-294`) perform that creation each
frame after checking animation completion; completion enters the matching end
state.  `ftFx_SpecialAirSEnd_Anim` (`ftfoxspecials.c:477-486`) applies the
shared freefall and landing attributes, and `ftFx_SpecialAirSEnd_Coll`
(`ftfoxspecials.c:554-563`) converts a ground contact into special landing.

Thus ownership is fighter-rooted at spawn, position/facing are sampled from
the fighter, and the fighter retains the article handle for the move's
follow-up bookkeeping.  The item table (`it_3F2F.c:365-392`) routes Fox
Illusion and Falco Phantasm through the shared `itFoxIllusion_Logic14_*`
callbacks.  Their native item-object lifecycle, animation/effects, and
collision callbacks are separate from the fighter state callbacks even though
the implementation is shared.

## Exact conformance vector

Use the existing native adapter fixture with one Falco fighter and issue:

```text
fighter.spawn_article(
    article_id = 55,
    position   = (1.0, 2.0, 0.0),
    angle      = 0.0,
    speed      = 5.0,
)
```

After the staged article is committed and drained, the result must be a
`ProjectileKind::FalcoLaser` with lifetime `100.0`, speed `5.0`, and first
hitbox `(damage=3, growth=100, fixed=5, base=0)`.  The regression is
`src/game/script/projectile_tests.rs::typed_falco_laser_spawn_preserves_native_kind_and_exported_lifetime`.
It checks the identity and the Falco-specific lifetime, speed, and knockback
fields while leaving position/angle available for the ordinary ray step.

## Current host boundary

The portable host supports Falco laser identity and exported ray attributes;
the exact vector above is the maintained parity seam.  It does not yet expose
the native item archive/object handle, command-variable-driven Phantasm spawn,
`ghostGObj` ownership flag, item animation/effect state, or the shared
`itFoxIllusion_Logic14_*` lifecycle callbacks.  `scripts/fighters/falco.py`
therefore exposes Phantasm's source ID as metadata but does not claim to
simulate the native Phantasm item.  Completing that behavior requires an
article/item API seam rather than another fighter-only callback alias.
