# Link article parity

This is the source-backed article contract for Link and Young Link (CLink).
The pinned reference is `../../External/melee`, with source paths below relative
to `src/melee`.  Young Link uses the Link special callbacks, but selects the CLink
article kind where the two fighters differ (`ft/kinds/ftCLink/ftclink.c`).

## Article identities

These are `ItemKind` ordinals from `it/forward.h` (the `It_Kind_*` enum):

| Article | Link | Young Link | Native identity |
| --- | ---: | ---: | --- |
| Bomb | 58 | 59 | `It_Kind_Link_Bomb`, `It_Kind_CLink_Bomb` |
| Boomerang | 60 | 61 | `It_Kind_Link_Boomerang`, `It_Kind_CLink_Boomerang` |
| Hookshot | 62 | 63 | `It_Kind_Link_HShot`, `It_Kind_CLink_HShot` |
| Arrow | 64 | 65 | `It_Kind_Link_Arrow`, `It_Kind_CLink_Arrow` |

The bow is a fighter animation/resource (not the flying arrow article):
`LINK_BOW`/`YOUNG_LINK_BOW` are resource kinds 76/77 in the host catalog.

## Fighter-side dispatch

The neutral bow callbacks in `ft/kinds/ftLink/ftlinkspecialn.c` select an arrow
from charge state (`ftLk_SpecialN_GetIndex`, lines 45–64), draw/cancel through
the static `isDrawback` and `isDrawn` helpers (143–197), and release it from
the stage-specific IASA callbacks (`ftLk_SpecialNStart_IASA`,
`ftLk_SpecialNLoop_IASA`, and their air variants, 412–473).  The matching
`ftLk_SpecialNStart_Coll`, `ftLk_SpecialNLoop_Coll`, and
`ftLk_SpecialNEnd_Coll` callbacks (551–586) route the fighter after collision.
The release callback calls the arrow article's
`itLinkArrow_802A850C` (around 252–314).

Side special arms a dash-smash release through the command and boomerang
checks in `ftlinkspecials.c` (31–57), spawns and updates the boomerang in the
`onAccessory4` callback (133–198), and chooses ground/air state through the
enter callbacks (201–247).  Down special's `onAccessory4` and `updateBomb`
helpers in `ftlinkspeciallw.c` (29–83) create and
maintain the bomb.  Up special's air animation callback in
`ftlinkspecialhi.c` (71–87) ends in fall-special; its collision callback
(117–133) uses landing-fall-special on air contact.

## Arrow (64/65)

The item state table is in `it/kinds/itlinkarrow.c` (41–51).  Creation in
`it_802A83E0` (178–220) records the parent fighter, charge attributes, and the
arrow's owner attachment.  `itLinkArrow_802A850C` (252–314) releases it into
flight: it computes speed from charge, resolves the launch angle, assigns
velocity and gravity, and switches to the collision-enabled state.

Observable lifecycle:

1. **Drawn/attached:** fighter owns the dormant arrow while charging.
2. **Released:** owner and launch parameters are copied into the flying item.
3. **Flight:** `itLinkarrow_UnkMotion0_Phys`/the flight callbacks (around
   478–551) apply gravity and stage collision; a surface can stop/stick the
   arrow after angle clamping.
4. **Owner teardown:** `itLinkArrow_Logic98_Destroyed` (316–348) and
   `itLinkarrow_UnkMotion0_Anim` (375–405) clear the fighter attachment when the
   owner or animation is gone.  `it_802A8A7C` (421–428) hides and destroys the
   article.

## Boomerang (60/61)

`it/kinds/itlinkboomerang.c` defines four item states (23–29).  The creation
routine `it_802A013C` (145–198) records owner and article attributes.  The throw
routine `it_802A0534` (228–265) normalizes the requested angle and gives the
item its owner-relative launch.  `it_802A07B4` (265–286) removes the article.

