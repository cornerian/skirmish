//! `ftCo_8008A7A8`'s idle-animation pick/re-draw loop (`tests/oracle/
//! waitanim.c`, `tests/oracle/original/waitanim.c`) checked against
//! `fighter::idle::pick`. `pick` is a pure, deterministic weighted walk
//! plus a bounded re-draw loop; compared here bit-exact over generated
//! tables/current animations/RNG sequences, plus exact boundaries (`max ==
//! count` inclusive, the repeat re-draw, and tables whose weights fall
//! short of 100 asserting).
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::idle::pick;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_wait_anim(
        current_anim: i32,
        frames_remaining: bool,
        table: *const i32,
        table_len: i32,
        rng_sequence: *const i32,
        rng_len: i32,
        out_draws: *mut i32,
        out_asserted: *mut bool,
    ) -> i32;
}

/// `entries` is `(animation, weight)` pairs; `rng_sequence` supplies raw
/// `HSD_Randi(100)` results (`0..100`) in draw order. Returns `(new_anim,
/// draws, asserted)`.
fn oracle_pick(current: i32, entries: &[(u32, i32)], rng_sequence: &[i32]) -> (i32, i32, bool) {
    let table: Vec<i32> = entries
        .iter()
        .flat_map(|&(animation, weight)| [animation as i32, weight])
        .collect();
    let mut draws = 0;
    let mut asserted = false;
    // SAFETY: `table`/`rng_sequence` are valid for the call's duration;
    // `out_draws`/`out_asserted` are valid `&mut` locals.
    let animation = unsafe {
        oracle_wait_anim(
            current,
            false,
            table.as_ptr(),
            entries.len() as i32,
            rng_sequence.as_ptr(),
            rng_sequence.len() as i32,
            &mut draws,
            &mut asserted,
        )
    };
    (animation, draws, asserted)
}

fn rust_pick(current: u32, entries: &[(u32, i32)], rng_sequence: &[i32]) -> (u32, u32) {
    let mut index = 0;
    pick(
        entries.iter().copied(),
        || {
            let value = rng_sequence[index];
            index += 1;
            value
        },
        current,
    )
}

