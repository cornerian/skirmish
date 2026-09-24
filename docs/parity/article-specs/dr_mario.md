# Dr. Mario article parity specification

Pinned source: `../../External/melee/src/melee/ft/kinds/ftMario/` and
`../../External/melee/src/melee/it/kinds/`.

Dr. Mario uses Mario's special callbacks. The fighter kind selects the article
and the vitamin color branch; it does not select a separate motion callback.
The motion table is in `ftDrMario/ftdrmario.c`, while the shared callbacks are
in `ftMario/ftmariospecialn.c` and `ftMario/ftmariospecials.c`.

## Article IDs

The native item enum in `it/forward.h` places these adjacent entries at the
same IDs exported by the Python article API:

| ID | Native entry | Use |
| ---: | --- | --- |
| 48 | `It_Kind_Mario_Fire` | Mario's neutral fireball comparison |
| 49 | `It_Kind_DrMario_Vitamin` | Dr. Mario's Megavitamin |

`Megavitamin` therefore must spawn ID 49. ID 48 is only the Mario branch of
the shared `ftMr_SpecialN_ItemFireSpawn` callback.

## Fighter states and shared callbacks

`ftdrmario.c` assigns the following states. The callback names are the Mario
implementations selected by the motion table.

| State | Ground/air role | Enter and article owner callback |
| ---: | --- | --- |
| 343 / 344 | Megavitamin / fireball neutral | `ftMr_SpecialN_Enter` / `ftMr_SpecialAirN_Enter`, then `ftMr_SpecialN_ItemFireSpawn` |
| 345 / 346 | Super Sheet / Cape side special | `ftMr_SpecialS_Enter` / `ftMr_SpecialAirS_Enter`, then `ftMr_SpecialS_CreateCape` |
| 347 / 348 | Super Jump Punch | `ftMr_SpecialHi_*` |
| 349 / 350 | Dr. Tornado | `ftMr_SpecialLw_*` |

Neutral entry clears command variable 0 and throw flags, selects 343 or 344,
and installs the accessory callback. When `throw_flags_b0` is set, that
callback consumes the flag and reads the `L1stNb` bone. Mario spawns
`It_Kind_Mario_Fire` and effect 1146; Dr. Mario calls
`ftMr_SpecialN_VitaminRandom` and spawns `It_Kind_DrMario_Vitamin` with the
selected color, owner, position, and facing.

`ftMr_SpecialN_VitaminRandom` builds colors 0 through 8 while excluding both
`x222C_vitaminCurr` and `x2230_vitaminPrev`. The selected color becomes the
current value and the old current value becomes previous in the native helper
used by the picker. The pill is initialized by `itDrMarioPill_Spawn` in
`itdrmariopill.c`; its motion table supplies animation, physics, and collision
callbacks, and its item event table supplies `itDrMarioPill_DmgDealt`,
`itDrMarioPill_Reflected`, `itDrMarioPill_Clanked`, `itDrMarioPill_HitShield`,
`itDrMarioPill_Absorbed`, and `itDrMarioPill_ShieldBounced`.

Side entry selects 345 or 346. Ground entry clears vertical velocity; air
entry divides horizontal velocity by `specials.vel_x_decay` before selecting
346. `changeAction` clears command variables 0, 1, and 2, clears the native
reflecting flag, and installs `ftMr_SpecialS_CreateCape`. That callback creates
the cape item using `specials.cape_kind`, attaches it to the right thumb, and
registers the owner hitlag callbacks. `reflect` enables the
`specials.cape_reflection` descriptor when command variable 1 is 1 and removes
the active reflection when it returns to 0. `itmariocape.c` resets the fighter
when the cape is destroyed or leaves the side-special states, and forwards
hitlag enter/exit to the attached item.

The script mirrors command variable 1 as the reflection window. Air entry
velocity decay, cape attachment, and cape destruction remain host
responsibilities because the authoring context does not expose the source
special attributes and article handles.

Dr. Mario's special attributes are the `ftMario_DatAttrs` fields in
`ftMario/types.h`: `specials.vel_x_decay`, `specials.vel`, `specials.grav`,
`specials.terminal_vel`, `specials.cape_kind`, and `cape_reflection`. The
vitamin current and previous colors are fighter variables
`x222C_vitaminCurr` and `x2230_vitaminPrev`.

## Exact test vector

For a grounded Dr. Mario neutral special with facing `-1`, current vitamin
color `2`, previous color `5`, and the picker returning index `0` from its
seven-entry candidate list:

1. `ftMr_SpecialN_Enter` selects state 343, clears command variable 0 and
   throw flags, and installs `ftMr_SpecialN_ItemFireSpawn`.
2. On `throw_flags_b0`, the accessory callback consumes the flag and chooses
   color `0` from `[0, 1, 3, 4, 6, 7, 8]`.
3. It calls `itDrMarioPill_Spawn` with article ID 49, the `L1stNb` position,
   color `0`, and facing `-1`; no Mario fireball or effect 1146 is created.

The corresponding API vector is the `Megavitamin` article assertion in
`scripts/api/tests/test_dr_mario.py`.

## Host gap

The script API can express the shared state graph, article ID, owner, bone,
facing, command resets, and cape reflection contact. It does not currently
own the native article object state: deterministic `HSD_Randi` selection,
pill motion and ground collision, pill event callbacks, hitbox/effect data,
cape attachment lifetime, or cape hitlag callbacks. Those remain native host
responsibilities for complete runtime parity.
