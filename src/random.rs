//! Deterministic RNGs from `sysdolphin/baselib/random.c` and `MSL/rand.c`.
//!
//! General-purpose RNG libraries do not promise these sequences. Arithmetic is
//! explicitly 32-bit, including the signed overflow in `HSD_Randi` on PowerPC.

fn step(seed: &mut u32, multiplier: u32, increment: u32) -> i32 {
    *seed = seed.wrapping_mul(multiplier).wrapping_add(increment);
    (*seed >> 16) as i32
}

/// HAL's generator. Every draw, including `randi(0)`, advances the seed once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HsdRng(u32);

impl Default for HsdRng {
    fn default() -> Self {
        Self::new(1)
    }
}

impl HsdRng {
    pub const fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub const fn seed(&self) -> u32 {
        self.0
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.0 = seed;
    }

    /// Returns the upper 16 bits of the updated seed, in `0..65536`.
    pub fn rand(&mut self) -> i32 {
        step(&mut self.0, 214_013, 2_531_011)
    }

    pub fn randf(&mut self) -> f32 {
        self.rand() as f32 / 65_536.0
    }

    /// Preserves signed multiplication and division, including negative bounds
    /// and wrapping products. Large positive bounds can yield negative results.
    pub fn randi(&mut self, max: i32) -> i32 {
        max.wrapping_mul(self.rand()) / 65_536
    }
}

/// Metrowerks' independent 15-bit generator, including `srand` seed replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MslRng(u32);

impl Default for MslRng {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MslRng {
    pub const fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub const fn seed(&self) -> u32 {
        self.0
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.0 = seed;
    }

    pub fn rand(&mut self) -> i32 {
        step(&mut self.0, 1_103_515_245, 12_345) & 0x7fff
    }
}

/// Safe ownership of HSD's fallback seed and optional borrowed `seed_ptr`.
/// Memory forgetting compares host addresses in the half-open range `[low, high)`;
/// address mapping into a future GameCube runtime is a separate responsibility.
#[derive(Debug)]
pub struct HsdSeedContext<'a> {
    fallback: HsdRng,
    external: Option<&'a mut u32>,
}

impl Default for HsdSeedContext<'_> {
    fn default() -> Self {
        Self::new(1)
    }
}

impl<'a> HsdSeedContext<'a> {
    pub const fn new(fallback_seed: u32) -> Self {
        Self {
            fallback: HsdRng::new(fallback_seed),
            external: None,
        }
    }

    pub fn use_seed(&mut self, external: &'a mut u32) {
        self.external = Some(external);
    }

    pub fn seed(&self) -> u32 {
        self.external.as_deref().copied().unwrap_or(self.fallback.0)
    }

    pub fn is_external(&self) -> bool {
        self.external.is_some()
    }

    /// Equivalent to `_HSD_RandForgetMemory`; the fallback seed is preserved.
    pub fn forget_memory(&mut self, low: usize, high: usize) {
        if self.external.as_deref().is_some_and(|seed| {
            let address = std::ptr::from_ref(seed).addr();
            low <= address && address < high
        }) {
            self.external = None;
        }
    }

    pub fn rand(&mut self) -> i32 {
        step(
            self.external.as_deref_mut().unwrap_or(&mut self.fallback.0),
            214_013,
            2_531_011,
        )
    }

    pub fn randf(&mut self) -> f32 {
        self.rand() as f32 / 65_536.0
    }

    pub fn randi(&mut self, max: i32) -> i32 {
        max.wrapping_mul(self.rand()) / 65_536
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_sequences_and_zero_bound_advancement() {
        let mut hsd = HsdRng::default();
        let mut msl = MslRng::default();
        assert_eq!([hsd.rand(), hsd.rand(), hsd.rand()], [41, 51_235, 6334]);
        assert_eq!([msl.rand(), msl.rand(), msl.rand()], [16_838, 5758, 10_113]);
        let old_seed = hsd.seed();
        assert_eq!(hsd.randi(0), 0);
        assert_ne!(hsd.seed(), old_seed);
    }

    #[test]
    fn forgetting_external_storage_restores_the_existing_fallback() {
        let mut external = 42;
        let address = std::ptr::from_ref(&external).addr();
        let mut context = HsdSeedContext::default();
        context.rand();
        let fallback = context.seed();
        context.use_seed(&mut external);
        assert_eq!(context.seed(), 42);
        context.forget_memory(address, address);
        assert!(context.is_external());
        context.forget_memory(address, address + 4);
        assert!(!context.is_external());
        assert_eq!(context.seed(), fallback);
    }
}
