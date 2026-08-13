# Phase 19 — Terrain: relief and cost

> **Status: not yet built.**
>
> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

**Goal:** the same building costs more on a hillside than on the flat, is refused outright on a cliff,
and the map it stands on is loaded from a file rather than being one terrain repeated.
**Depends on:** 18.
**Size:** L.
**Decisions involved:** [A2](../DECISIONS.md), [A4](../DECISIONS.md), D4, D6.
**Decisions it will produce, and which are *not* taken yet:** the tile's layout and the range of a
ground height; and whether a map is an asset with a hash or something generated from the seed. This
document argues for an answer to each — that is what a plan is — but the entries go into
`DECISIONS.md` when the phase runs and its answers are the real ones. Their numbers are deliberately
not reserved here: a reserved number is a number that goes wrong the moment other work lands first.
Neither holds a `TO_BE_DECIDED` slot either, because nothing is planned on top of them — they are
closed by the phase that needs them, in the same commit. The doc comments sketched below therefore
carry **no `A<n>` tag**, and each needs one before it lands in a `.rs`: the rule is stated in full in
every one of them, which is the half that matters, but the tag is the half `doc-check` can see.

## Why now

Because the map is the last placeholder left. Everything else the player touches has become real over
M1 — houses level, people are born and leave, the treasury is spent — while the ground under all of it
is still `Grid::new(side, side, Terrain::Plain)`, one terrain repeated sixty-five thousand times.

And because it has to come **before M2**. M2 is the isometric renderer, and a 2.5D renderer built for
a flat map is a different renderer from one that draws relief: the sort order changes, the tile
geometry changes, the sprite anchoring changes. Building it flat and then reworking it is the
expensive order, and the rework would land on the one part of the tree with no tests worth the name.

It is also the first phase that changes `Tile`'s **layout** rather than its contents, and that is
cheapest now. There are two recordings. There will be more.

What it is deliberately **not** doing is deciding the numbers. The structure is fixed here because the
renderer depends on it; what a step of slope costs and where the refusal falls are set when the phase
runs, after ticks have been watched — the same rule that kept M1 from being planned before M0 closed.

## What gets built

### No new terrain kinds, and that is the point

The temptation is `Mountain`, `River`, `Sea`, `Hill`, `Cliff`. All five are refused.

`Terrain` already has `Plain`, `Water` and `Rock`, and `grid.rs` already documents `Water` as "rivers
and sea". Once a tile carries a height, the rest **fall out** rather than needing to be declared:

| What the player sees | What it is |
|---|---|
| a mountain | `Rock` at a high ground height |
| a hill | `Plain` at a high ground height |
| a cliff | a large step between two neighbours |
| a coast | `Plain` at ground height zero beside `Water` |
| a valley floor | a low, level region of `Plain` |

