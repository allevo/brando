# Phase 14 — Births and deaths

**Goal:** a served city grows until it fills up its houses, one that loses its services empties out;
and `different_seeds_give_different_hashes`, `#[ignore]` since phase 08, is re-enabled and passes.
**Depends on:** 13.
**Size:** L — it is the riskiest phase of M1.
**Decisions involved:** [A12](open-decisions.md), [A14](open-decisions.md), A10, D4, D5.

## Why now

Because it is the **first real use of the RNG** in the whole project. Since phase 02 there have been
three streams seeded per domain, their position goes into the state hash, and nobody has ever used
them: `commands.rs` still contains
`assert_eq!(w.rng(), before.rng(), "no system draws from the RNG in M0")`. This phase closes that
circle, and with it test 7 of phase 08.

And it is the phase in which **the population starts moving every tick**, which is
[A12](open-decisions.md)'s choice and its price. Everything that follows in this document descends
from that.

It has to be kept separate from migration (phase 15) for two reasons. One of readability: births and
deaths are local to the house, migration needs a city-wide index and a distribution rule — different
ways to fail, different tests. One concrete: with **two distinct RNG domains**, writing 15 does not
knock this one's sequence out of phase and does not regenerate a recording that had no reason to
change. It is literally the use case `RngDomain` was written for in phase 02, and until now it had
never come up.

## The contract: the coverage chases the population

It comes first because it is the point where M1 touches the hot path, and it is the part that can be
got wrong silently.

A provider's capacity is consumed by the **residents present** ([A12](open-decisions.md)): the
services chase the population, they do not precede it. Until now `residents` only changed on a
command — and every command already invalidates the coverage — or on eviction (phase 13). From here
it changes **every tick**, and three things are needed.

**1. Step 6 invalidates the coverage if it moved anyone.** One line, and without it the next tick's
step 3 works on a stale population: the houses that grew would consume more than the provider set
aside, and *a house covered by food always eats* would fall over. It is the same shape as the
forgotten invalidation `coverage_equivalence` has been catching since phase 06.

**2. `covered ⇒ eats` holds, and it is tighter than before.** Within one tick the order is: step 3
assigns with the current population, step 4 consumes with the same one, step 6 changes it. The two
readings that matter happen at the same logical instant. And because capacity and consumption are
now counted in the same unit, `unsustainable_food_capacity`'s arithmetic becomes an exact equality
instead of an upper bound: `Σ residents served ≤ capacity`, and `capacity × consumption ≤ output`.
No output wasted on half-empty houses.

**3. `coverage_equivalence` has to be reformulated, and it has to be done carefully.** Today it
compares the stored coverage against `compute_from_scratch` **at the end of the tick** — i.e. after
step 6 has already moved the population. With the demographics running the two always diverge, and
not because there is a bug: the coverage was computed at step 3 and will be redone at the next step
3.

The wrong reformulation is "compare only if it is not dirty": in a growing city it is dirty every
tick, and the project's most valuable test would stop running without going red.

The right reformulation is **two tests that say two different things**:

- `coverage_equivalence` stays **identical in form**, but runs on a dataset with the demographic
  rates at zero. It keeps checking what it has always checked — the forgotten invalidation after a
  command — and stays the oracle that protects every future optimisation of step 3.
- `demographics_invalidate_the_coverage`, new and direct: if the population changed during a tick,
  then at the end of the tick `dirty.coverage_needs_recompute()` is true. It checks exactly the new
  contract, and nothing else.

The third, implicit in the two above, is already covered by `covered_means_fed`, which compares two
quantities **both** written between step 3 and step 4 and therefore stays valid unchanged.

**What it costs.** Recomputing the coverage goes from "when the player builds" to "almost every
tick": `G` is 3.05 ms against the empty tick's 248 µs, so the typical tick heads towards ~3.3 ms —
**~13×**. Ten years of play at 200×200 go from ~0.9 s to ~12 s. It is A12's explicit price, accepted,
and **it is not optimised in M1**: the measurement and the countermeasures are an open task to be
closed before M2 ([A17](open-decisions.md)). What this phase has to do is **measure it well**, not
reduce it — see the Verification section.

From here on the `DirtyFlags` hardly avoid anything at all in a living city. They remain the right
mechanism — they are what will make targeted invalidation possible when it comes — but the comment in
`DirtyFlags::coverage` saying "nobody reads its contents today" has to be updated with why it matters
more now.

## What gets built

