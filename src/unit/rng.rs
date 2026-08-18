//! Unit tests for `rng`

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
