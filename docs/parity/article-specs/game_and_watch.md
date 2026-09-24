# Mr. Game & Watch article parity

This is the source contract for the fighter owned by
`scripts/fighters/game_and_watch.py`. The pinned source is
`/mnt/shared/Projects/Code/External/melee`, at the revision used by the
parity effort.

## Article IDs

The native article table is initialized in
`src/melee/ft/kinds/ftGameWatch/ftgamewatch.c:542-551`. The corresponding
numeric IDs are:

| ID | Source kind | Use |
|---:|---|---|
| 114 | `It_Kind_GameWatch_Greenhouse` | forward tilt greenhouse spray |
| 115 | `It_Kind_GameWatch_Manhole` | down tilt manhole |
| 116 | `It_Kind_GameWatch_Fire` | forward smash Fire |
| 117 | `It_Kind_GameWatch_Parachute` | neutral aerial parachute |
| 118 | `It_Kind_GameWatch_Turtle` | back aerial turtle |
| 119 | `It_Kind_GameWatch_Breath` | up aerial breath |
| 120 | `It_Kind_GameWatch_Judge` | Side-B Judge |
| 121 | `It_Kind_GameWatch_Panic` | Down-B Oil Panic bucket |
| 122 | `It_Kind_GameWatch_Chef` | Neutral-B food |
| 124 | `It_Kind_GameWatch_Rescue` | Up-B Fire Rescue trampoline |

ID 123 is unused by the Game & Watch table. Article resources must preserve
these numeric identities; names are presentation metadata only.

## Special attributes

The native layout is `ftGameWatchAttributes` in
`src/melee/ft/kinds/ftGameWatch/types.h:40-110`.

| Source field | Offset | Meaning |
|---|---:|---|
| `x18_GAMEWATCH_CHEF_LOOPFRAME` | `0x18` | Chef loop command frame |
| `x1C_GAMEWATCH_CHEF_MAX` | `0x1c` | maximum sausages per use |
| `x34_GAMEWATCH_JUDGE_ROLL[9]` | `0x34` | Judge weights; signed 32-bit values |
| `x60_GAMEWATCH_RESCUE_LANDING` | `0x60` | Rescue landing/freefall branch |
| `x64_GAMEWATCH_PANIC_MOMENTUM_PRESERVE` | `0x64` | Oil Panic aerial momentum divisor |
| `x74_GAMEWATCH_PANIC_DAMAGE_ADD` | `0x74` | Oil Panic damage addition |
| `x78_GAMEWATCH_PANIC_DAMAGE_MUL` | `0x78` | Oil Panic damage multiplier |
| `x7C_GAMEWATCH_PANIC_TURN_FRAMES` | `0x7c` | Oil Panic turnaround duration |
| `x80_GAMEWATCH_PANIC_ABSORPTION` | `0x80` | Oil Panic absorb descriptor |

The remaining Judge fields at `0x20-0x30` and Rescue fields at `0x58-0x5c`
control momentum, friction, launch, and stick angle. They must remain typed
special attributes rather than ordinary movement parameters.

## Fighter and article lifecycles

### Chef

`ftgamewatchspecialn.c` owns setup and looping. `ftGw_SpecialN_CreateSausage`
creates article 122 and increments the per-use sausage count. The animation
callback (`ftGameWatch_SpecialN_ChefLoop`, lines 125-168) loops only when the
command variable is set, the count is below `x1C`, and looping has not been
disabled. `ftGw_SpecialN_IASA` (lines 187-214) disables looping when B is no
longer held and accepts another B press after the command frame.

### Judge

`ftgamewatchspecials.c:29-106` creates/removes article 120 and forwards
hitlag to it. `ftGw_SpecialS_GetRandomInt` (lines 120-164) excludes the last
two Judge rows, accumulates the nine nonnegative weights, and selects using a
native random integer. The selected row is stored as the newest previous row;
the older row shifts out. Row 7 (`x222C_judgeVar1 == 6`) additionally creates
the Judge trampoline effect. Ground and aerial states are 355-363 and
364-372.

### Oil Panic

`ftgamewatchspeciallw.c:25-101` creates/removes article 121 and forwards
damage callbacks and hitlag. The absorb path accumulates hit count and damage
at lines 545-551. Full charge is enum value 3
(`ftGw_Panic_Full`, `forward.h:150-155`) and forces the release branch.
Release damage is calculated in `ftGw_SpecialLwShoot_ReleaseOil` and its aerial
variant (lines 665-723):

```text
damage = accumulated_damage * x78_GAMEWATCH_PANIC_DAMAGE_MUL
damage += x74_GAMEWATCH_PANIC_DAMAGE_ADD
charge = 0
accumulated_damage = 0
```

The ground and aerial shoot states apply that cached damage to enabled hit
capsules in their animation callbacks (lines 565-603). Ground and aerial
states are 375-380, with catch and shoot sub-states included.

### Fire Rescue

`ftgamewatchspecialhi.c:32-112` creates article 124 through
`ftGw_SpecialHi_ItemRescueSetup`, forwards hitlag with
`ftGw_SpecialHi_ItemRescueEnterHitlag` and
`ftGw_SpecialHi_ItemRescueExitHitlag`, and removes it with
`ftGw_SpecialHi_ItemRescueRemove` / `ftGw_SpecialHi_ItemCheckRescueRemove`.
The ground and aerial animation, IASA, physics, and collision callbacks are
the `ftGw_SpecialHi_*` functions at lines 124-258. Rescue's landing attribute
is consumed by the landing animation branch; zero selects ordinary fall,
while a positive value selects the special-fall/landing-lag path.

The parachute article 117 is created by
`ftGw_AttackAirN_ItemParachuteSetup` in
`ftgamewatchattackair.c:24-108`, then finalized by
`ftGw_AttackAirN_ItemParachuteOnLand`,
`ftGw_AttackAirN_ItemParachuteSetFlag`, and
`ftGw_AttackAirN_ItemParachuteRemove`. It is separate from Up-B Rescue and
must not be substituted for article 124.

## Conformance vector

Use a Judge resource with weights `[1,2,3,4,5,6,7,8,9]`, previous rows
`[0, 1]`, and native roll `0`. Rows 1 and 2 are excluded, leaving total 42;
roll 0 selects row 3. The fighter must enter grounded state 357 (or aerial
state 366), create article 120 with Judge value 3, and carry article hitlag
through the fighter's hitlag callbacks. A second roll must exclude rows 2 and
3 after the previous-row shift.

## Host gaps

The fighter script mirrors the source-local Chef loop-disable flag, Judge
previous-row shift, and full Oil Panic release calculation when the host
supplies the corresponding typed inputs and attributes. The authoring layer
still does not own the native article object lifecycle, bone attachment,
article hitlag forwarding, absorb callbacks, command-variable traces, or
article-specific hitbox callbacks. Those gaps block full Chef food, Judge,
Oil Panic, Rescue, and Parachute behavioral parity without fabricating native
effects; cached Oil Panic damage remains fighter state until the article
bridge consumes it.
