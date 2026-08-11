# Phase 11 — Game difficulty

**Goal:** the same seed and the same commands at two different difficulties ⇒ different hashes; the
difficulty travels in the replay's header, and a file recorded at one difficulty does not replay at
another.
**Depends on:** 09 (M0 closed).
**Size:** S — it is the first phase of M1.
**Decisions involved:** [A13](open-decisions.md), A2, A3, D6.

## Why now

Because it touches the three things that regenerate the recordings — `World::new`, the replay
`Header`, `hash_world` — and touches nothing else. Doing it later means regenerating them twice, and
every extra regeneration is one less chance to read a diff and understand it.

It is the same argument as the `DirtyFlags` in phase 04: you put the structure in when it is cheap,
not when you need it. Here the table is born with **one knob only** — the residents of a
newly-built house — and phases 13, 14 and 16 will hang theirs off it without touching the header or
`World::new`'s signature again.

There is also a gameplay reason why that is the knob. With the demographics (phase 14) the
population arrives by migrating; if a house is born already full, the player never sees the hard
part of the game. But always starting from zero makes the first years painfully slow. Difficulty is
the dial that chooses between the two, and it has to be decided at the start of a game because it
changes the simulation.

## What gets built

### An accessor needed now and needed more later

```rust
// sim-core/src/data.rs
impl Rules {
    /// The most residents a house can hold at the given level (level 1 = index 0).
    ///
    /// `None` out of range, like `ServiceDef::range` and `ServiceDef::capacity`:
    /// a level that does not exist in the table is a data error, not a panic.
    ///
    /// It is a function and not a direct access to the `Vec` because phase 13
    /// restructures `residents_per_house_level` into a per-level table: there the
    /// **body** changes, not the callers.
    pub fn max_residents(&self, level: u8) -> Option<u16> {
        self.residents_per_house_level
            .get(usize::from(level).checked_sub(1)?)
            .copied()
    }
}
```

It is already needed here, by the cross-table validation below. The field stays
`residents_per_house_level`: renaming it would move the dataset hash, and phase 13 restructures it
anyway.

### The table, `sim-data/data/difficulty.ron`

The fourth file read by `load_from_dir`.

```ron
// Difficulty profiles. Born with a single field: phases 13, 14 and 16
// add their knobs here without touching anything else.
(
    profiles: [
        // The residents of a newly-built house. At "hard" it is zero: the
        // house only fills up by migration, and the city has to be earned.
        (id: "easy",   starting_residents_per_house: 4),
        (id: "normal", starting_residents_per_house: 2),
        (id: "hard",   starting_residents_per_house: 0),
    ],
)
```

`easy` has `starting_residents_per_house == max_residents(1)`, i.e. today's behaviour. That is what
makes this phase's `.hashes` diff attributable to **the header alone**: by recording the existing
scenarios at `easy`, the only thing changed in the state is the difficulty byte.

### The definitions, in `sim-core/src/data.rs`

```rust
/// An index into the profiles table.
///
/// Not an enum: difficulty is data (D6), and a civilisation or a scenario
/// will be able to declare its own without touching the code.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct DifficultyId(u8);

pub struct DifficultyDef {
    pub id: String,
    /// The residents of a newly-built house. Zero is legitimate: the house
    /// fills up by migration (phase 15).
    pub starting_residents_per_house: u16,
}

impl DataSet {
    pub fn difficulty(&self, d: DifficultyId) -> Option<&DifficultyDef>;
    pub fn difficulty_by_id(&self, s: &str) -> Option<DifficultyId>;
}
```

No `Default` for `DifficultyId`. It looks like an inconvenience and it is not: `World::new` has four
callers, and a default is exactly the mechanism by which one of the four would be left behind with
nothing to flag it.

### The state, in `sim-core/src/world.rs`

