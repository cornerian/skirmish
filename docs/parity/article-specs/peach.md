# Peach article specification

This is the source contract for Peach's native article families. The pinned
source revision is recorded in `upstream.lock.json`; the checkout is
`../../../External/melee` from this repository. Article ordinals below come
from `src/melee/it/forward.h` in that checkout.

## Article catalog

| Article | ID | Spawn and owned state | Contact and cleanup callbacks |
| --- | ---: | --- | --- |
| Peach Bomber explosion | 98 | `it_802BD158` in `it/kinds/itpeachexplode.c` creates the item; `it_802BD248` assigns the owner, selects ground/air animation, initializes command state, effects, and a 60-frame lifetime. | `itPeachExplode_Logic55_DmgDealt` leaves the article alive; `itPeachExplode_Logic55_EvtUnk` forwards owner/reference cleanup; animation expiry is `itPeachexplode_UnkMotion1_Anim`. |
| Turnip | 99 | `ftPe_SpecialLw_Enter` and `spawnVeg` in `ft/kinds/ftPeach/ftpeachspeciallw.c` choose and create the vegetable. `it_802BD4AC` in `it/kinds/itpeachturnip.c` stores lifetime, weighted type index, type damage, held flag, scale, and owner. | `itPeachTurnip_Logic56_PickedUp`, `Thrown`, and `Dropped` select item motions; `UnkMotion3_Coll` handles terrain; `DmgDealt`, `Clanked`, `HitShield`, and `ShieldBounced` bounce or change motion; `Reflected` reverses ownership/trajectory; `EvtUnk` clears the owner when referenced. |
| Peach Parasol | 103 | `it_802BDA64` in `it/kinds/itpeachparasol.c` creates and attaches the parasol to Peach's part 109. `ftPe_SpecialHi_8011D424` owns creation and hitlag callbacks. | `itPeachParasol_Logic60_Destroyed` detaches or releases the owner; `PickedUp` restores pickup state; `UnkMotion2_Anim` ends the article when `ftPe_SpecialHi_NotActive` becomes true; `EvtUnk` forwards reference cleanup. |
| Toad | 104 | `ftPe_SpecialN`'s `onAccessory4` calls `it_802BDE18` in `it/kinds/itpeachtoad.c`, which attaches Toad to Peach part 109. The fighter keeps `toad_gobj` and checks `ftPe_SpecialN_IsActive`. | `itPeachToad_Logic91_Destroyed` and `it_802BDF40` notify the owner and destroy the article; animation callbacks end at source frames 53/60 or when the owner leaves the special; `PickedUp` and `EvtUnk` handle item lifecycle. |
| Toad spore | 111 | `doHitAccessory4` in `ft/kinds/ftPeach/ftpeachspecialn.c` calls `it_802BE214` in `it/kinds/itpeachtoadspore.c`. Spawn stores owner, uses a 60-frame lifetime, samples speed and angle from `itPeachToadSporeAttributes`, and emits effect 0x4D3. | `DmgDealt`, `Clanked`, `HitShield`, and `Absorbed` destroy effects and consume the spore; `ShieldBounced` reflects; `Reflected` preserves remaining lifetime while reversing; `EvtUnk` forwards reference cleanup. |

## Turnip weighted and held/thrown contract

`ftPe_SpecialLw_Enter` first calls `throwVegIfHeld`. A held Peach turnip is
sent to the common light forward throw; Peach does not pull another article
in that branch. Otherwise `spawnVeg` obtains the article position from part
109, calls `getVeg`, and stores the spawned item as both `item_gobj` and
`veg_gobj` (`ftpeachspeciallw.c:84-123`). `getVeg` normally returns a turnip,
with the source's special-item gate and three-entry item chance table applied
before the spawn.

