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

## Headbutt, Spinning Kong, and Hand Slap audit

The side special (Headbutt) has no fighter IASA command branch: its ground and
air animations run to completion, then return to Wait or Fall. Spinning Kong
uses command variable 0 as a gravity mode: value 0 applies
`SpecialHi.x50_AERIAL_GRAVITY`, while a nonzero value uses ordinary fighter
gravity. The script declaration preserves that branch in its air motion
descriptor; this matters after the aerial collision callback sets the command.

Hand Slap starts at state 383, enters looping state 384 when the start motion
ends, and only samples B during the loop IASA callback. A B press latches one
additional loop; the loop animation consumes that latch at its end, otherwise
it enters end state 385. State 385 returns to Wait when complete, while state
386 is the landing continuation that exits through collision. These command
and hitbox callbacks remain native-owned until the script API exposes the
effect and collision resources.

Spinning Kong's grounded entry only reads the grounded horizontal velocity and
grounded mobility fields (`SpecialHi.x54` and `x5C`). The aerial gravity,
vertical launch, aerial steering, and landing-lag fields are read by the
aerial entry/physics path. The script therefore validates those attribute sets
per surface, so an incomplete aerial block does not disable a source-valid
grounded move.

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

The native host enters source state 351 (`ftDk_MS_ThrowFWait0`) when a Donkey
Kong forward throw animation completes with a live captured victim. Cargo input
then follows `ftCo_8009BF3C`: held A/B, main stick only, horizontal priority,
and source states 361..368. The victim is paired with common
`ftCo_MS_ThrownFF..ThrownFLw` states 271..274, and the host reuses the
directional throw resource for release damage. The typed exporter maps the
`SetThrowFlagsRelease` rows 315..318 to `Throw.release_frame` values 15, 15,
14, and 15 (forward, backward, up, and down), and the native host consumes
those values for cargo release timing.

Retail `ftCo_CargoThrow*_Anim` calls `ftCo_800DD724`, whose animation command
variable 0 freezes the throw animation and calls `ftCo_800DE920`, while its
release command invokes `ftCo_800DE2A8` and `ftCo_800DE7C0`. The current
`FighterData` resource model still does not expose those cargo animation
command rows or callbacks, so command-variable freeze/resume and callback
side effects remain unsupported; typed release-frame selection is sourced
from the exported throw resource.
