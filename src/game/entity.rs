//! Bounded identities for match entities.
//!
//! An entity id is a slot index plus a generation.  The generation makes a
//! handle captured by a callback stale as soon as its slot is released and
//! reused.  Ownership is kept as a separate value so a port can own several
//! entities without making a port number itself serve as an entity identity.

use serde::{Deserialize, Serialize};

/// Maximum number of match entities represented by the generic substrate.
///
/// This is deliberately a fixed array: entity allocation is match-local and
/// must not allocate during a frame.  The initial two fighter entities leave
/// room for bounded secondary entities in later stages.
pub const MAX_ENTITIES: usize = 16;

/// Stable reference to one slot in an [`EntityStore`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId {
    index: u8,
    generation: u32,
}

impl EntityId {
    pub const INVALID: Self = Self {
        index: u8::MAX,
        generation: 0,
    };

    pub const fn index(self) -> usize {
        self.index as usize
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Match ownership identity.  A port may own multiple entities, distinguished
/// by an ordinal that is stable for the lifetime of that entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityOwner {
    pub port: u8,
    pub ordinal: u16,
}

/// Generic secondary-entity payload. It contains no fighter-specific rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityPayload {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// Remaining simulation steps; `None` means explicit removal is required.
    pub lifetime: Option<u32>,
}

impl EntityOwner {
    pub const fn new(port: u8, ordinal: u16) -> Self {
        Self { port, ordinal }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
struct EntitySlot {
    generation: u32,
    owner: Option<EntityOwner>,
    payload: EntityPayload,
}

/// Fixed-capacity identity and ownership table for generic match entities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityStore {
    slots: [EntitySlot; MAX_ENTITIES],
    len: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityError {
    Capacity,
    GenerationExhausted,
    InvalidOwnerPort,
    OwnerAlreadyExists,
}

impl std::fmt::Display for EntityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capacity => f.write_str("entity capacity exhausted"),
            Self::GenerationExhausted => f.write_str("entity generation exhausted"),
            Self::InvalidOwnerPort => f.write_str("entity owner port exceeds u8 capacity"),
            Self::OwnerAlreadyExists => f.write_str("entity owner identity already exists"),
        }
    }
}

impl std::error::Error for EntityError {}

impl Default for EntityStore {
    fn default() -> Self {
        Self {
            slots: [EntitySlot::default(); MAX_ENTITIES],
            len: 0,
        }
    }
}

impl EntityStore {
    /// Build the initial table for the two existing fighter ports.
    pub fn for_fighters(ports: [u32; 2]) -> Result<Self, EntityError> {
        let mut store = Self::default();
        for port in ports {
            let port = u8::try_from(port).map_err(|_| EntityError::InvalidOwnerPort)?;
            // Each fighter is the leader for its port. Secondary entities
            // owned by that port start at ordinal 1.
            store.insert(EntityOwner::new(port, 0))?;
        }
        Ok(store)
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, owner: EntityOwner) -> Result<EntityId, EntityError> {
        self.insert_with_payload(owner, EntityPayload::default())
    }

