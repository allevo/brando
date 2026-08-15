---
id: 05
kind: phase
status: implemented
opened: 2026-08-08
closed: 2026-08-08
---

# Phase 05 — Roads and the road network

> It records how the phase was planned and how it went, frozen as it was written. It is
> **not** a description of the tree today: for that see [ARCHITECTURE.md](../ARCHITECTURE.md)
> and [RULES.md](../RULES.md).

**Goal:** the road network is rebuilt only when it is `dirty`, and its labelling is independent of
the order the roads were built in.
**Depends on:** 04.
**Size:** M.
**Decisions involved:** D2 (distance along roads), D4 (determinism), tick order step 2.

## Why now

Service coverage (phase 06) is defined on the distance walked along the network. Without a network
structure, phase 06 would end up computing distances as the crow flies "for now", which is exactly
what D2 forbids, and all the balancing that follows would be built on the wrong range.

## What gets built

```rust
/// A derived structure: rebuildable at any moment from the Grid.
/// It does not enter the state hash — if it did, a bug in the incremental
/// rebuild would show up as a hash divergence instead of a failing
/// equivalence test (phase 06, test 6).
pub struct RoadNetwork {
    /// For each tile: the id of its connected component, or NONE if it is not a road.
    component: Vec<ComponentId>,
    /// How many rebuilds have been performed. For the tests only (see test 4).
    rebuilds: u32,
}
```

`rebuild_roads(world)` (step 2 of the tick): if `!dirty.roads` it returns immediately; otherwise it
redoes the full labelling with a BFS, scanning the tiles in increasing `TileIdx` order.

**A labelling with no history in it.** A component's id is the smallest `TileIdx` among its tiles,
not a running counter. It costs the same (the scan is already in increasing order) and it makes the
labelling a function of the set of roads alone, not of the order things were inserted in. Without
this, two games that build the same roads in a different order have different states and test 3 is
impossible to write.

A full rebuild rather than an incremental one, deliberately: scanning 40,000 tiles once is
irrelevant until a profiler says otherwise, and the `dirty` flag already avoids doing it every
tick. What is needed straight away is the *flag*, not the clever algorithm.

**Hooking buildings up to the network.** A building is connected if at least one tile of its area is
4-adjacent to a road tile. The rule belongs in a doc comment: it is game semantics (in Zeus what
counts is the entrance, not the building) and in M1 it might become "one designated entrance tile".

```rust
/// The distance in tiles walked along the network, from one building to another.
/// None if they are not connected or if they are beyond `max`. A truncated BFS:
/// without the cutoff, a range of 12 in a large city would visit the whole network.
pub fn road_distance(&self, from: BuildingId, to: BuildingId, max: u16) -> Option<u16>;
```

## Out of scope

Road levels (dirt/paved), blocks, one-way streets, pathfinding for walkers (M3: the logistics
walkers will use this same network but they want a path, not a distance). No variable crossing
cost: every road tile costs 1.

## Tests

1. **Components**: two separate groups of roads ⇒ two distinct components. Adding the tile that
   joins them ⇒ a single component. Removing it ⇒ two again.
2. **Adjacency is right**: two roads on a diagonal are **not** connected (4-adjacency only). A road
   at x=0 and one at x=width-1 on the same row are not connected (no wraparound — it is the phase
   01 bug coming back one level up).
3. **Independence from the order** (property test): given a set of road positions, two permutations
   of the corresponding `PlaceRoad` commands produce the **same** `component` labelling. This test
   is the reason the id is the smallest tile.
4. **The dirty flag works** (property test): after a `rebuild`, N ticks with no road commands leave
   `rebuilds` unchanged; a single `PlaceRoad` increments it by exactly 1, no more (a regression
   against the case where someone marks dirty inside the loop).
5. **`road_distance`**: on a straight 10-tile corridor the distance is the expected one; with
   `max = 5` it returns `None` beyond that; on an L the distance is the length of the path, **not**
   the Euclidean one (it is the test that documents D2); buildings that are not connected ⇒ `None`.
6. **Symmetry**: `road_distance(a, b) == road_distance(b, a)`. It holds in M0 (the graph is
   undirected) and it should be written down: the day one-way streets are introduced, this test
   will fail and force a conscious decision.

## Verification

```sh
cargo test -p sim-core roads
PROPTEST_CASES=2000 cargo test -p sim-core --release labelling
```

**Done when:** tests 3 and 4 pass — independence from the order, and a rebuild only when dirty. The
others are basic correctness; these two are the phase's goal.
