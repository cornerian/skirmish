# Sheik article and effect source specification

This specification covers Sheik's native article paths behind states 341–364
and the fighter effects used by Vanish and Transform.  The fighter script
declares the state graph, while article ownership, article physics, hitbox
contact, and effect callbacks remain native responsibilities.

## Article IDs and source entry points

The pinned `ItemKind` enum assigns these stable article IDs:

| Article | Native ID | Spawn entry | Owner/contact path |
| --- | ---: | --- | --- |
| Thrown needle | `It_Kind_Seak_NeedleThrow` (`0x4F`) | `it_802AFD8C` | `it_802AFEA8` selects the throw mode; `itSeakneedlethrown_UnkMotion{0..4}_{Anim,Phys,Coll}` handle flight, bounce, terrain, and expiry |
| Held needle | `It_Kind_Seak_NeedleHeld` (`0x50`) | `it_802B19AC` | `itSeakNeedleHeld_Logic110_PickedUp`, `itSeakneedleheld_UnkMotion0_Anim/Phys/Coll`, and `itSeakNeedleHeld_Logic110_EvtUnk` keep the owner and six visible needles synchronized |
| Vanish smoke | `It_Kind_Seak_Vanish` (`0x55`) | `it_802B1C60` | `it_802B1D40` assigns owner/lifetime; `itSeakVanish_Logic42_DmgDealt` never deals damage; `it_802B1DCC` handles the item event |
| Chain | `It_Kind_Seak_Chain` (`0x61`) | `itSeakChain_Spawn` | `it_802BAEEC`/`it_802BAF0C` stop/start the chain; `it_802BB20C` destroys it; `itSeakChain_Logic54_EvtUnk` owns item events and the link callbacks drive contact |

The numeric values are the ordinals in the pinned
`src/melee/it/forward.h` enum.  They are article identities, not fighter
motion-state IDs.

## Needles

`ftSk_SpecialNStart_Anim` and `ftSk_SpecialAirNStart_Anim` create one held
needle with `it_802B19AC`, using fighter part `23` (`FtPart_L1stNb`).  The held
article stores its fighter owner and remains attached while the charge loop is
active. `it_802B18B0` queries the owner's current count and hides or reveals
the six child needle models; `ftSk_SpecialNLoop_Anim` increments the charge
count up to six and native `itSeakneedleheld_UnkMotion0_Anim` keeps the visual
stack updated.

When the end callback arms a shot, `ftSk_SpecialN_80111FBC` and
`shootNeedles` spawn `It_Kind_Seak_NeedleThrow` articles.  The throw article
attributes are:

| Field | Meaning in source |
| --- | --- |
| `x0` | initial lifetime passed to `it_80275158` |
| `x4` | bounce/ground-contact lifetime reset |
| `x8` | launch speed used by `it_802AFF08` and `it_802B00F4` |

The throw article stores its owner for damage attribution, its previous
position for terrain tests, and its current terrain line/normal.  Ground
contact selects the bounce state, updates the terrain line, rotates the
needle to the new normal, and eventually destroys it.  Damage, clank,
reflection, shield-bounce, and hit-shield callbacks are the native functions
`it_2725_Logic109_DmgDealt`, `it_2725_Logic109_Clanked`,
`it_2725_Logic109_DmgReceived`, `it_2725_Logic109_Reflected`,
`it_2725_Logic109_ShieldBounced`, and `it_2725_Logic109_HitShield`.

`ftSk_SpecialNLoop_IASA` tests whether B is still held before testing the
shoulder cancel bit.  A frame that releases B while also pressing L/R enters
the matching end phase and shoots; L/R cancels only while B remains held.  The
Python input adapter preserves this ordering when the host exposes its
held-button view.

## Chain

`ftSk_SpecialS_CheckInitChain` calls `itSeakChain_Spawn` at the source
attribute threshold `ftSeakAttributes.x1C`.  The article attaches to Sheik's
`FtPart_L3rdNa` joint, creates `itSeakChain_Attrs.x0` linked segments, and
stores the parent fighter in `parent_gobj`.  Its important source attributes
are:

* `x18`: vertical gravity/step and the fighter-side chain hit cooldown;
* `x1C` and `x20`: history-to-velocity multipliers and velocity limits used
  while following the sampled chain history;
