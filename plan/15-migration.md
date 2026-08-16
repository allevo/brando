---
id: 15
kind: phase
status: not-yet-built
opened: 2026-08-09
---

# Phase 15 — Immigration and emigration

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

**Goal:** two cities identical except for their coverage receive different migration flows; a city
with no free places receives nobody, and one that has places but has left them uncovered fills them
**and then gets worse**.
**Depends on:** 14.
**Size:** M.
**Decisions involved:** A15, A12, A10, A14, D3, D5.

## Why now

Because it closes the demographic square, and because it is the first use of `RngDomain::Migration`,
which has existed under that name since phase 02 and has never done anything.

It comes **after** births and deaths, not together with them: those are local to the house, this one
needs a city-wide index and a distribution rule. With the two RNG domains separate, writing it does
not knock phase 14's sequence out of phase and that phase's recording stays as it was — which is
exactly why the domains are separated by name and not by position.

And it is the phase in which A12's loop becomes observable in full: until now
the population only grew from the inside, with a natural ceiling in the existing house capacities.
With immigration the city can **outgrow its own services** quickly, which is the dynamic behind the
services chasing the population — and the first one in which it can go wrong.

## What gets built

### Attractiveness

```rust
/// How much the city attracts, in thousandths.
///
/// A weighted sum of whole-number terms, all from the state: the average
/// satisfaction weighted by residents, and the free places in **served** houses.
/// Phase 16 adds the tax rate as a third term, and not before: coupling the two
/// phases would mean balancing migration and taxes together, which is two
/// problems in one (A15).
///
/// A **pure** function of the state: no RNG in here. The jitter is in the flow,
/// not in the index — that way the attractiveness can be printed in the dump and
/// compared between two games, and it stays the number the semantic observation
/// for the LLM (M3) will show in place of the serialised state.
pub fn attractiveness(world: &World) -> i32;
```

The weights in `rules.ron`, never in the code.

**The weighted average divides by the population**, which can be zero: a city with no residents is a
perfectly normal state, and `no_panic_on_ten_thousand_commands` produces them constantly. The case
has to be handled explicitly — the attractiveness of an empty city equal to that of a just-satisfied
one, so the first immigrants do arrive — not with a `debug_assert`.

### Step 6, complete

```rust
/// Step 6 — levelling up, decay and the four demographic flows.
///
/// The internal order is **game semantics** as much as the order of the ten steps,
/// and the same rule applies: do not reorder without regenerating the recordings
/// and writing down why.
fn houses_and_migration(world: &mut World) {
    update_satisfaction(world);             // 6.1  phase 12
    if is_monthly_review(world) {
        decay(world);                       // 6.2a phase 13, includes the eviction
        level_up(world);                    // 6.2b phase 13
    }
    deaths(world);                          // 6.3  phase 14
    emigration(world);                      // 6.4  this phase
    births(world);                          // 6.5  phase 14
    immigration(world);                     // 6.6  this phase
}
```

The complete order and its reasons, to be written down once in `tick.rs`:

- **6.1 first**: it reads only what steps 3 and 4 have written this tick, and everything else reads
  it.
- **Decay before levelling up**: a house on its way down must not be able to rise in the same tick
  because of one leftover requirement.
- **6.2 before the demographics**: a new level changes the capacity, and the places it opens have to
  be fillable **in the same tick**. Otherwise every jump costs a tick of growth missed: invisible in
  a table, very visible in the curve.
- **Departures before arrivals** (6.3–6.4 before 6.5–6.6): it guarantees
  `residents <= max_residents` at every observable instant, and makes the population responsive
  instead of jerky.
- **Immigration last**: it is the residual flow and it depends on the *final* free places. Putting
  it earlier would mean computing them on a state that is still changing.

The only defensible reordering would be 6.5 before 6.3 — births before deaths — which in aggregate
changes nothing but would stop a full house from replacing someone who died in the same tick. Worse.
The order above is the one to fix.

### Where the immigrants go, and where a gate is **not** put

Zero free places ⇒ zero immigration, at any attractiveness. Not a reduced rate: zero. Houses are the
only container for population (D5), and with no container there is no flow.

Within the houses with room, the eligible set is the **served** houses. It is the cheapest part of
A10: *"migrants only go where life is good"* is not a separate mechanism, it is
the filter — the same houses levelling up considers worthy, for the same reason.

**The gate that does not get put in**, and it is the decision to write down: immigration is **not**
made conditional on the providers' remaining capacity. It would be easy and it would look prudent —
let nobody in if the well is at its limit — but it is exactly the opposite of
A12. With that gate the services would go back to preceding the population, and
the city would stop on its own without the player having to notice anything.

Without it, what has to happen does: the city fills up, outgrows its own services, the coverage
starts failing someone, satisfaction drops, emigration switches on. **The player sees the problem
and builds.** The overshoot is the signal, not a bug.

A warning worth writing alongside: with the capacity counted on residents, an **empty** house costs
zero and therefore always comes out served. At `hard` difficulty, where every house is born empty,
the "served houses only" filter filters nothing at the start. That is correct — an empty house needs
no water — but it means the first wave of immigrants spreads everywhere and the deficit shows up all
at once a few ticks later. It is the most delicate moment of the curve and it has to be watched in
the dump.

### The real risk of this phase: the loop might not damp

With the services chasing, the system has a natural negative feedback — too many people ⇒ coverage
falling short ⇒ emigration ⇒ fewer people ⇒ coverage returning. If the gains are badly tuned, it does
not converge: it oscillates, and the city pulses forever instead of settling.

Three things damp it, and all three are already in place for other reasons: satisfaction is a slow
accumulator (A10), so one tick of missing coverage makes nobody leave; the level review is monthly
(phase 13); and emigration has a threshold, it is not proportional to the shortfall.

