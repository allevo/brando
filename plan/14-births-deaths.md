# Phase 14 — Births and deaths

**Goal:** a served city grows until it fills up its houses, one that loses its services empties out;
and `different_seeds_give_different_hashes`, `#[ignore]` since phase 08, is re-enabled and passes.
**Depends on:** 13.
**Size:** L — it is the riskiest phase of M1.
**Decisions involved:** [A12](open-decisions.md), [A14](open-decisions.md), A10, D4, D5.

> **Revised on 2026-08-12**, before writing any of it, by reading the plan against the tree it has to
> land in. Four things had drifted or were wrong, and two of them would have sunk the phase's own
> goals: the conservation equality was missing the term for residents who arrive with a new house,
> the replacement for `population_stays_consistent` was `x == x`, test 6 asked for something the
> design makes impossible, and `RngDomain` had been renamed `RngKind` by the vocabulary review
> ([A19](open-decisions.md)). Each is corrected in place below and marked **(revised)**. The general
> lesson is A5's again: a plan written before the code it has to fit is a hypothesis, and the cheap
> moment to test it is before the first commit, not after the third.

## Why now

Because it is the **first real use of the RNG** in the whole project. Since phase 02 there have been
three streams seeded per kind, their position goes into the state hash, and nobody has ever used
them: `commands.rs` still contains
`assert_eq!(w.rng(), before.rng(), "no system draws from the RNG in M0")`. This phase closes that
circle, and with it test 7 of phase 08.

And it is the phase in which **the population starts moving every tick**, which is
[A12](open-decisions.md)'s choice and its price. Everything that follows in this document descends
from that.

It has to be kept separate from migration (phase 15) for two reasons. One of readability: births and
deaths are local to the house, migration needs a city-wide index and a distribution rule — different
ways to fail, different tests. One concrete: with **two distinct RNG kinds**, writing 15 does not
knock this one's sequence out of phase and does not regenerate a recording that had no reason to
change. It is literally the use case `RngKind` was written for in phase 02, and until now it had
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
    /// Residents who arrived with a newly-built house.
    ///
    /// **(revised.)** It was missing, and without it the phase's accounting goal
    /// cannot hold: a new house is born with `starting_residents_per_house`
    /// people out of nowhere ([A13](open-decisions.md), `tick.rs`), so at `easy`
    /// the equality below is out by four on the first `PlaceBuilding`. It is the
    /// inflow mirror of `lost_to_demolition`, and the third time this shape has
    /// been needed — the first two were a demolished farm's stock (phase 07) and
    /// a demolished house's residents.
    pub settled_on_construction: u64,
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
    /// phase's most important test would report a bug that is not there.
    pub lost_to_demolition: u64,
}