The turnip article has a lifetime, a table length, and entries containing
`odds` and `damage` (`itCharItems.h:556-563`). `it_802BD32C` draws
`HSD_Randi(sum)` and selects the first cumulative interval. On a throw,
`itPeachTurnip_Logic56_Thrown` applies the selected type's damage to the hit
capsule and switches to falling projectile physics. The owner and selected
variant remain article state, so a generic gravity projectile is insufficient
for pickup, drop, rethrow, reflection, and owner cleanup.

### Exact deterministic test vector

For the direct turnip type-selection function, use seed `11027`, a table of
three entries `[(odds=50, damage=5), (30, 10), (20, 20)]`, and total odds 100.
The HSD step produces upper-16-bit value `36048`, so
`HSD_Randi(100) = 55`; the selected index is `1`, and the stored thrown damage
must be `10`. Set lifetime to `300.0`, owner to fighter 0, held flag to false,
and scale to `1.0`. The spawn result must retain all four values before the
first pickup or throw callback.

## Toad and spore timing

`ftPe_SpecialN` arms `onAccessory4` on entry and creates Toad once. Command
variable 1 moves the fighter from SpecialN/SpecialAirN to the hit phases;
the article remains attached through hitlag callbacks. When the hit callback
arms `onHitAccessory4`, the spore is created at part 109 with `y += 2.5`,
`z = 0`, and the fighter's facing. Toad's owner checks end the item when the
fighter leaves the native special range, while spore motion decays both
velocity components by `x8_speed_decay_rate` each frame.

## Parasol and Bomber ownership

The parasol has an attachment relationship and a carried-item handoff. During
`ftPe_SpecialHi_8011D424`, an existing ordinary parasol is preserved while a
Peach parasol is attached; `ftPe_8011D518` releases the preserved item when
the Peach article ends. The item itself checks the owner's special/fall motion
range before detaching.

The Bomber is a short-lived owner-linked article. `it_802BD248` sets its timer
to 60 frames, initializes effects and command state, and chooses one of two
article animations from the source boolean. It does not implement a generic
gravity trajectory; its observable behavior is lifetime, hitbox/effect state,
owner cleanup, and persistence after damage.

## Fighter callback audit

The pinned fighter sources were audited alongside the article callbacks:

* `ftpeachfloat.c` and `ftpeachfloatfall.c` keep Float, FloatAttack, and
  FloatFall as a separate native motion family. The current script boundary
  has no float input, velocity, or aerial attack state, so it does not claim
  Float coverage.
* `ftpeachspecials.c` enters AirSJump on an unblocked start, moves to AirSEnd
  when command 3 is observed during AirSJump or when its animation completes,
  and uses command 2 for the wall end. The portable script maps command 3 to
  the available command edge and keeps the animation completion fallback.
* `ftpeachspecialn.c` uses command variable 1 to arm the native shield
  callback. The fighter changes to SpecialNHit only when the Toad shield
  callback fires at animation frame 9. The portable `before_hit` hook has no
  typed Toad shield payload, so its phase bridge is an intentionally broad
  host approximation; it does not claim generic-hit or Toad-article parity,
  and it leaves the frame-nine restart unsupported.
* `ftpeachspeciallw.c` sends a held turnip to the common light throw and
  otherwise spawns an article; weighted turnip selection remains article
  host work.
* `ftpeachspecialhi.c` owns parasol attachment and fall-special handoff;
  `itpeachexplode.c` owns the Bomber's 60-frame article lifetime. Their
  object and effect callbacks remain outside the fighter script boundary.

## Current host boundary

The host exposes numeric article IDs and a compact `spawn_article` command, but
the article resource linker currently rejects Peach IDs 98, 99, 103, 104, and
111 as native callback-owned. The runtime has no attached article object,
pickup/drop/throw handoff, weighted article variant state, article effect
cleanup, or owner-active callback. The smallest useful extension is a typed
turnip family with weighted variants and deterministic selection, followed by
an ownership state for held articles. Toad, Parasol, and Bomber should remain
explicitly blocked until those object and effect seams exist.
