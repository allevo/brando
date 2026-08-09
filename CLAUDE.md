Instructions for developing this repository. Read all of it before writing any code.

## What this project is

A 2.5D city builder inspired by Zeus: Master of Olympus. The player builds a city starting from a
few resources and makes it prosper, managing the economy, food, trade and safety.
The game supports several civilisations (Greek, Egyptian, ...), each with its own building rules.

Three non-functional requirements drive the whole architecture:

1. The core logic is extensively tested and deterministic.
2. The game is drivable by an AI (an LLM that plans + a bot that executes + an evaluator that
   judges), so the parameters can be balanced automatically.
3. The game runs headless, with no graphics dependency at all.

Language: Rust. Edition 2021+.

Every identifier, comment and document in this repository is in English. Any word you do not
recognise is in [GLOSSARY.md](GLOSSARY.md), together with the Italian it replaced.

---

## Architectural decisions already taken

These decisions have been discussed and are **binding**. Do not reopen them without an explicit
discussion. If an implementation seems to require breaking one, stop and ask.

### D1 — The core is not an ECS and does not depend on Bevy

`sim-core` is a pure Rust crate. The state is a concrete struct with `SlotMap`s/`Vec`s, not an ECS
`World`. The reason: the iteration order of an ECS's queries is not a stable contract, Bevy makes
frequent breaking releases, and for balancing runs a tight loop over dense arrays is orders of
magnitude faster.

Bevy is **only** a rendering client that reads snapshots and emits commands.
Headless means running `sim-core` alone, with no Bevy in memory.

### D2 — Aggregate coverage, not service walkers

The city services (water, food, culture, health) work by **aggregate coverage**: every building
serves the houses within a range computed as the distance walked along the road network, not as the
crow flies. The range and the capacity are parameters read from a data table and grow with the
building's level.

The walkers the player sees "walking around" for these services are **purely decorative**, they live
in the renderer, they are derived from the result of the coverage and they **can never influence the
core's state**. If a decorative walker shows up in `sim-core`, it is a bug.

### D3 — Some walkers, on the other hand, are real

Goods transport between warehouses, trade caravans and immigrants are genuinely simulated entities:
they take time, they can be blocked, they carry state, they live in `sim-core`.
The distinction is per kind of building and has to be documented in the data tables.

### D4 — Determinism through a seed + the command log

A save file is `seed + Vec<Command>`, not a dump of the state. The core is conceptually a pure
function `step(&mut World, &[Command])`.

Non-negotiable rules:

- **Never** iterate a `HashMap`/`HashSet`. Use `BTreeMap`, `IndexMap` or indexed `Vec`s.
- **Never** `rand::thread_rng()`. The RNG lives in the state and is seeded (`rand_pcg::Pcg64`).
- RNGs **separated per domain** (events, migration, production). That way adding a feature does not
  knock the existing sequences out of phase and does not invalidate every recorded replay.
- **No floats in the state.** Fractional quantities use the `Milli(i32)` newtype (thousandths) with
  checked operations. Floats are allowed only in the renderer.
- No parallelism in the core until the reduction order is provably fixed.
- No I/O, no access to the system clock inside the core.

### D5 — Houses, not citizens

The unit of population simulation is the **house** (it holds N residents, a level, and a
satisfaction state for each service). Individuals are not simulated.
Target scale: a 200×200 tile map, ~15,000 residents.

### D6 — Civilisation = data + Rust rules

Everything numeric (costs, production chains, ranges, requirements) lives in RON tables loaded and
validated at startup. The spatial and structural rules (e.g. the Egyptians bury to the west, the city
is split east/west) live in Rust implementations of the `CivilizationRules` trait.

The trait's hooks have to be **pure functions** of the state: no I/O, no RNG of their own. Keep the
trait minimal. Do not widen it on speculation: with a single implementation there is no way to know
what the right abstraction is. It gets widened when the second civilisation really requires it.

### D7 — Scenarios and sandbox

The game supports both, but **the AI player only plays scenarios** with explicit objectives
(mandatory + optional), campaign style. The evaluator's score is based on those.

---

## The shape of the workspace

```
sim-core/      state, tick, commands. No dependencies beyond serde/rand_pcg
sim-data/      RON tables + validation at load time
sim-civ/       the CivilizationRules trait + implementations
sim-scenario/  scenarios, objectives, victory conditions
sim-replay/    seed+log, save/load, hashing the state
agent-bot/     compiles Intents -> primitive Commands
agent-eval/    metrics and scoring for a game
agent-llm/     adapter: semantic observation, tool schema
game-bevy/     renderer, consumes snapshots and events
xtask/         headless runner, batches of games, regenerating the recordings
```

