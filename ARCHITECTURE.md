# Architecture

**This file answers one question: where does the code live, and how does data flow through it?**

It describes the tree **as it is today**, in the present tense. It carries no balancing values — those
live in `sim-data/data/*.ron` and are described, without their numbers, in [RULES.md](RULES.md). It
carries no history — that is [plan/](plan/README.md) — and no rationale — that is
[DECISIONS.md](DECISIONS.md).

**Every change checks this file.** The question to ask is narrow and answerable: *did you add, move,
remove or reorder a tick step; change what a step reads or writes; add a field to `World`; or add a
crate?* If yes, this file changes in the same commit. If no, say so and move on.

## The crates

Dependencies always point towards `sim-core`. If `game-bevy` ever appears among another crate's
dependencies, that is an architectural mistake.

| Crate | Exists | Responsibility |
|---|---|---|
| `sim-core` | **yes** | State, tick, commands. No dependency beyond serde/rand_pcg/slotmap/blake3. |
| `sim-data` | **yes** | RON tables, validated at load. The only crate doing I/O on game data. |
| `sim-replay` | **yes** | `seed + Vec<Command>`, the state hash, the recorded replays. |
| `xtask` | **yes** | Headless runner: `run`, `record`, `regen-expected`, `bench`, `doc-check`. |
| `sim-scenario` | no | Scenarios, objectives, victory conditions. Phase 17. |
| `sim-civ` | no | The `CivilizationRules` trait. **M3, together with the second civilisation** — not before. |
| `agent-bot` | no | Compiles `Intent`s into primitive `Command`s. M2. |
| `agent-eval` | no | Metrics and scoring for a game. M2. |
| `agent-llm` | no | Semantic observation and tool schema for the LLM. M3. |
| `game-bevy` | no | Renderer. Consumes snapshots and events. M2. |

The six absent crates are absent **deliberately**: empty scaffolding is surface that invites you to
fill it. See [ROADMAP.md](ROADMAP.md) for when each arrives.

### `sim-core` modules

| Module | Responsibility |
|---|---|
| `world.rs` | The game state: a concrete struct of `SlotMap`s and `Vec`s, not an ECS (D1). |
| `tick.rs` | The ten-step tick, whose order is game semantics. |
| `data.rs` | The validated dataset the core consumes, plus every **cross-table** consistency check. |
| `grid.rs` | The tile grid. `Tile` fits in 4 bytes so 40,000 tiles stay in cache. |
| `rng.rs` | One seeded PCG64 stream per kind, derived from the kind's **name**, not its index. |
| `network.rs` | Road connected components and walked distances. Derived, outside the hash. |
| `coverage.rs` | Aggregate service coverage along roads (D2). **The hot path.** |
| `production.rs` | Step 4 — farm output into local stock, consumption by covered houses. |
| `satisfaction.rs` | Step 6.1 — satisfaction as an accumulator of time served, plus the `Mood` bands. |
| `levels.rs` | Step 6.2 — the monthly house level-up and decay review. |
| `demographics.rs` | Steps 6.3/6.5 — aggregated births and deaths, with a frozen draw order. |
| `units.rs` | `Milli(i32)` and `Coins(i32)`: the only numeric quantities allowed in the state. |
| `ids.rs` | Newtype ids. A bare `usize` never appears in a public signature. |
| `service.rs` | `ServiceKind` — an enum, unlike building kinds, which are data (D6). |
| `data.rs` (`BuildingRole`) | An enum over the two **roles** — house, provider — while **which buildings exist** stays data (D6). A building's shape follows its role, so a house has no service field to leave empty (A21). |
| `command.rs` | The primitive commands and their structured errors. The only write channel in. |
| `event.rs` | Delta events for the renderer, emitted on state **change**, never per tick. |
| `data_hash.rs` | blake3 of the validated dataset, fed field by field by hand (A3). |

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

Two non-obvious invariants worth knowing before you touch any of this:

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
| 6 | Houses: satisfaction, levels, migration | **yes**, partly | `satisfaction.rs`, `levels.rs`, `demographics.rs` |
| 7 | Finance and taxes | no — phase 16 | — |
| 8 | Random events | no — M1. First use of `RngKind::Events` | — |
| 9 | Check the scenario objectives | no — phase 17, needs `sim-scenario` | — |
| 10 | Emit the events for the renderer | **yes** | `event.rs` |

**Six full, four empty.** An invalid command does not interrupt the tick and is not an `Err` of the
tick: it is discarded and reported with the index it had in `cmds`.

After step 10, `summarise` reads the tick's aggregates off the world. It is **not** a delta event: it
is the snapshot the renderer draws its bars from and the evaluator (M2) reads its metrics from. As
events these would be one per house per tick, which is the polling the core/renderer boundary forbids.

### Step 6's internal order — also game semantics

| # | Sub-step | Note |
|---|---|---|
| 6.1 | `satisfaction::update` | First, because it reads only what steps 3 and 4 wrote **this** tick, and everything else in step 6 reads it. |
| 6.2 | `levels::review` | Only on a month boundary. The cadence is what makes the absence of oscillation structural rather than a consequence of the thresholds. |
| 6.3 | deaths | `demographics::run` |
| 6.4 | *emigration* | **Reserved slot** — phase 15. |
| 6.5 | births | `demographics::run`. Departures before arrivals, so `residents <= max_residents` holds at every observable instant. |
| 6.6 | *immigration* | **Reserved slot** — phase 15. |
| — | invalidate the coverage if anyone moved | Stays the **last** thing step 6 does, including after 15 lands. |

Phases add their sub-steps **below** these, never above. That final conditional invalidation is
A12's cost site: coverage is counted on the residents present, so if anyone moved, yesterday's
assignment no longer holds. It is conditional rather than unconditional on purpose — but phase 14
measured the condition as true on **100%** of ticks at the reference scale ([A17](DECISIONS.md)).

### The demographics draw order — a determinism contract

1. deaths' jitter
2. deaths' choice of house, one draw per death
3. births' jitter
4. births' choice of house, one draw per birth

The rule the module implements: ***the rate is random, the distribution is deterministic***. Fifteen
thousand residents cost four draws and a handful more, not fifteen thousand. Nothing is drawn when
there is nothing to draw for.

## The frozen orders

Four declaration orders are load-bearing. Reordering any of them **silently invalidates every
recorded replay** — nothing about it will look like an error:

| Order | Why |
|---|---|
| The ten tick steps | Game semantics. |
| Step 6's sub-steps | Game semantics. |
| The demographics draws | The RNG sequence. |
| `RngKind` `[Events, Migration, Production, Demographics]` | Position in the state hash. Renaming a variant also changes its **salt**. |
| `Flow` `[Births, Deaths, Immigration, Emigration]` | Indexes `Demographics::remainder`, which is hashed. |
| `Terrain` `[Plain, Water, Rock]` · `ServiceKind` `[Water, Food]` | Table completeness checks and fixed-size array indices. |

These are listed in `GLOSSARY.md` under "Frozen strings and values" and are verified by
`cargo xtask doc-check`.

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
