# Marth special parity specification

Pinned source: `../../External/melee/src/melee/ft/kinds/ftMars/`.
Marth and Roy use the same `ftMars` callbacks; their fighter declarations bind
the shared motion table to the roster names.

## No native item articles

The four Marth specials do not create a native fighter article. Sword trails,
charge sparks, and Counter's shoulder effect are `efSync_Spawn` effects, not
`it_` article objects. A resource exporter must therefore preserve effect IDs
as presentation metadata and must not invent an article lifecycle for them.
Counter's effect callback selects effect 1265 for Marth and 1296 for the
Emblem kind in `ftmarsspeciallw.c:288-314`.

## Shield Breaker

The motion table states are 341–348:

| States | Role |
| --- | --- |
| 341 / 345 | Ground / air start |
| 342 / 346 | Ground / air charge loop |
| 343 / 347 | Ground / air released end |
| 344 / 348 | Ground / air fully charged end |

`ftMs_SpecialN_Enter` and its air variant select 341 or 345 and reset the
charge counter. Start animation completion enters the corresponding loop
(`ftmarsspecialn.c:30-80`, `236-249`). While looping, the source increments
`mv.ms.specialn.cur_frame`; reaching `x0 * 30` sets command variable 0 and
selects End1 (344 or 348). Releasing B selects End0 (343 or 347), while the
loop's animation callback also clears the charge hit state when command
variable 0 is still zero (`ftmarsspecialn.c:138-190`, `350-389`).

The active sword capsules are fighter hit capsules, not article hitboxes.
`inlineA0` applies `x4 + cur_frame / 30 * x8` damage to every enabled capsule
while the charge is incomplete and emits the charge effect at animation frame
9 (`ftmarsspecialn.c:250-275`). The source fields are:

- `MarsAttributes::x0` — charge duration in seconds;
- `x4` — starting capsule damage;
- `x8` — damage gained per 30-frame charge unit;
- `specialn_friction` and `specialn_start_friction` — entry and start physics.

Ground End phases return to Wait; air End phases return to Fall. Ground/air
collision preserves the matching End phase and frame (`ftmarsspecialn.c:326-378`).

## Dancing Blade

The source states are 349–366: start 349/358, second strikes 350–351 and
359–360, third strikes 352–354 and 361–363, and fourth strikes 355–357 and
364–366. Start animation completion returns to Wait/Fall if no command branch
fires. Each later strike follows the same source selector:

1. The first A+B press sets command variable 1.
2. After the animation command sets variable 0, a later A+B press consumes the
   latch and selects the next strike.
3. Ground and air use the matching state family. The first branch has only up
   and down; later branches use strict `y > p_ftCommonData->x21C`,
   `y < -p_ftCommonData->x21C`, or neutral selection.

The selectors and command reset are implemented by
`ftMs_SpecialS_80137A9C`, `_80137E0C`, and `_80138148` in
`ftmarsspecials.c:285-307`, `425-456`, and `560-590`. Ground/air callbacks
preserve the current animation frame while changing between paired phases
(`ftmarsspecials.c:238-279`, `377-421`, `510-556`). Native sword hitboxes and
their timing are carried by the motion state's collision data; the callbacks
manage phase selection, sword trail flags, and surface conversion rather than
constructing article objects.

## Dolphin Slash

States 367 and 368 share `ftMs_SpecialHi_Anim` and
`ftMs_SpecialAirHi_Anim`. Animation completion enters `FallSpecial` with
`allow_interrupt = false`, mobility `MarsAttributes::x28`, and landing lag
`x2C` (`ftmarsspecialhi.c:58-86`). During IASA, horizontal stick input above
`x34` updates the launch angle up to `x38`; the one-shot `throw_flags_b3` cue can
turn the fighter when the stick strictly exceeds `x30`, including after command
variable 0 has been consumed (`ftmarsspecialhi.c:90-139`). The travel physics
uses command variable 2 to switch from launch motion to gravity and air drift
(`ftmarsspecialhi.c:141-220`).

The Python callback consumes an exposed `fighter.throw_flags_b3` (or context
fallback) after applying that turn. The native host still owns the animation
command that raises the flag and the resulting model-part rotation.

## Counter

Counter starts in 369 or 371. Command variable 1 arms the shield descriptor,
then the native callback transitions to 370 or 372 and records the victim's
collision damage. Ground and air collision preserve the hit phase while
switching surfaces (`ftmarsspeciallw.c:33-107`, `155-196`, `256-285`).

The shield descriptor is `MarsAttributes::x64`; its shield values use `x60`.
The callback stores `x19A4 * x5C` in `mv.ms.speciallw.x0`, changes to the
matching hit state, and starts the effect callback (`ftmarsspeciallw.c:288-353`).
Hit-phase animation applies the stored damage to enabled fighter capsules only
for `FTKIND_EMBLEM` (`ftmarsspeciallw.c:198-237`). This is victim/contact
behavior, not a projectile article.

## Conformance vector

Use a Marth Counter fixture with ground state 369, command variable 1 changing
from 0 to 1, a shield descriptor hit that reports `x19A4 = 7`, and `x5C = 2`:

1. The command event changes command variable 1 to 1; the next animation
   callback consumes it, registers the shield descriptor, and remains in 369.
2. A contact callback stores damage 14 and enters state 370. The fighter
   remains in the ground hit phase until its animation ends.
3. A ground-to-air contact changes 370 to 372 while preserving the animation
   frame; the reverse contact changes 372 to 370.
4. With an enabled fighter capsule, the Emblem hit callback applies 14 damage;
   with no enabled capsule or a non-Emblem kind, it applies none.

## Host gaps

The current host can represent Marth's phase graph, atomic Dancing Blade
button chord, command-variable transitions, source landing-lag forwarding,
and surface pairing. It does not yet expose the source sword capsule table,
`ShieldDesc`/victim-contact callback ABI, Counter's `x19A4` collision result,
hitlag callback slots, or the native `MarsAttributes` layout. Those interfaces
block exact sword hitbox timing, Counter victim damage, and Dolphin Slash
steering/physics parity without fabricating resource fields or collision
state.