The dependencies always point towards `sim-core`, never the other way.
If `game-bevy` shows up among another crate's dependencies, it is an architectural mistake.

---

## The state model

```rust
pub struct World {
    tick: u32,
    grid: Grid,                          // Vec<Tile>, index y*W+x
    buildings: SlotMap<BuildingId, Building>,
    houses: SlotMap<HouseId, House>,
    walkers: Vec<Walker>,                // real logistics ones only (D3)
    economy: Economy,
    rng: RngSet,                         // RNGs separated per domain
    dirty: DirtyFlags,
}
```

`Tile` has to stay small: use `u16` indices, not pointers and not `Option<Box<...>>`.
40,000 tiles have to stay in cache as much as possible.

---

## The order of the tick

This order is **game semantics**, not an implementation detail. Do not reorder without regenerating
the recorded replays and documenting the reason.

1. Apply the incoming commands
2. Rebuild the road network if dirty
3. Propagate the service coverage (BFS along the roads from the dirty providers)
4. Production and consumption along the chains
5. Step the real logistics walkers
6. Houses levelling up / decaying, migration
7. Finance and taxes
8. Random events (fires, disease, invasions)
9. Check the scenario objectives
10. Emit the events for the renderer

Step 3 is the hot path. Implementing it naively at first is fine, but the `DirtyFlags` have to exist
from the very start: retrofitting them later is painful.

Unit of time: **1 tick = 1 game day**, the month is a fixed multiple. Scenario objectives are
expressed in months/years.

---

## Testing — mandatory, not optional

No PR adds a system to the core without the corresponding tests. In order of value:

**1. Property tests (`proptest`) on the invariants.**
Population never negative, goods conserved along the production chain, no overlap between buildings,
a treasury consistent with the transactions. These find the real bugs.

**2. Recorded replays.**
One `seed + Vec<Command>` file per scenario. Every N ticks a `blake3` hash is computed over a fixed
serialisation of the state, and compared against a reference file.
If the balancing changes, the hashes change on purpose and are regenerated with
`cargo xtask regen-expected`. **If they change when they should not, a source of non-determinism has
been introduced: stop and find it.** This is the project's most valuable test.

**3. Fuzzing the commands.**
Random sequences of commands, absurd or malformed ones included, must never cause a panic.
The LLM will produce invalid commands: the core rejects them with a structured error.

**4. Scenario tests.**
A heuristic bot has to complete scenario 1 within N months. It is the canary on the balancing: if it
fails after a parameter change, the difficulty curve is broken.

**Performance — not a test.** `cargo run --release -p xtask -- bench` measures the cost of `step()`
on a city at the reference scale, separating the application of the commands from the recomputations
that follow. It has no thresholds: absolute times depend on the machine, and the way to use it is to
run it before and after a change on the same machine. The one thing that has to stay the same in
absolute terms is the state hash it prints: if that moves without the rules or the tables having
changed, the optimisation has changed the semantics and it is a bug.

---

## The AI interface

The loop is: **the LLM produces `Intent`s → the bot compiles them into `Command`s → the core runs →
the evaluator judges the saved state → feedback to the LLM.**

The LLM never reasons in absolute coordinates: it is terrible at it and produces unplayable plans.

```rust
enum Intent {
    BuildDistrict { kind: DistrictKind, near: Landmark, size: u8 },
    EnsureService { service: ServiceKind, area: AreaRef },
    SetupProduction { chain: ChainId, target_rate: u32 },
    AdjustTax { delta: i8 },
}
```

The bot returns `Result<Vec<Command>, IntentFailure>`, where the failure is **descriptive**
(`NoFlatSpaceNear`, `InsufficientFunds { needed, available }`, `RoadNetworkDisconnected`).
That message goes back to the LLM as feedback and closes the loop.

The determinism log records the primitive `Command`s; the `Intent`s are kept as metadata for
analysis.

**Observation for the LLM**: never the serialised state. What is needed is a summary — aggregate
indicators, a list of problems ordered by severity, and an ASCII map downsampled to ~40×40 with
symbols per zone. Far more effective than any detailed JSON.

**Evaluator**: it produces a *vector* of metrics, not a scalar (mandatory objectives completed,
optional ones, ticks taken, stability as the variance of the treasury, resilience as collapses in
population). The scalar is derived afterwards, so the formula can change without redoing the runs.

