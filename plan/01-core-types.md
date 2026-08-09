# Phase 01 — Core types and the grid

**Goal:** `Milli`, `Coins`, the newtype ids and the `Grid` exist; `Tile` fits in its memory
budget and the position↔index conversion round-trips over the whole map.
**Depends on:** 00.
**Size:** M.
**Decisions involved:** D4 (no floats), D5 (scale), A1, A4.

## Why now

These types go into the signature of everything else. Changing `Milli` to i64 or `TileIdx` to u32
once five systems sit on top of them is a refactor that touches every file. Changing it now
touches only this phase's tests.

## What gets built

### `Milli(i32)` — fractional quantities

```rust
/// A quantity in thousandths of a unit. No floats in the state (D4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Milli(i32);
```

A minimal API, only what phases 04–07 need:

- `from_units(i32) -> Option<Milli>` (checked: 3,000,000 units do not fit)
- `const fn from_millis(i32) -> Milli`, `fn to_millis(self) -> i32`
- `checked_add`, `checked_sub`, `checked_mul_int(i32)`
- `saturating_add`, `saturating_sub` — allowed only where saturating is the intended game rule,
  with a comment saying so
- `div_int(self, d: i32) -> Option<Milli>`, **truncating towards zero, documented**
- `Display` printing `12.500` (for the snapshots and the ASCII map)

No bare `impl Add`/`Sub`: the operator invites you to ignore overflow, and in a core that must not
panic (conventions: no `unwrap` outside impossible invariants) a silent overflow is worse than the
visual noise of `checked_add`.

`Coins(i32)` for money, the same shape without thousandths (see [A1](open-decisions.md)).

### Newtype ids

```rust
slotmap::new_key_type! { pub struct BuildingId; pub struct HouseId; }

/// Linear tile index: y * width + x. Map at most 256×256 (A4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileIdx(u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TilePos { pub x: u8, pub y: u8 }
```

A bare `usize` appears in no public signature (conventions).

A note on `slotmap`: iteration goes by slot index, so it is deterministic given the same sequence
of insertions and removals — which the command log guarantees (D4). It should be written as a doc
comment on the `buildings` field, because it is a non-obvious invariant the hash depends on.

### `Grid` and `Tile`

```rust
pub struct Grid {
    width: u8,
    height: u8,
    tiles: Vec<Tile>,          // len = width * height
}

/// Budget: 4 bytes. 40,000 tiles ⇒ 160 KB, fits in L2 (D: Tile has to stay small).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Tile {
    terrain: Terrain,          // enum #[repr(u8)]
    flags: TileFlags,          // u8 bitflags: HAS_ROAD, ...
    occupant: OccupantSlot,    // u16: a compact index, not Option<Box<_>>
}
```

`Grid` exposes: `new(width, height, Terrain) -> Result<Grid, GridError>` (rejects 0 and >256),
`idx(TilePos) -> TileIdx`, `pos(TileIdx) -> TilePos`, `get`/`get_mut`, `in_bounds`,
`neighbors4(TileIdx) -> impl Iterator<Item = TileIdx>` **without wraparound** — the classic bug is
that the neighbour "to the right" of x=width-1 ends up on the row below.

`OccupantSlot` deserves attention: it has to say, given a tile, which building or house occupies
it, without an `Option<Box<...>>`. Recommended shape: a `u16` with `u16::MAX` as the empty
sentinel, plus a bit in `flags` saying whether the occupant is a house or a building. The map from
slot to `BuildingId` lives in the `World`, not in the tile.

## Out of scope

No `World`, no `Building`, no `House` (phase 04). No pathfinding (05). No terrains with game
properties: `Terrain` in this phase is just an enum, the numbers that go with it (buildable? what
cost?) arrive from the tables in phase 03.

## Tests

Property tests (`proptest`), because here the invariants are arithmetic and the properties really
do hold:

1. **Position↔index round-trips**: for every `width`, `height` in 1..=256 and every valid position,
   `pos(idx(p)) == p`. And for every valid idx, `idx(pos(i)) == i`.
2. **`neighbors4` does not leave the map and does not wrap**: every neighbour returned is in
   bounds and at Manhattan distance 1 from the centre. Corner tiles have exactly 2 neighbours,
   edge ones 3, interior ones 4.
3. **`Milli` never panics**: for every pair of `i32`s, the `checked_*` operations return `None` or
   a correct result, never a panic. Compared against `i64` as the oracle.
4. **`div_int` truncates towards zero** for negative operands too: `-1500 / 2 == -750`,
   `-1501 / 2 == -750` (not `-751`). A table-driven test, not a property one.
5. **The memory budget**, as a static test:
   ```rust
   #[test]
   fn tile_stays_within_budget() {
       assert_eq!(size_of::<Tile>(), 4, "Tile has grown: 40,000 tiles have to fit in cache");
   }
   ```
6. `Grid::new` rejects `0` and `257` with the right error.

## Verification

```sh
cargo test -p sim-core
cargo clippy --workspace --all-targets -- -D warnings
```

**Done when:** the six groups of tests are green and `size_of::<Tile>() == 4`. If `Tile` has left
its budget, the phase is not closed: you go back to `OccupantSlot` before building on top of it.
