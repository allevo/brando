# Phase 07 — The farm, food, and the first observable loop

**Goal:** food is conserved (produced = consumed + stock, always) and houses go uncovered when the
stock is empty.
**Depends on:** 06.
**Size:** M.
**Decisions involved:** D2, D4 (`Milli`), A5, tick order steps 4 and 10.

## Why now

Up to phase 06 the world only changes when a command arrives. With this phase the world **evolves
on its own**: there is a quantity going up, one going down, and a coupling between the two. It is
the first state interesting enough to put under a recorded replay (phase 08) — a replay over an
inert state checks very little.

## What gets built

Step 4 of the tick, `production(world)`:

1. Each farm adds `output_per_tick` to its own `stock`, saturating at `max_stock`. The saturation
   is game semantics (the granary is full and the rest is lost) and has to be commented as such,
   otherwise it looks like a shortcut against overflow.
2. Each house served with food consumes `food_per_resident * residents` from its provider's
   `stock`. The houses are served in `HouseId` order — a deterministic order, to be documented like
   the rule in phase 06.
3. If the stock does not cover a house's consumption, that house **consumes nothing** (no partial
   consumption) and is marked as unserved for food this tick.

The choice of "no partial consumption" is deliberate: it makes conservation checkable with an exact
equality (test 1) instead of an inequality, and in M1 it gives a clean binary signal for "the house
ate this month".

`food_per_resident` and `output_per_tick` live in `sim-data` (phase 03).

### Bookkeeping for the conservation test

```rust
/// Running totals, only to check conservation (test 1) and for M2's evaluator.
/// They influence no game decision.
pub struct FoodTotals { produced: Milli, consumed: Milli, lost_to_full_stock: Milli }
```

Invariant: `produced == consumed + lost + Σ of the farms' stock`. Without the `lost` term the
saturation in point 1 would break the equality — which is why the field exists.

### Events (step 10)

A minimal version, because phase 08 and M2 need it:

```rust
pub enum Event {
    BuildingPlaced { id: BuildingId, kind: BuildingKindId, origin: TilePos },
    BuildingRemoved { id: BuildingId },
    ServiceCoverageChanged { house: HouseId, service: ServiceKind, served: bool },
}
```

`ServiceCoverageChanged` is emitted only on **changes of state**, not every tick: it is a delta,
not polling. It is the boundary with the renderer (M2) and the wrong choice here costs 40,000
events per tick.

## Out of scope

Warehouses, logistics walkers, multi-stage chains (M3, D3). Houses levelling up or decaying from
hunger: the house records that it did not eat, it suffers no consequences (M1). No differentiated
kinds of food.

## Tests

1. **Conservation** (property test, the phase's goal): after any sequence of commands and any
   number of ticks, `produced == consumed + lost + Σ stock`. An exact equality.
2. **No negative stock** (property test): for every farm, in every tick, `stock >= 0` and
   `stock <= max_stock`.
3. **Hunger**: a farm whose `output_per_tick` is below the consumption of the houses it covers ⇒
   after a number of ticks you can work out by hand the stock runs out and the houses come out
   unserved. The number of ticks has to be computed in the test from the `DataSet`'s values,
   **not** hardcoded: otherwise the test breaks on every rebalancing without signalling anything
   real.
4. **Recovery**: with a second farm added, the houses become served again. Checks that the "hungry"
   state is not absorbing.

   > **Updated after M0.** In phase 07 this point failed: the hungry state *was* absorbing, and the
   > test was written to pin down the limitation instead of the property. Now the property really
   > does hold — *a house covered by food always eats* — because a producer's capacity cannot
   > exceed what its output sustains. See [A5](open-decisions.md).
5. **Saturation**: a farm with no houses covered ⇒ `stock` grows to `max_stock` and stops;
   `lost_to_full_stock` grows accordingly.
6. **Deterministic order**: enough stock for two houses out of three ⇒ it is always the same two
   that eat (the smaller `HouseId`s), over 100 repeated runs.
7. **Events**: `ServiceCoverageChanged` emitted exactly once on the served→unserved transition, not
   on every tick of hunger.

## Verification

```sh
cargo test -p sim-core food
PROPTEST_CASES=2000 cargo test -p sim-core --release conserved
```

A manual check worth doing to close the phase — a small textual dump in `xtask`:

```sh
cargo xtask run --scenario minimal --ticks 120 --dump-every 30
```

It has to show the stock and the served houses moving in a readable way. If the numbers look
absurd, that is balancing (`sim-data`), not code — but it has to be looked at now, because phase 08
freezes these numbers into a recording.

**Done when:** property tests 1 and 2 pass with 2000 cases and the dump at 120 ticks can be read.
