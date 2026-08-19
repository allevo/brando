---
id: 15
kind: phase
status: implemented
opened: 2026-08-09
closed: 2026-08-19
---

# Phase 15 — Immigration and emigration

> See [How it went](#how-it-went) at the end, which is where the shape it really took is written
> down. Everything above that section is the plan as it was written, including the parts the work
> proved wrong — the prediction next to the outcome is the point.

**Goal:** two cities identical except for their coverage receive different migration flows; a city
with no free places receives nobody, and one that has places but has left them uncovered fills them
**and then gets worse**.
**Depends on:** 14.
**Size:** M.
**Decisions involved:** A15, A12, A10, A14, D3, D5.

> **Revised on 2026-08-19**, before writing any of it, by reading the plan against the tree it has to
> land in. Slot 14.5 closed on 2026-08-17 — eight days after this plan was written and two before it
> was built — and its answer rules out this plan's own design for where an immigrant is sent: *"a
> house with nobody in it is not served"*, and immigration "must not make coverage a precondition for
> moving into a particular house," on pain of an absorbing state exactly like the one 14.5 itself
> closed. This plan's "served houses only" gate is that precondition, and on `hard` — every house born
> empty, hence never served — it would have meant no house is ever eligible, for ever: the very profile
> this phase's own verification section asks to watch would never take off. Three passages are
> corrected in place below and marked **(revised)**; a fourth number the plan proposed — immigration
> counted per thousand *residents* — has the identical bootstrap failure for the identical reason, and
> is corrected alongside it. The general lesson is phase 14's again: a plan written before the code it
> has to fit is a hypothesis, and the cheap moment to test it is before the first commit, not after.

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

**(revised)** Within the houses with room, the eligible set is **every house with room, served or
not**, and not "the served houses" as first written here. Slot 14.5, closed after this paragraph and
before the phase was built, answers a narrower question — does an empty house consume provider
capacity — and in doing so rules the wider filter out by name: *"Immigration must not make coverage a
precondition for moving into a particular house. Coverage drives the rate at which a city attracts
people ... if it also decided which house they enter, an emptied house could never be refilled —
uncovered because empty, empty because uncovered."* Coverage still decides how much a city attracts,
through `attractiveness`; it stops deciding which house within it a migrant lands in.

**The gate that does not get put in**, and it is the decision to write down: immigration is **not**
made conditional on the providers' remaining capacity. It would be easy and it would look prudent —
let nobody in if the well is at its limit — but it is exactly the opposite of
A12. With that gate the services would go back to preceding the population, and
the city would stop on its own without the player having to notice anything.

Without it, what has to happen does: the city fills up, outgrows its own services, the coverage
starts failing someone, satisfaction drops, emigration switches on. **The player sees the problem
and builds.** The overshoot is the signal, not a bug.

**(revised)** The warning this paragraph used to carry assumed the opposite of what 14.5 answered: it
read an empty house as costing nothing and therefore *always* coming out served, and reasoned from
there about the first wave spreading everywhere. Slot 14.5 answers the other way — an empty house is
never served, at any distance, for any provider — which is the sharper version of the same worry: on
`hard`, where every house is born empty, "served houses only" would not merely filter nothing, it would
admit nothing, for ever. That is exactly why the filter above is now "every house with room" instead.
The delicate moment the original paragraph was reaching for is real all the same and still has to be
watched in the dump: the first wave of immigrants lands wherever there is room, spreads before the
coverage has caught up to any of it, and the deficit this creates is what step 3 has to chase down over
the following ticks.

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
    // Maximum inbound flow, per thousand free places per month, at full attractiveness.
    immigration_per_thousand_per_month: 30,
    emigration_per_thousand_per_month_unhappy: 40,
    emigration_threshold: 25,
    jitter_per_thousand: 200,
),
```

**(revised)** `immigration_per_thousand_per_month` is counted against the **free places**, not against
the population as first sketched here. A rate against the population has the identical failure the
destination gate above had, for the identical reason: a `hard` city starts at zero residents, and zero
residents times any rate is zero, for ever. Free places exist the moment a house is built, whatever its
occupancy, so the rate can act from the first tick — and "zero free places ⇒ zero immigration" falls
out of that multiplication for free instead of needing a gate written on top of it.

**A cross-table check**: `emigration_threshold < birth_threshold`. If a house emigrated at a
satisfaction where it is still having children, the two flows would fight each other on every tick and
the population would oscillate with nothing to flag it. **(revised)** The second half of this check as
first written — "and below level 1's decay threshold" — does not survive contact with the table: level
1 has no level below it to decay to, so its `decay_threshold` is unread and written as zero by
convention (see `rules.ron`), and a check demanding `emigration_threshold` sit below zero could never
pass. Dropped; the relation that remains is the one the paragraph's own argument is about. And the name
this plan reached for, `Inconsistency::NoHysteresis`, does not survive either: the vocabulary review
retired "hysteresis" in favour of "gap" before this phase was built (`CLAUDE.md`'s naming rule), so the
variant that implements this check is named for the gap it guards instead.

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
   ticks. And on adding one house with room, the flow restarts in the same tick, whether or not that
   house is covered.
3. **(revised) Coverage is not a precondition for the destination**: an uncovered house with free
   places receives immigrants all the same, even when a covered house with room also exists — the
   opposite of what this test asked for as first written, and the direct consequence of slot 14.5's
   answer. The case is built with a house that is **already inhabited** and uncovered, the same
   construction the original test proposed: an empty one would not distinguish "not gated on coverage"
   from "not gated on anything at all", since an empty house is never served either way.
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

## How it went

All of it landed, and the biggest single change from the plan was decided **before** the first line of
code, not discovered afterwards: the three passages marked **(revised)** above, corrected against slot
14.5's answer and against the immigration-rate bootstrap trap 14.5's argument turns out to generalise
to. Both are recorded there, next to the sentences they replace, rather than repeated here.

**The fixture's own migration rates had to be found by trial, not chosen up front.** The plan asked for
numbers "deliberately large... a test that had to run five game years to see one [event] would be a
test nobody runs" — the same instinct phase 14 had for its own rates. The first attempt (500 per
thousand per month, both flows) broke two existing coverage tests that build a handful of houses over
a dozen ticks and were never about migration at all: `on_hard_an_empty_house_is_not_served` and
`the_capacity_serves_the_nearest_ones`. The cause was not a bug — a freshly built house's satisfaction
accumulator starts at zero and stays below `emigration_threshold` for the several ticks it takes to
climb, so *every* house is briefly emigration-eligible right after construction, and a large enough rate
moves somebody during exactly the short window those two tests build their cities in. `demographics.rs`'s
own large rates carry the identical exposure and simply never happened to fire in those two tests, on
that seed — luck, not a guarantee. The fixture's rates came down to a tenth of the first attempt
(`IMMIGRATION_PER_THOUSAND`/`EMIGRATION_PER_THOUSAND_UNHAPPY` at 60), which stopped colliding with the
short-window tests and meant the migration-specific tests could no longer rely on the shared fixture
alone: three of them build their own dataset variant, cloned from the shared one with a single field
changed — `dataset_with_fast_immigration` to see a slow, uncovered scene inside a few thousand ticks
instead of several hundred thousand, and `dataset_with_migration_off` to isolate the domain-separation
property from the base death rate every real city pays regardless of coverage.

**A rate of zero is not the same as a stream that never draws, and one test had to unlearn that.** The
first version of the domain-separation test expected a "full, served, satisfied" house to leave
`RngKind::Migration` untouched, and it does not: `jitter` draws exactly once for every eligible flow on
every tick *by construction*, regardless of what the table's rate says — the same rule
`demographics.rs` already lives by, so that `draws` stays a function of the city and never of a number
somebody could retune. A served, satisfied house still pays the demographics' own base death rate, so a
long-lived city will eventually free a place no matter how well it is served, and the moment it does,
immigration has something to draw jitter for again. The test that survives switches migration off at the
table instead (`MigrationRules::is_off`, the same shape `DemographicsRules::is_off` already had) and
checks the thing that really is guaranteed: no *event* matures, not that nothing is *drawn*.

**Two of phase 14's own tests needed correcting, and both for the same reason: migration's damping is
real.** `flows()` in `demographics.rs`'s test file compared only `born`/`died`/`evicted` before and
after a tick to decide whether "anybody moved" — blind to the two new flows, so
`demographics_invalidate_the_coverage_and_only_then` started reporting the coverage dirtied when
nothing it was watching had moved. Widening the tuple to all five running totals fixed it in one line.
The second was a real prediction proven wrong by the mechanism working: `the_coverage_follows_a_city_
that_outgrows_it` asserted the farm's capacity was still exceeded at exactly the one-year mark, which
held under phase 14 alone but stopped holding once emigration could correct an overshoot back down —
the population still outgrows the farm along the way (confirmed at many ticks in the same run), it just
no longer has to still be overshot at the one arbitrary tick the test happened to sample. The fix checks
the whole year for an overshoot instead of the last instant of it, which is the more honest reading of
what the test was always trying to prove.

**Test 5, 6 and 10 needed no new test at all.** Conservation was already exact in `invariants.rs` —
`PopulationTotals::balance` had carried `immigrated` and `emigrated` in its equation since phase 14,
written in ahead of the phase that would fill them. The empty-city reading of `attractiveness` has its
own inline unit test next to the function, `migration::tests::an_empty_city_is_maximally_attractive`,
in the same style `levels.rs` and `satisfaction.rs` already use for their own pure functions. And the
coverage was never sticky to begin with: `compute_from_scratch` reads no history, migration gave it
none to read, and the two existing tie-break tests in `coverage.rs` already pin the property down.

**The measurement, on the same machine, before and after this phase's changes** (200×200, 15,000
residents, `easy`, `bench --reps 40`):

| | before phase 15 | after |
|---|---|---|
| `A` empty tick, real rates | 2.092 ms | ~2.26 ms |
| `H` empty tick, every rate at zero (migration included) | — | ~0.38 ms |
| recomputations | 202 of 202 ticks | 202 of 202 ticks |

Migration adds roughly 8% to a cost that was already paid on every tick: `J` was already 100% after
phase 14, so the two new flows do not make the coverage recompute *more often* — they add two more
`O(houses)` scans (emigration's eligibility pass, immigration's free-places pass) and up to four more
RNG draws to a tick that was already the expensive case. `H` moved up from phase 13's ~280 µs region
too, for the same reason phase 14 found: step 6 now scans the houses five times even with every rate at
zero, not three. Nothing here is optimised — slot 18.5 is where that work belongs, and this phase's
job was only to measure it honestly, which the paired same-machine numbers above do.

**The by-eye proof asked for in the Verification section holds.** `cargo xtask run --difficulty hard
--ticks 1800 --dump-every 90` climbs from zero residents to the mid-twenties over five years, entirely
by immigration, catching up to within a few residents of `easy`'s trajectory by month 30 — the "starts
slower but gets there" the plan predicted. The population column visibly saws rather than climbing
smoothly or exploding, which is the loop test 9 asks for read by eye instead of by property test.
