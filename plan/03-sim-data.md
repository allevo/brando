---
id: 03
kind: phase
status: implemented
opened: 2026-08-08
closed: 2026-08-08
---

# Phase 03 — sim-data: validated RON tables

> It records how the phase was planned and how it went, frozen as it was written. It is
> **not** a description of the tree today: for that see [ARCHITECTURE.md](../ARCHITECTURE.md)
> and [RULES.md](../RULES.md).

**Goal:** a valid table loads; a broken one produces a report with **every** error, not just the
first; the dataset has a stable hash.
**Depends on:** 01 (it uses `Milli`, `Coins`). Can run in parallel with 02.
**Size:** M.
**Decisions involved:** D6 (civilisation = data + rules), conventions (no balancing number in the
code), A2, A6.

## Why now

The rule "if a numeric game constant shows up in a `.rs`, it is a bug" cannot be kept if there is
nowhere to put the numbers. If `sim-data` arrives after `World`, the numbers end up in the code
"temporarily" and stay there.

## What gets built

### The data files (`sim-data/data/`)

```
rules.ron       global constants: ticks_per_month, starting_treasury, residents_per_house_level
terrain.ron     for each Terrain: buildable, road_cost, walkable
buildings.ron   the three buildings of M0
```

> **Amended by the documentation audit (2026-08-13).** There are **four** tables now: phase 11 added
> `difficulty.ron`, the profiles of A13. `rules.ron` also grew well past "global constants" — it
> carries the `house_levels` ladder (phase 13, where `residents_per_house_level` ended up), the
> `satisfaction` curve (phase 12) and the `demographics` rates (phase 14). Its comments carry the
> derivations and are the closest thing the project has to a balancing reference; what each parameter
> *does*, without the values, is in [RULES.md](../RULES.md).

`buildings.ron`, the table that carries the weight:

```ron
(
  buildings: [
    (
      id: "house",
      size: (1, 1),
      cost: 10,
      levels: 1,                        // levelling up is M1
      service: None,
      required_services: ["water", "food"],
    ),
    (
      id: "well",
      size: (1, 1),
      cost: 12,
      levels: 1,
      service: Some((
        kind: "water",
        // one value per level: range in tiles walked along roads (D2)
        range_per_level: [12],
        capacity_per_level: [8],        // houses served at the same time
      )),
      required_services: [],
    ),
    (
      id: "farm",
      size: (2, 2),
      cost: 40,
      levels: 1,
      service: Some((
        kind: "food",
        range_per_level: [10],
        capacity_per_level: [6],
      )),
      // see A5: in M0 the farm covers just like a well. In M3 the goods will
      // arrive from a warehouse via walkers and this field will change.
      output_per_tick: 400,             // Milli of food
      max_stock: 20000,
      required_services: [],
    ),
  ],
)
```

### Loading and validation

```rust
pub struct DataSet {
    pub rules: Rules,
    pub terrain: BTreeMap<Terrain, TerrainDef>,     // BTreeMap, not HashMap (D4)
    pub buildings: Vec<BuildingDef>,                // indexed by BuildingKindId
    pub hash: [u8; 32],
}

pub fn load_from_dir(path: &Path) -> Result<DataSet, LoadError>;
pub fn validate(raw: &RawDataSet) -> Result<(), ValidationReport>;
```

`ValidationReport` is a `Vec<ValidationError>`, **not** a single error: whoever is fixing a table
wants to see every problem in one go, not recompile six times. Each error carries the logical path
of the field (`buildings[1].service.range_per_level`).

The minimum checks:

- building ids unique and non-empty
- `levels >= 1`; `range_per_level.len() == levels` and likewise for `capacity_per_level`
  (it is the likeliest mistake once levels get added in M1)
- costs and ranges non-negative; `size` with both dimensions `>= 1`
- every `required_services` entry and every `service.kind` resolves to a known `ServiceKind`
- `output_per_tick` present ⟺ the building is a producer; `max_stock > 0` if it produces
- `rules.ticks_per_month >= 1`

`hash` is `blake3` over the dataset's **normalised** content (the validated values, not the files'
bytes: that way a reformatting or a comment in the RON does not invalidate the recordings, while a
changed number does). It feeds the state hash (see A2).

Loading is I/O and lives in `sim-data`, never inside `sim-core` (D4: no I/O in the core).

## Out of scope

Multi-step production chains, requirements for houses levelling up, per-civilisation tables
(D6/M3), taxes. Three buildings, one level each.

## Tests

1. **A valid fixture**: the production `data/` loads with no errors. A permanent test: it is the
   safety net for every future balancing change.
2. **Broken fixtures**, one per check, in `tests/fixtures/broken/`: a duplicate id, `levels: 2`
   with only one range, a negative cost, `service.kind: "astral_transport"`, a missing field,
   syntactically invalid RON.
3. **The report is complete**: a fixture with **three** different errors produces three entries.
   It is the test that tells real validation apart from a `?` on the first check.
4. **Stable messages**: an `insta` snapshot of the rendered report. If a message gets worse, the
   diff shows it. (It doubles as readable documentation of the errors.)
5. **Hash**: stable across two loads; **unchanged** if the RON is reformatted or a comment is
   added; **different** if `cost: 10` becomes `cost: 11`.

## Verification

```sh
cargo test -p sim-data
cargo insta review        # only if the snapshots change
```

**Done when:** test 3 passes (three errors reported together) and test 5 shows that the hash tells
a balancing change apart from a reformatting.
