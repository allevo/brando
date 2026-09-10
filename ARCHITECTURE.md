# Architecture

**This file answers one question: where does the code live, and how does data flow through it?**

It describes the tree **as it is today**, in the present tense. It carries no balancing values — those
live in `sim-data/data/*.ron` and are described, without their numbers, in [RULES.md](RULES.md). It
carries no history — that is [plan/](plan/) — and no rationale, which lives in the comment on the
code each rule binds.

**Every change checks this file.** The question to ask is narrow and answerable: *did you add, move,
remove or reorder a tick step; change what a step reads or writes; add a field to `World`; or add a
crate?* If yes, this file changes in the same commit. If no, say so and move on.

## The crates

Dependencies always point towards `sim-core`. If `game-bevy` ever appears among another crate's
dependencies, that is an architectural mistake.

The same rule binds `map-gen`, and there it is not a sentence but something that fails:
`cargo xtask doc-check` reads the three manifests and refuses a workspace in which `sim-core`,
`sim-data` or `sim-replay` names it. A generator the simulation could reach would be a frozen part
of the determinism contract, and every improvement to it would move every recorded hash.

| Crate | Exists | Responsibility |
|---|---|---|
| `sim-core` | **yes** | State, tick, commands. No dependency beyond serde/rand_pcg/slotmap/blake3. |
| `sim-data` | **yes** | RON tables, validated at load. The only crate doing I/O on game data. Owns the map format in both directions: `parse_map` reads it, `render_map` and `save_map` write it. |
| `sim-replay` | **yes** | `seed + Vec<Command>`, the state hash, the recorded replays. |
| `map-gen` | **yes** | Generates a map from a seeded list of mountain/land/sea sources that compete for tiles and then shape their own ground. One entry point: `generate` turns a `Config` into the `MapDef` the game plays, the same type the loader returns. Pure arithmetic: it touches no disk, and nothing the simulation loads may depend on it. |
| `xtask` | **yes** | Headless runner: `run`, `record`, `regen-expected`, `bench`, `gen-map`, `doc-check`. |
| `sim-scenario` | no | Scenarios, objectives, victory conditions. Phase 17. |
| `sim-civ` | no | The `CivilizationRules` trait. **M3, together with the second civilisation** — not before. |
| `agent-bot` | no | Compiles `Intent`s into primitive `Command`s. M2. |
| `agent-eval` | no | Metrics and scoring for a game. M2. |
| `agent-llm` | no | Semantic observation and tool schema for the LLM. M3. |
| `game-bevy` | no | Renderer. Consumes snapshots and events. M2. |

The crates marked *no* are absent **deliberately**: empty scaffolding is surface that invites you to
fill it. See [ROADMAP.md](ROADMAP.md) for when each arrives.

### `sim-core` modules

| Module | Responsibility |
|---|---|
| `world.rs` | The game state: a concrete struct of `SlotMap`s and `Vec`s, not an ECS (D1). |
| `tick.rs` | The ten-step tick, whose order is game semantics. Also `Calendar`, the fixed month/year lengths (D6). |
| `data.rs` | The validated dataset the core consumes, plus every **cross-table** consistency check. `BuildingRole` is an enum over the **roles** a building can play — house, provider — while **which buildings exist** stays data (D6). A building's shape follows its role, so a house has no service field to leave empty. |
| `grid.rs` | The tile grid. `Tile` fits in 4 bytes so 40,000 tiles stay in cache — terrain, flags and ground height share a packed `u16`. `Grid::from_map` builds a grid from a `MapDef`; `slope_over` and `walkable_regions` are derived, never stored. |
| `map.rs` | `MapDef`: the validated shape `Grid::from_map` builds a grid from, and the blake3 of it that travels in the replay header (`MapSpec::File`). Loaded and validated by `sim-data` (`RawMap`, `validate_map`), the same split `Rules`/`BuildingDef` use. |
| `rng.rs` | One seeded PCG64 stream per kind, derived from the kind's **name**, not its index. |
| `network.rs` | Road connected components and walked distances. Derived, outside the hash. |
| `coverage.rs` | Aggregate service coverage along roads (D2). **The hot path.** |
| `production.rs` | Step 4 — farm output into local stock, consumption by covered houses. |
| `satisfaction.rs` | Step 6.1 — satisfaction as an accumulator of time served, plus the `Mood` bands. |
| `levels.rs` | Step 6.2 — the monthly house level-up and decay review. |
| `demographics.rs` | Steps 6.3/6.5 — aggregated births and deaths, with a frozen draw order. |
| `migration.rs` | Steps 6.4/6.6 — emigration, immigration and `attractiveness`. The destination is never gated on coverage (slot 14.5). |
| `units.rs` | `Milli(i32)` and `Coins(i32)`: the only numeric quantities allowed in the state. |
| `ids.rs` | Newtype ids. A bare `usize` never appears in a public signature. |
| `service.rs` | `ServiceKind` — an enum, unlike building kinds, which are data (D6). |
| `command.rs` | The primitive commands and their structured errors. The only write channel in. |
| `event.rs` | Delta events for the renderer, emitted on state **change**, never per tick. |
| `data_hash.rs` | blake3 of the validated dataset, fed field by field by hand rather than through serde, so the hash does not depend on a serialisation format. |