### The module, `sim-core/src/demographics.rs`

```rust
/// The four flows of population, aggregated: individuals are not simulated (D5).
///
/// The `remainder`s are **state** — they decide the following ticks — and go
/// into the hash. The running totals are diagnostics like `FoodTotals` and stay
/// out, for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Demographics {
    /// Fractions of an event not yet matured, one per flow, in thousandths of
    /// an event.
    ///
    /// Without the accumulation, a rate of 0.4 births per tick would be
    /// truncated to zero on **every** tick and a small city would never grow:
    /// the truncation is not a rounding error, it is a cliff. It is the same
    /// reason the treasury will have a remainder (phase 16).
    pub(crate) remainder: [i64; Flow::COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PopulationTotals {
    pub born: u64,
    pub died: u64,
    pub immigrated: u64,
    pub emigrated: u64,
    /// Evicted by a decay in level (phase 13).
    pub evicted: u64,
    /// Residents who vanished with a demolished house.
    ///
    /// It exists for the same reason as `FoodTotals::lost_to_demolition`:
    /// without it, population conservation stops being an equality and the
    /// phase's most important test would report a bug that is not there. It is
    /// the second time this term is needed — the first was the stock of a
    /// demolished farm, in phase 07.
    pub lost_to_demolition: u64,
}

pub enum Flow { Births, Deaths, Immigration, Emigration }
```

### Step 6, extended

```rust
fn houses_and_migration(world: &mut World) {
    update_satisfaction(world);             // 6.1, phase 12
    if is_monthly_review(world) {
        decay(world);                       // 6.2a, phase 13
        level_up(world);                    // 6.2b, phase 13
    }
    deaths(world);                          // 6.3
    births(world);                          // 6.5

    // The coverage is counted on the residents present (A12): if anyone has
    // moved, yesterday's assignment no longer holds and the next tick's step 3
    // has to redo it. Without this line the houses that grew would consume more
    // than the provider set aside, and *covered ⇒ eats* falls over.
    if world.demographics_moved_anyone() {
        mark_all_providers_dirty(world);
    }
}
```

6.4 and 6.6 (emigration and immigration) arrive with phase 15 and slot in **between** these, not at
the end: the final order is documented in `tick.rs` from now, so whoever reads it knows where they
will go. The invalidation stays the last thing in step 6 then too.

The invalidation is conditional and not unconditional on purpose: in a full or empty city the
population does not move, the recomputation is not needed, and the tick goes back to costing 248 µs.
It is also what lets `coverage_equivalence` run in its original form on a zero-rate dataset.

**Why the departures come before the arrivals.** Two reasons. One technical: after a decay there may
be a house full to the limit, and freeing up first guarantees `residents <= max_residents` holds at
every observable instant, not just at the end of the step. One of gameplay: a house that loses a
resident can win one back in the same tick, which makes the population responsive instead of jerky.

**Births have three conditions**: `residents > 0` (you need people to make people),
`residents < max_residents(level)`, and satisfaction above a threshold. The base rate is scaled by
the city's average satisfaction — it is the "tied to overall wellbeing" part of
[A14](open-decisions.md): a city that is doing well has children, one that is struggling does not,
even in the houses that are doing well.

**Deaths** have a base rate and a raised one for the houses below the hunger threshold. It is the
channel through which a city that loses its food really does empty out, instead of merely stopping
growing.

### The table, in `rules.ron`

Rates **per month and per thousand residents**, because that is the form you read and reason in. The
conversion to ticks divides by `ticks_per_month` with no loss, because the remainder accumulates in
`remainder`.

```ron
demographics: (
    births_per_thousand_per_month: 12,        // at maximum satisfaction
    deaths_per_thousand_per_month: 6,         // at maximum satisfaction
    deaths_per_thousand_per_month_hungry: 60,
    birth_threshold: 60,                      // minimum satisfaction to have children
    // +/- 20% on the rate, from the RNG. It can move into the difficulty
    // profile if high difficulty should also be more volatile.
    jitter_per_thousand: 200,
),
```

**A cross-table check** (`Inconsistency::UnsustainableDemographics`): at maximum satisfaction the
births have to exceed the deaths. Otherwise a perfect city empties out and **no growth scenario is
winnable** — a piece of balancing the game would never flag, and which would only be discovered with
M2's heuristic bot.

### The RNG: a new domain, and a serious trap

```rust
pub enum RngDomain { Events, Migration, Production, Demographics }
```