---

## The core ↔ renderer boundary

Bevy receives a **complete snapshot on the first frame** and then **delta events**
(`HouseEvolved`, `BuildingPlaced`, `WalkerSpawned`, `ServiceCoverageChanged`).
Rebuilding 40,000 entities every tick is unacceptable.

The renderer never calls methods that mutate the core. The only write channel is the `Command` queue.

---

## Code conventions

- Errors with `thiserror` in the libraries. No `unwrap()`/`expect()` in the core, except for provably
  impossible invariants, and in that case with a comment explaining why.
- No `async` in the core: it is a tick-based simulation, it has nothing to wait for.
- Newtypes for the ids (`BuildingId`, `HouseId`, `TileIdx`), never a bare `usize` in a signature.
- Balancing numbers **never** live in the code: they go in `sim-data`.
  If a numeric game constant shows up in a `.rs`, it is a bug.
- `#![forbid(unsafe_code)]` in every `sim-*` crate.
- Comments in English, like the rest of the repository. Doc comments on the public traits and on
  every non-obvious invariant.

---

## Roadmap

**M0 — Foundations.** `World` with a grid and roads, three kinds of building (house, well, farm), the
tick loop, primitive `Command`s, replay with hashing. No graphics.
Tests: property tests + one recorded replay.

**M1 — A minimal game loop.** Houses level up with water and food, and decay otherwise. Migration in
and out. Treasury and taxes. One scenario with an objective ("500 residents within 5 years").

**M2 — The two clients, in parallel.** Bevy renders M1's state isometrically with placeholder assets
(the goal is validating the snapshot/event boundary, not the graphics). In parallel: a heuristic bot
that completes the scenario + an evaluator.

**M3 — Depth.** Production chains with real logistics walkers. The `CivilizationRules` trait
introduced **together with the second civilisation**, not before. An LLM adapter on top of a bot that
already works.

### Current state

> Milestone: **M0 — complete** (2026-08-08)
>
> Update this section at every completed milestone.

What M0 covers, one line per crate:

- `sim-core` — a `World` with a grid of at most 256×256, roads with connected components, service
  coverage over walked distance, food production and consumption, a ten-step tick (four full, six
  empty pending M1/M3), primitive commands with structured errors, one RNG per domain. `Tile` fits in
  4 bytes.
- `sim-data` — three RON tables validated with a complete error report and a hash of the dataset.
- `sim-replay` — saving as `seed + Vec<Command>`, the state hash, two recorded replays with a
  checkpoint every 30 ticks.
- `xtask` — `run`, `record`, `regen-expected [--check]`, `bench`.

What M0 deliberately does **not** have: houses levelling up, migration, taxes, scenario objectives,
random events, real logistics walkers, a renderer, agents. Those are M1–M3.

Two things learned while implementing, which are worth more than the decisions taken in the abstract:

1. Step 3 (coverage) is the hot path, confirmed by measurement and not by reasoning: a tick that
   accepts a command costs ~75× an empty tick, because the invalidation is naive. The cost is **per
   tick, not per command**. The numbers are in
   [plan/09-invariants-closeout.md](plan/09-invariants-closeout.md).
2. Coverage on a "first capacity taken, first served" basis made the hungry state **absorbing** for a
   house already assigned. Dissolved right after M0, as a prerequisite for house levels: not by
   counting the capacity in residents — that just restates the same constraint in another unit, and
   the two scenarios give identical output — but by making a producer's capacity consistent with what
   its output sustains. From that follows the invariant *a house covered by food always eats*. The
   general lesson: a balancing number that has to stand in a particular relation with another one is
   a **validation check**, not a comment.

**After M0**, before going into M1: service capacity in residents served, and consistency between
capacity and output ([A5](plan/open-decisions.md)).

---

## What NOT to do

- Do not move the simulation into Bevy's `World`, for any reason.
- Do not introduce floats, iterated `HashMap`s or `thread_rng` into the core.
- Do not write balancing numbers into the code.
- Do not implement `CivilizationRules` before M3: with a single civilisation the abstraction would be
  invented.
- Do not add game systems without the corresponding recorded replay.
- Do not optimise before the profiler, but do not put off the `DirtyFlags`.
- Do not expand the scope: a city builder has an enormous number of interconnected systems and it is
  extremely easy to spend months on mechanics nobody ever plays. Every new system has to be reachable
  and observable in an existing scenario.
