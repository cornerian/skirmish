//! Object ID lookup from `sysdolphin/baselib/id.c`, using standard hash maps.
//!
//! Store object handles as values: `Some(&0)` can represent a present null handle,
//! while `None` means the ID is absent. Bucket ordering and allocator internals
//! are intentionally outside this lookup API.

use std::collections::HashMap;

pub type IdTable<T> = HashMap<u32, T>;

#[derive(Debug)]
pub struct IdRegistry<T> {
    pub default: IdTable<T>,
}

impl<T> Default for IdRegistry<T> {
    fn default() -> Self {
        Self {
            default: IdTable::new(),
        }
    }
}

impl<T> IdRegistry<T> {
    pub fn insert(&mut self, table: Option<&mut IdTable<T>>, id: u32, data: T) -> Option<T> {
        table.unwrap_or(&mut self.default).insert(id, data)
    }

    pub fn remove(&mut self, table: Option<&mut IdTable<T>>, id: u32) -> Option<T> {
        table.unwrap_or(&mut self.default).remove(&id)
    }

    pub fn get<'a>(&'a self, table: Option<&'a IdTable<T>>, id: u32) -> Option<&'a T> {
        table.unwrap_or(&self.default).get(&id)
    }

    /// `HSD_IDSetup` resets only the default table.
    pub fn setup(&mut self) {
        self.default.clear();
    }

    /// `_HSD_IDForgetMemory` ignores both endpoints and clears the default table.
    pub fn forget_memory(&mut self, _low: usize, _high: usize) {
        self.setup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_handles_and_explicit_tables_survive_default_reset() {
        let mut registry = IdRegistry::default();
        let mut explicit = IdTable::new();
        registry.insert(None, 1, 0u32);
        registry.insert(Some(&mut explicit), 1, 7);
        assert_eq!(registry.get(None, 1), Some(&0));
        assert_eq!(registry.get(None, 2), None);
        registry.forget_memory(9, 0);
        assert_eq!(registry.get(None, 1), None);
        assert_eq!(registry.get(Some(&explicit), 1), Some(&7));
    }
}
