//! Scheduler bridge for fighter-owned hit attribution and combo state.

use super::{Fighter, State};
use crate::fighter::combo;

pub(crate) fn update(fighters: &mut [Fighter; 2], active: [bool; 2]) {
    for attacker in 0..2 {
        if !active[attacker] {
            continue;
        }
        let Some(victim) = fighters[attacker].combo.victim else {
            combo::update_retention(&mut fighters[attacker].combo, false, 0);
            continue;
        };
        let victim_hitstun = fighters[victim].hitstun != 0;
        let victim_escape_timer = fighters[victim].combo.escape_timer;
        combo::update_retention(
            &mut fighters[attacker].combo,
            victim_hitstun,
            victim_escape_timer,
        );
    }
}

/// `reacted` distinguishes a hit that reaches the victim's own reaction
/// (`ftCo_8008DCE0`, the ordinary case, and every `grab.rs` caller) from a
/// zero-knockback hit that decomp's `ftCo_8008EC90` skips entirely
/// (`damage::apply_hit`'s own gate, `docs/parity.md`): the attacker's own
/// `combo::record` (last move landed, combo count) runs either way -- it is
/// the attacker's own bookkeeping, set independently of whether the victim
/// ever flinches -- but the victim's own `last_hit_by`/`last_hit_by_instance`
/// stay at their prior (no-hit) value when the reaction itself never fires.
pub(crate) fn record_hit(
    state: &mut State,
    attacker: usize,
    victim: usize,
    attack_id: u16,
    rules: &combo::Rules,
    reacted: bool,
) {
    let [first, second] = &mut state.fighters;
    let (source, target) = if attacker == 0 {
        (first, second)
    } else {
        (second, first)
    };
    combo::record(&mut source.combo, victim, attack_id, rules);
    if reacted {
        target.combo.last_hit_by = Some(attacker);
        target.combo.last_hit_by_instance = source.action_instance.id;
    }
}

pub(crate) fn push(fighter: &mut Fighter, rules: &combo::Rules) {
    combo::push(
        &mut fighter.combo,
        &mut fighter.position,
        fighter.facing,
        [fighter.floor_normal[0], fighter.floor_normal[1]],
        fighter.grounded,
        fighter.grab.victim.is_some(),
        rules,
    );
}