/// The declaration order is **frozen**: `remainder` is indexed by it and the
/// array goes into the state hash, so reordering the variants silently reassigns
/// every accumulated fraction. It is the same hazard as `RngKind`'s order and
/// `Terrain`'s, and it belongs in `GLOSSARY.md`'s frozen values alongside them.
pub enum Flow { Births, Deaths, Immigration, Emigration }
```

**`PopulationTotals` is not new** *(revised)*: phase 13 created it in `sim-core/src/levels.rs` with
`evicted` alone, and wrote in its doc comment that phase 14 is what makes the counter live. It moves
here, keeps `evicted` written by `levels::review`, and `lib.rs` re-exports it from its new home. The
same split `FoodTotals` already has — defined next to the system that owns the concept, written by
whoever causes the flow.

**The unit of `remainder`, spelled out** *(revised)*. "Thousandths of an event" is not enough
precision: dividing into thousandths once per tick truncates up to one thousandth per flow per tick,
which over 1,800 ticks is a systematic **downward** bias of a couple of events — small, invisible,
and exactly what test 7 exists to catch. The accumulator holds the undivided numerator instead:

```text
remainder[flow] += base_rate × eligible_residents × (1000 + jitter)
divisor          = 1000 (per thousand) × 1000 (jitter base) × ticks_per_month
events           = remainder[flow] / divisor;  remainder[flow] %= divisor
```

Nothing is truncated, the modulo keeps the accumulator below `divisor`, and at the reference scale
one tick's increment is ~2×10⁸ against an `i64` — no overflow, and no need to reason about one. The
divisor is a function of the tables, so `ticks_per_month == 0` needs the same guard
`Rules::is_month_boundary` already has: the core does not panic on data.

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

**Deaths** have a base rate and a raised one for the houses that are going without. It is the
channel through which a city that loses its food really does empty out, instead of merely stopping
growing.

**What "going without" means, and it is not "hungry"** *(revised)*. The obvious reading — food
satisfaction below a threshold — breaks the early game. Since phase 13 the production table's level 1
is a hut that requires **water only**, and it is that way on purpose: it is what makes the first level
reachable in a city that has not got a farm yet. Read hunger off the food accumulator regardless of
level and every hut in a farmless city dies at ten times the base rate, so the opening of every game
is a slow bleed by construction, and the level table would have to be rebalanced to compensate for a
rule nobody chose.

The reading that fits is the one the rest of step 6 already uses: **the worst service the house's own
level requires**, the same `required_at(level)` that `mood_of` and the decay check read. A hut with
water is fine; a level-2 house without food is not. One threshold, no new concept, and the raised rate
generalises to every service a level ever demands instead of being wired to food. The table key is
named for what it measures.

### The table, in `rules.ron`

Rates **per month and per thousand residents**, because that is the form you read and reason in. The
conversion to ticks divides by `ticks_per_month` with no loss, because the remainder accumulates in
`remainder`.

```ron
demographics: (
    births_per_thousand_per_month: 12,        // at maximum satisfaction
    deaths_per_thousand_per_month: 6,         // at maximum satisfaction
    // The rate for a house below `unserved_threshold` on any service its own
    // level requires — not "hungry": see above.
    deaths_per_thousand_per_month_when_unserved: 60,
    unserved_threshold: 25,
    birth_threshold: 60,                      // minimum satisfaction to have children
    // +/- 20% on the rate, from the RNG. It can move into the difficulty
    // profile if high difficulty should also be more volatile.
    jitter_per_thousand: 200,
),
```

**Two cross-table checks**, both in `DataSet::inconsistencies` so the `sim-core` fixture is protected
by them too (A5's lesson, generalised in phase 13):

- `UnsustainableDemographics` — at maximum satisfaction the births have to exceed the deaths.
  Otherwise a perfect city empties out and **no growth scenario is winnable**, a piece of balancing
  the game would never flag and which would only surface with M2's heuristic bot.
- the two thresholds have to be reachable, i.e. no greater than `satisfaction.max`. It is the same
  check as `UnreachableThreshold` and for the same reason: a birth threshold above the ceiling means
  no city ever has a child, in silence.

**Which residents the rate is counted against** *(revised, and it decides test 1)*. Births are
counted against the **eligible** residents — those in houses meeting all three conditions — not
against the population. That is what makes the plateau structural instead of a consequence of the
numbers: a city whose houses are all full has nobody eligible, so the rate is zero and the population
stops, and it stops for a reason a reader can point at. Deaths are counted against every resident,
split between the two rates by whether their house is going without.

### The RNG: a new kind, and a serious trap

```rust
pub enum RngKind { Events, Migration, Production, Demographics }
```

*(revised: the type was `RngDomain` when this was written and is `RngKind` since the vocabulary
review, [A19](open-decisions.md). The variant is the only thing being added.)*

Four lines (`ALL`, `salt`, `index` with the **new slot at the end**, `from_index`), and by
construction it does not knock the existing recordings out of phase —
`sequences_are_reproducible_with_the_expected_values` checks that. `RngKind::ALL` grows, so
`hash_world` changes anyway: this phase's regeneration is expected.

Two draw sites, and only two:

1. **The jitter on the rate**, one draw per flow per tick:
   `effective_rate = base_rate × (1000 + jitter) / 1000`, with `jitter ∈ [−J, +J]` symmetric. The
   result accumulates in `remainder` and matures into whole events.
2. **The choice of house** the event falls on, one draw per event, from the eligible set built in
   `HouseId` order.

**Nothing is drawn when there is nothing to draw for** *(revised)*: a flow whose eligible residents
are zero takes no jitter. It costs nothing to write, it keeps `draws` a readable function of the city
rather than of the calendar, and it is what lets `an_empty_tick_only_advances_the_tick` keep asserting
that a world with no houses touches no stream at all — the same sentence it has asserted since phase
04, now true for a reason instead of by absence.

The order of the draws within a tick — deaths' jitter, deaths' houses, births' jitter, births' houses
— is a determinism contract like the order of the ten steps, and is written down in
`demographics.rs`, not left to the reading order of the code.

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

It has to be written **before** the births, not after, with the test that pins it down (test 6a).

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

The only thing that stays an event is the rare change of state: `HouseAbandoned { house }` when the
residents reach zero. *(Revised: `HouseRepopulated` is **not** written here. Nothing in this phase can
put a resident back into an empty house — births need `residents > 0` — so the variant would be
unreachable from the day it was added, and events are outside the hash, so adding it with
immigration in phase 15 costs nothing. `taxable_per_resident` was declared early for the opposite
reason: it is in a **table**, and a table field arriving late means a second regeneration.)*

### The hash

`remainder` (4 × `i64`) in the state block, two of the four slots unused until phase 15 and hashed as
zeros on purpose: the array is sized for all four flows now so that adding migration moves no
recording. The totals stay out, like `FoodTotals` and for the same reason: they are diagnostics, they
decide nothing.

## Out of scope

Immigration and emigration (phase 15). Disease and epidemics: those are random events, step 8, and
they are outside M1. Age, families, trades — D5 says the unit is the house.

### What this phase hands to A18, deliberately *(new)*

[A18](open-decisions.md) — an empty house consumes no provider capacity — stays open and stays
phase 15's to close. But this phase changes the fact it rests on, and the entry has to be amended
rather than left as it reads.

A18 was written about `hard`, the profile where a house is **born** with zero residents. Deaths make
zero residents reachable on **every** profile, `easy` included, and therefore inside the two committed
recordings. Two consequences, both of which are the point of writing this down now:

- the recordings this phase freezes contain the unbounded assignment, so when A18 is closed its cost
  is a regeneration to attribute, not the free one-line change A18 currently promises. That promise
  is no longer true after this commit and the entry should say so;
- an emptied house keeps its coverage, so its satisfaction climbs while nobody lives there, and a
  house at maximum satisfaction with zero residents can be promoted at the monthly review. Harmless —
  a promotion grants permission and not people (A12) — but it is the state phase 15's immigration
  will be filling, so whichever answer A18 takes has to be checked against it.

Test 15 pins the behaviour so the change is visible when it comes.

## The order the work lands in *(new)*

One phase, one PR, and inside it the commits are ordered so that exactly one of them moves a hash.

1. **`Stream::below` and test 6a.** No behaviour, no regeneration. It is first because the trap it
   closes has to be closed before anything draws — the phase file has said so from the start, and it
   is the one instruction here that was already right.
2. **The phase proper, with the single regeneration.** `demographics.rs`; `RngKind::Demographics`;
   the `rules.demographics` table with its raw shape, its field checks, its two cross-table checks and
   its line in `dataset_hash`; step 6 extended and the conditional invalidation as its last act;
   `PopulationTotals` moved and completed; `Summary`; `HouseAbandoned`; `hash_world`, `every_field`
   and the `test-util` hook the hash-coverage test needs; and the test rework — conservation in place
   of `population_stays_consistent`, `coverage_equivalence` moved onto a zero-rate fixture with the
   comment that says why, `commands.rs`'s RNG assertion re-worded.

   It is one commit and not four because every one of its parts moves the dataset hash or
   `RngKind::ALL`, and four commits would mean four regenerations of the same two files.
3. **The measurement.** `bench --zero-demographics`, the recompute deltas, the three numbers written
   into [18](18-invariants-closeout-m1.md) and [A17](open-decisions.md). No hash moves.
4. **The documents.** A14 gains its "how it really went"; A18 gains the amendment above; `GLOSSARY.md`
   gains a row for **demographics** and one for **flow**, promotes **jitter** out of the
   "not written yet" note, and adds `Flow`'s declaration order to the frozen values next to
   `RngKind`'s; `README.md`'s "Implemented till" moves to 14.

## Tests

1. **Growth**: a served city, the population rises and **stops** when the houses are full. The
   plateau counts as much as the climb: it is the proof that the `residents <= max_residents`
   constraint bites.
2. **Emptying out**: with the farm demolished, the population falls. With the hunger rate the fall
   is computable from the `DataSet`.
3. **Population conservation** (property test, *the phase's accounting goal*) *(revised)*:

   ```text
   Σ residents == settled_on_construction + born + immigrated
                − died − emigrated − evicted − lost_to_demolition
   ```

   an **exact** equality after any sequence of commands. It is the analogue of food conservation, and
   like that one it finds the flow somebody forgot to count — starting with the one this line was
   itself missing. `immigrated` and `emigrated` stay at zero until phase 15 and are in the equation
   from now so that phase adds no term to it.

4. **`population_stays_consistent` is replaced, not rewritten** *(revised)*. The plan said it should
   become `population == Σ residents`, and that is `x == x`: `World::population()` is defined as the
   sum over the houses. It is the same trap phase 12 fell into when unifying the two `served` bits
   turned `covered_means_fed` into a comparison of a field with itself — twice in three phases, which
   is enough times to make it a thing to look for rather than an accident.

   The half that was worth keeping, `residents <= max_residents(level)`, already exists as
   `residents_stay_within_the_house_capacity` (phase 13). So `population_stays_consistent` goes away
   and test 3 takes its row in `invariants.rs`'s table. Conservation is also profile-agnostic by
   construction — it reads the flows, never `house_count × 4` — which is the prerequisite
   [A18](open-decisions.md) asks phase 14 for: from here the property suite *can* be pointed at
   `hard`.
5. **`different_seeds_give_different_hashes` is re-enabled**, and from here it can never be
   `#[ignore]` again.