Four lines (`ALL`, `salt`, `index` with the **new slot at the end**, `from_index`), and by
construction it does not knock the existing recordings out of phase —
`sequences_are_reproducible_with_the_expected_values` checks that. `RngDomain::ALL` grows, so
`hash_world` changes anyway: this phase's regeneration is expected.

Two draw sites, and only two:

1. **The jitter on the rate**, one draw per flow per tick:
   `effective_rate = base_rate × (1000 + jitter) / 1000`, with `jitter ∈ [−J, +J]` symmetric. The
   result accumulates in `remainder` and matures into whole events.
2. **The choice of house** the event falls on, one draw per event, from the eligible set built in
   `HouseId` order.

**Not a die per house** (3,750 draws per tick). It is not for the cost: D5 asks for a coarse
simulation, and the rule "the rate is random, the distribution is deterministic" is easier to
explain, to balance and to read in a recording.

#### The trap, to be closed before writing the births

`rand::Rng::random_range` uses **rejection sampling**: the number of `next_u64()` values consumed
depends on the values drawn, and therefore on the seed. There are two consequences, and the second is
serious:

- `rng.draws(d)` stops being a function of the game state and becomes noise, in exactly the field
  [phase 02](02-rng-determinism.md) put into the hash to make a divergence attributable.
- `different_seeds_give_different_hashes` would pass **even if the demographics did absolutely
  nothing**. A green test that checks nothing is worse than a red one.

What is needed is a fixed-cost draw in the core:

```rust
impl Stream {
    /// An integer in `0..n`, with **exactly one** draw, always.
    ///
    /// Widening multiplication instead of `random_range`'s rejection sampling:
    /// rejection consumes a number of values that depends on the seed, and then
    /// `draws()` stops being a function of the game state — that is, it stops
    /// being of use to the state hash, and it makes
    /// `different_seeds_give_different_hashes` pass without the game having
    /// changed.
    ///
    /// The bias is 2^-64 relative: irrelevant, and in any case preferable to a
    /// variable cost in a core that has to be deterministic in the number of
    /// draws as well as in the values.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { return 0; }
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }
}
```

It has to be written **before** the births, not after, with the test that pins it down (test 6).

#### How it squares with D4

There is nothing to square: the seed is in the header, the RNG is in the state, and the same seed ⇒
the same result bit for bit. What the player perceives as randomness is that they do not know the
seed. The formulation to keep: ***the randomness is in the choice of seed, not in the execution.***

### `StepReport` gains a summary

No event per house per tick — that would be the polling the boundary with the renderer forbids. In
its place, an aggregate for the tick, which `xtask run` and M2's evaluator also need:

```rust
pub struct StepReport {
    pub rejected: Vec<(usize, CommandError)>,
    pub events: Vec<Event>,
    /// The tick's aggregates. **Not** delta events: it is the snapshot the
    /// renderer uses for the bars at the top and the evaluator for its metrics.
    /// As events they would be one per house per tick.
    pub summary: Summary,
}

pub struct Summary {
    pub population: u32,
    pub places: u32,
    pub born: u16,
    pub died: u16,
    pub average_satisfaction: u8,
    // phase 15: immigrated, emigrated, attractiveness
    // phase 16: income
}
```

The only things that stay events are the rare changes of state: `HouseAbandoned { house }` when the
residents reach zero, `HouseRepopulated { house }` when they come back up.

### The hash

`remainder` (4 × `i64`) in the state block. The totals stay out, like `FoodTotals` and for the same
reason: they are diagnostics, they decide nothing.

## Out of scope

Immigration and emigration (phase 15). Disease and epidemics: those are random events, step 8, and
they are outside M1. Age, families, trades — D5 says the unit is the house.

## Tests

1. **Growth**: a served city, the population rises and **stops** when the houses are full. The
   plateau counts as much as the climb: it is the proof that the `residents <= max_residents`
   constraint bites.
2. **Emptying out**: with the farm demolished, the population falls. With the hunger rate the fall
   is computable from the `DataSet`.
