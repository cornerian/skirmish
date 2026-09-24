# Pichu electric article contract

This is the source contract for Pichu's electric articles.  Pichu reuses the
Pikachu special callbacks, but its fighter initialization supplies distinct
article kinds and its neutral special callback selects Pichu's sound effect.
This document records the pinned decomp behavior and the current host boundary.

The authoritative files are:

- `../../../../External/melee/src/melee/ft/kinds/ftPichu/ftpichu.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPichu/types.h`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachu.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspecialn.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspeciallw.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltground.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltair.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachuthunder.c`

## Article kinds and initialization

The item enum in `melee/it/forward.h` assigns these stable numeric kinds:

| ID | Source kind | Pichu use |
|---:|---|---|
| `0x52` | `It_Kind_Pichu_Thunder` | Down special Thunder segments |
| `0x5B` | `It_Kind_Pichu_TJolt_Ground` | Ground neutral special |
| `0x5C` | `It_Kind_Pichu_TJolt_Air` | Aerial neutral special |

`ftPc_Init_OnLoad` sets `can_walljump = true`, calls
`ftPk_Init_OnLoadForPichu`, and registers its fighter item archive entries from
`ftPichuAttributes.xDC`, `.x14`, and `.x18`.  The shared Pikachu attribute
layout supplies `specialn_itkind` and `specialairn_itkind` to the neutral
special callbacks.  The portable Pichu parameters preserve the three source
archive identities (`PICHU_THUNDER`, `PICHU_TJOLT_GROUND`, and
`PICHU_TJOLT_AIR`) while leaving article construction to the native host.
`ftPk_Init_OnLoadForPichu` preserves the shared Pikachu special attribute
layout while leaving the Pichu article archive entries distinct.

## Neutral special emission

`ftPk_SpecialN_Anim` and `ftPk_SpecialAirN_Anim` in
`ftpikachuspecialn.c` consume command variable 0 once, guard emission with
command variable 1, compute the owner-scaled spawn point from the relevant
`special[n]spawn_offset`, and call
`itPikachuThunderJolt_Spawn(owner, position, facing, specialn_itkind)`.
For Pichu the callback then plays sound ID `230067`; Pikachu uses `240076`.
The command-0 edge is the grounded and aerial spawn trigger; command 1 is the
one-spawn latch and is represented by `neutral_spawn_command = 0` in the
portable parameters.
The ground and air callbacks end in the source's normal motion completion or
landing-lag transition.

The Jolt article initializes its owner links and effect state in
`itPikachutjoltground.c` / `itpikachutjoltair.c`, follows the source surface
collision and animation callbacks, and destroys its linked effect state through
the item callback.  Thunder uses the multi-segment lifecycle in
`itpikachuthunder.c`; its owner destruction callback restores the source
special state when the first segment ends.

## Exact deterministic test vector

Invoke `ftPk_SpecialN_Anim` for a Pichu with state 341 (`SpecialN`), facing
`+1`, scale `y = 1`, position `(10, 20, 0)`, and these fixture attributes:

```text
specialn_spawn_offset = (2, 3)
specialn_itkind       = 0x5B  (It_Kind_Pichu_TJolt_Ground)
cmd_vars[0]           = true
cmd_vars[1]           = false
```

The callback must consume `cmd_vars[0]`, set `cmd_vars[1] = true`, request one
ground Jolt article owned by the Pichu at `(12, 23, 0)`, pass facing `+1`, and
play sound `230067`.  A second callback with `cmd_vars[1] = true` must not
spawn another article.

## Self-damage and host gaps

The pinned Pichu fighter and electric article callbacks above do not expose a
separate Pichu owner callback that applies self-damage.  The item spawn and
collision code is source-backed, but this audit does not infer a damage amount
or invent an owner self-damage trigger from article data.  The portable host
currently lacks article archive objects, owner-linked item lifecycle, native
effect cleanup, article ECB/surface state, Thunder segment ownership, and
article collision callbacks.  Those seams block full Pichu Jolt and Thunder
parity; the existing fighter script can represent the source motion phases,
Pichu sound identity, and archive-kind metadata only.