Declaring them as variants would produce five enum entries whose rows in `terrain.ron` are
byte-identical to rows that exist. It is the same speculative widening D6 forbids for
`CivilizationRules`, applied to an enum: **with one map there is no way to know what the right
distinctions are**, and a variant added now is a variant every hash is positional over
(`GLOSSARY.md`'s frozen-order row) for the rest of the project's life.

The dividend is concrete: `Terrain::ALL` does not move, so the frozen order stays true and
`doc-check`'s `frozen_orders_match_the_code` never has to be touched.

### The tile carries its ground height

```rust
/// Budget: 4 bytes, unchanged. `occupant_origin` takes half of it, so the
/// terrain, the flags and the ground height share the other `u16`.
///
/// Layout: terrain 0..4 · flags 4..8 · ground height 8..13 · 3 bits spare.
/// The fields are packed rather than given a byte each because a byte each
/// makes `Tile` six bytes with the alignment, and 40,000 tiles then take
/// 240 KB instead of 160 KB. The budget is the reason the assert exists.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Tile {
    packed: u16,
    /// Only valid if `flags().has_occupant()`.
    occupant_origin: TileIdx,
}

impl Tile {
    /// The kind of ground. Sixteen kinds fit in the field; three exist.
    pub const fn terrain(&self) -> Terrain;

    /// How high the ground is on this tile, counted in steps, `0..=31`.
    ///
    /// **The grid's own `height` is its size in tiles**, which is a different
    /// thing entirely — hence the longer name here.
    ///
    /// A step is a unit of gameplay, not of length. The renderer multiplies it
    /// by a scale of its own to get pixels, so the number of steps sets how
    /// finely relief can be described, not how tall a mountain may look.
    pub const fn ground_height(&self) -> u8;

    pub const fn flags(&self) -> TileFlags;
}
```

`pub terrain: Terrain` stops being a public field and becomes `terrain()`. That is the whole cost of
the refactor: about ten read sites, every one of which the compiler finds. `TileFlags` keeps its own
type and its own three bits — it is not dissolved into the packing, because the flag *names* are what
make `tick.rs` readable.

**Three bits are left spare on purpose.** A bridge flag, a fourth occupant kind, a second height for
water depth: each is one bit, and re-packing later moves every recorded hash. Leaving room in a layout
that is expensive to change is not speculation, it is the opposite of it.

### Slope is derived, never stored

```rust
impl Grid {
    /// The slope over a set of tiles: the highest ground height among them
    /// minus the lowest. Zero means level ground.
    ///
    /// It is **never stored on the tile**. A stored slope would be a second
    /// copy of a fact the heights already carry, and the two would drift apart
    /// the first time a height changed without it — the reason a save file is
    /// `seed + Vec<Command>` and not a dump of the state (D4), in miniature.
    pub fn slope_over(&self, tiles: &[TileIdx]) -> u8;
}
```

**What follows from this, and it is deliberate: a one-tile building never pays and can never be
refused.** One tile has one height; there is nothing to flatten. A hut can therefore go anywhere
buildable, however steep the ground around it, while a 2×2 farm cannot. That is the right way round —
huts scattering up a hillside and farms wanting the valley floor is the shape of every city this genre
has ever built — but it has to be **written down as a rule** rather than discovered as a surprise,
because the alternative (measuring a 1×1 against its neighbours) is the obvious-looking fix and it is
wrong: it would charge a building on level ground for standing near a cliff it never touches.

### The cost of flattening

`place_building` already walks every tile of the footprint in a loop that validates before it mutates,
and already holds both `tiles` and `origin` when that loop ends. The rule goes there, after the loop
and before the charge:

```
slope = grid.slope_over(&tiles)
if slope > max_build_slope   ⇒  Err(CommandError::TooSteep { at: origin, slope })
cost  = def.cost + flatten_cost_per_step × slope
```

Two new names in `rules.ron`, `max_build_slope` and `flatten_cost_per_step`, and one new variant:

```rust
/// The ground under the building is too uneven to flatten.
///
/// Carries the slope it found, not just the limit: a message the bot and later
/// the LLM can act on has to say how far off it was, or the only strategy
/// available is to try somewhere else at random.
TooSteep { at: TilePos, slope: u8 },
```

Integer arithmetic only — `clippy::float_arithmetic` is denied workspace-wide, and
`Coins::checked_mul_int` is already there for it. The multiply is checked, not raw: a table with an
absurd `flatten_cost_per_step` must produce a refusal, never a wrapped cost.

**Roads are deliberately left alone by slope.** `network.rs` is terrain-blind by design and says so at
the top of the file; a road already pays `road_cost` by terrain, which is 6 on `Rock` against 2 on
`Plain`, and that is enough to make crossing a mountain expensive. Coupling roads to relief in the
same phase would put a new rule in the hot path and give the regeneration two causes instead of one.

**The trap, and it is the reason test 4 defines the phase.** `sim-core/tests/invariants.rs` holds an
*independent reimplementation* of what a command costs, and `the_treasury_adds_up` asserts an exact
equality against it. A cost that depends on the site and is added in `tick.rs` alone leaves that
reimplementation reading the old rule, and the proptest goes red. It is the good kind of red: the
second implementation exists precisely so that a cost cannot be changed in one place only.

### The map is a file

```ron
// sim-data/maps/river-valley.ron
//
// Two parallel blocks rather than one interleaved one: a map nobody can read
// in a diff is a map nobody checks, and terrain and relief are read by eye
// separately anyway.
(
    id: "river-valley",
    width: 16,
    height: 8,
    // '.' plain   '~' water   '#' rock
    terrain: [
        "....~~~~........",
        // ...
    ],
    // ground height in base 32: '0'..'9' then 'a'..'v'
    ground: [
        "0000000011112223",
        // ...
    ],
)
```

It loads through the same `raw` → `validate` → `Def` pipeline the balancing tables use, for the same
reason: a map with a bad character has to produce a readable error naming the row and the column, not
an obscure deserialisation failure. `sim-data` gains `RawMap`, `validate_map` and `MapDef`, and
`Grid::from_map(&MapDef)` joins `Grid::new` rather than replacing it.

### The recording names a map and carries its hash

```rust
/// Where the map came from.
///
/// `Uniform` is what the format carried through version 2 and it stays: every
/// flat test world in the tree is one, and keeping it means this phase's
/// regeneration has exactly one cause.
pub enum MapSpec {
    Uniform { width: u16, height: u16, terrain: Terrain },
    /// The map's id, and the blake3 of the validated `MapDef` beside it — the
    /// same device as `dataset_hash` and for the same reason (A2): if the map
    /// is edited, the replay fails immediately and for the right reason
    /// instead of diverging ten ticks later through a side effect.
    File { id: String, hash: String },
}
```

`FORMAT_VERSION` goes 2 → 3, and `GLOSSARY.md`'s frozen row for it goes with it.

**The generator is not core code, and this is the decision worth stating out loud.** It arrives
later as `cargo xtask gen-map`, a tool that writes an asset which is then reviewed and committed. A
generator inside the core would become a frozen part of the determinism contract: every improvement to
it would move every recorded hash, and the rule the header of every `.hashes` file states — *if this
changes without the balancing having changed, a source of non-determinism has been introduced, stop
and find it* — would stop meaning anything at all. The core only ever loads files.

### What the map loader refuses

The project's own lesson, applied to a new kind of asset: *a relation that has to hold is a check, not
a comment.*

1. a row count that is not `height`, or any row whose length is not `width`
2. a character in either block that no terrain or digit claims
3. a ground height above the maximum the field can hold
4. **walkable tiles that do not form one connected region**

The fourth is the one that earns its place. Until bridges exist, a river drawn across a map severs it,
and half of it silently becomes unreachable. A rule saying "do not do that" in a comment is a rule
somebody breaks; a load error naming the two regions and their sizes is one nobody can. It is the same
shape as `Inconsistency::NoGap` — a fact about a data file that the data file is made to prove.

### The state hash consumes meanings, not bits

```rust
// Was: h.update(&[t.terrain as u8, t.flags.bits()]);
h.update(&[t.terrain() as u8, t.flags().bits(), t.ground_height()]);
```

The tile's **logical values**, never `packed` itself. Hashing the packed word would tie every recording
to a bit layout, and the three spare bits could then never be used without moving every hash in the
tree — which is exactly the freedom they were left for.

**The prediction this phase has to check against reality.** Both committed recordings sit on uniform
flat maps, so every `ground_height` is 0 and the new byte is a constant. Therefore:

- the hashes must diverge at the **first checkpoint, tick 30**, because a byte per tile was added; and
- the **dumps must be textually identical** to the ones taken before the change.

Those two facts together are the proof that the change is representational and not semantic. A dump
that differs anywhere is this phase's bug, and it is found before the commit rather than three phases
later. If instead the divergence starts *later* than tick 30, something is wrong with the reasoning
above and it has to be understood before regenerating anything.

## Out of scope

**Bridges.** A road crossing `Water` needs a flag bit, a rule in `place_road` and a change to
`bfs_roads`, which is step 2 of the tick. It is its own phase. Until it exists, the connectivity check
on map loading is what stands in for it, and that is a deliberate trade: the map author is constrained
so the player is not surprised.

**Roads affected by slope**, for the reason given above.

**Walking uphill costing more.** Coverage is walked distance along roads, and relief could plausibly
lengthen it. It is a real question and it is not this phase's: it would put terrain into step 3, the
hot path, in the same change that puts it into the tile. One thing at a time.

**Terraforming as a player command.** The ground is what the map says it is. Levelling a hillside is
paid for as part of a building, not bought separately.

**Fertile ground and ore deposits.** These are terrain's *economic* half — a farm yielding more on good
soil, a quarry needing marble under it — and they land with M3's production chains, when there is a
chain for a deposit to feed. Building them now would mean rebalancing food against a modifier while
M1's scenario is calibrated on flat ground.

**The map generator**, which is a tool and arrives after the format it writes.

**Any new `Terrain` variant.**

## Tests

1. **`Tile` is still four bytes.** The existing assert, unchanged and not relaxed. If this phase needs
   it relaxed, this phase has gone wrong.
2. **The packing round-trips**, for every terrain index up to 15 and every ground height up to 31,
   with the flags set and clear. Including the corners: terrain 15 with height 31 and an occupant at
   `TileIdx(u16::MAX)`.
3. **`slope_over`** is 0 over any single tile and over any level region, and `max − min` otherwise.
4. ***The test that defines the phase.*** A 2×2 building on level ground costs `def.cost`; the same
   building spanning a 3-step slope costs `def.cost + 3 × flatten_cost_per_step`; and
   `the_treasury_adds_up` stays an **exact equality** — which it can only do if `accepted_cost` in
   `invariants.rs` learned the same rule in the same change.
5. **A one-tile building never pays and is never refused**, however steep its surroundings. The rule
   from `slope_over`, pinned so nobody "fixes" it later by measuring against the neighbours.
6. **Too steep is refused cleanly**: a footprint above `max_build_slope` returns `TooSteep` carrying
   the slope it found, the treasury is untouched, and no tile is mutated. The no-partial-mutation
   discipline, on the new failure path.
7. **The map loader refuses each of its four faults**, one test each, and the message names where.
8. **A severed map fails to load**: a river drawn corner to corner, and the error names both regions.
9. **Property**: for every map that loads, every ground height is within range, every tile round-trips
   through the packing, and the walkable tiles are one region.
10. **The dumps do not move.** Both recordings replayed after the change produce dumps textually
    identical to the ones captured before it, with hashes differing from the first checkpoint. This is
    test 4's equal in value: it is the one that separates "the bytes changed" from "the game changed".
11. **`regen-expected` twice in a row**: the second says nothing to do.
12. **A real map plays.** A city built on `river-valley` reaches the same population band as one built
    on the flat, or the difference is explained. If relief makes the existing scenario unwinnable, that
    is balancing to be found here and not in M2.

## Verification

The protocol needs the *before*, so this is two stages and the order matters.

```sh
# Before touching anything. If this is not already green, the tree is dirty for
# other reasons and the signal this phase depends on is already lost.
cargo xtask regen-expected --check
cargo xtask run --ticks 900 --dump-every 30 > before.txt
```

```sh
# After.
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo xtask run --ticks 900 --dump-every 30 > after.txt   # diff before.txt after.txt must be empty
cargo xtask regen-expected
cargo xtask doc-check
```

`diff before.txt after.txt` being empty is the by-eye proof that closes the first half of the phase.
The hashes in the `.hashes` files change; the state they summarise does not. If the diff is not empty,
**stop** — the packing has changed a rule somewhere, and finding out which is worth more than
finishing on time.

The second half is the map. Run the same scenario on `river-valley` and watch two columns together:
the population and the fraction of residents covered. Relief takes buildable land away, so the same
commands will not build the same city — the point is that the city that *does* get built is a city,
not a failure. A run that cannot place its farms is a map to redraw, not a rule to loosen.

`bench` has to be re-run and compared. Nothing in this phase touches the per-tick path, so the three
numbers should not move; if they do, the packing has cost something in step 3's inner loop and the
accessors need looking at. The state hash `bench` prints **will** move, for the same one reason as the
recordings, and that is the only reason it is allowed to move.

**Done when:** test 4 passes with `the_treasury_adds_up` still an exact equality, test 8 refuses a
severed map, and test 10's dumps are identical — the last of which is what proves the hashes moved for
the packing and not for the game.
