# Fox class-move parity audit

Status: audit only. This document preserves the source provenance and
acceptance matrix for the consolidated `scripts/fighters/fox.py`. It
deliberately does not add frame data or infer hitboxes from commonly published
move lists. Fox specials and ordinary actions now share that one source file.

## Provenance and scope

The authoritative source is the pinned USA 1.02 Melee checkout at revision
`0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, recorded in
`upstream.lock.json`. Fox's per-character motion table starts at common motion
state `ftCo_MS_Count` (341) and defines only states 341--375 for neutral,
side, up, down specials and Fox's six-stage side taunt. The ordinary attacks,
grabs, throws, knockdown/getup actions, and ledge actions therefore use the
common motion table and common callbacks. This is an engine ownership fact; it
does not mean Fox's DAT action commands are interchangeable with another
fighter's commands or resources.

The upstream character resource names are visible in
`src/melee/ft/kinds/ftFox/ftfox.c`: `PlFx.dat` (base), `PlFxAJ.dat`
(animation data), and the costume model/material archives `PlFxNr.dat`,
`PlFxOr.dat`, `PlFxLa.dat`, and `PlFxGr.dat`. No decoded Fox gameplay/action
bundle is present in this checkout. The only Fox native-data fixtures currently
tracked by Skirmish are the focused `fox-jab1-*` capture/processed files and
`fox-native-subset.json`; these are regression evidence for one jab slice, not
a complete moveset. The v10 resource export contains substantial Fox pose and
animation material, but its complete action-command, hitbox, hurtbox/ECB, and
source-hash coverage must be checked per move before claiming parity. A move is
`asset-blocked` only when the required data has not been verified for that
move; the label is not a claim that the whole v10 export is absent.

## Audit matrix

| Move family | Upstream common state/handler ownership | Required class move data | Current evidence/status |
| --- | --- | --- | --- |
| Jab 1/2/3 | `ftCo_MS_Attack11/12/13`; `ftCo_Attack1.c` (`Anim`, `IASA`, physics/collision callbacks) | command entry, animation/action resource, every hitbox group with bone/offset/radius and flags, active/clear cues, combo windows and terminal transition | `fox-jab1-captured.csv` and `fox-jab1-processed.json` cover only jab 1; jab 2/3 and complete decoded command data are missing |
| Rapid jab | `ftCo_MS_Attack100Start/Loop/End`; `ftCo_Attack100.c` | start/loop/end resources, repeat/stop input policy, per-pulse hitbox/effect cues, loop exit and interruptibility | parity fields not yet independently verified; asset-blocked until then |
| Dash attack | `ftCo_MS_AttackDash`; `ftCo_AttackDash.c` | startup/early/late hit groups, animation cue mapping, ground movement, clear and recovery | parity fields not yet independently verified; asset-blocked until then |
| Forward/up/down tilt | `ftCo_MS_AttackS3*`, `ftCo_MS_AttackHi3`, `ftCo_MS_AttackLw3`; `ftCo_AttackS3.c`, `ftCo_AttackHi3.c`, `ftCo_AttackLw3.c` | height variants where command data selects them, exact hitbox groups/flags, IASA and transition cues | parity fields not yet independently verified; asset-blocked until then |
| Forward/up/down smash | `ftCo_MS_AttackS4*`, `ftCo_MS_AttackHi4`, `ftCo_MS_AttackLw4`; `ftCo_AttackS4.c`, `ftCo_AttackHi4.c`, `ftCo_AttackLw4.c` | charge entry/release semantics, charge cap/resource, early/late hit groups, facing/ground behavior, clear and recovery | parity fields not yet independently verified; asset-blocked until then |
| Neutral/forward/back/up/down aerial | `ftCo_MS_AttackAirN/F/B/Hi/Lw`; `ftCo_AttackAir.c` | aerial command mapping, hitbox groups and active/clear cues, landing-lag/autocancel windows, fast-fall and interruptibility policy | parity fields not yet independently verified; asset-blocked until then |
| Standing/dash grab | `ftCo_MS_Catch` and `ftCo_MS_CatchDash`; `ftCo_Catch.c` | catch volume attachment and flags, startup/active/clear cues, pull transition and whiff recovery | common handlers exist; Fox-specific resource parity is not independently verified |
| Pummel | `ftCo_MS_CatchAttack`; `ftCo_CatchAttack.c` | captive damage event, hitbox/effect/audio cue, repeat/interrupt policy and animation marker | common handler exists; Fox resource parity is not independently verified |
| Forward/back/up/down throw | `ftCo_MS_ThrowF/B/Hi/Lw`; `ftCo_Throw.c`, plus Fox's `ftFx_Throw_Anim` in `ftfoxspecialn.c` for Fox-specific throw animation callbacks | captive release marker, throw damage/angle/KB/flags, victim animation/offset, any multi-hit effects, terminal recovery | Fox callback is authoritative code, but numeric throw/action data and resource cue timing still require decoded assets; do not copy the authoring sketch values as data |
| Knockdown/getup/down attack | `ftCo_MS_Down*`; `ftCo_Down*.c` (`Down`, `DownBound`, `DownWait`, `DownDamage`, `DownStand`, `DownAttack`, `DownFoward`, `DownBack`, `DownSpot`) | all downed entry/loop/choice actions, wakeup attack hitboxes, directional getup resources, input windows and collision transitions | common handlers exist; Fox resource parity is not independently verified |
| Ledge catch/wait/climb | `ftCo_MS_CliffCatch/Wait/ClimbSlow/ClimbQuick`; `ftCo_Cliff*.c` | cliff snap/catch transition, wait, slow/quick climb animation and hurtbox/ECB eligibility, input/edge transitions | common handlers exist; Fox resource parity is not independently verified |
| Ledge attack | `ftCo_MS_CliffAttackSlow/Quick`; `ftCo_CliffAttack.c` | slow/quick attack selection, full hitbox timeline, ledge release/ground transition and collision/hurtbox changes | common handlers exist; Fox resource parity is not independently verified |
| Ledge escape/jump | `ftCo_MS_CliffEscapeSlow/Quick`, `ftCo_MS_CliffJumpSlow1/2`, `ftCo_MS_CliffJumpQuick1/2`; corresponding `ftCo_Cliff*.c` | selection rules, invulnerability/hurtbox tracks, movement/animation tracks, landing/ground transition | common handlers exist; Fox resource parity is not independently verified |
| Taunt/appeal | Fox states 370--375 (`ftFx_AppealS*`); `ftfoxappeals.c` | six direction/stage action resources, right/left selection, stage progression, interruption-on-damage behavior, voice/effect markers | engine callback and state IDs are available; Fox resource parity is not independently verified |

The common state list is in `src/melee/ft/kinds/ftCommon/forward.h`. The
callback declarations are in the matching `ftCo_*.h` files beside each source
body. Those handlers supply lifecycle, input, physics, and collision policy;
the class move must supply Fox's resource identity and gameplay events at the
animation/action boundaries.

## Class authoring contract

The consolidated Fox source uses immutable `ActionMove` instances for ordinary
slots and special `Move` classes, exposing the
normal, tilt, smash, aerial, grab, pummel, throw, knockdown/getup, ledge, and
taunt groups required by the fighter definition. Execution state belongs to
the host action instance. Each move needs:

1. A required canonical `Action` enum value, action string, or
   `ActionDescriptor`, plus an optional resource reference. `ActionMove` has
   no `run` or timer behavior.
2. Its common `ftCo_MS_*` state and callback family, or the Fox
   `ftFx_AppealS*` callback/state for taunt. State IDs must not be guessed from
   an animation index.
3. An animation resource reference with original frame origin, rate,
   interpolation, and marker/command stream. Markers such as `hit`, `clear`,
   `release`, `landing`, and `interruptible` are authored only when the
   decoded command/animation data proves them.
4. Exact hitbox/catch/throw payloads: attached bone/part, center or endpoints,
   radius, damage, angle, knockback, set-off/element/flags, and all active,
   replacement, and clear intervals. Hurtbox/ECB and intangible eligibility
   tracks are equally gameplay data.
5. Input and lifecycle bindings that reproduce common IASA, animation-end,
   ground/air, landing, ledge, captive, and interruption transitions. A
   callback that is common engine policy should be referenced/reused rather
   than duplicated in Fox script.

The existing class API has `Move`, `MoveContext`, `AerialMoves`,
`GroundedMoves`, `TiltMoves`, `SmashMoves`, `GrabMoves`, `ThrowMoves`,
`DefenseMoves`, `LedgeMoves`, `GetupMoves`, `TauntMoves`, and finite event
bindings in `scripts/api/fighter/`. The ordinary Fox descriptors map
the DAT subaction name separately from the engine action ID. For example,
`Attack11` is the native subaction resource while the engine action is
`Action::Jab`/`"jab"`; `CliffClimbSlow` and `CliffClimbQuick` both use the
single engine action `"cliff_climb"`. A mapping must be checked against the
actual `Action` enum and parser, never generated by blindly snake-casing a
DAT name.
The consolidated source maps each ordinary slot directly with `ActionMove(...)`;
there is no `NativeActionRequest` scheduler handoff. `AerialMoves()`,
`GroundedMoves()`, and the other ordinary groups materialize their documented
canonical defaults; `SpecialMoves` has no defaults and must be given
`neutral`, `side`, `up`, and `down`. The special classes retain their event
bindings and resource validation. Their finite `on_end`, `on_ground`, and
`on_air` transition tables express action completion and ground/air changes
with `Transition` objects. The ordinary action string may differ from the DAT
subaction name, so mappings must be checked against the native action catalog
rather than generated by blindly snake-casing a DAT name. Resource references
and hashes belong to the validated resource bundle once each move's export is
audited.

Fox's `FoxActionState.command` is a fixed four integer tuple, initialized to
`(0, 0, 0, 0)`. Other action-state fields are declared explicitly by the Fox
special policies. This describes the current authoring contract; it does not
claim complete heap freezing, full Fox parity, or performance equivalence.

## Verification and gap policy

Every authored move needs an independent observable test. At minimum, tests
should cover command-to-state selection, marker-to-hitbox scheduling, clear or
replacement timing, animation-end transition, landing/autocancel or ledge
transition where applicable, and throw/captive behavior. Differential tests
should compare the native/original C callback or an equivalent host-compiled
reference at the event boundary; a passing common handler test does not prove
Fox's resource data.

Use these status labels in future inventories and reviews:

- `verified`: source identity, decoded resources, and independent observable
  comparison are present;
- `fixture-covered`: a focused native/replay fixture exists but the complete
  resource is not decoded;
- `engine-covered`: common or Fox callback code is available, while Fox
  action/animation data is absent;
- `asset-blocked`: implementation would require missing resource data;
- `missing-test`: data and implementation exist but independent coverage is
  still required.

Current overall status is `fixture-covered` for Fox jab 1 and
`engine-covered` for the common action families and Fox taunt/throw callbacks;
all other Fox move data is `asset-blocked`. This distinction prevents a
synthetic move script from being reported as Melee parity.
