# Phase 04 — World, tick, commands

**Goal:** `step()` advances the tick, applies the valid commands, rejects the invalid ones with a
structured error, and does not panic on 10,000 random commands.
**Depends on:** 01, 02, 03.
**Size:** M.
**Decisions involved:** D1, D4, D5, A2. Testing point 3 (fuzzing).

## Why now

This is the phase in which the project becomes runnable. After it there is a `World` you can
advance and inspect, and every later phase adds **one step** to the tick order that is already
written down.

## What gets built

### State

```rust
pub struct World {
    tick: u32,
    grid: Grid,
    buildings: SlotMap<BuildingId, Building>,
    houses: SlotMap<HouseId, House>,
    walkers: Vec<Walker>,          // empty in M0: the real walkers are M3 (D3)
    economy: Economy,              // { treasury: Coins }
    rng: RngSet,
    dirty: DirtyFlags,
    data: Arc<DataSet>,            // A2
}

pub struct Building { kind: BuildingKindId, origin: TilePos, level: u8, stock: Milli }
pub struct House    { origin: TilePos, level: u8, residents: u16, served: ServiceFlags }
```

`House` already exists here, even though it does not level up in M0: it is the unit of population
simulation (D5) and phases 06–07 need something to serve. In M0 the residents are a fixed value
from the `rules`; migration is M1.

### DirtyFlags — from the very start

```rust
/// The flags exist from the start as a deliberate choice: retrofitting them later is painful
/// (CLAUDE.md, tick order). In M0 the use is naive, the structure is the final one.
pub struct DirtyFlags {
    roads: bool,
    /// Providers whose coverage has to be recomputed. An ordered Vec, not a HashSet (D4).
    coverage: Vec<BuildingId>,
}
```

### Commands

```rust
pub enum Command {
    PlaceRoad     { at: TilePos },
    PlaceBuilding { kind: BuildingKindId, origin: TilePos },
    Demolish      { at: TilePos },
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("position outside the map: {0:?}")]               OutOfBounds(TilePos),
    #[error("tile already taken by {occupant:?}")]            TileOccupied { .. },
    #[error("cannot build on this terrain: {0:?}")]           UnsuitableTerrain(Terrain),
    #[error("not enough funds: {needed} needed, {available} available")]
                                                              InsufficientFunds { needed: Coins, available: Coins },
    #[error("unknown kind of building: {0:?}")]               UnknownBuildingKind(BuildingKindId),
    #[error("nothing to demolish at {0:?}")]                  NothingToDemolish(TilePos),
}
```

The messages are not cosmetic: they are the feedback that will go back to the LLM (M3). It is
worth writing them well now, they are free.

### The tick

```rust
pub fn step(world: &mut World, cmds: &[Command]) -> StepReport {
    let mut r = StepReport::default();
    apply_commands(world, cmds, &mut r);   // 1
    rebuild_roads(world);                  // 2  -> phase 05
    propagate_coverage(world);             // 3  -> phase 06 (hot path)
    production(world);                     // 4  -> phase 07
    step_walkers(world);                   // 5  -> M3
    houses_and_migration(world);           // 6  -> M1
    finance(world);                        // 7  -> M1
    random_events(world);                  // 8  -> M1
    check_objectives(world, &mut r);       // 9  -> M1
    emit_events(world, &mut r);            // 10 -> phase 07 (minimal)
    world.tick += 1;
    r
}
```

**All ten functions exist from the start**, empty ones included, each with a comment saying which
phase fills it in. The order is game semantics: if the functions come into being one at a time,
the order becomes an accident of the development timeline.

`StepReport { rejected: Vec<(usize, CommandError)>, events: Vec<Event> }`. An invalid command does
**not** interrupt the tick and is not an `Err` of the tick: it is discarded and recorded with the
command's index. The LLM will produce invalid commands (Testing point 3): rejecting them is normal
behaviour, not a failure of the system.

## Out of scope

The road network (05), coverage (06), production (07), levelling/taxes/events (M1). In this phase
`PlaceRoad` only sets the flag in the tile and sets `dirty.roads = true`.

## Tests

Unit tests, table-driven:

1. `step(&mut w, &[])` increments the tick by 1 and changes nothing else (compared on the hash once
   phase 08 exists; here a field-by-field comparison is enough).
2. `PlaceBuilding` of a 2×2 farm takes up **four** tiles and takes the cost out of the treasury.
3. An overlap is rejected: a second farm touching a single tile of the first ⇒ `TileOccupied`, and
   **the state has not changed at all** (no partial occupation: validating the whole area comes
   before any mutation).
4. An area sticking out past the edge ⇒ `OutOfBounds`, with no mutations.
5. Not enough treasury ⇒ `InsufficientFunds` with the right numbers in the message.
6. `Demolish` frees every tile of the area and removes the id from the slotmap.
7. A `kind` that does not exist ⇒ `UnknownBuildingKind` (it will come from the LLM).

Property tests — these are the ones that close the phase:

8. **No panic**: sequences of 10,000 commands generated randomly by `proptest`, including
   coordinates out of range and `kind`s that do not exist, over 1,000 ticks. The core never panics
   and `rejected` grows consistently.
9. **No overlap**, a global invariant: after any sequence of commands, every tile has at most one
   occupant, and for every building the tiles it covers point back at it. (Both directions: it is
   the invariant that catches demolition bugs.)
10. **The treasury adds up**: `treasury == starting_treasury − Σ accepted costs` (in M0 there is no
    income). A conservation test, not a behaviour one.

## Verification

```sh
cargo test -p sim-core
PROPTEST_CASES=2000 cargo test -p sim-core --release invariants
```

**Done when:** property tests 8–10 pass with 2000 cases, and the ten tick functions exist in
`CLAUDE.md`'s order.
