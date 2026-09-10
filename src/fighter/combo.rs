//! Fighter-owned hit attribution from `ftColl_800763C0`/`ftColl_800764DC`.
//! The recorded "combo count" is retained state, not derived from damage.

use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub last_attack_landed: u16,
    pub count: u16,
    pub victim: Option<usize>,
    pub escape_timer: u16,
    pub last_hit_by: Option<usize>,
}

/// Exact non-self fighter branch of `ftColl_800763C0`; the caller supplies a
/// valid fighter index, corresponding to the original object pointer.
pub fn record(state: &mut State, victim: usize, attack_id: u16) {
    match state.victim {
        None => {
            state.last_attack_landed = attack_id;
            state.count = 1;
            state.victim = Some(victim);
        }
        Some(current) if current == victim => {
            if attack_id != 1 && state.last_attack_landed == attack_id {
                state.count = state.count.wrapping_add(1);
            } else {
                state.count = 0;
                state.last_attack_landed = attack_id;
            }
        }
        Some(_) => {}
    }
}

pub fn tick(state: &mut State) {
    state.escape_timer = state.escape_timer.saturating_sub(1);
}

pub fn finish_hitstun(state: &mut State, escape_frames: u16) {
    state.escape_timer = escape_frames;
}

pub fn clear_escaped_victim(attacker: &mut State, victim_hitstun: bool, victim_escape_timer: u16) {
    if attacker.victim.is_some() && !victim_hitstun && victim_escape_timer == 0 {
        attacker.victim = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_matches_the_retained_original_branches() {
        let mut state = State::default();
        record(&mut state, 1, 7);
        assert_eq!(
            (state.last_attack_landed, state.count, state.victim),
            (7, 1, Some(1))
        );
        record(&mut state, 1, 7);
        assert_eq!((state.last_attack_landed, state.count), (7, 2));
        record(&mut state, 1, 8);
        assert_eq!((state.last_attack_landed, state.count), (8, 0));
        record(&mut state, 1, 1);
        assert_eq!((state.last_attack_landed, state.count), (1, 0));
        record(&mut state, 0, 9);
        assert_eq!(
            (state.last_attack_landed, state.count, state.victim),
            (1, 0, Some(1))
        );
    }

    #[test]
    fn count_wrap_and_escape_timer_use_native_widths() {
        let mut attacker = State {
            last_attack_landed: 7,
            count: u16::MAX,
            victim: Some(1),
            ..State::default()
        };
        record(&mut attacker, 1, 7);
        assert_eq!(attacker.count, 0);
        let mut victim = State::default();
        finish_hitstun(&mut victim, 2);
        tick(&mut victim);
        clear_escaped_victim(&mut attacker, false, victim.escape_timer);
        assert_eq!(attacker.victim, Some(1));
        tick(&mut victim);
        clear_escaped_victim(&mut attacker, false, victim.escape_timer);
        assert_eq!(attacker.victim, None);
    }
}