The outbound, turnaround, and return behavior is driven by
`it_802A0C34` (378–404) and `itLinkboomerang_UnkMotion1_Anim` (around 490+),
including the life timer and trail.  The fighter callback's
`on21EC` gate checks the stick threshold and dash-smash window, then
`onAccessory4` computes the angle and calls
`it_802A0534`.  Therefore a host implementation needs a persistent owner link,
returning velocity/state, and explicit removal; a one-shot gravity projectile
cannot represent this article.

## Bomb (58/59)

The bomb state table has states 0–6 in `it/kinds/itlinkbomb.c` (18–31).
`it_8029DD58` (around 149+) creates it with parent and lifetime attributes.
`it_8029DB5C` (84–145) advances the timer and transitions to the explosion
state when the fuse reaches the attribute threshold.  The bounce/friction
callbacks `itLinkbomb_UnkMotion4_Anim`, `..._Phys`, and `..._Coll` (around
486–540) preserve velocity and apply surface response before detonation.
Generic item callbacks in the same file (`it_8029D968`, `it_8029D9A4`, and the
`itLinkbomb_UnkMotion*` family) cover pickup/owner transitions.

The parity state machine is therefore **spawned → carried/thrown → bouncing or
held → fused → explosion → destroyed**, with owner identity retained across
throw and pickup.

## Hookshot (62/63)

Hookshot is created by `it_802A2BA4` in `it/kinds/itlinkhookshot.c` (313–367).
It allocates a linked chain, records the owning fighter in `x8`, attaches the
chain to the fighter's hand/joint, and installs the article as the fighter's
accessory.  The item state table (37–58) has the initial state plus eight
physics states.

The active chain/collision path is `itLinkhookshot_UnkMotion5_Phys` and
`it_802A3630` (684–724): it tests grapple collision, retracts on fighter
collision, and transitions to hit/retract behavior on a successful target or
stage contact.  `it_802A2B10` (282–311) tears down the chain and clears all
fighter callbacks.  Pickup/attachment state callbacks are
`itLinkHookshot_Logic20_PickedUp` and `it_802A76EC`/`it_802A7764`
(2109–2133).

The parity state machine is **hand-attached → extending chain → stage/target
contact or fighter collision → retracting → hand-attached/destroyed**.  It
requires a persistent chain/attachment handle and contact callbacks rather than
just a projectile position.

## Exact deterministic test vector

Use `itLinkArrow_802A850C` as a host-independent arithmetic fixture.  Set
`facing = +1`, `angle = pi/6`, charge `arg4 = 0.5`, denominator `arg5 = 1.0`,
and synthetic arrow attributes `x4 = 2.0`, `x8 = 4.0`.  The source computes

```text
speed = 0.5 * ((x8 - x4) / arg5) + x4 = 3.0
velocity = (speed*cos(pi/6), speed*sin(pi/6))
         = (2.598076, 1.500000)  # approximately
```

The next physics tick applies the configured gravity (`x1C`) and collision
policy.  This vector is deliberately an explicit fixture: it validates source
math without depending on a particular stage or extracted character-attribute
table.

## Current host gap

The host already names IDs 58–65 in `src/game/script/resources.rs` and
`fighter.spawn_article` accepts a numeric ID in
`src/game/script/lifecycle/host.rs:646–696`.  The gap is behavior and lifetime:

* `ArticleBehavior` in `src/game/script/lifecycle/resources.rs:54–74` only
  models ray and gravity articles; Link IDs must not be silently treated as
  generic gravity (the resource linker rejects that fallback).
* `PendingArticleSpawn` in `src/game/projectile.rs:53–57` has only position and
  facing/angle-speed launch data.  It has no owner handle, attachment, chain,
  return state, fuse, pickup, or contact transition.
* The projectile drain can therefore support the arrow's initial launch math,
  but parity needs a typed stateful article interface: persistent article ID and
  owner, commands for attach/release/return/retract/pickup, per-article contact
  events, and explicit destruction/explosion.  Those additions should be
  reusable for both Link and CLink while keeping article-specific state machines
  isolated from fighter move selection.
