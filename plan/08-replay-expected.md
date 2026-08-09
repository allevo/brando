# Phase 08 — Replay, the state hash and xtask

**Goal:** a `seed + Vec<Command>` replayed produces exactly the same state hashes;
`cargo xtask regen-expected` produces no diff when nothing has changed.
**Depends on:** 07 (or 05, if you want to bring it forward — see README).
**Size:** L.
**Decisions involved:** D4 (a save file is seed+log), Testing point 2 (the project's most valuable
test), A2, A3.

## Why now

It is the test that protects all the others. From here on, every source of non-determinism
introduced by accident shows up as a hash that changes, within one commit of being introduced,
instead of as an inexplicable balancing bug six months later.

## What gets built

### `sim-replay`

```rust
pub struct Recording {
    pub header: Header,
    /// Sorted by increasing tick. Several commands within the same tick keep
    /// the order they were added in: the order is part of the determinism contract.
    pub commands: Vec<(u32, Command)>,
}

pub struct Header {
    pub format_version: u16,
    pub seed: u64,
    pub grid: GridSpec,
    /// blake3 of the DataSet (phase 03). If the balancing changes, the replay
    /// fails immediately and for the right reason (A2).
    pub dataset_hash: [u8; 32],
}

pub fn replay(rec: &Recording, data: Arc<DataSet>, until: u32) -> Result<World, ReplayError>;
```

`ReplayError::DatasetMismatch { expected, found }` is a distinct error and its message says what to
do: regenerate the recordings if the change was intended.

### The state hash

```rust
/// A fixed serialisation for the hash. Written by hand, not delegated to serde (A3):
/// the field order here is a contract, and moving a field inside a struct must not
/// invalidate the recordings.
pub fn hash_world(w: &World) -> [u8; 32];
```

What goes in, in a fixed order: `tick`, `dataset_hash`, the grid's dimensions, the tiles in
`TileIdx` order, the buildings in `BuildingId` order (with kind, origin, level, stock), the houses
in `HouseId` order, the economy, **the position of every RNG stream** (phase 02).

What does **not** go in: `RoadNetwork`, `Coverage`, `DirtyFlags`, `FoodTotals`. They are derived or
diagnostic structures: if they went into the hash, a bug in an incremental rebuild would show up as
a hash divergence — while the test meant to catch it is phase 06's incremental/from-scratch
equivalence, which says *where* the problem is.

**The known risk of writing the hash by hand** is that a new field of the state gets forgotten,
making the hash blind to part of the state. Mitigation: test 6.

### The recordings

```
sim-replay/tests/expected/
  minimal.ron          seed + commands (readable: a 32×32 grid, A4)
  minimal.hashes       "tick,hex_hash" every 30 ticks
  hunger.ron           a scenario that runs the food out (phase 07, test 3)
  hunger.hashes
```

The `.ron` files are written by hand or recorded by `xtask`, and they have to stay **readable**: a
recording nobody can read does not help anyone work out why it changed.

### `xtask`

```sh
cargo xtask run --scenario minimal --ticks 360 [--dump-every N]
cargo xtask record --out sim-replay/tests/expected/new.ron   # records a game
cargo xtask regen-expected                                   # regenerates every .hashes
cargo xtask regen-expected --check                           # fails if there would be a diff (CI)
```

## Out of scope

Saving the state (D4: there is no such thing, the save file *is* seed+log). Compression, a binary
format, versioning the format beyond the `format_version` field. Scenarios with objectives (M1).

## Tests

1. **Determinism within one process**: the same `Recording` replayed twice in the same process
   gives identical hashes at every checkpoint.
2. **Determinism across processes**: the hashes match the ones committed in `.hashes`. It is the
   recorded-replay test proper — it catches what test 1 cannot, i.e. dependence on memory
   addresses, on `RandomState`, on the iteration order of a hash collection.
3. **Determinism of partial execution**: replaying to tick 150 and then carrying on to 360 gives
   the same final hash as replaying 360 ticks in one go. It catches the "hidden" state rebuilt
   wrongly on restart.
4. **Dataset mismatch**: with one number in the `DataSet` altered, the replay fails with
   `DatasetMismatch`, **not** with a different hash at tick 200.
5. **`regen-expected` is idempotent**: running it on a clean tree leaves `git diff` empty. This test
   goes into CI as `regen-expected --check`: it is the difference between recordings that mean
   something and recordings that get regenerated out of habit every time they go red.
6. **The hash covers the whole state** — the mitigation of A3's risk. A test that, for every mutable
   field of the `World`, perturbs it and checks that the hash changes:
   ```rust
   /// If this test fails, hash_world has stopped covering a field of the state:
   /// from that moment the recordings are blind on that field. Do not silence it: add
   /// the field to hash_world.
   ```
   Pragmatic coverage (the tick, a tile, a building, a house, the treasury, the position of every
   RNG), not automatic reflection.
7. **Sensitivity to the seed**: a different seed with the same commands ⇒ a different hash *as soon
   as* an RNG domain is used. In M0 no system uses the RNG, so this test **is expected to fail** and
   should be written as `#[ignore]` with the reason, to be re-enabled in M1 with the random events.
   Write it now because now is when you can see why it is needed.

## Verification

```sh
cargo test -p sim-replay
cargo xtask regen-expected --check     # has to pass without modifying anything
```

Then the manual check that closes the phase: change `output_per_tick` from `400` to `401` in
`buildings.ron`. The replay has to fail with `DatasetMismatch` (test 4). Restore the value and check
that everything goes green again.

**Done when:** tests 2, 5 and 6 pass and the deliberate perturbation of the dataset has been seen to
fail with the right error. From here on every phase adds lines to the recordings.
