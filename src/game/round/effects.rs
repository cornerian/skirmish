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
}
