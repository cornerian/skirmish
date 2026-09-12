#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::{
    compat::math::random::HsdRng,
    fighter::death::{Kind, Query, select},
};

unsafe extern "C" {
    fn oracle_blast_death(
        exclusions: u32,
        x: f32,
        y: f32,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        grounded: bool,
        forced_top_eligible: bool,
        knockback_y: f32,
        top_threshold: f32,
        normal_top: bool,
        disable_screen: bool,
        screen_chance: i32,
        ice: bool,
        seed: *mut u32,
    ) -> i32;
}

fn code(kind: Option<Kind>) -> i32 {
    match kind {
        None => 0,
        Some(Kind::Left) => 1,
        Some(Kind::Right) => 2,
        Some(Kind::Down) => 3,
        Some(Kind::Up) => 4,
        Some(Kind::UpStar) => 5,
        Some(Kind::UpStarIce) => 6,
        Some(Kind::UpScreen) => 7,
        Some(Kind::UpScreenIce) => 8,
    }
}

proptest! {
    #[test]
    fn blast_order_eligibility_selection_and_rng_match_original_c(
        exclusions in 0_u8..32,
        coordinates in any::<[f32; 6]>(),
        grounded in any::<bool>(),
        forced_top_eligible in any::<bool>(),
        knockback_y in any::<f32>(),
        top_threshold in any::<f32>(),
        normal_top in any::<bool>(),
        disable_screen in any::<bool>(),
        screen_chance in any::<i32>(),
        ice in any::<bool>(),
        seed in any::<u32>(),
    ) {
        let [x, y, left, right, bottom, top] = coordinates;
        let mut original_seed = seed;
        let original = unsafe {
            oracle_blast_death(
                exclusions.into(), x, y, left, right, bottom, top, grounded,
                forced_top_eligible, knockback_y, top_threshold, normal_top,
                disable_screen, screen_chance, ice, &mut original_seed,
            )
        };
        let mut rng = HsdRng::new(seed);
        let rust = select(
            Query {
                excluded: core::array::from_fn(|bit| exclusions & (1 << bit) != 0),
                position: [x, y],
                blast: [left, right, bottom, top],
                grounded,
                forced_top_eligible,
                knockback_y,
                top_knockback_threshold: top_threshold,
                force_normal_top: normal_top,
                camera_disables_screen: disable_screen,
                screen_chance_percent: screen_chance,
                ice,
            },
            &mut rng,
        );
        prop_assert_eq!(code(rust), original);
        prop_assert_eq!(rng.seed(), original_seed);
    }
}

#[test]
fn adapter_uses_the_complete_pinned_definition() {
    let original = include_str!("oracle/original/death.c");
    assert!(original.contains("bool ftCo_800D3158(Fighter_GObj* gobj)\n{"));
    let adapter = include_str!("oracle/death.c");
    assert!(adapter.contains("#include \"death_original.inc\""));
}
