# Kirby neutral special and copied ability article

This is the source contract for Kirby's Inhale, capture, spit, and Copy
article. The pinned source is
`../../External/melee/src/melee/ft/kinds/ftKirby/`; function names below are
the decomp symbols and line references are from that checkout.

## State and object lifecycle

The motion IDs are part of Kirby's public fighter state table. The current
fighter script mirrors these IDs (`scripts/fighters/kirby.py`):

| IDs | Source phase | Meaning |
| --- | --- | --- |
| 353--355 | `SpecialN`, `SpecialNLoop`, `SpecialNEnd` | Inhale startup, held loop, release |
| 356--357 | `SpecialNCapture`, `SpecialNCaptureWait` | fighter capture and capture hold |
| 358--359 | `Eat`, `EatWait` | swallow entry and decision state |
| 360--366 | `EatWalkSlow` .. `EatLanding` | movement while holding a target |
| 367--370 | `SpecialNDrink0/End`, `SpecialNSpit0/End` | ground item consume and item spit |
| 371--377 | `SpecialAirN` .. `SpecialAirNEatFall` | airborne Inhale/capture |
| 378--381 | `SpecialAirNDrink/End`, `SpecialAirNSpit/End` | airborne consume and spit |
| 382 | `SpecialAirNEatTurn` | turn while holding a target |

Surface collision keeps these source phase pairs aligned by motion identity:

| Ground phase | Air phase | Source collision handoff |
| --- | --- | --- |
| 356 `SpecialNCapture0` | 375 `SpecialAirNCapture1` | `ftKb_SpecialNCapture0_Coll` / `ftKb_SpecialAirNCaptured_Coll` |
| 357 `SpecialNCapture1` | 374 `SpecialAirNCapture0` | `ftKb_SpecialNCapture1_Coll` / `ftKb_SpecialAirNCapture_Coll` |
| 358 `Eat` | 376 `EatAir` | `ftKb_Eat_Coll` / `ftKb_SpecialAirNCaptured_Coll` |
| 359 `EatWait` | 377 `EatFall` | native capture wait collision callbacks |
| 367 `SpecialNDrink0` | 379 `SpecialAirNDrink1` | `ftKb_SpecialNDrink0_Coll` / `ftKb_SpecialNDrink1_Coll` |
| 368 `SpecialNDrink1` | 378 `SpecialAirNDrink0` | `ftKb_SpecialNDrink_Coll` / `ftKb_SpecialAirNDrink_Coll` |
| 369 `SpecialNSpit0` | 381 `SpecialAirNSpit1` | `ftKb_SpecialNSpit0_Coll` / `ftKb_SpecialAirNSpit1_Coll` |
| 370 `SpecialNSpit1` | 380 `SpecialAirNSpit0` | `ftKb_SpecialNSpit1_Coll` / `ftKb_SpecialAirNSpit_Coll` |
| 363 `EatTurn` | 382 `EatTurnAir` | `ftKb_EatTurn_Coll` / `ftKb_SpecialAirNCaptureTurn_Coll` |

Capture attaches one of two target types to Kirby. Fighter capture uses
`victim_gobj`; item capture uses `target_item_gobj` and sets the internal item
branch flag (`u.kb.xF4_b0`). `ftKb_Eat_Anim` and
`ftKb_SpecialAirNCaptured_Anim` (ftkirbyspecialn.c:852-880) enter `EatWait` or
`EatFall` after the capture animation. Their completion callbacks preserve the
target until an explicit consume or spit command.

`ftKb_EatWait_IASA` (ftkirbyspecialn.c:1260-1310) is the complete decision
order:

1. With an item target, a fresh B press or down stick enters motion 368/379
   (consume); a fresh A press enters 370/381 (spit).
2. With a fighter target, the same B/down input enters motion 367/378
   (Copy/consume); a fresh A press enters 369/380 (spit).
3. If no target action fired, an opposing stick direction enters turn 363/382,
   then jump and walk inputs are considered.

The animation callbacks are the ownership boundaries. Item consume calls
`it_802F28C8(item, 0, 0)` in `ftKb_SpecialNDrink0_Anim` and
`ftKb_SpecialNDrink1_Anim` (ftkirbyspecialn.c:990-1005, 1036-1055), then clears
both item references. Fighter consume first detaches the victim with
`ftCo_800DE2CC` and `ftCo_800BE000`, derives the victim kind with
`ftCo_800BD9E0`, and calls `ftKb_SpecialN_800F1BAC(..., kind, true)`
(ftkirbyspecialn.c:1008-1033, 1058-1080). Fighter spit detaches with
`ftCo_800DE2CC`, releases with `ftCo_800BDB58`, and invokes
`ftColl_8007B8CC` (ftkirbyspecialn.c:882-894).

