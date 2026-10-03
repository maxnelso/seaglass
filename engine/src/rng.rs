//! Seedable deterministic PRNG (`SplitMix64`).
//!
//! All randomness in Seaglass flows through [`Rng`]. The algorithm is self-contained,
//! version-stable, and platform-independent so that a given seed always produces the
//! exact same draw sequence.

/// A deterministic `SplitMix64` pseudo-random number generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Initialize from a 64-bit seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Draw the next raw `u64`.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Draw a uniform index in `0..bound`.
    ///
    /// Panics if `bound == 0`.
    #[inline]
    pub fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "Rng::below called with bound == 0");
        if bound == 1 {
            return 0;
        }
        (self.next_u64() % (bound as u64)) as usize
    }

    /// Flip a fair coin (`true` if `below(2) == 0`).
    #[inline]
    pub fn coin_flip(&mut self) -> bool {
        self.below(2) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_is_reproducible() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