* `x24` and `x28`: horizontal and vertical velocity clamps;
* `x48`: low-stick dead-zone threshold used by
  `ftSk_SpecialS_80110788`;
* `x4C`: squared movement threshold used by
  `ftSk_SpecialS_80110BCC` to activate/deactivate chain hit collision;
* `x50`: initial forward segment velocity;
* `x64`/`x68`: segment joint resources.

The B-release edge is latched by `ftSk_SpecialS_IASA` and consumed by the
ground and aerial loop animation callbacks only after their
`ftSeakAttributes.x14` minimum frame.  The fighter script records that edge
for replay/state inspection and leaves the 350/353 to 351/354 transition to
the native callback, so a release during the minimum-frame window cannot
skip the source timer.

`it_802BAEEC` releases link motion, `it_802BAF0C` resumes it after hitlag,
and `it_802BB20C` frees every segment and clears the owner relation.  The
fighter's four moving chain hitboxes are updated by
`ftSk_SpecialS_UpdateHitboxes`; `ftSk_SpecialS_80110BCC` compares the current
and previous endpoint positions and toggles native collision windows.

## Vanish

`ftSk_SpecialHi_80112F48` spawns `It_Kind_Seak_Vanish` at the hip joint and
`it_802B1C60` assigns the parent owner.  `it_802B1D40` initializes command
variable 0, starts the item animation, installs item callbacks, and sets the
fixed lifetime to exactly `60.0F` frames.  The smoke item does not deal damage:
`itSeakVanish_Logic42_DmgDealt` returns false.  The fighter's effect callbacks
are separate: `ftSk_SpecialHi_80112FA8` emits effect `1284` for the grounded
travel path, and `fn_80113038` emits effect `1285` for the aerial travel path.

## Transform effects

Transform has no article.  `fn_80114034` emits effect `0x4FC` on the grounded
transform joint, `fn_801140B0` emits effect `0x4FD` on the hip joint, and
`fn_8011412C` installs the native Zelda/Sheik replacement callback.  The
effect callbacks are hitlag-aware and clear their accessory callback after
the one-shot effect is installed.  `ftSk_SpecialLw_80114758` chooses the
ground or aerial finish state and applies the source attribute delay.

## Exact test vector

Use the Vanish spawn because it has a deterministic source lifetime and no
random article attributes:

```text
input:  Sheik in state 355 (SpecialHiStart_0), facing=+1
pose:   hip joint position=(10.0, 20.0, 0.0)
event:  ftSk_SpecialHi_80112F48 invokes it_802B1C60
expect: article.kind=It_Kind_Seak_Vanish (0x55)
        article.owner=Sheik
        article.position=(10.0, 20.0, 0.0)
        article.life_timer=60.0
        article.command[0]=0
        itSeakVanish_Logic42_DmgDealt(article)=false
```

## Current host gap

The Python layer and current native host expose Sheik's phase transitions but
do not yet expose generic article creation with these four source article
kinds, linked chain segments, owner cleanup, terrain-normal contact, hitlag
callbacks, or effect IDs.  Until that bridge exists, the fighter script must
retain these paths as native-owned behavior; registering an invented generic
projectile would lose owner, contact, and lifetime semantics.

## Source anchors

* `ftSeak/ftseakspecialn.c`: `ftSk_SpecialNStart_Anim`,
  `ftSk_SpecialNLoop_Anim`, `ftSk_SpecialNEnd_Anim`,
  `ftSk_SpecialN_80111FBC`, and `shootNeedles`.
* `ftSeak/ftseakspecials.c`: `ftSk_SpecialS_CheckInitChain`,
  `ftSk_SpecialS_80110BCC`, `ftSk_SpecialS_UpdateHitboxes`,
  `ftSk_SpecialS_ChainSomething`, and `ftSk_SpecialS_CheckAndDestroyChain`.
* `ftSeak/ftseakspecialhi.c`: `ftSk_SpecialHi_80112F48`,
  `ftSk_SpecialHi_80112FA8`, and `fn_80113038`.
* `ftSeak/ftseakspeciallw.c`: `fn_80114034`, `fn_801140B0`,
  `fn_8011412C`, and `ftSk_SpecialLw_80114758`.
* `it/kinds/itseakneedleheld.c`, `itseakneedlethrown.c`,
  `itseakchain.c`, and `itseakvanish.c`: article state tables, owner setup,
  contact callbacks, and destruction paths.
