//! Deterministic RNG, one stream per kind (D4).
//!
//! The hard requirement is not seeding the RNG: it is that the day a new
//! kind is added, the recorded replays of the existing scenarios stay green.
//! That is why each stream's seed derives from the kind's **name** and not
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
pub enum RngKind {
    Events,
    Migration,
    Production,
    /// Births and deaths (phase 14) — the first kind any system actually draws
    /// from. Migration keeps its own so that phase 15 does not knock this one's
    /// sequence out of phase: that separation is the whole reason `RngKind`
    /// exists, and in four phases of M0 it had never come up.
    Demographics,
}

impl RngKind {
    /// Every variant, in declaration order. The order matters only for the
    /// state hash (phase 08), not for deriving the seeds.
    pub const ALL: [RngKind; 4] = [
        RngKind::Events,
        RngKind::Migration,
        RngKind::Production,
        RngKind::Demographics,
    ];

    pub const COUNT: usize = Self::ALL.len();

    /// The kind's stable salt.
    ///
    /// Do not rename a variant without regenerating the recordings: the name is
    /// part of the determinism contract, not a cosmetic detail.
    const fn salt(self) -> &'static str {
        match self {
            Self::Events => "brando/rng/v1/events",
            Self::Migration => "brando/rng/v1/migration",
            Self::Production => "brando/rng/v1/production",
            Self::Demographics => "brando/rng/v1/demographics",
        }
    }

    /// The slot this kind takes up in [`RngSet`].
    ///
    /// Assigned per variant and not by position in [`RngKind::ALL`]: that is
    /// what lets a kind be added at the top of the enum without moving the
    /// existing streams.
    const fn index(self) -> usize {
        match self {
            Self::Events => 0,
            Self::Migration => 1,
            Self::Production => 2,
            // The new slot goes at the **end**: that is what keeps the three
            // streams above exactly where they were, which
            // `sequences_are_reproducible_with_the_expected_values` checks.
            Self::Demographics => 3,
        }
    }

    /// The inverse of [`RngKind::index`]. `index_is_a_round_trip` checks the
    /// two really are inverses.
    const fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Events),
            1 => Some(Self::Migration),
            2 => Some(Self::Production),
            3 => Some(Self::Demographics),
            _ => None,
        }
    }
}

/// One RNG stream, with a count of how many times it has stepped the generator.
///
/// The count is of no use to the game: it serves the state hash (phase 08).
/// For a given seed, the number of draws uniquely determines the generator's
/// state, so hashing `(seed, draws)` covers the RNG's state without having to
/// serialise its internals.
///
/// That sentence is an **invariant of this type**, not a remark about it: it
/// holds only as long as every method of [`RngCore`] adds the number of steps
/// it really takes. `fill_bytes` is the one that does not take exactly one, and
/// `fill_bytes_counts_a_draw_per_step` is what pins it down. Get it wrong and
/// two states with divergent generators hash the same — which is to say the
/// recorded replays stop protecting the RNG, silently.
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

    /// A whole number in `0..n`, at a cost of **exactly one draw**, always.
    ///
    /// Widening multiplication instead of the rejection sampling
    /// `rand::Rng::random_range` uses. Rejection consumes a count of values
    /// that depends on the values themselves, and therefore on the seed — and
    /// then [`draws`](Self::draws) stops being a function of the game state,
    /// which is the whole reason phase 02 put it in the state hash. The second
    /// consequence is the worse one: `different_seeds_give_different_hashes`
    /// would pass even if the demographics did nothing at all, and a green test
    /// that checks nothing is worse than a red one.
    ///
    /// The bias is 2^-64 relative — irrelevant, and in any case preferable to a
    /// cost that varies in a core which has to be deterministic in the number
    /// of draws as well as in the values.
    ///
    /// `n == 0` returns 0 and still draws. The multiplication already yields 0
    /// there, so an early return would buy nothing except the one thing this
    /// method exists to refuse: a cost that depends on the argument.
    pub fn below(&mut self, n: u64) -> u64 {
        let v = u128::from(self.next_u64()) * u128::from(n);
        // The high 64 bits: `v >> 64` is below `n` by construction.
        (v >> 64) as u64
    }

    const fn count_draws(&mut self, n: u64) {
        // Wrapping rather than `+=`: the core does not panic. In practice it is
        // never reached, 2^64 draws are not attainable within one game.
        self.draws = self.draws.wrapping_add(n);
    }
}

