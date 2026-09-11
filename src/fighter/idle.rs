//! Pure idle-animation selection from `getAnimID` (`ftwaitanim.c:47-60`) and
//! `inlineA0`'s re-draw gate (`ftwaitanim.c:39-45`, consulted by
//! `ftCo_8008A7A8`'s do/while at `ftwaitanim.c:75-79`). Callers (`game::idle`)
//! own the frame/length bookkeeping, the RNG and the no-table restart path;
//! this module only walks a weighted table.

/// `getAnimID`'s walk plus `ftCo_8008A7A8`'s re-draw loop:
///
/// ```c
/// int max = HSD_Randi(100) + 1;
/// int count = 0;
/// while (wait_data->u.i.x != -1) {
///     count += wait_data->u.i.y;
///     if (max <= count) {
///         return (enum_t) wait_data->u.p.x;
///     }
///     wait_data += 1;
/// }
/// HSD_ASSERTREPORT(86, 0, "wait anim data illegal!!\n", max);
/// ```
///
/// and:
///
/// ```c
/// do {
///     temp = anim_id = getAnimID(arg1);
/// } while (!inlineA0(fp) && fp->anim_id == temp);
/// ```
///
/// `entries` yields `(sub_motion, weight)` pairs in `WaitStruct` order (the
/// `-1` terminator is implicit as the end of the sequence, not a sentinel
/// value); it is walked again on every re-draw, so it must be cheaply
/// `Clone` (an iterator over a slice, not a one-shot generator). `draws`
/// supplies each call's raw `HSD_Randi(100)` result (`0..100`); this
/// function adds the source's own `+ 1`. `current` is `fp->anim_id` at
/// entry (the animation about to be replaced). Returns the picked
/// sub-motion and the number of `HSD_Randi` calls consumed (at least 1;
/// more only while the pick keeps repeating a non-Wait1/-31 `current`).
///
/// Panics if a draw's accumulated weight never reaches `max` (the table
/// falls off the end, `HSD_ASSERTREPORT`'s condition in the source):
/// callers that need to observe this instead of aborting (the C oracle)
/// walk the table themselves; `game::idle::validate` rejects tables that
/// can trigger it before a match is ever constructed.
pub fn pick(
    entries: impl Iterator<Item = (u32, i32)> + Clone,
    mut draws: impl FnMut() -> i32,
    current: u32,
) -> (u32, u32) {
    let mut used = 0u32;
    loop {
        let max = draws() + 1;
        used += 1;
        let mut count = 0;
        let mut picked = None;
        for (animation, weight) in entries.clone() {
            count += weight;
            if max <= count {
                picked = Some(animation);
                break;
            }
        }
        let picked = picked.expect("idle animation weights must sum to at least `max`");
        // inlineA0: accepted unconditionally from Wait1_0 (2) or 31; from
        // any other current animation, re-draw while the pick repeats it.
        if current == 2 || current == 31 || picked != current {
            return (picked, used);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<(u32, i32)> {
        vec![(2, 60), (3, 40)]
    }

    #[test]
    fn draw_at_or_below_the_first_weight_selects_the_first_entry() {
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
        let mut draws = [59].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
    }

    #[test]
    fn draw_above_the_first_weight_selects_the_second_entry() {
        let mut draws = [60].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (3, 1)
        );
        let mut draws = [99].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (3, 1)
        );
    }

    #[test]
    fn a_repeated_pick_from_wait1_or_31_is_accepted_without_a_redraw() {
        let mut draws = [0, 0, 0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 2),
            (2, 1)
        );
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 31),
            (2, 1)
        );
    }

    #[test]
    fn a_repeated_pick_from_a_non_wait1_animation_redraws() {
        // current = 3 (Wait2); the first draw (0) picks 2, which differs
        // from 3, so it is accepted on the first draw despite current != 2.
        let mut draws = [0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 1)
        );
        // current = 2 is the accepted-unconditionally case; force a repeat
        // of a non-Wait1 current instead: current = 3, first draw picks 3
        // again (>= 60), second draw picks 2.
        let mut draws = [60, 0].into_iter();
        assert_eq!(
            pick(table().into_iter(), || draws.next().unwrap(), 3),
            (2, 2)
        );
    }

    #[test]
    #[should_panic(expected = "weights must sum")]
    fn a_draw_past_the_accumulated_weight_panics() {
        let short = vec![(2, 50)];
        pick(short.into_iter(), || 99, 2);
    }
}
