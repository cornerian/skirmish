# Roy (`ftEmblem` / `ftMars`) parity notes

Source anchor: `../../External/melee/src/melee/ft/kinds/ftMars/`.
Roy is the emblem fighter (`FTKIND_EMBLEM`) using the Mars callback family;
`ftMs_Init_OnLoadForRoy` in `ftmars.c` loads the Roy data while the special
callbacks remain in `ftmarsspecial*.c`.

## Flare Blade (neutral special)

The motion table in `ftmars.c` assigns states 341–348 to the charge, loop, and
uncharged/fully charged end phases:

| source state | phase | behavior |
| ---: | --- | --- |
| 341 / 345 | ground / air start | enters the charge loop |
| 342 / 346 | ground / air loop | holds B; release selects End0 |
| 343 / 347 | ground / air End0 | uncharged release |
| 344 / 348 | ground / air End1 | fully charged release |

`ftMs_SpecialNLoop_Anim` (`ftmarsspecialn.c:148–169`) raises command variable
0 when `cur_frame > da->x0 * 30`. `ftMs_SpecialN_80137354` and
`ftMs_SpecialN_801373B8` (`:356–379`) select End1 when that variable is set;
otherwise they select End0. End animation applies the moving sword hitbox in
`ftMs_SpecialNEnd_Anim` (`:250–280`): each enabled capsule receives damage
`da->x4 + cur_frame / 30 * da->x8`.

The Python declaration exposes all eight states, looping metadata, release
transition, and the command 0 full-charge branch. The native host still owns
the charge timer, capsule geometry, effects, and Mars attribute values
(`x0`, `x4`, `x8`).

## Dancing Blade (side special)

States 349–366 are the four-stage ground/air tree. The IASA callbacks in
`ftmarsspecials.c` (`ftMs_SpecialS2_IASA`, `ftMs_SpecialS3_IASA`,
`ftMs_SpecialS4_IASA`)
require a later A+B press after command variable 0 is armed. The stick chooses
up/down at stage 1 and up/neutral/down at stages 2 and 3. The final stage has
no further IASA branch. The Python declaration mirrors this tree and requires
both A and B for phase selection.

## Blazer (up special)

States 367 and 368 are ground and air Blazer. `ftMs_SpecialHi_Anim` and
`ftMs_SpecialAirHi_Anim` in `ftmarsspecialhi.c` enter fall-special with Mars
attributes `x28` (air speed multiplier) and `x2c` (landing lag). The Python
callback exposes the same host-owned fall-special handoff; hitbox and launch
physics remain data/native responsibilities.

## Counter and articles

Counter uses states 369–372. `ftMs_SpecialLw_Anim`/`ftMs_SpecialAirLw_Anim`
arm the native shield descriptor at `da->x64`; command variable 1 selects the
hit states 370/372. `ftMars_SpecialLwHit_ApplyDamage` applies accumulated
counter damage to enabled capsules, and `ftMs_SpecialLw_80139140_inline`
spawns the emblem counter effect (effect 1296). This is contact/shield
behavior, not a projectile article.

Roy has no fighter-owned projectile article in these special callbacks. The
only article-like work is the native counter shield/effect path above.

## Test vector

Start grounded in state 342 with command tuple `(0, 0, 0, 0)` and deliver a
command-trace event `(index=0, value=1)`: the expected destination is state
344. Start grounded in state 349 with both A and B newly pressed and a positive
vertical stick: the first event arms command variable 1; after command variable
0 is set, the next A+B event selects state 350. A-only or B-only input must
leave state 349 unchanged.

## Host gap

The script layer does not own Roy's `PlMs.dat` Mars attributes, sword capsule
geometry, effect archives, or counter shield descriptor. Those values must be
provided by the native resource host for numerical and visual parity.
