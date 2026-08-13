# Beyond M0 — sketched, not planned

> **Status: superseded.**
>
> A sketch of M1–M3 written before M1 was planned, kept unrewritten on purpose so the
> sketch can be compared with what the real plan turned out to be. **Not authoritative for
> anything.** The real phases are 11–18; see [ROADMAP.md](../ROADMAP.md).

This file is **not** a plan. It is the list of what M0 has to make possible, with the questions M0
itself will answer. Planning M1 in detail now would mean deciding on the balancing before having
watched a single tick run, and every number decided in the abstract has to be redone afterwards.

## M1 — A minimal game loop

> **Planned on 2026-08-09**, after M0 was closed and step 3 optimised. The real phases are
> [11](11-difficulty.md)–[18](18-invariants-closeout-m1.md), listed in [README.md](README.md). This
> section stays as it was written: comparing it with the real plan is the only way to know how much
> sketching ahead is worth.
>
> **What held up.** The order — levels → migration → taxes → scenario — and the fact that the
> prerequisite for the levels was the services' capacity, which had indeed already been sorted out
> ([A5](../DECISIONS.md)).
>
> **What did not.** Three things, all of which emerged while planning and none of which could have
> been sketched beforehand.
>
> 1. The prerequisite **was not finished**, and what followed was not a technical question but a
>    gameplay one. With the population changing every tick, a capacity counted against the residents
>    present makes the coverage go stale within the tick itself: `coverage_equivalence` falls over,
>    and it falls over both when growing and when shrinking. Counting it in **places** — the level's
>    capacity, taken or not — would have solved everything at no cost, but it would have forced the
>    services to be sized *before* the population. The opposite was chosen:
>    [A12](../DECISIONS.md), **the services chase the population**, with a recomputation every
>    tick and its price (~13× on the empty tick). That is where [A17](../DECISIONS.md) comes
>    from, the first decision in this project to stay open — and, after the bug hunt that follows
>    phase 13, [A18](../DECISIONS.md) as well.
> 2. The five phases became **eight**. "House levels" and "migration" were two lines that contained
>    four phases; and the closing phase was missing entirely, which in M0 was 09 and is where the
>    lessons get written down instead of remembered.
> 3. The **random events dropped out** of M1 — see below.

How it had been sketched, before being planned:

1. **House levels.** `levels` in the tables becomes > 1; service requirements per level. Houses
   level up if served for N consecutive ticks, and decay otherwise.
   A verifiable goal: a house served with water and food reaches level 2 in a number of ticks you
   can compute from the `DataSet`; take the water away and it goes back to level 1.
   The prerequisite — service capacity in residents rather than in houses — was done first, right
   after M0: see [A5](../DECISIONS.md). The shape of satisfaction (an accumulator of time per
   service, with hysteresis, **not** a resource level) is decided in [A10](../DECISIONS.md). This
   is where `required_services` stops being declarative data and starts being read
   ([A9](../DECISIONS.md)).
2. **Migration.** The first real use of `RngDomain::Migration` — and it is the tick on which test 7
   of phase 08 (sensitivity to the seed) is re-enabled and has to pass.
   Goal: an attractive city grows, a hungry city empties out, and two different seeds give
   different but equally plausible trajectories.
   Aggregate, not per individual: the net flow is decided by a city-wide attractiveness index
   ([A10](../DECISIONS.md)), because immigrants as real walkers are M3 (D3).
3. **Treasury and taxes.** Step 7 of the tick. Goal: phase 09's treasury invariant extends to the
   income and stays an exact equality.
4. **`sim-scenario`.** Mandatory and optional objectives, victory conditions, step 9 of the tick.
   Scenario 1: "500 residents within 5 years" (D7).
   Goal: the scenario declares itself complete on the right tick, and not before.
5. **Random events** (fires, disease): step 8. The riskiest one for determinism, so the last —
   once the recorded replays are mature and already cover a lot.

Every phase adds lines to the existing recordings and, if it introduces an observable mechanic, a
new recorded scenario.

## Random events — out of M1, not yet planned

Fires, disease, invasions: step 8, which at the end of M1 will be the only empty one along with
step 5 (logistics walkers, M3). The first use of `RngDomain::Events`.

Out of M1 for three reasons: `CLAUDE.md` does not list them among that milestone's mechanics, they
are the riskiest for determinism, and the reason they seemed necessary — re-enabling
`different_seeds_give_different_hashes` — is already covered by the demographics.

When they do arrive, one lesson from [phase 14](14-births-deaths.md) is already worth writing down:
the **number** of draws must not depend on the seed, or `rng.draws()` stops being a function of the
state and the state hash stops saying anything. `Stream::below` will already exist.

## Between M1 and M2 — A12's debt

One thing only, and it is [A17](../DECISIONS.md): recomputing the coverage every tick, which is
the price of [A12](../DECISIONS.md). The typical tick goes from 248 µs to ~3.3 ms, and the cost is
paid on every game — and therefore on every batch of automatic balancing, which is requirement 2 of
`CLAUDE.md`.

It has to be closed **before** M2 and not inside M1, for the reason A11 has just demonstrated: you
measure in one phase and optimise in another, or you have no *before*. Phase 18 produces the numbers
(`H`, `I`, `J`); A17 lists the candidate countermeasures in order of payoff-to-risk, and the first
is finally reading the `DirtyFlags::coverage` list, which has existed since phase 04 and has never
been of use to anyone.

To be done in the same batch, and before touching performance: **rebuild a single oracle for step
3**. A12 broke `coverage_equivalence` into two halves that together check less than the original,
and optimising with a wide-meshed net is how you introduce a correctness bug instead of a slowness
one.

## M2 — The two clients

The point of M2 is not the graphics: it is **validating the snapshot/event boundary** from phase 07.
Bevy gets one snapshot on the first frame and then only deltas. If rendering requires reading the
`World` every frame, the boundary is badly designed and you find out here — which is the right
moment.

In parallel and independent: a heuristic bot + an evaluator, which close Testing point 4 (a bot
completes scenario 1 within N months: the canary on the balancing).

## M3 — Depth

Production chains with real logistics walkers (D3), `CivilizationRules` **together with** the second
civilisation (D6), an LLM adapter on top of a bot that already works.

This is where [A5](../DECISIONS.md) has to be revisited: the farm as a coverage provider is an M0
simplification, and in M3 the goods will come from a warehouse carried by real walkers. Coverage
stays, where the goods come from changes.

## Questions M0 will answer (and which have no useful answer now)

- ~~Is step 3 (coverage) really the hot path, or is it the network rebuild?~~ **Answered:** step 3,
  by a long way — `G` is 19.83 ms out of `D`'s 20.05. But the useful answer came later: the cost was
  not the number of BFS runs, it was what each one dragged along with it. Removing that gave 6.5×
  with no new state, and the cache that looked obvious is not being built
  ([A11](../DECISIONS.md)). The number to watch now is `A`, the empty tick.
- ~~Does service capacity make sense in "houses served" or "residents served"?~~ **Answered:** in
  residents, and the answer came before M1 because it was phase 1's prerequisite. The part that
  mattered was not the unit but the rebalancing that goes with it — a producer's capacity has to be
  what its output sustains. Details in [A5](../DECISIONS.md).
- Does 30 ticks/month ([A6](../DECISIONS.md)) give a playable curve? The first scenario with a
  deadline will say.
- Is a 4-byte `Tile` enough once the walkers and the warehouses arrive? Phase 01's budget test will
  flag it the moment it stops being enough.
