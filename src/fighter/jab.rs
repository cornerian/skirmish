//! Jab-combo input arithmetic from `ftCo_Attack1.c` (the first, second and
//! third jab with their buffered follow-up windows) and `ftCo_Attack100.c`
//! (the rapid-jab entry count and loop continuation).

/// `Fighter::unk_msid` restricted to the jabs that can follow up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    First,
    Second,
    Third,
}

/// `ftCo_Attack1_CheckInput` with a fresh logical A press: inside the
/// follow-up window (`hitlag_mul`, reused as the jab timer) with the script's
/// combo flag raised, the remembered jab selects its follow-up; any other
/// remembered jab yields nothing; otherwise a new first jab starts.
pub fn wait_press(window: f32, follow_up: bool, last: Option<Stage>) -> Option<Stage> {
    if window > 0.0 && follow_up {
        match last {
            Some(Stage::First) => Some(Stage::Second),
            Some(Stage::Second) => Some(Stage::Third),
            _ => None,
        }
    } else {
        Some(Stage::First)
    }
}

/// The window counts down once per frame the jab check runs without a press.
pub fn decay(window: &mut f32) {
    if *window > 0.0 {
        *window -= 1.0;
    }
}

/// `checkAttack12` / `checkAttack13`: the window counts down every frame of
/// the current jab and latches a press inside it; the follow-up starts once
/// the script's combo flag is raised. Returns whether it starts.
pub fn buffer_follow_up(
    window: &mut f32,
    a_pressed: bool,
    buffered: &mut bool,
    follow_up: bool,
) -> bool {
    if *window > 0.0 {
        *window -= 1.0;
        if a_pressed {
            *buffered = true;
        }
    }
    *buffered && follow_up
}

/// `ftCo_Attack_800D6A50`: every press or release of the logical A counts;
/// reaching the rapid window with the script's rapid flag raised starts the
/// rapid jab.
pub fn rapid_count(
    count: &mut i32,
    a_pressed: bool,
    a_released: bool,
    rapid_window: i32,
    rapid: bool,
) -> bool {
    if a_pressed || a_released {
        *count += 1;
    }
    *count >= rapid_window && rapid
}

/// `ftCo_Attack100Loop_Anim` at the script's loop check: the loop ends when
/// it has started and no A activity was latched since the previous check;
/// otherwise the latch clears for the next cycle. Returns whether it ends.
pub fn loop_check(started: bool, latched: &mut bool) -> bool {
    if started && !*latched {
        return true;
    }
    *latched = false;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_press_follows_the_remembered_jab_only_inside_the_window() {
        assert_eq!(
            wait_press(3.0, true, Some(Stage::First)),
            Some(Stage::Second)
        );
        assert_eq!(
            wait_press(3.0, true, Some(Stage::Second)),
            Some(Stage::Third)
        );
        assert_eq!(wait_press(3.0, true, Some(Stage::Third)), None);
        assert_eq!(wait_press(3.0, true, None), None);
        assert_eq!(
            wait_press(0.0, true, Some(Stage::First)),
            Some(Stage::First)
        );
        assert_eq!(
            wait_press(3.0, false, Some(Stage::First)),
            Some(Stage::First)
        );
    }

    #[test]
    fn follow_up_latches_inside_the_window_and_starts_on_the_flag() {
        let (mut window, mut buffered) = (2.0, false);
        assert!(!buffer_follow_up(&mut window, true, &mut buffered, false));
        assert_eq!((window, buffered), (1.0, true));
        assert!(buffer_follow_up(&mut window, false, &mut buffered, true));
        assert_eq!(window, 0.0);
        let mut late = false;
        assert!(!buffer_follow_up(&mut window, true, &mut late, true));
        assert!(!late);
        let mut timer = 1.0;
        decay(&mut timer);
        decay(&mut timer);
        assert_eq!(timer, 0.0);
    }

    #[test]
    fn rapid_count_needs_the_flag_and_counts_releases() {
        let mut count = 0;
        assert!(!rapid_count(&mut count, true, false, 2, true));
        assert!(rapid_count(&mut count, false, true, 2, true));
        assert!(!rapid_count(&mut count, false, false, 2, false));
        assert_eq!(count, 2);
    }

    #[test]
    fn loop_check_ends_without_activity_and_clears_the_latch() {
        let mut latched = true;
        assert!(!loop_check(true, &mut latched));
        assert!(!latched);
        assert!(loop_check(true, &mut latched));
        assert!(!loop_check(false, &mut latched));
    }
}
