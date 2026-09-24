# Pikachu Article and Special Effects Contract

This is a source backed contract for Pikachu's Thunder Jolt, Thunder, Quick
Attack, and Skull Bash behavior.  It records the pinned decomp seams that a
native article and effect host must expose; it does not claim that the
fighter script alone implements the item engine.

The authoritative pinned files are:

- `../../../../External/melee/src/melee/it/forward.h`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltground.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltground.h`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltair.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachutjoltair.h`
- `../../../../External/melee/src/melee/it/kinds/itpikachuthunder.c`
- `../../../../External/melee/src/melee/it/kinds/itpikachuthunder.h`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspecialn.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspecialhi.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspecials.c`
- `../../../../External/melee/src/melee/ft/kinds/ftPikachu/ftpikachuspeciallw.c`

## Article IDs and attributes

`ItemKind` starts at `0x00` (`It_Kind_Capsule`).  The relevant enum values are
therefore:

| Article | Symbol | ID |
| --- | --- | ---: |
| Pikachu Thunder | `It_Kind_Pikachu_Thunder` | `0x51` (81) |
| Pichu Thunder | `It_Kind_Pichu_Thunder` | `0x52` (82) |
| Pikachu Thunder Jolt, ground | `It_Kind_Pikachu_TJolt_Ground` | `0x59` (89) |
| Pikachu Thunder Jolt, air | `It_Kind_Pikachu_TJolt_Air` | `0x5A` (90) |

The adjacent Pichu and Kirby copy kinds are distinct IDs and must not be
silently substituted for Pikachu's archive entries.  `ftpikachu/types.h`
contains the fighter-owned `specialn_itkind` and `specialairn_itkind` fields;
`ftPk_SpecialN_Anim` and `ftPk_SpecialAirN_Anim` pass those fields to
`itPikachuThunderJolt_Spawn`.

The Thunder Jolt article attribute struct has four float slots (`x0`, `x4`,
`x8`, `xC`) in `itCharItems.h`.  Ground and air share the item variable
contract, but their state tables and archive IDs are separate.  Thunder has
three float attributes (`x0`, `x4`, `x8`) and per instance stores owner, chain
link, delay, velocity, and scale in `itPikachuthunder_ItemVars`.

## Thunder Jolt lifecycle and callbacks

`ftPk_SpecialN_Anim` and `ftPk_SpecialAirN_Anim` consume command variable 0
once, guard command variable 1 against duplicate emission, compute the
owner-scaled spawn offset, and call `itPikachuThunderJolt_Spawn` (ground or
air kind selected by the fighter attributes).  The spawn routine in
`itpikachutjoltground.c` assigns both parent references, performs initial
environment placement, initializes command variables and owner linkage, then
starts state 0.

Ground Jolt uses `it_803F7190[]`:

- state 0: `itPikachutjoltground_UnkMotion0_Anim`,
  `itPikachutjoltground_UnkMotion0_Phys`, and
  `itPikachutjoltground_UnkMotion0_Coll`;
- state 1: `itPikachutjoltground_UnkMotion1_Anim`,
  `itPikachutjoltground_UnkMotion1_Phys`, and
  `itPikachutjoltground_UnkMotion1_Coll`.

The article follows stage collision and can create/maintain a linked ground
effect in `it_802B3554`; destruction clears that link and destroys effects in
`it_2725_Logic106_Destroyed`.  The source exposes distinct damage, reflected,
clank, absorbed, shield hit, shield bounce, and event callbacks in the
`it_2725_Logic106_*` family declared by `itpikachutjoltground.h`.  Air Jolt
uses the one state in `it_803F71D8[]`; its owner and linked effect are managed
by `it_802B4224`, `it_802B43D0`, and `itPikachuTJoltAir_Logic107_EvtUnk`, with
the corresponding `it_2725_Logic107_*` contact callbacks.

## Thunder lifecycle and owner callback

`ftPk_SpecialLw_SpawnEffect` is the article owner callback.  It starts the
Thunder chain with `it_802B1DF8(owner, position, velocity, count, delay,
kind)`, using fighter attributes `xC0`, `xCC`, `xD0`, `xD4`, `xD8`, and `xDC`,
and emits fighter effect `1219` at the configured effect position.  The
Thunder article state table `it_803F70C8[]` has three states:

1. state 0: `itPikachuthunder_UnkMotion0_Anim` (delayed, not moving);
2. state 1: `itPikachuthunder_UnkMotion1_Anim` plus
   `itPikachuthunder_UnkMotion1_Coll` (active moving segment);
3. state 2: `itPikachuthunder_UnkMotion2_Anim` (shrinking tail segment).

`it_802B211C` releases a delayed segment into state 1, installs its stored
velocity, and enables its hit processing.  On ground contact the first
segment calls `it_802B22B8`, which transitions to state 2, starts its lifetime
timer, and propagates the contact position through the linked chain.  The
first segment's `it_2725_Logic39_Destroyed` callback calls back into the owning
fighter (`ftPk_SpecialLw_SetState_Unk0`) when the owner is still attached.
The Thunder damage, shield, clank, absorb, and event hooks are the
`itPikachuThunder_Logic39_*` functions declared in `itpikachuthunder.h`.

## Quick Attack and Skull Bash effects

Quick Attack is fighter-only and has no article kind.  Its motion states are
ground `353` and air `356`; `ftPk_SpecialHiStart1_Anim` and
`ftPk_SpecialAirHiStart1_Anim` emit effect `1012` while the two zip segments
run, set the fighter effect hitlag callbacks, and skip the effect entirely for
Pichu.  The ground path jitters the intermediate effect by
`(6 * rand - 3, 6 * rand - 3)`; the air path uses `(10 * rand - 5,
10 * rand - 5)`.  The terminal frame emits the same effect at the XRotN bone
without jitter before updating velocity.

Skull Bash is the fighter side-special state family, ground `343` through
`347` and air `348` through `352`.  Its hold and release transitions install
the shared effect callbacks `ftPk_SpecialN_SpawnEffect0` and
`ftPk_SpecialN_SpawnEffect1` from `ftpikachuspecialn.c`; these emit effects
`1214` and `1215`, respectively, then clear `accessory4_cb` and install the
fighter hitlag callbacks.  During the active `SpecialS0` state,
`ftPk_SpecialS0_Anim` and its air counterpart apply the charge based damage
`mv.pk.unk3.x0 * attrs->x2C + attrs->x28` to the active capsule and consume
command variable 0 to enter the release path.

The script exposes the source-owned deterministic seams for these callbacks:
`quick_attack_effect_offset` returns `(0, 0)` for the terminal XRotN effect,
ground jitter `(6 * rand - 3, 6 * rand - 3)`, or aerial jitter
`(10 * rand - 5, 10 * rand - 5)`, and returns no effect for Pichu.  The caller
provides the two already sampled random values; the helper does not replace
the host RNG.  `thunder_jolt_spawn_position` mirrors the owner scale and
facing arithmetic while leaving article allocation and ownership to the
native item host.

The initializer contract is also source complete: `ftPk_Init` names
`ftDataPikachu`, the four costume model archives and their matching joint and
material animation archives, the four demo motion files, and the Pikachu
Thunder Jolt sound `240076` used by both ground and aerial neutral-special
callbacks.

## Exact deterministic test vector

Use a ground Pikachu, facing `+1`, at position `(10, 5, 0)`, with the ground
neutral-special motion active and command variables
`cmd[0] = 1`, `cmd[1] = 0`.  Set `specialn_spawn_offset = (3, 2)`,
`specialn_itkind = It_Kind_Pikachu_TJolt_Ground (89)`, and owner scale `1`.
One animation callback must clear `cmd[0]`, set `cmd[1] = 1`, and call
`itPikachuThunderJolt_Spawn` with position `(13, 7, 0)`, facing `+1`, and kind
`89`; a second callback with the same command values must not spawn another
article.  This vector directly exercises the branch at
`ftpikachuspecialn.c:47-83` and the spawn initialization at
`itpikachutjoltground.c:51-108`.

For Quick Attack, with a non-Pichu owner and `specialhi.x4 > 0`, one start
animation callback must emit effect `1012`; with Pichu as owner it must emit
nothing.  For Skull Bash, releasing the hold must select the correct ground or
air release state and schedule effect `1215` through
`ftPk_SpecialN_SpawnEffect1`.

## Current host gaps

The fighter script can model motion states and command transitions, but the
portable host still needs article archive registration, owner-linked item
objects, item state tables, stage collision normals, linked Thunder/Jolt
effects, article contact callbacks, and effect lifetime/hitlag cleanup to
reach parity.  It also needs an effect sink that preserves IDs `1012`, `1214`,
`1215`, and `1219`, plus the source's Pichu suppression and deterministic
Quick Attack jitter.  Until those seams exist, a fighter-only implementation
can match state selection but cannot claim parity for item ownership,
article collision, or effect lifecycle.
