# Ganondorf Dark Dive source specification

This specification covers Ganondorf's Dark Dive (`ftCa_MS_SpecialHi` and
`ftCa_MS_SpecialAirHi`). Ganondorf uses Captain Falcon's special up
implementation and motion callbacks. The Ganon motion table in
`ftGanon/ftganon.c` assigns states 353 through 356 and state 363 to the
`ftCa_SpecialHi*` and `ftCa_SpecialHiThrow1*` callbacks; the fighter specific
difference is the motion and attack data, not a second capture algorithm.

## Capture path

`ftCaptain/ftcaptainspecialhi.c` is the behavioral source:

1. `ftCa_SpecialHi_Enter` and `ftCa_SpecialAirHi_Enter` initialize the
   Captain special state and enter state 353 or 354. The normal collision
   callbacks (`ftCa_SpecialHi_Coll` and `ftCa_SpecialAirHi_Coll`) handle floor,
   ledge, and ordinary airborne continuation.
2. On a successful aerial catch, the common capture path enters state 355,
   `ftCa_MS_SpecialHiCatch`. `ftCa_SpecialLw_800E5128` marks the attacker as
   owning the victim, starts the capture animation, and attaches the victim
   when the victim is grounded. `ftCa_SpecialHiCatch_Coll` falls out only when
   the capture relation was not established.
3. `ftCa_SpecialHiCatch_Anim` calls `doCatchAnim` at animation end. That helper
   clears the command cue and transient special velocity, enters state 356,
   `ftCa_MS_SpecialHiThrow`, and releases the victim into the common thrown
   path. This is the throw transition, even though the source state is named
   `Throw0`.
4. State 356 ends in ordinary fall through `ftCa_SpecialHiThrow0_Anim`.
   During the throw state, the command cue can enable the special movement
   branch; `ftCa_SpecialHiThrow0_Phys` then combines special steering with
   gravity. The victim relation must be cleared when the throw begins.

The source does not create a projectile or persistent article for Dark Dive.
`ftCommon_8007E2F4` and the capture helpers update fighter and victim
relations/effects; they are presentation or common grab state, not an article
owned by Ganondorf. The state 363 rebound motion is also an attacker-only
fighter state.

## Ganondorf-specific state

`ftGanon/ftganon.c` assigns the shared callbacks as follows:

| Source state | Motion | Callback family | Observable behavior |
| --- | ---: | --- | --- |
| 353 `ftCa_MS_SpecialHi` | `ftCa_SM_SpecialHi` | `ftCa_SpecialHi_*` | Ground Dark Dive startup/travel |
| 354 `ftCa_MS_SpecialAirHi` | `ftCa_SM_SpecialAirHi` | `ftCa_SpecialAirHi_*` | Aerial Dark Dive startup/travel |
| 355 `ftCa_MS_SpecialHiCatch` | `ftCa_SM_SpecialHiCatch` | `ftCa_SpecialHiCatch_*` | Victim capture and attachment |
| 356 `ftCa_MS_SpecialHiThrow` | `ftCa_SM_SpecialHiThrow0` | `ftCa_SpecialHiThrow0_*` | Release/throw, then fall |
| 363 `ftCa_MS_SpecialHiThrow1` | `ftCa_SM_SpecialHiThrow1` | `ftCa_SpecialHiThrow1_*` | Wall rebound continuation, then fall |

Ganon's source-specific surface branch is in the shared Captain down-special
collision routine: `ftCa_SpecialLw_Coll` checks command variable 0 and a wall
opposite the facing direction, clears throw flags, then enters state 363.
`ftCa_SpecialHiThrow1_Coll` delegates to `ftCo_AirCatchHit_Coll`, while
`ftCa_SpecialHiThrow1_Anim` enters ordinary fall when its animation ends.
The host implementation should therefore keep state 363 available to Ganon,
preserve the command gate and facing-wall test, and finish it into fall.

## Test vector

Use a Ganon fighter in state 354 with the victim airborne and within the Dark
Dive catch volume. On the frame where the catch collision succeeds, assert:

```text
before: action=SpecialAirHi, victim=unpaired, grounded=false
event:  aerial catch hit
after:  action=SpecialHiCatch, special_capture.owner=attacker,
        victim.special_capture.holder=attacker
next animation end:
        action=SpecialHiThrow, special_capture cleared,
        victim enters common thrown state
throw animation end:
        action=Fall
```

A second wall vector covers Ganon's source-only rebound branch:

```text
before: action=WizardFoot air phase, command[0]=1, facing=+1
event:  left-wall contact
after:  action=DarkDive.throw_rebound (source state 363), throw_flags=0
animation end: action=Fall
```

With `command[0]=0`, the same wall contact must leave the Wizard's Foot phase
unchanged and must not enter state 363. The wall must be opposite the facing
direction; a same-facing wall is not a rebound.

The rebound branch is installed only on grounded Wizard's Foot state 357.
The aerial state 359 uses `ftCa_SpecialAirLw_Coll`, which has no state-363
wall branch. Before entering state 363, `ftCa_SpecialLw_Coll` clears command
variables 0 through 2 and `throw_flags`; the authoring callback preserves
command slot 3 because the host state model keeps the fourth slot separate.

## Source anchors and remaining boundary

- `ftCaptain/ftcaptainspecialhi.c`: `ftCa_SpecialHi_Enter`,
  `ftCa_SpecialAirHi_Enter`, `ftCa_SpecialHiCatch_Anim`, `doCatchAnim`,
  `ftCa_SpecialHiThrow0_Anim`, and `ftCa_SpecialHiThrow1_Coll`.
- `ftCaptain/ftcaptainspecialhi.c`: `ftCa_SpecialLw_800E5128` capture setup,
  `ftCa_SpecialLw_Coll` state-363 wall rebound, and
  `ftCa_SpecialHiThrow1_Anim`.
- `ftGanon/ftganon.c`: motion table entries for states 353–363.

The common capture relation and victim throw object lifecycle remain shared
Captain-family behavior. Ganon only needs the state table, Dark Dive motion
and attack resources, and the state-363 availability described above.