impl RngCore for Stream {
    fn next_u32(&mut self) -> u32 {
        self.count_draws(1);
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.count_draws(1);
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        // Not one draw. `Lcg128Xsl64::fill_bytes` is `impls::fill_bytes_via_next`,
        // which steps the generator once per eight bytes plus once for the tail
        // — exactly `len.div_ceil(8)`. (`next_u32` and `next_u64` really are one
        // step each, so those two are right as they stand.)
        //
        // Counting one here would break the invariant on `draws` documented
        // above: two streams from the same seed, one filling eight bytes and
        // one sixty-four, would report the same count from divergent
        // generators, and `hash_world` — which hashes the count and nothing
        // else about the RNG — would call the two states identical.
        self.count_draws(u64::try_from(dst.len().div_ceil(8)).unwrap_or(u64::MAX));
        self.rng.fill_bytes(dst);
    }
}

/// The set of streams, one per kind.
///
/// The fields are deliberately private: if two systems could draw from the same
/// stream in the same tick, the draw order would become an implicit contract.
/// [`RngSet::get`] lends out one kind at a time.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RngSet {
    streams: [Stream; RngKind::COUNT],
}

impl RngSet {
    /// Derives one stream per kind from the game's seed.
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
            let d = RngKind::from_index(i).expect("a slot < COUNT always has a kind");
            Stream::from_seed_bytes(derive_seed(seed, d))
        });
        Self { streams }
    }

    /// Lends out a kind's stream for writing.
    pub fn get(&mut self, kind: RngKind) -> &mut Stream {
        &mut self.streams[kind.index()]
    }

    /// The stream's position, for the state hash (phase 08).
    pub fn draws(&self, kind: RngKind) -> u64 {
        self.streams[kind.index()].draws()
    }
}

