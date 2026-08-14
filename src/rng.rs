use rand::rngs::SysRng;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GridScriptRng {
    inner: ChaCha8Rng,
}

impl GridScriptRng {
    pub fn from_seed(seed: u64) -> Self {
        GridScriptRng {
            inner: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn from_entropy() -> Self {
        GridScriptRng {
            inner: ChaCha8Rng::try_from_rng(&mut SysRng).expect("failed to seed from OS entropy"),
        }
    }

    /// GO RANDOM; random direction, degrees from 0-359 inclusive
    pub fn random_direction(&mut self) -> u16 {
        self.inner.random_range(0..360) as u16
    }

    /// STORE RANDOM; random fp number in [0,1)
    pub fn random_unit_float(&mut self) -> f32 {
        self.inner.random::<f32>()
    }

    /// SWITCH RANDOM; 50% chance
    pub fn coin_flip(&mut self) -> bool {
        self.inner.random_bool(0.5)
    }

    /// REMOVE ANY POSITION and GOTO equidistant checkpoint tiebreak;
    /// pick a random index in [0,len).
    /// Panics if `len == 0`.
    pub fn random_index(&mut self, len: usize) -> usize {
        self.inner.random_range(0..len)
    }

    /// SHUFFLE; rearrange buffer in place (Fisher–Yates)
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let n = slice.len();
        for i in 0..n.saturating_sub(1) {
            let j = i + self.random_index(n - i);
            slice.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_rng_is_reproducible() {
        let mut a = GridScriptRng::from_seed(42);
        let mut b = GridScriptRng::from_seed(42);
        for _ in 0..100 {
            assert_eq!(a.random_direction(), b.random_direction());
        }
    }

    #[test]
    fn random_direction_stays_in_range() {
        let mut rng = GridScriptRng::from_seed(1);
        for _ in 0..1000 {
            assert!(rng.random_direction() < 360);
        }
    }

    #[test]
    fn shuffle_preserves_elements() {
        let mut rng = GridScriptRng::from_seed(7);
        let mut v: Vec<i32> = (0..10).collect();
        let original = v.clone();
        rng.shuffle(&mut v);
        v.sort();
        assert_eq!(v, original);
    }
}
