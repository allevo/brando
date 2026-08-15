---
id: 06
kind: phase
status: implemented
opened: 2026-08-08
closed: 2026-08-08
---

# Phase 06 — Aggregate service coverage

> It records how the phase was planned and how it went, frozen as it was written. It is
> **not** a description of the tree today: for that see [ARCHITECTURE.md](../ARCHITECTURE.md)
> and [RULES.md](../RULES.md).

**Goal:** the well serves the houses within its range **walked along roads**, with a limited
capacity and a deterministic assignment; the result of the incremental computation matches the one
computed from scratch.
**Depends on:** 05.
**Size:** L — it is step 3 of the tick, the project's hot path.
**Decisions involved:** D2 (aggregate coverage, decorative walkers), D4, D5.

## Why now

It is the heart of D2 and the mechanic all of M1 rests on (houses level up with water and food).
And it is the point where it is easiest to slip towards service walkers: if coverage does not
exist, the temptation to simulate a water carrier is strong, and at that point D2 is broken
irreversibly.

## What gets built

`propagate_coverage(world)`, step 3 of the tick.

```rust
/// For each service, who provides it. A derived structure, like RoadNetwork.
pub struct Coverage {
    /// served_by[house][service] = Some(provider) | None
    served_by: Vec<[Option<BuildingId>; ServiceKind::COUNT]>,
}
```

> **Amended by the documentation audit (2026-08-13).** The sketch above is not the shape that was
> built. `served_by` is a `BTreeMap<HouseId, [Option<BuildingId>; ServiceKind::COUNT]>` and not a
> `Vec` — a `Vec` indexed by house cannot survive houses being removed — and the struct carries a
> second field, `recomputes`, which counts recomputations for the dirty-flag tests and takes no part
> in the game. It stays a derived structure, outside the state hash, exactly as the sketch says.

The algorithm, for each provider in `dirty.coverage`:

1. A BFS over the road network from the road tiles adjacent to the provider, cut off at
   `range_per_level[level]` (from the `DataSet`, never a constant in the code).
2. Collect the houses whose tiles are 4-adjacent to a visited road tile, with the distance they
   were reached at.
3. Assign up to `capacity_per_level[level]` houses.

**The assignment rule — it is game semantics, it has to be documented and not changed lightly.**
When the candidate houses exceed the capacity, the nearest ones are served; at equal distance, the
one with the smaller `TileIdx`. Sorted on `(distance, TileIdx)`, which is a total order: without
the second criterion two equidistant houses would be ordered by the BFS's visit order, i.e. by an
implementation detail, and the recorded replay would become fragile.

**Contention between providers.** Two wells covering the same house: in M0 the house is either
served or not (the field is an `Option<BuildingId>`, and the first to take it in the providers'
iteration order wins — which is `BuildingId` order, deterministic). To be noted as a
simplification: if in M1 the capacity were to become "residents served" instead of "houses served",
this point has to be reopened.

> **Updated after M0.** It happened: the capacity is counted in residents, and with it came two
> rules this phase did not have — no partial assignment, and whoever does not fit in the remaining
> capacity is *skipped* instead of stopping the scan. The first provider still wins a contest, but
> a contested house no longer consumes the capacity of whoever comes second.
> See A5 and `coverage::pick_within_capacity`.

**In M0 `dirty.coverage` may be recomputed naively**: when the roads change, every provider is
dirty. `CLAUDE.md` licenses the naivety here, as long as the flags exist — and they have since
phase 04. What must not be put off is test 6.

### What it does not do

It does not produce walkers. The water carriers the player will see are decorative, live in the
renderer and are derived from `Coverage` (D2). If a `Walker` shows up in this phase, it is a bug —
it is worth a comment at the top of the module saying so.

## Out of scope

The effects of coverage (levelling up, decay, dissatisfaction): those are M1. This phase
**records** who is served, it draws no consequences. The food that gets eaten is phase 07.

## Tests

1. **The base case**: a well, a road, a house 3 road tiles away, range 12 ⇒ served.
2. **Out of range**: a house at distance 13, range 12 ⇒ not served. With range 13 ⇒ served
   (checks that the bound is inclusive, and pins it down).
3. **Distance along roads, not through the air**: a house 2 tiles from the well as the crow flies
   but reachable only by a 20-tile detour, range 12 ⇒ **not served**. It is the test that embodies
   D2; if it passes, the implementation has no Euclidean shortcuts.
4. **A road is required**: a house not adjacent to any road tile ⇒ never served, at any distance.
5. **Capacity**: capacity 2 and three candidate houses at distances 3, 5, 7 ⇒ the ones at 3 and 5
   get served. At equal distance, the smaller `TileIdx` wins. An explicit table-driven test: it is
   the game rule.
6. **Incremental ↔ from-scratch equivalence** (property test, *the* test of the phase): given a
   random sequence of commands (roads, houses, wells, demolitions), the `Coverage` obtained with
   the dirty flags is **identical** to the one recomputed from scratch on the final state. This
   test is what will make optimising step 3 safe when the time comes: any future cleverness about
   incrementality is covered.
7. **Demolition**: with the well demolished, the houses go back to unserved in the same tick. With
   a road demolished so that the network breaks, the houses beyond the break go back to unserved.
8. **Idempotence**: two consecutive ticks with no commands leave `Coverage` unchanged and recompute
   nothing (the `rebuilds`/BFS counter unchanged).

## Verification

```sh
cargo test -p sim-core coverage
PROPTEST_CASES=1000 cargo test -p sim-core --release coverage_equivalence
```

**Done when:** test 3 (distance along roads) and test 6 (incremental/from-scratch equivalence)
pass. They are the two that define the phase; the rest is correctness around the edges.