## The state

`World` is the complete state of a game. Fields split three ways, and the split is what the state
hash depends on:

**Hashed — this is the state.**
`tick`, `grid`, `buildings`, `houses`, `walkers`, `economy`, `demographics`, `rng`, `data` (via its
hash), `difficulty`.

**Derived — outside the hash**, so that a rebuild bug shows up as a failing equivalence test rather
than as a hash divergence: `roads`, `coverage`.

**Diagnostic — outside the hash**, because they influence no game decision: `food`, `population`.

**Bookkeeping:** `dirty`, `buildings_by_origin`, `houses_by_origin`. The two indexes are `BTreeMap`s
and not `HashMap`s (D4) — their iteration order is a contract.

The budgets inside that state that are not free to grow:

- **`Tile` stays small** — `u16` indices, no pointers, no `Option<Box<...>>`. Forty thousand tiles have
  to stay in cache as much as possible; `Tile` fits in 4 bytes today, which is 160 KB for the grid, and
  a test in `sim-core/src/grid.rs` guards the budget against the next field somebody wants to add.
- **The `DirtyFlags` are not optional.** They exist from the start because retrofitting them is
  painful. Using them naively — when the roads change, every provider goes dirty — is allowed and is
  what M0 does; not having them is not.

Non-obvious invariants worth knowing before you touch any of this:

- Iterating a `SlotMap` goes by slot index, so it is deterministic given the same sequence of
  insertions and removals — which the command log guarantees (D4).
- `walkers` is empty until M3 and is hashed anyway. The length prefix alone is what makes the first
  walker to exist move a recording.

Adding a field to `World` breaks the build at `World::every_field` (`sim-core/src/world.rs`), an
exhaustive destructure that exists to force the question *does this belong in the hash?* It breaks
the build a second time at `World::first_difference`, the exhaustive comparison beside it.

**`World` is not `Clone`, in any configuration**, and `not_clone` in the same file refuses the derive
at compile time. A world is played, never copied: whoever wants a second one holding a given state
builds it and steps it — `twins` in `sim-core/tests/common`, a second `replay` in `sim-replay`.

## The tick — the spine

**1 tick = 1 game day.** The month is a fixed multiple. `pub fn step(&mut World, &[Command])` in
`sim-core/src/tick.rs`.

The order of these ten steps is **game semantics, not an implementation detail**. All ten functions
exist even when empty: if they came into being one at a time, the order would end up an accident of
the development timeline. Reordering requires regenerating the recordings and writing down why.

| # | Step | Implemented | Lives in |
|---|---|---|---|
| 1 | Apply the incoming commands | **yes** | `tick.rs`, types in `command.rs` |
| 2 | Rebuild the road network if dirty | **yes** | `network.rs` |
| 3 | Propagate service coverage | **yes** | `coverage.rs` — **hot path** |
| 4 | Production and consumption | **yes** | `production.rs` |
| 5 | Step the real logistics walkers | no — M3 (D3) | — |
| 6 | Houses: satisfaction, levels, migration | **yes** | `satisfaction.rs`, `levels.rs`, `demographics.rs`, `migration.rs` |
| 7 | Finance and taxes | no — phase 17 | — |
| 8 | Random events | no — not scheduled. First use of `RngKind::Events` | — |
| 9 | Check the scenario objectives | no — phase 18, needs `sim-scenario` | — |
| 10 | Emit the events for the renderer | **yes** | `event.rs` |

An invalid command does not interrupt the tick and is not an `Err` of the tick: it is discarded and
reported with the index it had in `cmds`.

After step 10, `summarise` reads the tick's aggregates off the world. It is **not** a delta event: it
is the snapshot the renderer draws its bars from and the evaluator (M2) reads its metrics from. As
events these would be one per house per tick, which is the polling the core/renderer boundary forbids.

### Step 6's internal order — also game semantics

