# Donkey Kong parity specification

Pinned source: `../../External/melee/src/melee/ft/kinds/ftDonkey/`.

## Giant Punch

The motion table in `ftdonkey.c` assigns these source states:

| State | ID | Role |
| --- | ---: | --- |
| `ftDk_MS_SpecialNStart` | 369 | Ground charge entry |
| `ftDk_MS_SpecialNLoop` | 370 | Ground charge loop |
| `ftDk_MS_SpecialNCancel` | 371 | Ground cancel |
| `ftDk_MS_SpecialN` | 372 | Ground punch |
| `ftDk_MS_SpecialNFull` | 373 | Ground full charge punch |
| `ftDk_MS_SpecialAirNStart` | 374 | Air charge entry |
| `ftDk_MS_SpecialAirNLoop` | 375 | Air charge loop |
| `ftDk_MS_SpecialAirNCancel` | 376 | Air cancel |
| `ftDk_MS_SpecialAirN` | 377 | Air punch |
| `ftDk_MS_SpecialAirNFull` | 378 | Air full charge punch |

`ftDk_SpecialN_Enter` and `ftDk_SpecialAirN_Enter` in `ftdonkeyspecialn.c`
select Start unless `u.dk.x222C` equals `SpecialN.x2C_MAX_ARM_SWINGS`, in
which case they select Full. Both reset command variables and Giant Punch
motion variables. Ground entry clears vertical self velocity; air entry keeps
the current aerial state.

Start animation completion enters Loop. `ftDk_SpecialNLoop_IASA` and its air
variant release on B into state 372/377, storing the current arm swing count;
L/R enters Cancel. Each Loop animation cycle increments `u.dk.x222C` and the
native callback enters the full punch at the configured maximum. Punch
animation uses command variable 0 to spawn effect 1224 (ground) or 1225 (air),
adds `xC * SpecialN.x30_DAMAGE_PER_SWING` to both active capsules, and applies
horizontal velocity `facing * SpecialN.x34_PUNCH_HORIZONTAL_VEL * xC`.

The four Giant Punch attributes are defined in `ftdonkey/types.h`:

- `x2C_MAX_ARM_SWINGS` — charge cap;
- `x30_DAMAGE_PER_SWING` — per-cycle damage increment;
- `x34_PUNCH_HORIZONTAL_VEL` — release drift;
- `x38_LANDING_LAG` — aerial punch landing lag.

On punch completion, ground returns through `ft_8008A2BC`; air selects Fall
when landing lag is zero and FallSpecial with mobility 1 otherwise. Collision
callbacks in `ftdonkeyspecialn.c` preserve the matching ground/air state and
frame, and destroy hitlag effects through
`ftDk_SpecialN_DestroyAllEffects`.

### Test vector

Given `max_arm_swings = 4`, `u.dk.x222C = 2`, facing `-1`,
`damage_per_swing = 3`, and `punch_horizontal_vel = 0.5`:

1. A fresh grounded B enters 369, then animation completion enters 370.
2. The next Loop cycle records count 3; B enters 372 with `xC = 3`.
3. The first active hit capsule receives `+9` damage and horizontal velocity
   becomes `-1.5`.
4. A later Loop cycle reaching count 4 selects 373 immediately and plays the
   full charge punch.

The Python fighter declaration exports states 369–378 and the charge/release
and cancel transitions. Native ownership still covers the charge counter,
capsules, effect IDs, damage scaling, and landing lag because those fields are
not part of the current script API.

## Cargo and grab object dependencies

Donkey Kong's cargo hold, walk, turn, jump, landing, and throw states are
listed in `ftdonkey.c` and use the common callbacks from
`melee/ft/kinds/ftCommon/ftCo_CargoWait.h`, `ftCo_CargoWalk.h`,
`ftCo_CargoTurn.h`, `ftCo_CargoJump.h`, `ftCo_CargoLanding.h`,
`ftCo_CargoFall.h`, and `ftCo_CargoThrow.h`. The source fighter table binds
these callbacks to the character's cargo motion IDs, while the grabbed
opponent and throw flags remain fighter and object state owned by the native
grab system. A script declaration can expose the motion graph, but cannot
recreate target attachment, cargo object lifetime, throw direction selection,
or common cargo collision callbacks without a native object bridge.