    pub fn insert_with_payload(
        &mut self,
        owner: EntityOwner,
        payload: EntityPayload,
    ) -> Result<EntityId, EntityError> {
        if self.find_owner(owner).is_some() {
            return Err(EntityError::OwnerAlreadyExists);
        }
        let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.owner.is_none())
        else {
            return Err(EntityError::Capacity);
        };
        let generation = slot
            .generation
            .checked_add(1)
            .ok_or(EntityError::GenerationExhausted)?;
        slot.generation = generation;
        slot.owner = Some(owner);
        slot.payload = payload;
        self.len += 1;
        Ok(EntityId {
            index: index as u8,
            generation,
        })
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.slot(id).is_some()
    }

    pub fn owner(&self, id: EntityId) -> Option<EntityOwner> {
        self.slot(id).and_then(|slot| slot.owner)
    }

    pub fn payload(&self, id: EntityId) -> Option<EntityPayload> {
        self.slot(id).map(|slot| slot.payload)
    }

    pub fn payload_mut(&mut self, id: EntityId) -> Option<&mut EntityPayload> {
        let slot = self.slots.get_mut(id.index())?;
        (slot.generation == id.generation && slot.owner.is_some()).then_some(&mut slot.payload)
    }

    /// Advance live entities in deterministic slot order without allocation.
    pub fn advance(&mut self) {
        for index in 0..MAX_ENTITIES {
            let slot = &mut self.slots[index];
            if slot.owner.is_none() {
                continue;
            }
            for axis in 0..3 {
                slot.payload.position[axis] += slot.payload.velocity[axis];
            }
            if let Some(lifetime) = &mut slot.payload.lifetime {
                *lifetime = lifetime.saturating_sub(1);
                if *lifetime == 0 {
                    slot.owner = None;
                    slot.payload = EntityPayload::default();
                    self.len -= 1;
                }
            }
        }
    }

    pub fn get_owned(&self, id: EntityId, owner: EntityOwner) -> Option<EntityId> {
        (self.owner(id) == Some(owner)).then_some(id)
    }

    pub fn find_owner(&self, owner: EntityOwner) -> Option<EntityId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            (slot.owner == Some(owner)).then_some(EntityId {
                index: index as u8,
                generation: slot.generation,
            })
        })
    }

    pub fn remove(&mut self, id: EntityId) -> bool {
        let Some(slot) = self.slots.get_mut(id.index()) else {
            return false;
        };
        if slot.generation != id.generation || slot.owner.is_none() {
            return false;
        }
        slot.owner = None;
        slot.payload = EntityPayload::default();
        self.len -= 1;
        true
    }

    pub fn remove_owned(&mut self, id: EntityId, owner: EntityOwner) -> bool {
        if self.owner(id) != Some(owner) {
            return false;
        }
        self.remove(id)
    }

    fn slot(&self, id: EntityId) -> Option<&EntitySlot> {
        let slot = self.slots.get(id.index())?;
        (slot.generation == id.generation && slot.owner.is_some()).then_some(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handle_cannot_resolve_after_slot_reuse() {
        let mut store = EntityStore::default();
        let old = store.insert(EntityOwner::new(0, 0)).unwrap();
        assert!(store.remove(old));
        let fresh = store.insert(EntityOwner::new(0, 1)).unwrap();
        assert_eq!(old.index(), fresh.index());
        assert_ne!(old.generation(), fresh.generation());
        assert!(!store.contains(old));
        assert_eq!(store.owner(fresh), Some(EntityOwner::new(0, 1)));
    }

    #[test]
    fn owners_are_exact_and_cannot_cross_ports_or_ordinals() {
        let mut store = EntityStore::default();
        let first = store.insert(EntityOwner::new(0, 0)).unwrap();
        let second = store.insert(EntityOwner::new(1, 0)).unwrap();
        assert_eq!(store.find_owner(EntityOwner::new(0, 0)), Some(first));
        assert_eq!(store.find_owner(EntityOwner::new(1, 0)), Some(second));
        assert_eq!(store.get_owned(first, EntityOwner::new(1, 0)), None);
        assert!(!store.remove_owned(first, EntityOwner::new(1, 0)));
        assert!(store.contains(first));
    }

    #[test]
    fn fighter_ports_have_distinct_initial_entities() {
        let store = EntityStore::for_fighters([0, 1]).unwrap();
        assert_eq!(store.len(), 2);
        assert_ne!(
            store.find_owner(EntityOwner::new(0, 0)),
            store.find_owner(EntityOwner::new(1, 0))
        );
        assert!(store.find_owner(EntityOwner::new(0, 0)).is_some());
        assert!(store.find_owner(EntityOwner::new(1, 0)).is_some());
    }

    #[test]
    fn cloned_store_round_trips_as_a_checkpoint() {
        let mut store = EntityStore::for_fighters([0, 1]).unwrap();
        let checkpoint = store.clone();
        let transient = store.insert(EntityOwner::new(0, 1)).unwrap();
        assert!(store.contains(transient));
        store = checkpoint.clone();
        assert_eq!(store, checkpoint);
        assert!(!store.contains(transient));
        assert_eq!(
            store
                .find_owner(EntityOwner::new(0, 0))
                .unwrap()
                .generation(),
            1
        );
    }

    #[test]
    fn owner_identity_cannot_alias_an_existing_entity() {
        let mut store = EntityStore::default();
        store.insert(EntityOwner::new(0, 0)).unwrap();
        assert_eq!(
            store.insert(EntityOwner::new(0, 0)),
            Err(EntityError::OwnerAlreadyExists)
        );
    }

    #[test]
    fn two_secondary_entities_advance_and_expire_deterministically() {
        let mut store = EntityStore::default();
        let first = store
            .insert_with_payload(
                EntityOwner::new(0, 1),
                EntityPayload {
                    position: [0.0, 1.0, 0.0],
                    velocity: [1.0, -1.0, 0.0],
                    lifetime: Some(2),
                },
            )
            .unwrap();
        let second = store
            .insert_with_payload(
                EntityOwner::new(1, 1),
                EntityPayload {
                    position: [4.0, 0.0, 0.0],
                    velocity: [-1.0, 0.0, 0.0],
                    lifetime: None,
                },
            )
            .unwrap();
        store.advance();
        assert_eq!(store.payload(first).unwrap().position, [1.0, 0.0, 0.0]);
        assert_eq!(store.payload(second).unwrap().position, [3.0, 0.0, 0.0]);
        store.advance();
        assert!(!store.contains(first));
        assert_eq!(store.payload(second).unwrap().position, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn capacity_is_bounded() {
        let mut store = EntityStore::default();
        for ordinal in 0..MAX_ENTITIES as u16 {
            store.insert(EntityOwner::new(0, ordinal)).unwrap();
        }
        assert_eq!(store.len(), MAX_ENTITIES);
        assert_eq!(
            store.insert(EntityOwner::new(1, 0)),
            Err(EntityError::Capacity)
        );
    }

    #[test]
    fn generation_exhaustion_does_not_reuse_a_stale_handle() {
        let mut store = EntityStore::default();
        store.slots[0].generation = u32::MAX;
        assert_eq!(
            store.insert(EntityOwner::new(0, 0)),
            Err(EntityError::GenerationExhausted)
        );
        assert_eq!(store.len(), 0);
    }
}
