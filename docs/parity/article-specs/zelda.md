# Zelda article and transformation contract

This is the source-backed native contract for Zelda's specials.  The pinned
source is `../../External/melee/src/melee`; fighter phase declarations live in
`scripts/fighters/zelda.py`, while articles, effects, and character replacement
remain native responsibilities.

## Article identities and registration

`ftZd_Init_OnLoad` in
`melee/ft/kinds/ftZelda/ftzelda.c:260-269` registers two article kinds:

| Native kind | Role |
| --- | --- |
| 108 (`It_Kind_Zelda_DinFire`) | Din Fire's travelling/charging article |
| 109 (`It_Kind_Zelda_DinFire_Explode`) | Din Fire's terminal explosion article |

The source registers both from Zelda's item archive.  The numeric enum entries
are adjacent in `melee/it/forward.h` (enum values 108 and 109); the
host must preserve those identities rather than treating the explosion as a
visual effect-only alias.

## Din Fire spawn and attributes

`ftZd_SpecialSStart_Anim` and `ftZd_SpecialAirSStart_Anim` in
`melee/ft/kinds/ftZelda/ftzeldaspecials.c:97-171` call `it_802C3BAC` when their
animation command sets slot 0.  The loop variants at `:134-190` and
`:223-309` use the same spawn path.  Spawn initializes the owner and secondary
owner, facing, position, command variables, and the fighter back-reference;
the fighter then installs death and damage cleanup callbacks while the article
exists.

The article's `ItZeldaDinFire_ItemVars` is defined in
`melee/it/kinds/itzeldadinfire.c:27-40`.  Its archive attributes are consumed
as follows:

| Field | Use |
| --- | --- |
| `x0` | initial article lifetime passed to `it_80275158` |
| `x4` | growth duration for the scale interpolation |
| `x8`, `xC` | minimum and maximum visual scale |
| `x10` | initial launch-angle offset |
| `x14` | initial speed |
| `x18` | owner-stick steering speed increment |
| `x1C` | maximum speed |
| `x20` | minimum owner-stick magnitude for steering |
| `x24` | steering multiplier |
| `x28` | maximum steering-angle magnitude |
| `x2C` | travelling-state lifetime reset |

`it_802C3D74` (`itzeldadinfire.c:119-158`) starts motion state 0, sets the
launch angle to `0` or `π` from facing, computes velocity as
`speed * (cos(angle + offset), sin(angle + offset), 0)`, and emits effect
`1272`.  State 0 animation (`:161-205`) grows the scale, applies owner steering
when `ftZd_SpecialLw_8013B574` arms the article, and refreshes the travelling
effect as needed with effect `1273`.  State 0 physics (`:237-274`) applies the
same steering path and clamps speed to `x1C`.

## Contact and owner callbacks

The state table at `itzeldadinfire.c:19-24` has travelling state 0 and terminal
state 1.  State 0 collision (`:276-284`) checks the directional floor,
ceiling, and wall masks in `it_802C3AFC` (`:50-69`); contact moves the article
through the explosion setup path.  State 1 physics resets velocity and its
collision callback is inert.

The article callbacks in `itzeldadinfire.h` and `itzeldadinfire.c` are:

| Callback | Source result |
| --- | --- |
| `itZeldaDinFire_GetOwner` | Returns the current article owner. |
| `itZeldaDinFire_Logic65_Destroyed` | Clears effects, clears the fighter back-reference when still owned, and resets article flags. |
| `itZeldaDinFire_Logic65_Reflected` | Arms terminal state, flips facing, negates velocity, zeros Z velocity, and returns `false` so the article remains alive. |
| `itZeldaDinFire_Logic65_Clanked` | Returns `true`, consuming the article contact. |
| `itZeldaDinFire_Logic65_Absorbed` | Returns `true`, consuming the article. |
| `itZeldaDinFire_Logic65_EvtUnk` | Forwards the event to the generic article event helper twice. |

