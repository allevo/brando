//! Deterministic RNG, one stream per domain (D4).
//!
//! The hard requirement is not seeding the RNG: it is that the day a new
//! domain is added, the recorded replays of the existing scenarios stay green.
//! That is why each stream's seed derives from the domain's **name** and not
//! from its position in the enum.

use rand::{RngCore, SeedableRng};
use rand_pcg::Pcg64;
use serde::{Deserialize, Serialize};

/// Independent RNG domains.
///
/// Adding a variant must not change the sequences of the existing ones: if it
/// does, the derivation is positional and that is a bug, not a recording to
/// regenerate.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum RngDomain {
    Events,
    Migration,
    Production,
}

impl RngDomain {
    /// Every variant, in declaration order. The order matters only for the
    /// state hash (phase 08), not for deriving the seeds.
    pub const ALL: [RngDomain; 3] = [
        RngDomain::Events,
        RngDomain::Migration,
        RngDomain::Production,
    ];

    pub const COUNT: usize = Self::ALL.len();

    /// The domain's stable salt.
    ///
    /// Do not rename a variant without regenerating the recordings: the name is
    /// part of the determinism contract, not a cosmetic detail.
    const fn salt(self) -> &'static str {
        match self {
            Self::Events => "brando/rng/v1/events",
            Self::Migration => "brando/rng/v1/migration",
            Self::Production => "brando/rng/v1/production",
        }
    }

    /// The slot this domain takes up in [`RngSet`].
    ///
    /// Assigned per variant and not by position in [`RngDomain::ALL`]: that is
    /// what lets a domain be added at the top of the enum without moving the
    /// existing streams.
    const fn index(self) -> usize {
        match self {
            Self::Events => 0,
            Self::Migration => 1,
            Self::Production => 2,
        }
    }

    /// The inverse of [`RngDomain::index`]. `index_is_a_round_trip` checks the
    /// two really are inverses.
    const fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Events),
            1 => Some(Self::Migration),
            2 => Some(Self::Production),
            _ => None,
        }
    }
}

/// One RNG stream, with a count of how many values it has produced.
///
/// The count is of no use to the game: it serves the state hash (phase 08).
/// For a given seed, the number of draws uniquely determines the generator's
/// state, so hashing `(seed, draws)` covers the RNG's state without having to
/// serialise its internals.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Stream {
    rng: Pcg64,
    draws: u64,
}

impl Stream {
    fn from_seed_bytes(bytes: [u8; 32]) -> Self {
        Self {
            rng: Pcg64::from_seed(bytes),
            draws: 0,
        }
    }

    /// How many draws this stream has made.
    pub const fn draws(&self) -> u64 {
        self.draws
    }

    const fn count_draw(&mut self) {
        // Wrapping rather than `+=`: the core does not panic. In practice it is
        // never reached, 2^64 draws are not attainable within one game.
        self.draws = self.draws.wrapping_add(1);
    }
}

impl RngCore for Stream {
    fn next_u32(&mut self) -> u32 {
        self.count_draw();
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.count_draw();
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.count_draw();
        self.rng.fill_bytes(dst);
    }
}

/// The set of streams, one per domain.
///
/// The fields are deliberately private: if two systems could draw from the same
/// stream in the same tick, the draw order would become an implicit contract.
/// [`RngSet::get`] lends out one domain at a time.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RngSet {
    streams: [Stream; RngDomain::COUNT],
}

impl RngSet {
    /// Derives one stream per domain from the game's seed.
    ///
    /// `blake3(seed_le || salt)` in XOF mode fills all 32 seed bytes of
    /// `Pcg64`: `seed_from_u64` would throw away entropy and make a collision
    /// between domains easier.
    ///
    /// The streams are filled by **slot** (`index()`), not by walking `ALL`:
    /// otherwise adding a variant at the top of the enum would shift the
    /// existing streams and knock every recording out of phase.
    pub fn from_seed(seed: u64) -> Self {
        let streams = std::array::from_fn(|i| {
            // Provable invariant: `from_index` is the inverse of `index` over
            // 0..COUNT, guarded by `index_is_a_round_trip`.
            let d = RngDomain::from_index(i).expect("a slot < COUNT always has a domain");
            Stream::from_seed_bytes(derive_seed(seed, d))
        });
        Self { streams }
    }

    /// Lends out a domain's stream for writing.
    pub fn get(&mut self, domain: RngDomain) -> &mut Stream {
        &mut self.streams[domain.index()]
    }

    /// The stream's position, for the state hash (phase 08).
    pub fn draws(&self, domain: RngDomain) -> u64 {
        self.streams[domain.index()].draws()
    }
}

