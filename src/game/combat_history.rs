//! Scheduler bridge for fighter-owned hit attribution and combo state.

use super::{Fighter, State};
use crate::fighter::combo;

pub(crate) fn update(fighters: &mut [Fighter; 2], active: [bool; 2]) {
    for attacker in 0..2 {
        if !active[attacker] {
            continue;
        }
        combo::tick(&mut fighters[attacker].combo);
        let Some(victim) = fighters[attacker].combo.victim else {
            continue;
        };
        let victim_hitstun = fighters[victim].hitstun != 0;
        let victim_escape_timer = fighters[victim].combo.escape_timer;
        combo::clear_escaped_victim(
            &mut fighters[attacker].combo,
            victim_hitstun,
            victim_escape_timer,
        );
    }
}

pub(crate) fn record_hit(state: &mut State, attacker: usize, victim: usize, attack_id: u16) {
    let [first, second] = &mut state.fighters;
    let (source, target) = if attacker == 0 {
        (first, second)
    } else {
        (second, first)
    };
    combo::record(&mut source.combo, victim, attack_id);
    target.combo.last_hit_by = Some(attacker);
}
