# Yoshi article parity specification

This is the source contract for Yoshi's article-backed specials. The pinned
reference is `../../External/melee` at the revision recorded by
`upstream.lock.json`. Fighter motion IDs below are the native Yoshi motion
table entries in `src/melee/ft/kinds/ftYoshi/ftyoshi.c`.

## Fighter phases and article ownership

| Special | Motion IDs | Article | Native owner/contact boundary |
| --- | --- | --- | --- |
| Egg Lay | 346–355 | Yoshi tongue, egg-lay egg | `ftyoshispecialn.c`: `ftYs_SpecialN_Enter`, `ftYs_SpecialAirN_Enter`, `inlineB0`, `ftYs_SpecialN2_0_Anim`; tongue attaches to the fighter and captures a victim; egg spawn is `it_802F2F34` |
| Egg Toss | 364–365 | Yoshi egg throw | `ftyoshispecialhi.c`: `ftYs_SpecialHi_Enter`, `ftYs_SpecialAirHi_Enter`, `fn_8012E110`; throw egg is spawned by `it_802B2A10` and released by `it_802B28C8` |
| Egg Roll | 356–363 | none | `ftyoshispecials.c`: `ftYs_SpecialS_Enter`/`ftYs_SpecialAirS_Enter` both enter motion 360; `ftYs_SpecialAirSStart_1_Coll` selects motion 356 after aerial start lands; loop IASA and collision callbacks own the remaining phases; velocity, wall bounce, scale, and hit capsule remain fighter-native |
| Ground Pound | 366–368 | Yoshi star pair | `ftyoshispeciallw.c`: `fn_8012E644` spawns two stars with `it_802B2FC8`; star owner is the pounder and item collision callbacks own destruction/reflect behavior |

Egg Lay's state selection is command and victim dependent. The source uses
`cmd_vars[0]`/`cmd_vars[1]` and either `target_item_gobj` or `victim_gobj`:
`ftyoshispecialn.c:changeMotionState`, `inlineB0`, and `inlineA1`. A generic
phase end must therefore not claim that every phase always advances to the
next phase; article availability and victim capture select the branch.

Egg Roll's grounded B entry also has a source-specific handoff. Both special
entry functions call `Fighter_ChangeMotionState(..., 0x168, ...)`, which is
motion 360 (`SpecialAirSStart_1`). Motion 356 (`SpecialAirSStart_0`) is chosen
only by `ftYs_SpecialAirSStart_1_Coll` when that start phase reaches ground.
The script therefore enters 360 for both ground and air input, then maps the
ground contact callback to 356; mapping ground input directly to 356 skips the
native start animation and command timing.

## Egg Lay and tongue

`it_802F2BFC` in `src/melee/it/kinds/ityoshitongue.c` attaches the tongue to
the Yoshi bone, sets both `atk_victim` and `grab_victim`, and initializes the
single tongue item state. `it_802F2CE0` detaches it, restores the victim bone
position, clears victim links, and destroys the tongue. The fighter-side
capture callbacks are `ftCo_800BBB8C` and `ftCo_800BBC88` in
`ftyoshispecialn.c`'s setup and victim branches.

An egg-lay egg is created by `it_802F2F34` in
`src/melee/it/kinds/ityoshiegglay.c`. It copies position, facing, velocity,
lifetime, and damage-time scaling into the item, then starts item state 0.
Item states 0 and 1 use the common motion callbacks; state 2 is the hidden
post-burst lifetime state. `it_27CF_Logic114_DmgReceived` scales the remaining
lifetime by received damage and `it_27CF_Logic114_EvtUnk` forwards the event to
the common item handler.

## Egg Toss

`it_802B2A10` in `ityoshieggthrow.c` creates the held egg with the fighter as
both parent references and attaches it to part `0x1f`. `fn_8012E110` releases
it only when the command variable is armed and the egg exists. The release
position and velocity are calculated by `ftYs_SpecialS_8012DF8C`; the source
uses facing, stick magnitude, `xFC`/`x100`, and throw offsets `x104`/`x108`.

The thrown egg has item states 0 (held/dormant), 1 (active falling), and 2
(burst). `it_802B2C38` performs the burst transition, effects, sound, and
life timer setup. Ground contact (`itYoshieggthrow_UnkMotion1_Coll`) and
clank (`it_2725_Logic43_Clanked`) both route to that burst transition unless
the egg is already in state 2. `it_802B2890` clears the owner when the fighter
abandons an unthrown egg.

## Ground Pound stars

`fn_8012E644` in `ftyoshispeciallw.c` samples `TransN` and spawns two stars:
the first at `(-speciallw_star_offset.x, +speciallw_star_offset.y)` with
direction `-1`, and the second at `(+speciallw_star_offset.x,
+speciallw_star_offset.y)` with direction `+1`. It then clears the accessory
callback so the pair is emitted once.

`it_802B2FC8` in `ityoshistar.c` assigns the pounder as owner, initializes
horizontal speed from `StarAttrs.speed * facing`, vertical speed from the
common item attribute, and starts item state 0. `itYoshistar_UnkMotion0_Phys`
applies gravity and `StarAttrs.accel`; `itYoshistar_UnkMotion0_Coll` invokes
the common item collision path. Damage, shield, clank, reflection, and event
callbacks are `it_802B309C`, `it_802B30C0`, `it_802B312C`, `it_802B314C`, and
`it_802B3348`.

## Exact article test vector

Use this host-independent Egg Lay spawn vector to validate the article
boundary before integrating resources:

```text
TransN                 = (10, 20, 0)
special bone offset    = (2, 3, 0)
facing                 = -1
Yoshi x10/x14 attrs    = (1.5, 0.75)
Yoshi x18/x24 attrs    = (0.5, 120)
Yoshi x44 attr         = 2.0

egg position           = (12, 23, 0)
egg velocity           = (1.5, 0.75, 0)
egg facing             = -1
egg lifetime           = 120
egg damage scale       = 0.5
egg secondary scalar   = x44 / x18 = 4.0
item state             = 0
owner                  = none (SpawnItem parent fields are zero)
```

This is the direct field mapping in `ftyoshispecialn.c:inlineB0` and
`ityoshiegglay.c:it_802F2F34`; it intentionally tests article construction,
not animation timing.

## Current host gap

The Python fighter package can expose Yoshi's 346–368 motion phases and
resource/directional gates, but the current host has no article API for
tongue attachment, victim capture, egg ownership/lifetime, egg burst
callbacks, or star spawning. It also cannot express the source command
variable branches that choose target-item versus victim paths. Until those
article and victim interfaces exist, Yoshi's phase graph is declarative
coverage; full behavioral parity requires native item ownership, contact,
damage, clank, reflection, and capture integration.
