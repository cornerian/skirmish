//! Exact ordinary stale-move bookkeeping from plstale.c and ft_0881.c.
//! Ten physical slots retain duplicate-instance history; only the newest nine
//! affect damage. Fresh moves have factor 1.0, with no invented fresh bonus.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Fighter_804D6548, newest first. Values come from native common resources.
    pub penalties: [f32; 9],
    /// ft_80089228's DbLevel >= DbLKind_DebugRom branch.
    pub debug_bypass: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub move_id: u16,
    pub attack_instance: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Queue {
    next: u8,
    entries: [Entry; 10],
}

impl Queue {
    pub fn from_parts(next: u8, entries: [Entry; 10]) -> Option<Self> {
        (next < 10).then_some(Self { next, entries })
    }
    pub fn next(&self) -> u8 {
        self.next
    }
    pub fn entries(&self) -> &[Entry; 10] {
        &self.entries
    }
    /// plStale_ResetStaleMoveTableForPlayer. Other player statistics are external.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// plStale_UpdateStaleMovesFromFighter. Caller resolves owner/player routing;
    /// self-hits and move 1 do not stale. Duplicate search includes the tenth slot.
    pub fn record(&mut self, attack: Entry, self_hit: bool) -> bool {
        if self_hit || attack.move_id == 1 || self.entries.contains(&attack) {
            return false;
        }
        self.entries[usize::from(self.next)] = attack;
        self.next = if self.next == 9 { 0 } else { self.next + 1 };
        true
    }
    /// ft_80089118. A zero move marks an empty slot and stops scanning early.
    /// The attack-instance argument in the C multiplier is unused.
    pub fn multiplier(&self, move_id: i32, penalties: &[f32; 9]) -> f32 {
        let mut factor = 1.0;
        if move_id == 1 {
            return factor;
        }
        let mut index = if self.next == 0 { 9 } else { self.next - 1 };
        for penalty in penalties {
            let entry = self.entries[usize::from(index)];
            if entry.move_id == 0 {
                return factor;
            }
            if move_id == i32::from(entry.move_id) {
                factor -= penalty;
            }
            index = if index == 0 { 9 } else { index - 1 };
        }
        factor
    }
    /// ft_80089228. Multiplication is skipped when factor equals one, preserving
    /// the original no-op behavior for exceptional float values and signed zero.
    pub fn damage(&self, move_id: i32, base: f32, rules: &Rules) -> f32 {
        if rules.debug_bypass {
            return base;
        }
        let factor = self.multiplier(move_id, &rules.penalties);
        if factor != 1.0 { base * factor } else { base }
    }
}

/// The match-wide plstale.c sequence, independent of any one player's stock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct InstanceCounter(u16);
impl Default for InstanceCounter {
    fn default() -> Self {
        Self(1)
    }
}
impl InstanceCounter {
    pub fn from_next(next: u16) -> Option<Self> {
        (next != 0).then_some(Self(next))
    }
    pub fn next_value(self) -> u16 {
        self.0
    }
    pub fn allocate(&mut self) -> u16 {
        let before = self.0;
        self.0 = self.0.wrapping_add(1);
        if self.0 == 0 {
            self.0 = 1;
        }
        before
    }
}

/// ft_800890BC/ft_800890D0/ft_800892A0 identity operations.
impl Entry {
    pub const INACTIVE: Self = Self {
        move_id: 1,
        attack_instance: 0,
    };
    pub fn change_move(&mut self, move_id: u16, counter: &mut InstanceCounter) {
        if move_id == 1 || move_id != self.move_id {
            self.move_id = move_id;
            self.restart(counter);
        }
    }
    pub fn restart(&mut self, counter: &mut InstanceCounter) {
        self.attack_instance = counter.allocate();
    }
}