## Spit and star values

The ground item spit path (`ftKb_SpecialNSpit0_Anim` and
`ftKb_SpecialNSpit1_Anim`, ftkirbyspecialn.c:896-981) creates the star/article
using the target item position and these attributes:

* `specialn_ground_spit_initial_horizontal_velocity` sets the x velocity;
* `specialn_spit_deceleration_rate` is passed as the star's deceleration;
* `specialn_star_base_duration` is passed as its base lifetime;
* y and z velocity start at zero.

The source helper `ftKb_SpecialN_800F58AC` (ftkirbyspecialn.c:65-75) is the
exact reusable form: for `victim_facing_dir`,
`velocity = (-victim_facing_dir * initial_horizontal_velocity, 0, 0)` and the
return value is `spit_deceleration_rate`.

Fighter spit uses the same release callback but does not create an item star.
The airborne release helper `ftKb_SpecialN_800F58D8`
(ftkirbyspecialn.c:77-91) returns `specialn_swallow_star_gravity` and sets
`x = facing_dir * swallow_star_vertical_velocity * cos(release_angle)`,
`y = swallow_star_vertical_velocity * sin(release_angle)`, `z = 0`. The angle
attribute is stored in radians (`types.h:120`).

### Exact helper test vector

For a source-level helper test, use

```text
victim_facing_dir = +1
ground_initial_horizontal_velocity = 2.0
spit_deceleration_rate = 0.125
```

`ftKb_SpecialN_800F58AC` must produce `velocity=(-2.0, 0.0, 0.0)` and return
`0.125`. For the airborne helper, use release angle `pi/6`, star vertical
velocity `4.0`, and gravity `0.2`; with `facing_dir=+1` it must produce
`(3.4641016, 2.0, 0.0)` and return `0.2` (within float tolerance). This vector
tests the source formulas without requiring an item or fighter object.

## Copied-special identity and dispatch

Copy is an object lifecycle, not just a replacement move ID. The copied fighter
kind is stored in `fp->u.kb.hat.kind`. `ftKb_SpecialN_800F1BAC`
(ftkirby.c:4049-4075) performs the lifecycle in this order:

1. If the kind changed, write the new kind.
2. Call `ftKb_SpecialN_800F190C` to install the copied data callbacks.
3. Call the per-kind initializer from `ftKb_Init_803C9CC8[kind * 2]`, when it
   exists, to create/reset the hat and copied article state.
4. Call `ftKb_SpecialN_800F16D0` to finish the copied setup.
5. Play the new-copy sound when requested; an unchanged kind plays the repeat
   sound instead.
6. Always install `ftKb_Init_800EE74C` and `ftKb_Init_800EE7B8` as the death
   callbacks that restore copied-special state.

`ftKb_SpecialN_800F19AC` and `ftKb_SpecialN_800F1A8C`
(ftkirby.c:3938-4047) dispatch the active copied special by the hat kind.
They cover Ice Climbers, Peach, Fox/Falco, Link/Young Link, Mewtwo, Ness,
Samus, Sheik, Yoshi, Donkey Kong, and Mr. Game & Watch, with kind-specific
callbacks and state differences for Mewtwo. The default branch intentionally
does nothing. The per-kind implementations live in the corresponding
`ftkirbyspecial*.c` files and own their article objects, callbacks, and
resource cleanup.

## Smallest host interface and current gap

The native host needs one reusable Kirby target/ability boundary:

```text
CaptureTarget = Fighter(player_id) | Item(item_id)
KirbyCopy = { kind, hat/article state, dispatch key, death cleanup callbacks }
```

It must expose `capture`, `consume_or_copy`, `spit`, and `release/break`, while
preserving the target relation until the animation callback consumes it. The
existing generic grab relation can represent a fighter attachment, but the
host has no item target object, Kirby copied-kind/hat state, per-kind special
dispatch table, or article lifetime callbacks. Consequently the Python script
can accurately express the motion IDs, surface phase pairs, release phases,
and exposed Stone/Hammer transitions, but cannot yet provide source parity for
the neutral special's target-dependent input decision order, victim
consumption, item/star creation, copied-special dispatch, or hat cleanup. The
script leaves those branches native-owned because they depend on target
objects, animation command variables, article creation, and copied-kind data.

The next native seam should be this target/ability interface; extending generic
grab with Kirby-specific item or copied-special logic would otherwise conflate
the source's fighter and item lifecycles. Stone's defensive temporary state is
also native-only (`ftkirbyspeciallw.c:362-555`).
