# Generic secondary entities

The stage 2 substrate keeps the existing `State::fighters: [Fighter; 2]`
interface intact. `State::entities` is a separate fixed-capacity table for
secondary entities, so adding entities does not clone sixteen `Fighter` values
on every step and does not allocate while advancing a frame.

Each entity has an `EntityId` containing a slot index and generation, plus an
`EntityOwner` `(port, ordinal)`. Fighter leaders occupy ordinal `0`; secondary
entities owned by a port begin at ordinal `1`. Owner identities are unique in a
store. Releasing a slot makes its old id stale, and reuse increments its
generation.

The current runnable payload is generic: position, velocity, and an optional
finite lifetime. `EntityStore::advance` walks slots in index order, applies one
fixed simulation step, and removes entities whose lifetime reaches zero. It
performs no allocation and no game-specific callback dispatch.

Future integration points are script owner routing, scheduler spawn/despawn
staging, collision/contact and KO policy hooks, and replay observations.
Because the store is already a field of `State`, cloned checkpoints carry its
identity and payload state atomically.