6. **The draws are a function of the state, not of rejection sampling** *(revised)* — the test that
   makes `draws` information rather than an accident. As first written it asked for the same game
   under ten seeds to report identical `draws` at every tick, and that cannot hold: the choice of
   house is one draw **per event**, and how many events mature depends on the jitter, i.e. on the
   seed. The property is real, the formulation was not. It splits in two, and both are needed:

   - **6a — `below(n)` consumes exactly one draw**, for every `n` and every seed, including
     `n == 0` and `n == u64::MAX`. This is the anti-rejection-sampling guard itself, tested on
     `Stream` where it can be asked directly instead of inferred from a game.
   - **6b — a game in which no event can mature consumes a known number of draws**, the same under
     every seed: zero for a world with no houses, and `flows × ticks` for a city whose rates are at
     zero. It is 6a's consequence observed through `step`, and it is what says the count is decided
     by the city and not by the values that came out.

   Without both, test 5 is green and empty.
7. **The jitter does not move the mean**: over ten thousand ticks the number of births is within 1%
   of the base rate. It catches asymmetric jitter, which would shift the whole balancing invisibly.
8. **A hundred seeds give different but always plausible trajectories**: the population at five years
   sits in a narrow band and is never twice the same. It is the operational definition of "a minimum
   of randomness but not two identical games" — without the first half the balancing is a lottery,
   without the second the RNG is of no use.