```rust
pub struct World {
    // ...
    /// Chosen at the start of a game, never changeable afterwards: it changes the
    /// simulation, so it goes into the hash and travels in the replay's header.
    pub(crate) difficulty: DifficultyId,
}

impl World {
    pub fn new(grid: Grid, data: Arc<DataSet>, seed: u64, difficulty: DifficultyId) -> Self;
    pub const fn difficulty(&self) -> DifficultyId;
}
```

The four callers to update: `sim-replay/src/replay.rs::initial_world`,
`xtask/src/scenario.rs::Scenario::world`, `xtask/src/bench.rs`,
`sim-core/tests/common/mod.rs::world_of`.

`place_building` reads `starting_residents_per_house` from the profile instead of from
`rules.residents_per_house_level.first()`.

### The replay, in `sim-replay/src/recording.rs`

```rust
pub const FORMAT_VERSION: u16 = 2;

pub struct Header {
    pub format_version: u16,
    pub seed: u64,
    /// The profile's **textual id**, not its index.
    ///
    /// A recording saying `difficulty: 1` cannot be read, and reordering the
    /// table would silently change the meaning of every save file already
    /// written. The cost is one lookup on opening; the return is that the file
    /// stays what a recording has to be, which is readable.
    pub difficulty: String,
    pub grid: GridSpec,
    pub dataset_hash: String,
}
```

```rust
/// The header's profile does not exist in the table.
///
/// A distinct error from `DatasetMismatch` because the cause is different: there
/// the balancing has changed, here a profile has been removed or renamed.
#[error("unknown difficulty profile: {found:?} (known: {known})")]
UnknownDifficulty { found: String, known: String },
```

### The two hashes

`sim-replay/src/hash.rs`, right after `w.data().hash`:

```rust
h.update(&[w.difficulty().get()]);
```

`sim-core/src/data_hash.rs`, a block for the profiles at the end, with the length before the
content as with everything else.

### The compile-time canary — the most important thing in the phase

M1 adds seven fields to the state across six phases, and [A3](open-decisions.md) says each one has
to be added to `hash_world` by hand. The safety net is `the_hash_covers_the_whole_state`, but that
test **does not fail on its own** when a new field arrives: it fails only if someone also writes the
matching perturbation. That is, the net is only there if someone remembers to string it.

```rust
// sim-core/src/world.rs, behind `test-util`
/// A **compile-time** canary for the state hash (A3).
///
/// It does nothing at runtime. It exists because the exhaustive `let World { .. }`
/// stops compiling the moment a field is added to the state: the reminder arrives
/// while you are writing the field, not when a test fails — and it arrives even if
/// nobody has added the matching perturbation to `the_hash_covers_the_whole_state`,
/// which today is the only way that test notices anything.
///
/// If you are reading this because it does not compile: add the field here, then
/// decide whether it belongs in `hash_world` (state) or not (derived/diagnostic),
/// and either way write which of the two in the field's doc comment.
#[cfg(feature = "test-util")]
pub fn field_canary(&self) {
    let World {
        tick: _, grid: _, buildings: _, houses: _, walkers: _, economy: _,
        rng: _, dirty: _, roads: _, coverage: _, food: _,
        buildings_by_origin: _, houses_by_origin: _, data: _, difficulty: _,
    } = self;
}
```

The same trick, for free, wherever the fields are already `pub`: `hash_world` can destructure
`Building`, `House` and `Economy` in its loops instead of accessing the fields one by one, and
`dataset_hash` can destructure `Rules`, `BuildingDef` and `TerrainDef`. One line per struct, and
A3's known risk stops depending on the memory of whoever is writing.

### Validation

One check of shape — `profiles` non-empty, ids unique and non-empty — and one **cross-table** check,
which touches two tables and therefore goes into `DataSet` like `unsustainable_food_capacity`:

```
starting_residents_per_house <= rules.max_residents(1)
```

A house born beyond its own capacity is a state the rest of the game cannot represent: phase 13
assumes `residents <= max_residents(level)` everywhere.

### `xtask`

`run --difficulty <id>`, `Scenario.difficulty`, and the difficulty in the dump's header next to the
seed and the dataset hash.