fn derive_seed(seed: u64, domain: RngDomain) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(domain.salt().as_bytes());
    let mut out = [0u8; 32];
    hasher.finalize_xof().fill(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence(seed: u64, domain: RngDomain, n: usize) -> Vec<u64> {
        let mut set = RngSet::from_seed(seed);
        let s = set.get(domain);
        (0..n).map(|_| s.next_u64()).collect()
    }

    /// The expected values are written out by hand: comparing two instances
    /// against each other would pass even with a broken derivation.
    ///
    /// If this test breaks after **adding** a variant to `RngDomain`, the
    /// derivation is positional and not by name: that is a bug, not a
    /// recording to regenerate.
    ///
    /// If it breaks after **renaming** a variant or changing a salt, that is
    /// expected: the domain name is part of the contract (the recorded replays
    /// have to be regenerated too).
    #[test]
    fn sequences_are_reproducible_with_the_expected_values() {
        assert_eq!(
            sequence(42, RngDomain::Events, 8),
            [
                16_951_895_464_066_535_839,
                10_528_375_347_967_641_558,
                6_732_969_908_935_899_607,
                15_963_220_654_581_270_673,
                3_540_804_277_719_876_773,
                15_145_133_130_928_511_643,
                8_827_474_317_410_546_936,
                2_537_525_544_625_458_919,
            ]
        );
        assert_eq!(
            sequence(42, RngDomain::Migration, 4),
            [
                6_744_713_315_132_201_080,
                14_951_314_113_004_524_121,
                14_001_924_983_284_815_708,
                9_329_562_633_619_259_286,
            ]
        );
        assert_eq!(
            sequence(42, RngDomain::Production, 4),
            [
                7_953_230_224_566_493_169,
                13_149_629_979_718_932_555,
                474_551_565_481_971_545,
                1_458_220_722_024_169_394,
            ]
        );
    }

    /// `index` and `from_index` are each other's inverse over `0..COUNT`.
    /// That is the invariant that licenses the `expect` in `RngSet::from_seed`.
    #[test]
    fn index_is_a_round_trip() {
        for d in RngDomain::ALL {
            assert!(d.index() < RngDomain::COUNT, "{d:?} outside the slots");
            assert_eq!(RngDomain::from_index(d.index()), Some(d));
        }
        for i in 0..RngDomain::COUNT {
            let d = RngDomain::from_index(i).expect("slot covered");
            assert_eq!(d.index(), i);
        }
        assert_eq!(RngDomain::from_index(RngDomain::COUNT), None);
    }

    /// Catches the copy-paste in which two domains share a salt.
    #[test]
    fn the_domains_are_independent() {
        let e = sequence(42, RngDomain::Events, 8);
        let m = sequence(42, RngDomain::Migration, 8);
        let p = sequence(42, RngDomain::Production, 8);
        assert_ne!(e, m);
        assert_ne!(e, p);
        assert_ne!(m, p);
    }

    /// The order in which systems call the domains must not affect their
    /// sequences: A,B,A,B and A,A,B,B give the same results.
    #[test]
    fn the_call_order_does_not_matter() {
        let mut interleaved = RngSet::from_seed(7);
        let mut a1 = Vec::new();
        let mut b1 = Vec::new();
        for _ in 0..4 {
            a1.push(interleaved.get(RngDomain::Events).next_u64());
            b1.push(interleaved.get(RngDomain::Production).next_u64());
        }

        let mut grouped = RngSet::from_seed(7);
        let a2: Vec<_> = (0..4)
            .map(|_| grouped.get(RngDomain::Events).next_u64())
            .collect();
        let b2: Vec<_> = (0..4)
            .map(|_| grouped.get(RngDomain::Production).next_u64())
            .collect();

        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
        assert_eq!(interleaved, grouped, "the final state matches too");
    }

    #[test]
    fn different_seeds_give_different_sequences() {
        for d in RngDomain::ALL {
            assert_ne!(sequence(42, d, 8), sequence(43, d, 8), "domain {d:?}");
        }
    }

    /// The draw count is what the state hash will use to tell "5 values
    /// consumed" from "6" (phase 08).
    #[test]
    fn draws_are_counted_per_domain() {
        let mut set = RngSet::from_seed(1);
        assert_eq!(set.draws(RngDomain::Events), 0);
        for _ in 0..5 {
            set.get(RngDomain::Events).next_u64();
        }
        set.get(RngDomain::Migration).next_u32();
        assert_eq!(set.draws(RngDomain::Events), 5);
        assert_eq!(set.draws(RngDomain::Migration), 1);
        assert_eq!(set.draws(RngDomain::Production), 0);
    }

    /// The state is equal iff the seed and the draw count are: that is the
    /// assumption that makes hashing (seed, draws) enough instead of Pcg64's
    /// internals.
    #[test]
    fn the_state_is_determined_by_seed_and_draw_count() {
        let mut a = RngSet::from_seed(9);
        let mut b = RngSet::from_seed(9);
        for _ in 0..3 {
            a.get(RngDomain::Events).next_u64();
        }
        assert_ne!(a, b);
        for _ in 0..3 {
            b.get(RngDomain::Events).next_u64();
        }
        assert_eq!(a, b);
    }
}