9. **Adding a kind does not knock the others out of phase**:
   `sequences_are_reproducible_with_the_expected_values` stays green with `Demographics` in the
   table. It is the proof, four phases later, that phase 02 was right.
10. **Demolishing an inhabited house** counts the residents in `lost_to_demolition`. The same test as
    `demolishing_a_farm_records_the_stock_it_loses`, and for the same reason.
11. **`commands.rs:25` keeps its outcome and changes its meaning** *(revised)*.
    `assert_eq!(w.rng(), before.rng(), "no system draws from the RNG in M0")` was expected to go
    false. It does not: that world has no houses, so no flow has an eligible resident and nothing is
    drawn. The assertion stays, the message stops being true, and it is rewritten to say what now
    holds — *a tick with nothing to decide draws nothing* — which is a live rule rather than a note
    about a milestone that is over. Its sharper companion is 6b.
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
15. **An emptied house is pinned down, and the test is written to change its outcome** *(new)*. See
    "What this phase hands to A18" below: deaths make a house of zero residents reachable on every
    profile, and such a house goes on consuming no provider capacity. The behaviour gets a test that
    states it as it is today — a house emptied by deaths keeps its coverage, and a small well keeps
    serving it — so that when phase 15 closes A18 the test **changes its outcome** rather than
    breaking. It is the third time this device is used, after
    `a_hungry_house_is_not_saved_by_a_second_farm` and its successor
    `hunger_is_cured_by_building_a_second_farm`, and it is the cheapest way to make a deferred
    decision visible in the suite instead of only in a document.

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

**What the curve is expected to look like, so that "plausible" is not decided after the fact.**
On `minimal` at the production numbers: four houses of four, so nobody is eligible for a child until
the first monthly review promotes them to level 2 and the ceiling goes from four to eight. From tick
30 the eligible population is sixteen, births run at 12 per thousand per month and deaths at six, so
the year the recordings cover contains roughly two births and one death — enough to move the hash and
few enough to read by hand, which is what the 32×32 scenarios are for.

