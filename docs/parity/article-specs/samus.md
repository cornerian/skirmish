# Samus article contract

This is the source contract for Samus's Charge Shot, Missile, and Bomb
articles.  The IDs are the portable resource IDs used by this repository;
the state and attribute names below retain the decomp's field offsets so an
article host can map them without inventing fighter-side semantics.

The pinned source paths are relative to `../../../../External/melee` from
this file.

## Article IDs and ownership

| Article | Native kind | `ArticleId` | Source entry |
| --- | --- | ---: | --- |
| Bomb | `It_Kind_Samus_Bomb` | `93` (`SAMUS_BOMB`) | `it/kinds/itsamusbomb.c`, `it_802B4AC8` |
| Charge Shot | `It_Kind_Samus_Charge` | `94` (`SAMUS_CHARGE`) | `it/kinds/itsamuschargeshot.c`, `it_802B55C8` |
| Missile | `It_Kind_Samus_Missile` | `95` (`SAMUS_MISSILE`) | `it/kinds/itsamusmissile.c`, `it_802B62D0` |

All three are spawned with the fighter as both parent references.  The
missile keeps a separate owner pointer and the bomb keeps one in
`samusbomb.owner`; Charge Shot stores its owner in `samuschargeshot.xE00`.
The owner links are cleared by each article's `EvtUnk` callback when the
referenced fighter is removed.

## Charge Shot

`it_802B55C8` creates the article and initializes lifetime from
`itSamusChargeShot_Attributes.lifetime`.  The seven remaining attribute
fields are used by `it_802B56E4` to interpolate the charge level into launch
speed (`x8..xC`), damage (`x10..x14`), and knockback (`x18..x1C`).  The
fighter supplies the charge value and maximum charge frame count; the item
clamps the value to `[0, max]`, then enters motion state 1 and writes the
resulting velocity from the supplied launch angle.

The item table has state 0 for the held or pre-release article and states 1
through 8 for the released motion variants.  `UnkMotion0_Coll` checks stage
contact through `it_802B5518`; the released states use
`UnkMotion8_Anim/Phys/Coll`.  A terrain contact does not imply fighter
damage, so the article remains item-owned until lifetime expiry or a native
contact callback consumes it.

The registered callbacks in `it_3F2F.c` are:

- `itSamusChargeshot_Logic108_DmgDealt`, `Clanked`, and `Absorbed`, each
  returning `true`;
- `itSamusChargeshot_Logic108_HitShield`, which returns `true`;
- `it_2725_Logic108_Reflected`, which reverses facing and adds pi to the
  launch angle; and
- `itSamusChargeshot_Logic108_EvtUnk`, which detaches the owner link.

The decomp's `DmgDealt` callback is intentionally a no-op (`return true`),
so repeat-hit policy belongs to the item collision scheduler and attack
instance, not to `combat_history`.

## Missile: normal, smash, and homing phases

`it_802B62D0` stores `is_smash_missile` and selects one of two initial motion
states:

| Variant | Initial state | Launch setup | Contact result |
| --- | ---: | --- | --- |
| Normal missile | 0 | `it_802B66A8`: horizontal velocity `attrs.xC * facing`, lifetime `attrs.x4` | `DmgDealt` calls `it_802B701C` unless already in terminal state 2 |
| Smash missile | 1 | `it_802B6A60`: horizontal velocity `attrs.x2C * facing`, lifetime `attrs.x24` | `DmgDealt` calls `it_802B70A0` unless already in terminal state 3 |

Normal state 0 and smash state 1 each have their own animation, physics, and
stage collision callbacks.  The normal missile begins homing after its
life timer reaches `attrs.x4 - attrs.x8`; the smash missile uses the
corresponding state 1 path.  `it_802B64FC` obtains the nearest eligible lock
target, computes the signed angular difference, and changes the turn field
by `attrs.x18`, clamped to `[-attrs.x1C, attrs.x1C]`.  The target-angle gate
uses `attrs.x20`.  Normal and smash acceleration use `attrs.x10`/`x14` and
`attrs.x30`/`x34` respectively.

Normal expiry and fighter contact enter state 2 through `it_802B701C`;
smash expiry and contact enter state 3 through `it_802B70A0`.  Both terminal
states run the generic article animation callback and no longer have motion
or collision callbacks.

The shared `Logic52` callbacks in `itsamusmissile.c` handle damage, clank,
and shield contact with the same normal-versus-smash terminal-state split.
`ShieldBounced` only bounces smash missiles.  `Reflected` reverses the
article, restores smash launch speed when applicable, and emits the
variant-specific effect.  `EvtUnk` detaches the owner when its reference is
the removed fighter.

## Bomb: ground, air, and explosion

`it_802B4AC8` creates the bomb and `it_802B4BA0` initializes its lifetime
from `itSamusBombAttributes.x0`, initial vertical velocity from the common
item attribute, and owner link.  The item table is:

| State | Source callbacks | Meaning |
| ---: | --- | --- |
| 0 | `UnkMotion0_Anim/Phys/Coll` | Initial airborne or pre-ground state |
| 1 | `UnkMotion1_Anim/Phys/Coll` | Grounded rolling state |
| 2 | `UnkMotion2_Anim/Phys/Coll` | Airborne state after `Logic50_EnteredAir` |
| 3 | `UnkMotion3_Anim` | Explosion; no physics or collision callback |

Ground contact enters state 1 through `it_802B4CF4`; leaving the ground
enters state 2 through `itSamusBomb_Logic50_EnteredAir`.  The explosion
transition `it_802B53CC` changes to state 3, enables the explosion hitbox,
and invokes the owner accessory callback once.  The explosion's hitbox is
therefore a new article phase, rather than a second fighter attack phase.

`Logic50_DmgDealt`, `Clanked`, and `HitShield` transition to explosion when
the current state is not 3 and return `false`.  `ShieldBounced` delegates to
`itColl_BounceOffShield`; `Reflected` delegates to `it_80273030`, adjusts a
downward reflected velocity by the article multiplier, clears the owner, and
returns `false`.  `Logic50_EvtUnk` also detaches the owner after the generic
event callback.

## Exact deterministic test vector

Use a normal missile with owner facing `+1` and the following fixture values
from `itSamusMissileAttributes`: `x4 = 60`, `x8 = 10`, `xC = 3`, `x10 = 0.5`,
`x14 = 4`, `x18 = 0.1`, `x1C = 0.25`, and `x20 = 0.0`.  Immediately after
`it_802B66A8`, the expected state is motion state `0`, lifetime `60`, and
velocity `(3, 0, 0)`.  After 50 animation ticks the homing gate is active;
with no eligible lock target, `it_802B64FC` leaves the turn field at `0` and
the velocity remains `(3, 0, 0)`.  A fighter contact then invokes
`it_2725_Logic52_DmgDealt` and enters terminal state `2`.

## Current host gap

The fighter script can express Samus's neutral, side, and down special
phases and typed article IDs, but the portable article host still lacks the
native state required for these contracts: owner-linked article instances,
per-article attribute layouts, lock-target selection and turn clamps,
state-specific stage collision, explosion hitbox replacement, and the
article contact callback table.  The current generic projectile path can
cover straight motion and ordinary fighter damage, but using it for these
articles would lose the normal/smash split, homing behavior, bomb state
transitions, and native owner callbacks.
