//! Deterministic fixed-step scheduling for native presentation hosts.

use std::time::{Duration, Instant};

const TICKS_PER_SECOND: u8 = 60;
const BASE_TICK_NANOS: u64 = 1_000_000_000 / TICKS_PER_SECOND as u64;
const REMAINDER_NANOS: u8 = (1_000_000_000 % TICKS_PER_SECOND as u64) as u8;
const MAX_CATCH_UP_TICKS: u8 = 4;

/// A deadline-relative 60 Hz clock with bounded catch-up.
///
/// Callers supply every observation of monotonic time, keeping scheduling
/// deterministic in tests and independent of any particular event loop.
#[derive(Debug, Clone)]
pub struct FixedStepClock {
    next_tick: Instant,
    phase: u8,
}

impl FixedStepClock {
    /// Starts a clock whose first tick is due at `now`.
    pub fn new(now: Instant) -> Self {
        Self {
            next_tick: now,
            phase: 0,
        }
    }

    /// Returns whether at least one tick is due without advancing the clock.
    pub fn is_due(&self, now: Instant) -> bool {
        now >= self.next_tick
    }

    /// Advances through the ticks due at `now`, capped at four.
    ///
    /// Deadlines advance relative to their preceding deadline. If four ticks
    /// cannot clear the backlog, old debt is discarded and the next deadline
    /// is scheduled one 60 Hz interval after `now`.
    pub fn consume_due(&mut self, now: Instant) -> u8 {
        let mut due = 0;
        while self.is_due(now) && due < MAX_CATCH_UP_TICKS {
            self.advance_deadline();
            due += 1;
        }
        if self.is_due(now) {
            self.drop_debt(now);
        }
        due
    }

    /// Restarts the schedule with a tick due immediately at `now`.
    pub fn rebase(&mut self, now: Instant) {
        self.next_tick = now;
        self.phase = 0;
    }

    /// Returns the non-negative delay until the next scheduled tick.
    pub fn time_until_next(&self, now: Instant) -> Duration {
        self.next_tick.saturating_duration_since(now)
    }

    fn advance_deadline(&mut self) {
        let mut nanos = BASE_TICK_NANOS;
        self.phase += REMAINDER_NANOS;
        if self.phase >= TICKS_PER_SECOND {
            self.phase -= TICKS_PER_SECOND;
            nanos += 1;
        }
        self.next_tick += Duration::from_nanos(nanos);
    }

    fn drop_debt(&mut self, now: Instant) {
        self.next_tick = now;
        self.phase = 0;
        self.advance_deadline();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_tick_intervals_sum_to_exactly_one_second() {
        let start = Instant::now();
        let mut clock = FixedStepClock::new(start);

        for _ in 0..TICKS_PER_SECOND {
            clock.advance_deadline();
        }

        assert_eq!(
            clock.next_tick.duration_since(start),
            Duration::from_secs(1)
        );
        assert_eq!(clock.phase, 0);
    }

    #[test]
    fn due_ticks_are_deadline_relative_and_inclusive() {
        let start = Instant::now();
        let mut clock = FixedStepClock::new(start);

        assert!(clock.is_due(start));
        assert_eq!(clock.consume_due(start), 1);
        assert_eq!(
            clock.time_until_next(start),
            Duration::from_nanos(BASE_TICK_NANOS)
        );
        assert!(!clock.is_due(start + Duration::from_nanos(BASE_TICK_NANOS - 1)));

        let second_deadline = start + Duration::from_nanos(BASE_TICK_NANOS);
        assert_eq!(clock.consume_due(second_deadline), 1);
        assert_eq!(
            clock.next_tick.duration_since(start),
            Duration::from_nanos(2 * BASE_TICK_NANOS + 1)
        );
    }

    #[test]
    fn catch_up_is_capped_and_excess_debt_is_dropped() {
        let start = Instant::now();
        let delayed = start + Duration::from_secs(1);
        let mut clock = FixedStepClock::new(start);

        assert_eq!(clock.consume_due(delayed), MAX_CATCH_UP_TICKS);
        assert!(!clock.is_due(delayed));
        assert_eq!(
            clock.time_until_next(delayed),
            Duration::from_nanos(BASE_TICK_NANOS)
        );
        assert_eq!(
            clock.consume_due(delayed + Duration::from_nanos(BASE_TICK_NANOS)),
            1
        );
    }

    #[test]
    fn rebase_resets_deadline_and_rational_phase() {
        let start = Instant::now();
        let resumed = start + Duration::from_secs(10);
        let mut clock = FixedStepClock::new(start);
        assert_eq!(clock.consume_due(start), 1);
        assert_ne!(clock.phase, 0);

        clock.rebase(resumed);

        assert!(clock.is_due(resumed));
        assert_eq!(clock.consume_due(resumed), 1);
        assert_eq!(
            clock.time_until_next(resumed),
            Duration::from_nanos(BASE_TICK_NANOS)
        );
    }
}
