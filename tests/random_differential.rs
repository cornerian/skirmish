#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::compat::math::random::{HsdRng, HsdSeedContext, MslRng};
use std::sync::Mutex;

// Upstream reference functions intentionally retain their shared global seeds.
static ORACLE: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_hsd_set_seed(seed: u32);
    fn oracle_hsd_get_seed() -> u32;
    fn HSD_Rand() -> i32;
    fn HSD_Randf() -> f32;
    fn HSD_Randi(max: i32) -> i32;
    fn oracle_msl_set_seed(seed: u32);
    fn oracle_msl_get_seed() -> u32;
    fn oracle_msl_rand() -> i32;
    fn oracle_hsd_forget(fallback: u32, external: u32, low: u32, high: u32, output: *mut u32);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn hsd_mixed_draws_match_original_c(
        seed in any::<u32>(),
        draws in prop::collection::vec((0u8..3, any::<i32>()), 1..128),
    ) {
        let _guard = ORACLE.lock().unwrap();
        let mut rust = HsdRng::new(seed);
        // SAFETY: signatures match the linked fixed-width C wrapper; the mutex
        // prevents concurrent access to the upstream reference's global state.
        unsafe { oracle_hsd_set_seed(seed); }
        for (kind, bound) in draws {
            match kind {
                0 => prop_assert_eq!(rust.rand(), unsafe { HSD_Rand() }),
                1 => prop_assert_eq!(rust.randf().to_bits(), unsafe { HSD_Randf() }.to_bits()),
                _ => prop_assert_eq!(rust.randi(bound), unsafe { HSD_Randi(bound) }),
            }
            prop_assert_eq!(rust.seed(), unsafe { oracle_hsd_get_seed() });
        }
    }

    #[test]
    fn msl_draws_and_reseeding_match_original_c(
        seed in any::<u32>(), replacement in any::<u32>(), count in 1usize..128,
    ) {
        let _guard = ORACLE.lock().unwrap();
        let mut rust = MslRng::new(seed);
        unsafe { oracle_msl_set_seed(seed); }
        for seed in [seed, replacement] {
            rust.set_seed(seed);
            unsafe { oracle_msl_set_seed(seed); }
            for _ in 0..count {
                prop_assert_eq!(rust.rand(), unsafe { oracle_msl_rand() });
                prop_assert_eq!(rust.seed(), unsafe { oracle_msl_get_seed() });
            }
        }
    }

    #[test]
    fn forgetting_memory_matches_original_c(
        fallback in any::<u32>(), external in any::<u32>(), low in 0u32..=3, high in 0u32..=3,
    ) {
        let _guard = ORACLE.lock().unwrap();
        let mut expected = [0u32; 6];
        // SAFETY: output has the six elements written by this wrapper. Both
        // offsets are within the wrapper's array, including one-past-the-end.
        unsafe { oracle_hsd_forget(fallback, external, low, high, expected.as_mut_ptr()); }
        let mut memory = [0u32, external, 0];
        let base = memory.as_ptr().addr();
        let mut actual = [0u32; 6];
        {
            let mut rust = HsdSeedContext::new(fallback);
            rust.use_seed(&mut memory[1]);
            rust.forget_memory(base + low as usize * 4, base + high as usize * 4);
            actual[0] = rust.seed();
            actual[2] = rust.rand() as u32;
            actual[1] = rust.seed();
            actual[5] = u32::from(rust.is_external());
            rust.forget_memory(0, usize::MAX);
            actual[4] = rust.seed();
        }
        actual[3] = memory[1];
        prop_assert_eq!(actual, expected);
    }
}

#[test]
fn every_possible_hsd_output_has_identical_float_bits_and_signed_bounds() {
    let _guard = ORACLE.lock().unwrap();
    // Odd LCG multipliers have inverses modulo 2^32. This inverse selects an
    // input seed for each possible 16-bit output, so the float check is exhaustive.
    let inverse = 214_013u32.wrapping_pow(u32::MAX);
    assert_eq!(214_013u32.wrapping_mul(inverse), 1);
    for output in 0u32..65_536 {
        let seed = (output << 16).wrapping_sub(2_531_011).wrapping_mul(inverse);
        unsafe {
            oracle_hsd_set_seed(seed);
        }
        assert_eq!(
            HsdRng::new(seed).randf().to_bits(),
            unsafe { HSD_Randf() }.to_bits()
        );
        for bound in [i32::MIN, -65_537, -1, 0, 1, 32_768, 65_536, i32::MAX] {
            unsafe {
                oracle_hsd_set_seed(seed);
            }
            assert_eq!(HsdRng::new(seed).randi(bound), unsafe { HSD_Randi(bound) });
        }
    }
}