Over five years the interesting part arrives: the farm sustains twenty residents, the city grows past
it, and the house that falls out of the food coverage loses its food satisfaction at eight a tick,
drops under the decay threshold in ten, comes down a level at the next review and **evicts** whoever
no longer fits. So expect a plateau with a sawtooth on it, not a smooth ceiling — and expect
`evicted` to be the largest outflow in `PopulationTotals`, because in this phase eviction and death
are the only ways to leave. Emigration is phase 15, and until it exists the city's answer to
overcrowding is to delete people. That is worth seeing before it is frozen, and it is the strongest
argument for keeping 14 and 15 adjacent.

**Attribution, and why protocol point 5 does not apply here** *(revised)*.
[README](README.md)'s regeneration protocol says to look at the first diverging tick in the `.hashes`
diff. That check cannot say anything in this phase: `hash_world` hashes the dataset hash at every
checkpoint, so **any** new table field diverges the recordings from the first checkpoint, whatever the
mechanic does. The same is true of `RngKind::ALL` growing. The point-5 question — *did the new
mechanic act when it could first have acted, and not before* — has to be asked of the textual dump
instead:

```sh
cargo xtask run --scenario minimal --ticks 1800 --dump-every 1 > after.txt   # and before.txt on HEAD~
```

and the first line where the population column differs has to be at or after the first review that
promotes a house, because before that no house has room for a child. If it differs earlier, something
else moved and it has to be found before committing. This is worth adding to the protocol in
`README.md` as the general rule: **point 5 is about the recording, not about the hashes, whenever the
change touches a table.**

### Measuring A12's price, which is this phase's real job

The cost has to be **attributed**, not just noted, or before M2 nobody will know where to start
taking it away. Three runs on the same machine are needed:

```sh
cargo run --release -p xtask -- bench                       # real rates
cargo run --release -p xtask -- bench --zero-demographics   # zero rates
```

*(Revised: the flag was written as `--difficulty normal`. It should not be. `bench` measures on
`easy` by an explicit choice documented in `BENCH_DIFFICULTY`, because that is the profile the numbers
in [09](09-invariants-closeout.md) and phase 13 were taken on, and at `normal` the synthetic city has
half the residents — the measure would stop being comparable with the ones A17 has to be closed
against. `--zero-demographics` is the flag this phase adds, and it builds the same dataset variant
`with_unlimited_treasury` already builds.)*

- **`A` at zero rates** (`H`) has to stay where it was at the end of phase 13. If it has moved, the
  cost is not the coverage recomputation but step 6 itself, and that is a different thing to optimise.
- **`A` at real rates** is the number A12 costs. The expectation is ~3.3 ms against 248 µs; if it
  were much worse, there is something else and it has to be found now.
- **`I` (step 6 alone) is derived, not measured, and that is a limit to state rather than work
  around** *(revised)*. `G` can be measured directly because `compute_from_scratch` is pure and can be
  run on the same world a hundred times; step 6 mutates, so measuring it in isolation would mean
  either cloning the world per repetition — the clone dominates — or exposing a mutating internal of
  the core to `xtask`, which is a worse thing to own than an imprecise number. What is measured is
  `H`, `A`, `G` and `J` (the fraction of ticks where the population moved, **counted** over a real
  1,800-tick game and not estimated), and `I` follows as `(A − H) − J × G`. Every term on the right is
  measured, so the derivation is arithmetic and not a guess. If it comes out noisy, the next step is a
  bench preset whose city cannot move — every house full at the top level, fully served — which
  isolates step 6's fixed cost with no new API; it is not worth building before the number says it is
  needed.

The **count of recomputations** should be added too: `coverage().recomputes()` before and after each
measure, with the delta printed. An `A` paying milliseconds with zero recomputations would be a
completely different diagnosis.

The three numbers go into [18](18-invariants-closeout-m1.md) and are the input to
[A17](open-decisions.md), the open performance task to be closed before M2. **Nothing gets optimised
in this phase**: `CLAUDE.md` says not to optimise before the profiler, and A11 is the story of what
happens when you guess instead of measuring.

**Done when:** test 3 (exact conservation), tests 6a and 6b (the draws are decided by the city, not by
rejection sampling) and test 12 (the invalidation contract) pass, and the three numbers above are
recorded. Test 6a comes **before** test 5 in writing order: without it, 5 means nothing.