What is missing is the **proof that it is enough**, and that is test 9. If it does not damp, the
remedy is balancing — a lower emigration threshold, a slower accumulator — not a new mechanism.

One thing that must **not** be done to damp it: making the coverage "sticky", i.e. giving priority to
houses already served. It would reduce the churn but it would make the coverage depend on history,
and `compute_from_scratch` has no history — `coverage_equivalence` would fall over, and with it the
guard on every future optimisation of step 3.

### Emigration

Houses below a satisfaction threshold lose residents, at a rate from the table. It is the channel by
which a city that decays empties out **even without deaths** — the difference between "people are
starving to death" and "people are leaving", which are two different things and have to read
differently in the dump.

Those evicted by decay (phase 13) flow in here in the totals: they are emigration, not a fifth flow.

### The table and the checks

```ron
migration: (
    // Attractiveness weights, in thousandths.
    satisfaction_weight: 700,
    free_places_weight: 300,
    // Maximum inbound flow, per thousand residents per month, at full attractiveness.
    immigration_per_thousand_per_month: 30,
    emigration_per_thousand_per_month_unhappy: 40,
    emigration_threshold: 25,
    jitter_per_thousand: 200,
),
```

**A cross-table check**: `emigration_threshold < birth_threshold`, and below level 1's decay
threshold. If a house emigrated at a satisfaction where it is still having children, the two flows
would fight each other on every tick and the population would oscillate with nothing to flag it — it
is the same shape as `Inconsistency::NoHysteresis`.

The `Summary` gains `immigrated`, `emigrated` and `attractiveness`.

## Out of scope

**Immigrant walkers.** D3 says immigrants are genuinely simulated entities, and they will be: in M3,
along with the logistics walkers. In M1 migration is a number (A14), and what M3
will add is the **travel time**, not the attractiveness rule. It has to be written here, because it
is this phase's simplification and it will be whoever reads it in M3 who has to dismantle it — the
same shape as A5 for the farm.

Emigration towards a destination, rival cities, immigrants with trades or wealth: none of this
exists, and no scenario asks for it.

## Tests

1. **The goal**: two cities identical except for their coverage ⇒ different flows, and the served
   one grows faster. With the same seed, so the only difference is the coverage.
2. **The hard gate**: attractiveness at maximum, zero free places ⇒ zero immigrants, for a hundred
   ticks. And on adding one served house, the flow restarts in the same tick.
3. **Served houses only**: an uncovered house with free places receives nobody, even if it is the
   only one with room in the whole city. The case has to be built with a house that is **already
   inhabited** and uncovered: an empty one is served at zero cost and would distinguish nothing.
4. **Emigration**: with the well demolished, the houses empty out through emigration **and** through
   deaths, and the totals tell the two apart. The test is on the totals, not on the population: it is
   the distinction that has to be checked.
5. **Population conservation stays exact** with the two new flows in it. Phase 14's property test
   does not change form, and that is the sign that the totals were written right.
6. **An empty city**: attractiveness computed over zero residents does not panic and is not absurd.
   It is also covered by `no_panic_on_ten_thousand_commands`, but it is worth a dedicated test that
   names the case.
7. **Phase 14's recording does not change**: `Migration` is a domain distinct from `Demographics`,
   and a scenario with no free places draws nothing from the first. Checkable with a
   purpose-built scenario — and it is the practical proof that the domains are separate.
8. **A hundred seeds**: the population at five years sits in a narrow band, never twice the same.
   Phase 14's test extended, now that the dominant flow is migration.
9. **The loop damps** (property test, *the test that defines the phase*): a city that outgrows its
   own services and is left to itself for five years ⇒ the population converges to a band and **the
   amplitude of the oscillation does not grow**. It is the check that A12's feedback is negative and
   not a pendulum. If it fails, that is balancing — a lower emigration threshold or a slower
   accumulator — not a new mechanism.
10. **The coverage is not sticky**: two equidistant houses contending for the last place ⇒ the
    winner is always the same by `(distance, TileIdx, HouseId)`, not "the one that had it
    yesterday". It pins the choice down and stops anyone introducing stickiness to reduce the churn,
    which would break `coverage_equivalence`.
11. **The overshoot is reachable**: a scenario in which immigration takes the city beyond the
    providers' capacity, and the coverage really does start to fall short. It is A12's game loop: if
    it cannot be built, the implicit gate is tighter than you think.

## Verification

```sh
cargo test -p sim-core migration
PROPTEST_CASES=2000 cargo test -p sim-core --release population_conservation
cargo xtask run --ticks 1800 --dump-every 90
cargo xtask run --difficulty hard --ticks 1800 --dump-every 90
cargo xtask regen-expected
```

The two runs at different difficulties are the by-eye proof that closes the phase: at `hard` the
houses are born empty, so **all** the population arrives by migrating, and the curve has to start
slower but get there. If at `hard` the city never takes off, the attractiveness threshold is badly
tuned — and it is better to find out now than in phase 17, when a scenario will have to declare
itself won.

**Two columns have to be watched together** in the dump: the population and the fraction of residents
covered. With the services chasing, the second has to fall when the first accelerates and come back
up when the player builds. If the coverage stays flat at 100%, A12's loop is not being triggered and
the scenario is too generous; if it collapses and does not come back, it is too harsh.

`bench` has to be re-run and compared with phase 14's three numbers: immigration moves population
every tick like the births, so the cost of the recomputation does not grow, but the **frequency** of
the ticks in which the population moves does. The count of recomputations is the number to watch, and
it goes into A17's data.

**Done when:** test 5 (conservation still exact) and test 9 (the loop damps) pass, and the game at
`hard` reaches a living city within five years.
