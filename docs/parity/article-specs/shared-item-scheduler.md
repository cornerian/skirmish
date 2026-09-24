# Shared item and article scheduler parity specification

This document defines the observable ordering that the native item scheduler
and Skirmish's typed article path must preserve.  It covers gameplay state and
events; model animation, rendering, and audio callbacks are outside this
contract.

## Native scheduler

Items are created as class-9 GObjs in `Item_8026862C` (`melee/it/item.c:891-963`).
The setup priorities are the ordering contract:

1. `Item_802693E4`, priority 0, clears hitlag and transient item state.
2. `Item_80269528`, priority 1, advances item animation, runs the item
   animation callback, and decrements `xD44_lifeTimer`.  An expired item is
   destroyed before later callbacks (`melee/it/item.c:1280-1319`).
3. `Item_802697D4`, priority 4, calls the article's physics callback and then
   applies velocity and platform motion to `pos` (`melee/it/item.c:1366-1410`).
   Fox's ray physics callback snapshots the previous position in
   `itFoxlaser_UnkMotion1_Phys` (`melee/it/kinds/itfoxlaser.c:92-96`).
4. `Item_80269978`, priority 5, calls the article terrain callback and then
   publishes the transform (`melee/it/item.c:1372-1400`).  Fox's callback
   `itFoxlaser_UnkMotion1_Coll` performs the swept `it_8026E9A4` query and
   arms a one-frame lifetime on contact (`melee/it/kinds/itfoxlaser.c:98-107`).
5. Later item priorities run generic item contact, hit, and cleanup services.
   Their exact callbacks depend on the item kind; they may dispatch clank,
   absorb, reflect, shield, or fighter-hit behavior before the item is
   destroyed.  `Item_8026A8EC` is the native destruction path.

The fighter update completes before item priority 1 runs.  In the native
engine this is expressed by the separate fighter and item GObj priorities;
the host's explicit equivalent is `game::simulation::advance`: fighter
animation, script dispatch, physics, and fighter collision resolve first,
then `specials::emit_projectiles`, then `projectile::advance`
(`src/game/simulation.rs:584-849`).

## Typed host ordering

For each simulation frame, the required host sequence is:

1. **Fighter update.** Apply input, fighter scripts and action transitions,
   fighter animation, stage movement, fighter physics, and fighter collision.
   Script callbacks append typed `PendingProjectile` or
   `PendingArticleSpawn` values; they do not mutate the active article list.
2. **Primary and deferred spawn drain.** `specials::emit_projectiles`
   (`src/fighter/specials.rs:961-1100`) drains pending article spawns in player
   order, then pending projectiles in player order.  Each spawn appends to the
   active list and emits `Event::ProjectileSpawned`.
3. **Article lifetime.** `projectile::tick_lifetime` runs before article
   physics and collision (`src/game/projectile.rs:375-397`).  A non-positive
   timer removes the article without motion or contact.  A live article loses
   one frame before callbacks, matching `Item_80269528`.
4. **Article physics and motion.** Gravity articles apply gravity and terminal
   velocity, then every article advances by its current velocity.  The prior
   position is retained for swept terrain, shield, reflector, and hurtbox
   tests (`src/game/projectile.rs:398-405`).
5. **Terrain.** The host queries floor, ceiling, left wall, and right wall in
   that order and keeps the nearest contact.  Rays arm lifetime `1.0` and
   stop processing this frame.  Gravity articles reflect across the contact
   normal, apply `surface_multiplier`, and despawn below
   `terrain_stop_speed` (`src/game/projectile.rs:423-487`).
6. **Reflector.** If the target is reflecting and the typed policy or script
   contact hook accepts the contact, ownership and velocity reverse, optional
   half-life is restored, and `ProjectileReflected` is emitted.  This branch
   precedes ordinary shield and hurtbox contact
   (`src/game/projectile.rs:493-618`).
7. **Shield.** An active ordinary shield mirrors velocity across the swept
   contact normal and emits `ProjectileHit`; a typed `Despawn` shield policy
   removes the article first (`src/game/projectile.rs:620-677`).
8. **Hurtbox and damage.** Eligible hurtboxes are tested with swept capsules.
   Accepted damage goes through the normal hit-resolution pipeline, records
   staling, emits `ProjectileHit`, and removes the article unless its typed
   persistence policy is `Persist` (`src/game/projectile.rs:679-808`).
9. **Cleanup.** `advance` removes `Outcome::Despawn` entries immediately
   while retaining `Outcome::Keep` entries in stable vector order
   (`src/game/projectile.rs:350-369`).  Natural lifetime or terrain removal
   has no event; contact and reflection do.

## Secondary spawn behavior

Native item callbacks can create another item while the item GObj pass is in
progress.  The new object's visibility in the same frame is governed by its
GObj priority and insertion position; this is item-system behavior rather than
a fighter script queue.

The host currently stages commands into the owning fighter's pending queues.
Because `emit_projectiles` runs once before `projectile::advance`, a command
issued by a projectile contact hook after that drain is observed on the next
simulation frame.  This is deterministic and bounded, but it is a parity gap
for source callbacks that create a child item during the current item pass.

## Deterministic event-order vector

Use a frame with player 0's fighter callback staging one ray at position
`(0, 0, 0)`, lifetime `3`, and velocity `(1, 0)`.  The target has an ordinary
active shield intersecting the segment from `(0, 0)` to `(1, 0)`; no terrain,
reflector, or hurtbox contact is present.  Starting with no active articles
and an empty event list, the expected frame trace is:

```text
fighter callbacks        -> pending ray (no event yet)
spawn drain               -> ProjectileSpawned { owner: 0, kind: FoxLaser }
lifetime                  -> lifetime 2; article remains eligible
physics/motion            -> position (1, 0, 0)
terrain                   -> no contact
reflector                 -> no contact
shield                    -> velocity mirrors; ProjectileHit { owner: 0, victim: 1 }
hurtbox                   -> skipped after shield branch
cleanup                   -> article retained
```

If the same article instead starts with lifetime `0`, the trace ends after
the lifetime stage: there is no motion, contact, or event, and cleanup removes
it.  This catches the ordering regression where an expired article moved or a
shield/reflector early return skipped its timer decrement.

## Current gaps

- The host has no generic item GObj scheduler.  It models typed projectiles
  in one post-fighter pass and therefore cannot reproduce source priority and
  insertion behavior for child items created during that pass.
- Clank and absorb callbacks are not represented.  The current projectile path
  handles terrain, reflector, shield, and hurtbox contact only; no absorbing
  fighter exists in the supported host state.
- Moving-platform terrain uses current geometry only.  The native
  `mpCheckAllRemap` path can remap against previous line geometry; the host
  `projectile::advance` does not receive `previous_geometry`.
- Native generic item hitlag pause/resume and visual animation callbacks are
  not part of the typed gameplay state.
- Reflected damage uses the source scaling arithmetic, but the native global
  reflected-damage cap (`it_804D6D28->xD8`, `item.c:1613-1619`) is not sourced.
- Natural despawn has no event in both the host contract and the current
  observation model; only spawn, hit, and reflection are externally visible.
