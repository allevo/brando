# Phase 02 — One RNG per domain

> **Status: implemented — M0.**
>
> It records how the phase was planned and how it went, frozen as it was written. It is
> **not** a description of the tree today: for that see [ARCHITECTURE.md](../ARCHITECTURE.md)
> and [RULES.md](../RULES.md).

**Goal:** the same seed ⇒ the same sequence for every domain, and **adding a new domain does not
knock the existing domains out of phase**.
**Depends on:** 00 (01 is not needed).
**Size:** S.
**Decisions involved:** D4.

## Why now

It is the smallest phase in the plan and the one with the highest return. The hard requirement is
not "seed the RNG" — it is that in six months' time the `Invasions` domain gets added and the
recorded replays of the existing scenarios **stay green**. If the domains are derived by
sequentially splitting a master, that day every recording changes and nobody can say whether the
new domain or a bug is to blame. The derivation has to be done right now, while there is only one
domain to test.

## What gets built

```rust
/// Independent RNG domains. Adding a variant must NOT change the sequences of
/// the existing variants (D4): that is why each stream's seed derives from the
/// domain's *name*, not from its position in the enum.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RngDomain { Events, Migration, Production }
// Amended: today this is `RngKind`, and it has four variants. See below.

impl RngDomain {
    /// A stable salt. Do not rename a variant without regenerating the recordings:
    /// the name is part of the determinism contract.
    const fn salt(self) -> &'static str {
        match self {
            Self::Events     => "brando/rng/v1/events",
            Self::Migration  => "brando/rng/v1/migration",
            Self::Production => "brando/rng/v1/production",
        }
    }
}

pub struct RngSet { streams: [Pcg64; RngDomain::COUNT] }

impl RngSet {
    pub fn from_seed(seed: u64) -> Self { /* blake3(seed_le || salt) -> the stream's seed */ }
    pub fn get(&mut self, d: RngDomain) -> &mut Pcg64;
}
```

Derivation: a `blake3::Hasher` fed with `seed.to_le_bytes()` and then with `salt().as_bytes()`,
with the XOF used to fill `Pcg64`'s seed. During implementation, check the exact size of
`rand_pcg::Pcg64`'s seed and fill all of it from the XOF (not from `seed_from_u64`, which throws
away entropy and makes a collision between domains easier).

Two details that look like pedantry and are not:

- **`get` takes `&mut self`**: one domain lent out at a time. Do not expose the fields: if two
  systems can draw from the same stream in the same tick, the draw order becomes an implicit
  contract.
> **Amended by the documentation audit (2026-08-13).** Two things moved. The type is called
> **`RngKind`** — the vocabulary review ([A19](../DECISIONS.md)) retired `domain` as a hard word that
> bought nothing — and it has a **fourth** variant, `Demographics`, added in phase 14. That fourth
> one is the first kind any system actually draws from: for the whole of M0 the separation this phase
> built was never exercised, and phase 14 is where it paid, because `Migration` keeps its own stream
> so that phase 15 will not knock the demographics' sequence out of phase.
>
> **The declaration order is frozen**, and so are the salt strings: the salt is derived from the
> variant's *name*, so renaming one silently changes every sequence it produces.

- **The stream's position goes into the state hash** (phase 08). A state in which the `Events` RNG
  has consumed 5 values is not the same state as one in which it has consumed 6, even if
  everything else matches: if it does not go into the hash, a divergence shows up many ticks
  later, where it is almost impossible to attribute.

`RngSet` is serialisable for debugging, but a save file is still `seed + Vec<Command>` (D4).

## Out of scope

No use of the RNG. No random events (M1+), no migration (M1). This phase produces only the
generator and the proof that it is trustworthy.

## Tests

1. **Reproducibility**: `RngSet::from_seed(42)` drawn 8 times per domain produces a sequence
   compared against **expected values written out by hand in the test**. Not `assert_eq!(a, b)`
   between two instances — that passes even if the derivation is wrong. The numbers have to be
   hardcoded (generate them once and paste them in).
2. **Independence between domains**: the three sequences differ for the same seed. A trivial test,
   but it catches the copy-paste mistake in which two domains share a salt.
3. **Stability when a domain is added**: this is *the* test. Test 1 with hardcoded values already
   covers it, as long as its documentation says so explicitly:
   ```rust
   /// If this test breaks after ADDING a variant to RngDomain,
   /// the derivation is positional and not by name: that is a bug, not a recording to regenerate.
   ```
   To be reinforced with a test that builds an `RngSet` and checks that the order in which the
   domains are *called* does not affect their sequences (drawing A,B,A,B vs A,A,B,B gives the same
   sequences per domain).
4. **Sensitivity to the seed**: seeds 42 and 43 produce different sequences in every domain.

## Verification

```sh
cargo test -p sim-core rng
```

Then the manual check that closes the phase: temporarily add
`RngDomain::Test => "brando/rng/v1/test"` as the **first** variant of the enum and re-run the
tests. They have to stay **green**. If they go red, the derivation depends on the position: it
will have to be fixed anyway the day a new domain is needed, and that day it will cost every
recording in the project. Remove the variant and commit.

**Done when:** the tests are green with hardcoded values and the variant added at the top of the
enum does not break them.
