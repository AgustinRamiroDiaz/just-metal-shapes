//! Small deterministic PRNG (SplitMix64). Core logic never uses global randomness so
//! charts and bot decisions are reproducible from a seed.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Independent stream derived from this generator's seed and a stream id, so a
    /// sub-system can draw numbers without shifting everyone else's sequence.
    pub fn fork(&self, stream: u64) -> Self {
        let mut mixer = Rng::new(self.state ^ stream.wrapping_mul(0xA24B_AED4_963E_E407));
        Rng::new(mixer.next_u64())
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }

    /// Uniform in `[low, high)`.
    pub fn range_f32(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_f32()
    }

    /// Uniform integer in `[low, high]` (inclusive). Returns `low` if `high < low`.
    pub fn range_u32(&mut self, low: u32, high: u32) -> u32 {
        if high <= low {
            return low;
        }
        let span = (high - low) as u64 + 1;
        low + (self.next_u64() % span) as u32
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.next_f64() < probability
    }

    /// Index chosen proportionally to non-negative weights; `None` if all are zero.
    pub fn weighted_index(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
        if total <= 0.0 {
            return None;
        }
        let mut target = self.next_f32() * total;
        for (index, weight) in weights.iter().enumerate() {
            let weight = weight.max(0.0);
            if target < weight {
                return Some(index);
            }
            target -= weight;
        }
        weights.iter().rposition(|w| *w > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_per_seed() {
        let a: Vec<u64> = (0..8)
            .scan(Rng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..8)
            .scan(Rng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        let c: Vec<u64> = (0..8)
            .scan(Rng::new(8), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut rng = Rng::new(42);
        for _ in 0..10_000 {
            let f = rng.next_f64();
            assert!((0.0..1.0).contains(&f));
            let r = rng.range_f32(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&r));
            let i = rng.range_u32(3, 5);
            assert!((3..=5).contains(&i));
        }
        assert_eq!(rng.range_u32(4, 4), 4);
        assert_eq!(rng.range_u32(9, 2), 9);
    }

    #[test]
    fn weighted_index_respects_weights() {
        let mut rng = Rng::new(1);
        let mut counts = [0; 3];
        for _ in 0..10_000 {
            counts[rng.weighted_index(&[1.0, 0.0, 3.0]).unwrap()] += 1;
        }
        assert_eq!(counts[1], 0);
        assert!(counts[2] > counts[0] * 2);
        assert_eq!(rng.weighted_index(&[0.0, 0.0]), None);
        assert_eq!(rng.weighted_index(&[]), None);
    }

    #[test]
    fn forks_are_independent_and_stable() {
        let root = Rng::new(99);
        let mut a = root.fork(1);
        let mut a2 = root.fork(1);
        let mut b = root.fork(2);
        assert_eq!(a.next_u64(), a2.next_u64());
        assert_ne!(a.next_u64(), b.next_u64());
    }
}