The terminal explosion is registered in `itZeldadinfireexplode.c` and uses
effect/article cleanup callbacks at symbols `itZeldaDinFireExplode_Logic66_*`
(`config/GALE01/symbols.txt:15525-15534`).

## Nayru's Love and transformation effects

Nayru does not create an `it_` article.  Ground and air accessory callbacks
`ftZd_SpecialN_8013A830` and `ftZd_SpecialN_8013A8AC`
(`ftzeldaspecialn.c:8-29`) emit effects `1268` and `1269`, respectively, and
install the native reflect-hit descriptor from `ftZelda_DatAttrs::x84`.
The reflect hit is created once the command variable reaches 1 in
`ftZd_SpecialN_Anim` / `ftZd_SpecialAirN_Anim` (`:91-148`); the matching
ground/air collision helpers (`:185-243`) recreate it after surface changes.

Down-special transformation emits effect `1276` on ground and `1277` in air
through `ftZd_SpecialLw_8013ADB4` and `ftZd_SpecialLw_8013AE30`
(`ftzeldaspeciallw.c:1-49`).  When the transformation animation ends,
`ftZd_SpecialLw_8013AEAC` (`:52-67`) calls
`ftCommon_8007EFC8(gobj, ftSk_SpecialLw_80114758)`, which performs the native
Zelda-to-Sheik replacement.  The Python `Transform` phase graph preserves
states 355–358, surface transitions, and the source command-0 reset.  Its
`native_completion` contract returns the typed `TransformOutcome.UNSUPPORTED`
result until the host exposes fighter identity and resource swapping; it does
not claim that Zelda has become Sheik.

## Exact deterministic test vector

Create a Din Fire article from an owner at `(1, 2, 0)` with facing `+1`, and
use article attributes `x10 = 0`, `x14 = 5`, `x0 = 30`, and `x2C = 30`.
Immediately after `it_802C3D74`:

```text
state       = 0
owner       = the spawning fighter
position    = (1, 2, 0)
velocity    = (5, 0, 0)
facing      = +1
life_timer  = 30
effect      = 1272
```

Calling `itZeldaDinFire_Logic65_Reflected` next must leave the article alive,
set terminal-state marker `xDDC = 1`, flip facing to `-1`, and change velocity
to `(-5, 0, 0)`.  This vector exercises `it_802C3BAC`, `it_802C3D74`, and the
reflection callback without relying on animation timing or random state.

## Current host gaps

The fighter script currently exposes Zelda's source phases, directional input,
Din loop release, and terminal/surface transitions.  The native host still
needs:

1. Article archive registration and numeric kind IDs for Din Fire and its
   explosion.
2. Owner-linked article state, article attributes, steering from the owner's
   stick, ECB contact masks, reflection/clank/absorb callbacks, and cleanup
   callbacks.
3. Nayru's native reflect descriptor and effect lifecycle for effect IDs
   1268/1269.
4. Effect IDs 1272/1273/1276/1277 and transformation replacement through
   `ftCommon_8007EFC8`.

The Din command callback only forwards a cue when the host explicitly reports
that no Din Fire article is owned and supplies the source joint-89 position.
Without both native-owned values, it leaves the command for the host rather
than guessing a root position or spawning a duplicate article.

These are native article/effect and fighter replacement seams; fabricating them
inside the declarative fighter phase graph would lose owner and contact parity.

## Transform lifecycle contract

The source transform entry clears command slot 0 in
`ftZelda_SpecialLw_StartAction_Helper`.  The Python entry callback mirrors that
reset.  Completion remains an explicit unsupported outcome because the current
host has no operation equivalent to `ftCommon_8007EFC8` that atomically swaps
fighter identity, resources, and native callbacks.  A future native host may
consume `TransformOutcome.UNSUPPORTED` only after adding that identity swap
seam; the declarative script must not substitute `Action.WAIT` or `Action.FALL`
for the replacement itself.
