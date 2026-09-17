//! Shared nonzero 16-bit allocation used by Melee's independent action and
//! stale-move instance globals.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Counter(u16);

impl Default for Counter {
    fn default() -> Self {
        Self(1)
    }
}

impl Counter {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_returns_the_current_value_and_wraps_past_zero() {
        assert!(Counter::from_next(0).is_none());
        let mut counter = Counter::from_next(u16::MAX).unwrap();
        assert_eq!(counter.allocate(), u16::MAX);
        assert_eq!(counter.next_value(), 1);
        assert_eq!(counter.allocate(), 1);
        assert_eq!(counter.next_value(), 2);
    }
}