3. **Population conservation** (property test, *the phase's accounting goal*):
   `Σ residents == born + immigrated − died − emigrated − evicted − lost_to_demolition`, an
   **exact** equality after any sequence of commands. It is the analogue of food conservation, and
   like that one it finds the flow somebody forgot to count.
4. **`population_stays_consistent` changes its outcome on purpose**: `population == house_count × 4`
   is false by construction from here. It becomes two stronger invariants —
   `population == Σ residents` and `residents <= max_residents(level)` for every house. The second
   is what holds up *covered ⇒ eats*, so it is not cosmetic.
5. **`different_seeds_give_different_hashes` is re-enabled**, and from here it can never be
   `#[ignore]` again.
6. **The number of draws does not depend on the seed** — the test that makes `draws` information
   rather than an accident: the same game with ten different seeds, `draws(d)` identical for every
   domain at every tick. Without this, test 5 is green and empty.
7. **The jitter does not move the mean**: over ten thousand ticks the number of births is within 1%
   of the base rate. It catches asymmetric jitter, which would shift the whole balancing invisibly.
8. **A hundred seeds give different but always plausible trajectories**: the population at five years
   sits in a narrow band and is never twice the same. It is the operational definition of "a minimum
   of randomness but not two identical games" — without the first half the balancing is a lottery,
   without the second the RNG is of no use.
9. **Adding a domain does not knock the others out of phase**:
   `sequences_are_reproducible_with_the_expected_values` stays green with `Demographics` in the
   table. It is the proof, four phases later, that phase 02 was right.
10. **Demolishing an inhabited house** counts the residents in `lost_to_demolition`. The same test as
    `demolishing_a_farm_records_the_stock_it_loses`, and for the same reason.
11. **`commands.rs:25` changes its outcome**: `assert_eq!(w.rng(), before.rng())` becomes false as
    soon as the demographics run. To be rewritten as "an empty tick consumes a **known** number of
    draws", which is stronger.
12. **`demographics_invalidate_the_coverage`** (new, A12's contract): if the population changed
    during a tick, then at the end of the tick `dirty.coverage_needs_recompute()` is true. And the
    converse holds too — a still population, a clean coverage — because that is what says the
    invalidation is conditional and not an unconditional `mark_all_providers_dirty`.
13. **`coverage_equivalence` still runs, on a zero-rate dataset**, and stays identical in form. The
    comment explaining *why* the fixture has zero rates has to be added: without it, someone will
    "fix" it by putting the real values in and the test will stop running without going red.
14. **The coverage adapts to growth**: a served house that grows beyond what the provider can serve
    ⇒ on the next tick it falls out of the coverage, and *covered ⇒ eats* stays green. It is A12's
    game loop observed at its smallest: the city outgrows its services.

## Verification

```sh
cargo test -p sim-core demographics
PROPTEST_CASES=2000 cargo test -p sim-core --release population_conservation
cargo test -p sim-replay                       # different_seeds_give_different_hashes no longer ignored
cargo xtask run --ticks 1800 --dump-every 90   # five years: the curve has to be plausible
cargo xtask regen-expected
cargo xtask bench
```

The 1800 ticks are the by-eye proof that closes the phase, and they should be looked at with the same
suspicion as phase 07's dump: if the curve explodes or dies out, that is balancing (`sim-data`), not
code — but it has to be sorted out now, because `regen-expected` freezes it.

### Measuring A12's price, which is this phase's real job

The cost has to be **attributed**, not just noted, or before M2 nobody will know where to start
taking it away. Three runs on the same machine are needed:

```sh
cargo xtask bench --difficulty normal                       # real rates
cargo xtask bench --difficulty normal --zero-demographics   # zero rates
```

- **`A` at zero rates** has to stay where it was at the end of phase 13. If it has moved, the cost is
  not the coverage recomputation but step 6 itself, and that is a different thing to optimise.
- **`A` at real rates** is the number A12 costs. The expectation is ~3.3 ms against 248 µs; if it
  were much worse, there is something else and it has to be found now.
- **Measure `I`** (step 6 alone) separates the demographic work from the recomputation it triggers.
  Without it, the cost of the demographics can only be deduced by subtraction, and the difference is
  dominated by noise.

The **count of recomputations** should be added too: `coverage().recomputes()` before and after each
measure, with the delta printed. An `A` paying milliseconds with zero recomputations would be a
completely different diagnosis.

The three numbers go into [18](18-invariants-closeout-m1.md) and are the input to
[A17](open-decisions.md), the open performance task to be closed before M2. **Nothing gets optimised
in this phase**: `CLAUDE.md` says not to optimise before the profiler, and A11 is the story of what
happens when you guess instead of measuring.

**Done when:** test 3 (exact conservation), test 6 (draws independent of the seed) and test 12 (the
invalidation contract) pass, and the three numbers above are recorded. Test 6 comes **before** test 5
in writing order: without it, 5 means nothing.