fn derive_seed(seed: u64, kind: RngKind) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(kind.salt().as_bytes());
    let mut out = [0u8; 32];
    hasher.finalize_xof().fill(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence(seed: u64, kind: RngKind, n: usize) -> Vec<u64> {
        let mut set = RngSet::from_seed(seed);
        let s = set.get(kind);
        (0..n).map(|_| s.next_u64()).collect()
    }

    /// The expected values are written out by hand: comparing two instances
    /// against each other would pass even with a broken derivation.
    ///
    /// If this test breaks after **adding** a variant to `RngKind`, the
    /// derivation is positional and not by name: that is a bug, not a
    /// recording to regenerate.
    ///
    /// If it breaks after **renaming** a variant or changing a salt, that is
    /// expected: the kind name is part of the contract (the recorded replays
    /// have to be regenerated too).
    #[test]
    fn sequences_are_reproducible_with_the_expected_values() {
        assert_eq!(
            sequence(42, RngKind::Events, 8),
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
            sequence(42, RngKind::Migration, 4),
            [
                6_744_713_315_132_201_080,
                14_951_314_113_004_524_121,
                14_001_924_983_284_815_708,
                9_329_562_633_619_259_286,
            ]
        );
        assert_eq!(
            sequence(42, RngKind::Production, 4),
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
        for d in RngKind::ALL {
            assert!(d.index() < RngKind::COUNT, "{d:?} outside the slots");
            assert_eq!(RngKind::from_index(d.index()), Some(d));
        }
        for i in 0..RngKind::COUNT {
            let d = RngKind::from_index(i).expect("slot covered");
            assert_eq!(d.index(), i);
        }
        assert_eq!(RngKind::from_index(RngKind::COUNT), None);
    }

    /// Catches the copy-paste in which two domains share a salt.
    #[test]
    fn the_domains_are_independent() {
        let e = sequence(42, RngKind::Events, 8);
        let m = sequence(42, RngKind::Migration, 8);
        let p = sequence(42, RngKind::Production, 8);
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
            a1.push(interleaved.get(RngKind::Events).next_u64());
            b1.push(interleaved.get(RngKind::Production).next_u64());
        }

        let mut grouped = RngSet::from_seed(7);
        let a2: Vec<_> = (0..4)
            .map(|_| grouped.get(RngKind::Events).next_u64())
            .collect();
        let b2: Vec<_> = (0..4)
            .map(|_| grouped.get(RngKind::Production).next_u64())
            .collect();

        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
        assert_eq!(interleaved, grouped, "the final state matches too");
    }

    #[test]
    fn different_seeds_give_different_sequences() {
        for d in RngKind::ALL {
            assert_ne!(sequence(42, d, 8), sequence(43, d, 8), "kind {d:?}");
        }
    }

    /// The draw count is what the state hash will use to tell "5 values
    /// consumed" from "6" (phase 08).
    #[test]
    fn draws_are_counted_per_domain() {
        let mut set = RngSet::from_seed(1);
        assert_eq!(set.draws(RngKind::Events), 0);
        for _ in 0..5 {
            set.get(RngKind::Events).next_u64();
        }
        set.get(RngKind::Migration).next_u32();
        assert_eq!(set.draws(RngKind::Events), 5);
        assert_eq!(set.draws(RngKind::Migration), 1);
        assert_eq!(set.draws(RngKind::Production), 0);
    }

    /// The state is equal iff the seed and the draw count are: that is the
    /// assumption that makes hashing (seed, draws) enough instead of Pcg64's
    /// internals.
    #[test]
    fn the_state_is_determined_by_seed_and_draw_count() {
        let mut a = RngSet::from_seed(9);
        let mut b = RngSet::from_seed(9);
        for _ in 0..3 {
            a.get(RngKind::Events).next_u64();
        }
        assert_ne!(a, b);
        for _ in 0..3 {
            b.get(RngKind::Events).next_u64();
        }
        assert_eq!(a, b);
    }

    /// `fill_bytes` counts every step it takes, not one per call.
    ///
    /// This is the test that keeps the previous one's assumption true. A single
    /// count per call left two streams from the same seed reporting `draws = 1`
    /// with generators eight steps apart — and the state hash, which knows
    /// nothing about the RNG except that number, would have said they were the
    /// same state. Nothing draws yet, so today it costs no recording; the first
    /// system that does (phase 14) is where it would have stopped being free.
    #[test]
    fn fill_bytes_counts_a_draw_per_step() {
        let mut short = RngSet::from_seed(7);
        let mut long = RngSet::from_seed(7);
        short.get(RngKind::Events).fill_bytes(&mut [0u8; 8]);
        long.get(RngKind::Events).fill_bytes(&mut [0u8; 64]);

        assert_eq!(short.draws(RngKind::Events), 1);
        assert_eq!(long.draws(RngKind::Events), 8);
        assert_ne!(
            short, long,
            "eight steps apart, and the count has to say so"
        );

        // The count is not merely different, it is *right*: filling 64 bytes
        // leaves the generator exactly where eight `next_u64` would.
        let mut stepped = RngSet::from_seed(7);
        for _ in 0..8 {
            stepped.get(RngKind::Events).next_u64();
        }
        assert_eq!(stepped, long);

        // The tail costs a step of its own, whatever its length.
        for (bytes, expected) in [(0usize, 0u64), (1, 1), (5, 1), (9, 2), (17, 3)] {
            let mut s = RngSet::from_seed(7);
            s.get(RngKind::Events).fill_bytes(&mut vec![0u8; bytes]);
            assert_eq!(s.draws(RngKind::Events), expected, "{bytes} bytes");
        }
    }

    /// Phase 14, test 6a — `below(n)` costs exactly one draw, for every `n` and
    /// every seed.
    ///
    /// The guard against rejection sampling, asked of `Stream` directly instead
    /// of inferred from a game. If this ever fails, `draws()` has stopped being
    /// a function of the state and the state hash has gone blind on the RNG in
    /// the one field phase 02 put there to make a divergence attributable.
    #[test]
    fn below_costs_exactly_one_draw() {
        for seed in [0, 1, 42, u64::MAX] {
            for n in [0, 1, 2, 3, 7, 1_000, u64::MAX] {
                let mut set = RngSet::from_seed(seed);
                let before = set.draws(RngKind::Events);
                let v = set.get(RngKind::Events).below(n);

                assert_eq!(
                    set.draws(RngKind::Events) - before,
                    1,
                    "seed {seed}, n {n}: the cost has to be one draw whatever the argument"
                );
                assert!(
                    v < n.max(1),
                    "seed {seed}: {v} is not below {n}, and n == 0 has to give 0"
                );
            }
        }
    }

    /// And the values really are spread over the range.
    ///
    /// Without this, a `below` that returned a constant would satisfy test 6a
    /// perfectly: the draw count is the property that matters for the hash, but
    /// it is not the property that makes the number useful. Six faces, enough
    /// rolls that missing one is not bad luck.
    #[test]
    fn below_covers_its_range() {
        let mut set = RngSet::from_seed(3);
        let mut seen = [false; 6];
        for _ in 0..200 {
            let v = set.get(RngKind::Events).below(6);
            seen[usize::try_from(v).expect("below 6")] = true;
        }
        assert_eq!(seen, [true; 6], "every face of a six-sided die has to come up");
    }
}
