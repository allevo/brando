//! One `Pcg64` stream per purpose, seeded from the run's seed and a fixed
//! salt — the same blake3-then-`Pcg64` device `sim_core::rng` uses to keep
//! its own streams apart, with this crate's own salt so its streams can
//! never collide with anything the simulation itself draws.

use rand::{RngCore, SeedableRng};
use rand_pcg::Pcg64;

/// Keeps this crate's dice apart from every other stream drawn from a seed.
const SALT: &str = "brando/map-gen/v1";

/// One stream for one purpose. Two purposes of the same seed draw from
/// independent streams, so a later change to one pass's draws can never
/// shift another pass's.
pub(crate) fn stream(seed: u64, purpose: &str) -> Pcg64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(SALT.as_bytes());
    // The purpose's length goes in front of it, so "ab" and "a" followed by
    // something else cannot hash to the same bytes.
    hasher.update(&(purpose.len() as u64).to_le_bytes());
    hasher.update(purpose.as_bytes());
    Pcg64::from_seed(*hasher.finalize().as_bytes())
}

/// A whole number drawn uniformly from `0..n`, `n > 0`.
///
/// Widening multiplication against one `next_u64` draw — the same technique
/// `sim_core::rng::Stream::below` uses — without that type's "exactly one
/// draw" bookkeeping: nothing this crate draws ever enters a state hash, so
/// only the *value* has to be reproducible for a given seed, not the number
/// of steps the generator took to produce it.
pub(crate) fn below(rng: &mut Pcg64, n: u64) -> u64 {
    debug_assert!(n > 0, "below(0) has nothing to return");
    let v = u128::from(rng.next_u64()) * u128::from(n);
    (v >> 64) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_purposes_of_the_same_seed_draw_different_streams() {
        let mut a = stream(7, "assign");
        let mut b = stream(7, "relief");
        let av: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let bv: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_ne!(av, bv);
    }

    #[test]
    fn the_same_seed_and_purpose_draw_the_same_stream() {
        let mut a = stream(7, "assign");
        let mut b = stream(7, "assign");
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn below_never_reaches_n() {
        let mut rng = stream(3, "assign");
        for n in [1u64, 2, 6, 1000] {
            for _ in 0..500 {
                assert!(below(&mut rng, n) < n);
            }
        }
    }

    #[test]
    fn below_covers_its_range() {
        let mut rng = stream(3, "assign");
        let mut seen = [false; 6];
        for _ in 0..500 {
            seen[below(&mut rng, 6) as usize] = true;
        }
        assert_eq!(
            seen, [true; 6],
            "every face of a six-sided die has to come up"
        );
    }
}
