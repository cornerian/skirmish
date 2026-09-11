//! Pure match-start (Entry/EntryStart/EntryEnd) arithmetic. `game::entry`
//! owns the stateful side (spawn, action transitions, position writes);
//! see `docs/match-start.md`.
//! `melee/gm/gmvs.c:1601-1606` (per-slot entry delay), `:1780-1830`/
//! `:2198-2206` (facing), `melee/ft/ft_0C31.c:75-135` (the `1.497345`
//! amplitude literal and the EntryStart/EntryEnd progress-fraction shapes).

/// `gmvs.c:1603`: `tmp->unk_A += 5` accumulated once per processed slot
/// before the port is assigned its own delay, so port 0 (P1) gets 5, port 3
/// (P4) gets 20.
pub const DELAY_STEP: u32 = 5;

/// `gmvs.c:1601-1606`. `slot` is the 0-indexed port (P1=0 .. P4=3).
pub fn entry_delay(slot: u32) -> u32 {
    DELAY_STEP * (slot + 1)
}

/// `gmvs.c:1780-1830`, restricted to Skirmish's fixed two-slot, no-teams
/// scope (the "largest `|dx|` opponent" search is trivial with one other
/// occupied slot, and `Player_GetTeam` never applies). `spawns` is indexed
/// by Skirmish's own internal player index (0/1), independent of any real
/// Slippi port number; `player` selects which of the two gets a facing.
pub fn spawn_facing(spawns: [[f32; 2]; 2], player: usize) -> f32 {
    let opponent = 1 - player;
    let dx = spawns[opponent][0] - spawns[player][0];
    if dx < -5.0 {
        -1.0
    } else if dx > 5.0 {
        1.0
    } else if player == 1 {
        // gmvs.c processes slots in ascending order; by the time slot 1 is
        // considered, slot 0's facing (computed first, above) is final.
        -spawn_facing(spawns, 0)
    } else {
        // Slot 0 is always processed first, with no opponent facing yet
        // assigned (`Player_GetFacingDirection(var_r23) == 0.0F`'s `else`).
        1.0
    }
}

/// `ftCo_800C6408:117`: `1.497345 * (x34_scale.y * trophy_scale)`. Skirmish
/// keeps no separate uniform fighter scale, so `scale` is `trophy_scale`
/// alone (an absent resource contributes `0.0`, matching an unset optional
/// amplitude source rather than inventing a nonzero default). The literal
/// is an unsuffixed C double (`1.497345 * sp48.y` promotes `sp48.y` to
/// `double`, multiplies, then truncates back to `f32` on assignment to
/// `temp_f0_2`); computing directly in `f32` differs by up to a few ulps,
/// caught by `tests/entry_differential.rs`'s bit-exact oracle comparison.
pub fn amplitude(scale: f32) -> f32 {
    (1.497345_f64 * scale as f64) as f32
}

/// `ftCo_EntryStart_Phys:164`: `(x6BC - timer) / x6BC`, as the source's own
/// int-to-float cast and division (`temp_r6` is `s32`, matching `x6BC`'s
/// declared type in `ft/types.h:473`).
pub fn start_progress(timer: u32, start_frames: u32) -> f32 {
    (start_frames as f32 - timer as f32) / start_frames as f32
}

/// `ftCo_EntryEnd_Phys:291`: `timer / x6BC` -- the divisor is EntryStart's
/// own frame count (`x6BC`), not EntryEnd's (`x6C0`); cited directly in the
/// design note as a fact to preserve exactly.
pub fn end_progress(timer: u32, start_frames: u32) -> f32 {
    timer as f32 / start_frames as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_delay_matches_the_replay_verified_table() {
        assert_eq!(entry_delay(0), 5);
        assert_eq!(entry_delay(1), 10);
        assert_eq!(entry_delay(2), 15);
        assert_eq!(entry_delay(3), 20);
    }

    #[test]
    fn spawn_facing_faces_the_opponent_on_final_destination_spawns() {
        let spawns = [[-60.0, 10.0], [20.0, 10.0]];
        assert_eq!(spawn_facing(spawns, 0), 1.0);
        assert_eq!(spawn_facing(spawns, 1), -1.0);
    }

    #[test]
    fn spawn_facing_within_five_units_defaults_slot_zero_then_opposes_it() {
        let spawns = [[0.0, 0.0], [1.0, 0.0]];
        assert_eq!(spawn_facing(spawns, 0), 1.0);
        assert_eq!(spawn_facing(spawns, 1), -1.0);
    }

    #[test]
    fn spawn_facing_handles_a_reversed_layout() {
        // Player 0 spawns to the right of player 1: the old hardcode
        // (player 0 always +1) would face it away from its opponent.
        let spawns = [[60.0, 10.0], [-60.0, 10.0]];
        assert_eq!(spawn_facing(spawns, 0), -1.0);
        assert_eq!(spawn_facing(spawns, 1), 1.0);
    }

    #[test]
    fn amplitude_matches_the_literal() {
        assert_eq!(amplitude(0.9), (1.497345_f64 * 0.9_f64) as f32);
        assert_eq!(amplitude(0.0), 0.0);
    }

    #[test]
    fn progress_fractions_bound_zero_to_one() {
        assert_eq!(start_progress(30, 30), 0.0);
        assert_eq!(start_progress(0, 30), 1.0);
        assert_eq!(end_progress(30, 30), 1.0);
        assert_eq!(end_progress(0, 30), 0.0);
    }
}
