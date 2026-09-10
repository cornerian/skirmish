//! Fighter-owned hit attribution from `ftColl_800763C0`/`ftColl_800764DC`.
//! The recorded "combo count" is retained state, not derived from damage.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Common x4C4: repeated-hit count that starts attacker separation.
    pub push_count: i32,
    /// Common x4C8: count that selects the larger separation distance.
    pub strong_push_count: i32,
    /// Common x4CC: victim grace after hitstun ends.
    pub escape_frames: u16,
    /// Common x4D0/x4D4: ordinary and strong floor-tangent distances.
    pub push_distance: [f32; 2],
    /// Common x4D8, narrowed into fighter x2092 on assignment.
    pub push_frames: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub last_attack_landed: u16,
    pub count: u16,
    pub victim: Option<usize>,
    pub escape_timer: u16,
    pub push_timer: u16,
    pub last_hit_by: Option<usize>,
    /// Source action instance copied into victim x18ec on the last hit.
    pub last_hit_by_instance: u16,
}

/// Exact non-self fighter branch of `ftColl_800763C0`; the caller supplies a
/// valid fighter index, corresponding to the original object pointer.
pub fn record(state: &mut State, victim: usize, attack_id: u16, rules: &Rules) {
    match state.victim {
        None => {
            state.last_attack_landed = attack_id;
            state.count = 1;
            state.victim = Some(victim);
        }
        Some(current) if current == victim => {
            if attack_id != 1 && state.last_attack_landed == attack_id {
                state.count = state.count.wrapping_add(1);
                if i32::from(state.count) >= rules.push_count {
                    state.push_timer = rules.push_frames as u16;
                }
            } else {
                state.count = 0;
                state.last_attack_landed = attack_id;
            }
        }
        Some(_) => {}
    }
}

pub fn update_retention(state: &mut State, victim_hitstun: bool, victim_escape_timer: u16) {
    state.escape_timer = state.escape_timer.saturating_sub(1);
    if state.victim.is_some() && !victim_hitstun && victim_escape_timer == 0 {
        state.victim = None;
    }
}

pub fn finish_hitstun(state: &mut State, rules: &Rules) {
    state.escape_timer = rules.escape_frames;
}

/// Exact arithmetic and eligibility in `ftColl_80076528`/`comboCount_Push`.
pub fn push(
    state: &mut State,
    position: &mut [f32; 2],
    facing: f32,
    floor_normal: [f32; 2],
    grounded: bool,
    holding_victim: bool,
    rules: &Rules,
) {
    if state.push_timer == 0 {
        return;
    }
    state.push_timer -= 1;
    if holding_victim || !grounded {
        return;
    }
    let distance =
        rules.push_distance[usize::from(i32::from(state.count) >= rules.strong_push_count)];
    let tangent = facing * distance;
    position[0] = -(floor_normal[1] * tangent - position[0]);
    position[1] = -(-floor_normal[0] * tangent - position[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_matches_the_retained_original_branches() {
        let rules = rules();
        let mut state = State::default();
        record(&mut state, 1, 7, &rules);
        assert_eq!(
            (state.last_attack_landed, state.count, state.victim),
            (7, 1, Some(1))
        );
        record(&mut state, 1, 7, &rules);
        assert_eq!((state.last_attack_landed, state.count), (7, 2));
        assert_eq!(state.push_timer, 3);
        record(&mut state, 1, 8, &rules);
        assert_eq!((state.last_attack_landed, state.count), (8, 0));
        record(&mut state, 1, 1, &rules);
        assert_eq!((state.last_attack_landed, state.count), (1, 0));
        record(&mut state, 0, 9, &rules);
        assert_eq!(
            (state.last_attack_landed, state.count, state.victim),
            (1, 0, Some(1))
        );
    }

    #[test]
    fn count_wrap_and_escape_timer_use_native_widths() {
        let rules = rules();
        let mut attacker = State {
            last_attack_landed: 7,
            count: u16::MAX,
            victim: Some(1),
            ..State::default()
        };
        record(&mut attacker, 1, 7, &rules);
        assert_eq!(attacker.count, 0);
        let mut victim = State::default();
        finish_hitstun(&mut victim, &rules);
        update_retention(&mut victim, false, 1);
        update_retention(&mut attacker, false, victim.escape_timer);
        assert_eq!(attacker.victim, Some(1));
        update_retention(&mut victim, false, 1);
        update_retention(&mut attacker, false, victim.escape_timer);
        assert_eq!(attacker.victim, None);
    }

    #[test]
    fn push_uses_the_source_floor_tangent_and_timer_order() {
        let mut state = State {
            count: 3,
            push_timer: 2,
            ..State::default()
        };
        let mut position = [4.0, 5.0];
        push(
            &mut state,
            &mut position,
            -1.0,
            [0.6, 0.8],
            true,
            false,
            &rules(),
        );
        assert_eq!(state.push_timer, 1);
        assert_eq!(position, [4.4, 4.7]);
        push(
            &mut state,
            &mut position,
            -1.0,
            [0.6, 0.8],
            false,
            false,
            &rules(),
        );
        assert_eq!(state.push_timer, 0);
        assert_eq!(position, [4.4, 4.7]);
    }

    fn rules() -> Rules {
        Rules {
            push_count: 2,
            strong_push_count: 4,
            escape_frames: 2,
            push_distance: [0.5, 1.0],
            push_frames: 3,
        }
    }
}