For `regen-expected`: the existing recordings are recorded at `easy`. The full matrix (every
scenario × every profile) triples the number of files and adds no coverage — the determinism is the
same. A single recording at a different difficulty arrives when it is needed, i.e. when a profile
has knobs that really change the simulation (phase 14).

**The benchmark has to choose the profile explicitly**, not take the first one or a default: if
rebalancing the default moved the numbers recorded in [09](09-invariants-closeout.md), the tripwire
would stop being comparable. The profile used has to be printed in the header, next to the dataset
hash and for the same reason — a number that moves has to be attributable.

## Out of scope

Knobs other than `starting_residents_per_house`: they arrive with the phases that use them.
Difficulty changeable mid-game (it would be a `Command`, and changing the rules halfway through a
game is a balancing problem no scenario asks for). Per-scenario difficulty: that is phase 17, and it
will be the scenario proposing a profile, not defining a new one.

## Tests

1. **The goal**: the same seed, the same commands, `easy` and `hard`, in three parts.

   As written when this phase was planned, the test asked for different hashes on the first tick
   where a house gets built and **the same before that** — and that cannot hold, because the
   difficulty byte enters `hash_world` at tick 0 (see "The two hashes" above) and test 6 requires it
   to. The two halves of the goal are still the right two questions; asking them takes one more
   piece, `World::set_difficulty` behind `test-util`, which test 6's perturbation needs anyway:

   - un-normalised, the hashes differ **from tick 0**: the profile travels in the state hash, which
     is what makes a recording attributable to the game it was played on;
   - with both worlds forced onto the same profile, the hashes are **equal** before the first house
     is built;
   - and **different** after.

   The second and third count as much as the first: net of the byte itself they say the difficulty
   acts where it should and nowhere else. Without the normalisation the question cannot even be put,
   because the byte alone would answer it.
2. **`easy` reproduces M0**: with `starting_residents_per_house == max_residents(1)`, the population
   after a game is the same as before the phase. It pins down that the knob is the only effect.
3. **The header round-trips**: a `Recording` saved and read back keeps the textual id; a header with
   a profile that does not exist gives `UnknownDifficulty`, not a panic and not a silent default.
4. **The format version**: a header with `format_version: 1` is rejected with `UnsupportedFormat`
   (the test already exists, the number has to be updated).
5. **Cross-table validation**: a broken fixture with `starting_residents_per_house` beyond the
   capacity ⇒ one error with the right path. Like the other seven in
   `sim-data/tests/fixtures/broken/`.
6. **The hash covers the difficulty**: one perturbation in `the_hash_covers_the_whole_state`.
7. **The canary compiles**, and that is all it has to do. The manual check that closes the phase is
   what counts.

## Verification

```sh
cargo test --workspace
cargo xtask regen-expected          # a **deliberate** regeneration: header v2 + difficulty
cargo xtask regen-expected --check  # and idempotent afterwards
cargo xtask run --difficulty hard --ticks 360
```

The `.hashes` diff has to be **read**, not just committed: it has to diverge from the first
checkpoint, because both the things this phase puts into the hash — the difficulty byte and the
dataset hash, which moves with the fourth table — act from tick 0. If it diverged later, the
difficulty is not in the hash where you think it is.

That says *when*, though, not *what*, and with two causes acting at tick 0 it cannot on its own rule
out a third. What rules it out is comparing the observable state with M0's: `cargo xtask run` over a
game year on both scenarios, from a worktree at the previous commit and from this one, has to print
the **same table**. If it does, nothing in the simulation moved and the hashes moved for exactly the
two reasons intended.

Then the manual check that closes the phase: **add any field at all to `World` and check that the
project does not compile** until you add it to the canary. Remove it and commit. It is the same
ritual as phase 02 with the variant at the top of `RngDomain`, and for the same reason: the safety
net has to be seen to trip once, or you do not know it is there.

**Done when:** test 1 passes in all three of its parts, and the canary has been seen to break the
build.