| # | Sub-step | Note |
|---|---|---|
| 6.1 | `satisfaction::update` | First, because it reads only what steps 3 and 4 wrote **this** tick, and everything else in step 6 reads it. |
| 6.2 | `levels::review` | Only on a month boundary. The cadence is what makes the absence of oscillation structural rather than a consequence of the thresholds. |
| 6.3 | deaths | `demographics::deaths` |
| 6.4 | emigration | `migration::emigration` |
| 6.5 | births | `demographics::births`. Departures before arrivals, so `residents <= max_residents` holds at every observable instant. |
| 6.6 | immigration | `migration::immigration`. Reads `migration::attractiveness`, itself a pure read of the state as this tick's deaths, emigration and births already left it. |
| — | invalidate the coverage if anyone moved | Stays the **last** thing step 6 does. |

That final conditional invalidation is where the services chasing the population is paid for:
coverage is counted on the residents present, so if anyone moved, yesterday's assignment no longer
holds. It is conditional rather than unconditional on purpose — but phase 14 measured the condition as
true on **100%** of ticks at the reference scale, and what to do about that is the open question at
slot 19.5.

**Immigration's destination is not gated on coverage.** The eligible set is every house with a free
place, served or not, occupied or empty — coverage only shapes the *rate*, through `attractiveness`.
Slot 14.5, closed before this module was written, ruled the narrower gate out explicitly: a coverage
precondition on the destination would leave an emptied, uncovered house unable to ever be refilled.

### The demographics draw order — a determinism contract

1. deaths' jitter
2. deaths' choice of house, one draw per death
3. emigration's jitter
4. emigration's choice of house, one draw per departure
5. births' jitter
6. births' choice of house, one draw per birth
7. immigration's jitter
8. immigration's choice of house, one draw per arrival

The rule the two modules implement: ***the rate is random, the distribution is deterministic***.
Fifteen thousand residents cost eight draws and a handful more, not fifteen thousand. Nothing is
drawn when there is nothing to draw for. Deaths and births draw from `RngKind::Demographics`,
emigration and immigration from `RngKind::Migration` — two independent streams, so adding one phase's
flows never knocked the other's sequence out of phase.

## The frozen orders

The orders this file defines are load-bearing. Reordering one **silently invalidates every recorded
replay** — nothing about it will look like an error:

| Order | Why |
|---|---|
| The tick steps | Game semantics. |
| Step 6's sub-steps | Game semantics. |
| The demographics draws | The RNG sequence. |

The declaration order of an enum is load-bearing the same way — a position stored in the state hash,
or an index into a fixed-size array — but it is not stated here. It is stated once, in `GLOSSARY.md`
under "Frozen strings and values", where `cargo xtask doc-check` reads each row and compares it
against that type's own `const ALL`. A second copy here would be prose nothing keeps in agreement
with the code.

## The boundary with the renderer

The renderer receives a **complete snapshot on the first frame** and **delta events** afterwards
(`HouseEvolved`, `BuildingPlaced`, `WalkerSpawned`, `ServiceCoverageChanged`). Rebuilding 40,000
entities every tick is unacceptable; step 10 emits only what changed relative to the start of the
tick, so a house hungry for ten ticks generates one event and not ten.

The renderer never calls a method that mutates the core. **The only write channel is the `Command`
queue.** A save file is `seed + Vec<Command>`, not a dump of the state (D4).

## Where determinism is enforced

Not by convention — by things that fail:

| Mechanism | Where | Catches |
|---|---|---|
| `disallowed-types` / `disallowed-methods` | `clippy.toml` | `HashMap`, `HashSet`, `thread_rng`, clock access |
| `unsafe_code = "forbid"`, `float_arithmetic = "deny"` | `Cargo.toml` | floats and `unsafe` in the core |
| `World::every_field` | `sim-core/src/world.rs` | a new field silently missing from the hash |
| `not_clone` | `sim-core/src/world.rs` | `World` regaining `Clone`, i.e. a state reached by copying rather than by playing |
| `the_hash_covers_the_whole_state` | `sim-replay/tests/expected.rs` | a field in the struct but not in the hash |
| `DataSet::inconsistencies` | `sim-core/src/data.rs` | balancing numbers that must stand in a relation |
| Recorded replays, checkpointed | `sim-replay/tests/expected/*.hashes` | **any** new source of non-determinism |
| `cargo xtask doc-check` | `xtask/src/doc_check.rs` | the documents drifting from the code |

If a replay hash moves when the rules and tables have not changed, **stop and find the cause**. It is
the project's most valuable test.