/// Equal-share weights summing to exactly 100 (remainder folded into the
/// last entry), animation ids `2..=6` (`ftCo_Submotion`, `forward.h:637-
/// 641`, plus one invented id beyond the common table): no single entry
/// exceeds roughly 100 / len, which -- combined with a 64-draw scripted
/// sequence -- keeps a worst-case run of consecutive re-draws
/// astronomically unlikely to exhaust the script (the probability of 64
/// straight collisions against a <= 50%-weighted entry is at most 0.5^64).
/// A `len == 1` table always uses animation 2 specifically, which can only
/// collide with an *exempt* current (`inlineA0`), so it never re-draws
/// regardless of the scripted sequence -- see `pick`'s own doc comment for
/// the re-draw gate this relies on.
fn equal_share_entries(len: usize) -> Vec<(u32, i32)> {
    let base = 100 / len as i32;
    let remainder = 100 - base * len as i32;
    (0..len)
        .map(|i| {
            let weight = if i + 1 == len { base + remainder } else { base };
            (2 + i as u32, weight)
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_pick_matches_the_oracle(
        current in prop_oneof![Just(2u32), Just(3), Just(6), Just(31), Just(40)],
        len in 1usize..=5,
        rng_sequence in prop::collection::vec(0i32..100, 64),
    ) {
        let entries = equal_share_entries(len);
        let (expected_animation, expected_draws) = rust_pick(current, &entries, &rng_sequence);
        let (actual_animation, actual_draws, asserted) =
            oracle_pick(current as i32, &entries, &rng_sequence);
        prop_assert!(!asserted, "equal-share weights always sum to exactly 100");
        prop_assert_eq!(actual_animation, expected_animation as i32);
        prop_assert_eq!(actual_draws, expected_draws as i32);
    }
}

#[test]
fn max_equal_to_the_accumulated_weight_is_inclusive() {
    // getAnimID: `if (max <= count) return ...`. A draw of 49 (max 50)
    // against a first entry weighted exactly 50 lands exactly on the
    // boundary and must still select it, not fall through to the next row.
    let entries = [(2u32, 50), (3u32, 50)];
    let (rust_animation, rust_draws) = rust_pick(2, &entries, &[49]);
    assert_eq!((rust_animation, rust_draws), (2, 1));
    let (oracle_animation, oracle_draws, asserted) = oracle_pick(2, &entries, &[49]);
    assert_eq!((oracle_animation, oracle_draws, asserted), (2, 1, false));

    // One past the boundary (max 51) falls through to the second entry.
    let (rust_animation, rust_draws) = rust_pick(2, &entries, &[50]);
    assert_eq!((rust_animation, rust_draws), (3, 1));
    let (oracle_animation, oracle_draws, asserted) = oracle_pick(2, &entries, &[50]);
    assert_eq!((oracle_animation, oracle_draws, asserted), (3, 1, false));
}

#[test]
fn a_repeated_pick_from_a_non_exempt_current_redraws_in_both_implementations() {
    // current = 3 (not exempt): the first draw (max 1) selects entry 0
    // (animation 3, weight 62 >= 1), which equals current and forces a
    // re-draw; the second draw (max 90) selects entry 1 (animation 2).
    let entries = [(3u32, 62), (2u32, 38)];
    let rng_sequence = [0, 89];
    let (rust_animation, rust_draws) = rust_pick(3, &entries, &rng_sequence);
    assert_eq!((rust_animation, rust_draws), (2, 2));
    let (oracle_animation, oracle_draws, asserted) = oracle_pick(3, &entries, &rng_sequence);
    assert_eq!((oracle_animation, oracle_draws, asserted), (2, 2, false));
}

#[test]
fn a_repeated_pick_from_wait1_or_31_never_redraws() {
    let entries = [(2u32, 100)];
    for current in [2u32, 31] {
        let (rust_animation, rust_draws) = rust_pick(current, &entries, &[0, 0, 0]);
        assert_eq!((rust_animation, rust_draws), (2, 1));
        let (oracle_animation, oracle_draws, asserted) =
            oracle_pick(current as i32, &entries, &[0, 0, 0]);
        assert_eq!((oracle_animation, oracle_draws, asserted), (2, 1, false));
    }
}

#[test]
fn weights_summing_below_100_assert_in_the_oracle_and_panic_in_pick() {
    // getAnimID (ftwaitanim.c:50-59): the walk falls off the end once a
    // draw's `max` exceeds the accumulated weight; HSD_ASSERTREPORT fires.
    let entries = [(2u32, 40), (3u32, 59)];
    let (_, _, asserted) = oracle_pick(2, &entries, &[99]); // max 100 > 99.
    assert!(asserted);

    let result = std::panic::catch_unwind(|| rust_pick(2, &entries, &[99]));
    assert!(
        result.is_err(),
        "pick must panic when the table falls off the end"
    );
}

#[test]
fn frames_still_remaining_draws_nothing_and_leaves_anim_id_unchanged() {
    let entries = [(2u32, 60), (3u32, 40)];
    let table: Vec<i32> = entries
        .iter()
        .flat_map(|&(animation, weight)| [animation as i32, weight])
        .collect();
    let mut draws = -1;
    let mut asserted = true;
    let rng_sequence = [0i32];
    // SAFETY: as in `oracle_pick`.
    let animation = unsafe {
        oracle_wait_anim(
            3,
            true, // frames_remaining
            table.as_ptr(),
            entries.len() as i32,
            rng_sequence.as_ptr(),
            rng_sequence.len() as i32,
            &mut draws,
            &mut asserted,
        )
    };
    assert_eq!(animation, 3, "unchanged: ftCo_8008A7A8 returns immediately");
    assert_eq!(draws, 0);
    assert!(!asserted);
}

#[test]
fn no_table_restarts_the_current_animation_without_drawing() {
    for current in [2i32, 3, 6, 31, 40] {
        let (animation, draws, asserted) = oracle_pick(current, &[], &[0, 0, 0]);
        assert_eq!(animation, current, "ftCo_8008A6D8(gobj, fp->anim_id)");
        assert_eq!(draws, 0);
        assert!(!asserted);
    }
}

#[test]
fn adapter_retains_the_complete_pinned_functions() {
    let source = include_str!("oracle/original/waitanim.c");
    assert!(source.contains("bool ftCo_8008A698(Fighter* fp)"));
    assert!(source.contains("void ftCo_8008A6D8(Fighter_GObj* gobj, s32 anim_id)"));
    assert!(source.contains("static inline bool inlineA0(Fighter* fp)"));
    assert!(source.contains("static inline enum_t getAnimID(WaitStruct* arg1)"));
    assert!(source.contains("void ftCo_8008A7A8(Fighter_GObj* gobj, WaitStruct* arg1)"));
    assert!(include_str!("oracle/waitanim.c").contains("#include \"waitanim_original.inc\""));
}
