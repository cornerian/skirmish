//! Rollback-safe ownership of fighter-attached visual effects.

use serde::{Deserialize, Serialize};

/// A resource-backed effect owned by one fighter until explicitly cleared.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnedEffect {
    pub id: u32,
    pub resource: String,
    pub bone: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectState {
    pub next_id: u32,
    pub owned: Vec<OwnedEffect>,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            next_id: 1,
            owned: Vec::new(),
        }
    }
}

impl EffectState {
    pub const MAX_OWNED: usize = 32;

    pub fn spawn(&mut self, resource: String, bone: Option<u16>) -> Result<u32, &'static str> {
        if self.owned.len() >= Self::MAX_OWNED {
            return Err("too many owned effects");
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("effect id space exhausted")?;
        self.owned.push(OwnedEffect::new(id, resource, bone));
        Ok(id)
    }

    pub fn clear(&mut self) {
        self.owned.clear();
    }

    /// Remove one effect by its stable ownership handle.
    ///
    /// Source callbacks often tear down one fighter-owned effect while other
    /// effects remain active.  Retaining vector order keeps checkpoints and
    /// rollback serialization deterministic; IDs remain monotonic so a stale
    /// handle can never refer to a later effect.
    pub fn despawn(&mut self, id: u32) -> bool {
        let Some(index) = self.owned.iter().position(|effect| effect.id == id) else {
            return false;
        };
        self.owned.remove(index);
        true
    }

    /// Remove every instance of one resource while retaining other effects.
    ///
    /// This models source cleanup paths that destroy all instances of one
    /// effect family before replacing it, without requiring callers to retain
    /// every individual handle.
    pub fn clear_resource(&mut self, resource: &str) -> usize {
        let before = self.owned.len();
        self.owned.retain(|effect| effect.resource != resource);
        before - self.owned.len()
    }
}

impl OwnedEffect {
    pub fn new(id: u32, resource: String, bone: Option<u16>) -> Self {
        Self { id, resource, bone }
    }
}

#[cfg(test)]
mod tests {
    use super::EffectState;

    #[test]
    fn ids_remain_unique_after_clear_and_state_round_trips() {
        let mut state = EffectState::default();
        assert_eq!(state.spawn("effects/test".into(), Some(0)), Ok(1));
        let snapshot = serde_json::to_vec(&state).unwrap();
        state.clear();
        assert_eq!(state.spawn("effects/test".into(), None), Ok(2));
        assert_ne!(snapshot, serde_json::to_vec(&state).unwrap());
        let restored: EffectState = serde_json::from_slice(&snapshot).unwrap();
        assert_eq!(restored.next_id, 2);
        assert_eq!(restored.owned[0].id, 1);
    }

    #[test]
    fn ownership_bound_is_enforced() {
        let mut state = EffectState::default();
        for _ in 0..EffectState::MAX_OWNED {
            state.spawn("effects/test".into(), None).unwrap();
        }
        assert_eq!(
            state.spawn("effects/test".into(), None),
            Err("too many owned effects")
        );
    }

    #[test]
    fn despawn_removes_only_the_requested_effect_and_keeps_handles_monotonic() {
        let mut state = EffectState::default();
        let first = state
            .spawn("effects/pk-thunder-trail".into(), Some(1))
            .unwrap();
        let second = state
            .spawn("effects/pk-thunder-gfx".into(), Some(2))
            .unwrap();
        let third = state.spawn("effects/other".into(), None).unwrap();

        assert!(state.despawn(second));
        assert!(!state.despawn(second));
        assert_eq!(
            state
                .owned
                .iter()
                .map(|effect| effect.id)
                .collect::<Vec<_>>(),
            vec![first, third]
        );
        assert_eq!(state.spawn("effects/new".into(), None), Ok(4));
    }

    #[test]
    fn clear_resource_removes_all_matching_instances_only() {
        let mut state = EffectState::default();
        state
            .spawn("effects/pk-thunder-gfx".into(), Some(1))
            .unwrap();
        state
            .spawn("effects/pk-thunder-trail".into(), Some(2))
            .unwrap();
        state
            .spawn("effects/pk-thunder-gfx".into(), Some(3))
            .unwrap();

        assert_eq!(state.clear_resource("effects/pk-thunder-gfx"), 2);
        assert_eq!(state.owned.len(), 1);
        assert_eq!(state.owned[0].resource, "effects/pk-thunder-trail");
        assert_eq!(state.clear_resource("effects/missing"), 0);
        assert_eq!(state.spawn("effects/replacement".into(), None), Ok(4));
    }
}
