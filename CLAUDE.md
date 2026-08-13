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

Language: Rust, edition 2024.

Every identifier, comment and document in this repository is in English. Any word you do not
recognise is defined in [GLOSSARY.md](GLOSSARY.md).

---

## The documents, and which one to update

Each document answers **exactly one question**, and nothing else may answer it. Two documents that
can both answer the same question will eventually disagree, and then neither can be trusted.

| Question | File | Lifecycle |
|---|---|---|
| Where does the code live, how does data flow? | [ARCHITECTURE.md](ARCHITECTURE.md) | present tense, edited in place |
| What does the game do? | [RULES.md](RULES.md) | present tense, **no values** — names of parameters only |
| Why is it this way? | [DECISIONS.md](DECISIONS.md) | **append only**, never rewritten |
| What is next, what is undecided? | [ROADMAP.md](ROADMAP.md) | the only statement of how far the tree has got |
| What does this word mean? | [GLOSSARY.md](GLOSSARY.md) | edited in place |
| How do I work here? | this file | edited in place — **no statements of current state** |
| What happened, in order? | [plan/](plan/README.md) | **frozen history** |

**`plan/` is a record of moments in the past and is never edited to match the present.** When a phase
document turns out to describe behaviour that later changed, you do **not** rewrite the sentence: you
append a dated amendment block below it. The original prediction next to what really happened is the
most useful thing in the whole record — rewriting it destroys the only evidence of how the design
moved.

### The definition of done for a phase

A phase is not finished until all of these are true:

1. `cargo test --workspace` is green, and the new behaviour has the tests its phase file promised.
2. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean.
3. `cargo xtask regen-expected --check` is green, **or** the recordings were regenerated on purpose
   and the reason is written down. A hash that moves without the rules or tables changing is a
   source of non-determinism: stop and find it.
4. `cargo xtask doc-check` is green.
5. Each of `ARCHITECTURE.md`, `RULES.md`, `ROADMAP.md` and `GLOSSARY.md` has either been updated or
   consciously declared unaffected. Say which, explicitly; "I did not think about it" is the failure
   mode this list exists to prevent.
6. The phase file gets its `## How it went` section — including the ways the plan was wrong — and its
   status header.
7. Any decision taken along the way is a new entry in `DECISIONS.md`. Any decision **deferred** gets a
   half-numbered `TO_BE_DECIDED` slot in `ROADMAP.md`, at the point where it has to be closed.

### `DECISIONS.md` records what was decided, never what will be

**Do not write an entry for a decision that has not been taken yet.** Not for a phase that is planned
and unbuilt, not "so it is not forgotten", not with a note saying it is provisional. The file is
append-only: an entry written early cannot be withdrawn, only amended, so the cheapest repair is
already more expensive than never having written it. Worse, it is a lie about the project's own
history — the register's whole value is that reading it tells you what was really known when.

**Do not reserve numbers either.** A reserved `A<n>` goes wrong the moment other work lands first, and
a plan file citing an id that turned out to belong to something else is worse than one citing none.

Where the argument goes instead, by case:

- **Planning an unbuilt phase.** It goes in that phase's file under `plan/`, which opens by saying it
  is a plan and may diverge. Arguing for an answer there is what a plan *is*. Name the decisions the
  phase will produce, describe them, and leave them unnumbered.
- **A decision something else is planned on top of.** That is the one case for a
  `TO_BE_DECIDED` entry plus its half-numbered slot in `ROADMAP.md`. The slot exists because such a
  decision goes invisible otherwise — which is exactly what happened to A17 and A18, both of which
  stayed unseen while four phases were planned over them.
- **A decision a phase closes by itself, in its own commit.** No entry and no slot until it is closed.
  Nothing is planned on top of it, so it cannot go invisible; the phase file carries the reasoning
  until the phase runs, and the entry is written then, saying what was really chosen.

The test to apply before adding an entry: *has the work that settles this actually been done?* If the
answer is "no, but I am confident", the entry is premature. Confidence is not a decision, and this
file's most useful entries are precisely the ones where the recommendation was overturned.

### Regenerating the recordings without losing the signal

Almost every phase regenerates the recordings, and that is the moment the project's most valuable
test risks becoming a ritual. The header of every `.hashes` states the rule — *if it changes without
the balancing having changed, a source of non-determinism has been introduced: stop and find it, do
not regenerate* — but applying it takes a protocol:

1. **Green before you start.** `cargo xtask regen-expected --check` has to be green *before* you
   touch the code. If it is not, the tree is already dirty for other reasons and the signal is lost.
2. **Exactly the files you expected.** After the change, `--check` has to list the files the phase
   declares it regenerates, not one more. A `.ron` that changes in a phase that does not touch the
   header is already the clue.
3. **Idempotence.** `regen-expected` twice in a row: the second has to say "nothing to do". It is
   where non-determinism within a process shows up first.
4. **A separate process.** `cargo test -p sim-replay` catches what point 3 cannot: memory addresses,
   `RandomState`, the iteration order of hash collections.
5. **Look at the first diverging tick** in the `.hashes` diff. It is the check nobody does and it is
   worth more than the other four: if the new mechanic cannot act before tick 60 and the diff starts
   at 30, the cause is something else and has to be found before committing. The recording's textual
   format exists for this.
6. **One reason to regenerate per commit.** A commit that regenerates the recordings and changes two
   mechanics is no longer diffable.

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
- RNGs **separated per kind**. That way adding a feature does not knock the existing sequences out of
  phase and does not invalidate every recorded replay. The kinds that exist, and the fact that their
  declaration order is frozen, are in [ARCHITECTURE.md](ARCHITECTURE.md).
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

## The workspace, the state and the tick

**These live in [ARCHITECTURE.md](ARCHITECTURE.md), which is the only file that describes them.**
Read it before writing code. It is kept in the present tense and is updated in the same commit as the
code it describes; this file used to carry copies of all three, and every copy went stale.

What binds regardless:

- Dependencies always point towards `sim-core`, never the other way. If `game-bevy` shows up among
  another crate's dependencies, it is an architectural mistake.
- `Tile` has to stay small: `u16` indices, no pointers, no `Option<Box<...>>`. Forty thousand tiles
  have to stay in cache as much as possible.
- The order of the tick is **game semantics**, not an implementation detail. Do not reorder without
  regenerating the recorded replays and documenting the reason. The same is true of step 6's internal
  order and of the demographic draw order.
- Step 3 is the hot path. Implementing it naively is fine; the `DirtyFlags` are not optional, because
  retrofitting them later is painful.
- Unit of time: **1 tick = 1 game day**, the month is a fixed multiple. Scenario objectives are
  expressed in months and years.

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
(`NoFlatSpaceNear`, `NotEnoughMoney { needed, available }`, `RoadNetworkDisconnected`).
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
- **A comment states its rule in full and tags it with the id** (`A12`, `D4`) — the tag is a label on
  a self-contained sentence, never a substitute for one. A reader with no access to the documents
  should still learn the rule. Where an id resolves to a code item, link it there
  (`/// [A13]: crate::data::DifficultyDef`): rustdoc then checks the link for you.
- **Naming — plain words, and which hard words earn their place ([A19](DECISIONS.md)).**
  A hard word earns its place when it is the domain's own word, and then it is defined in
  [GLOSSARY.md](GLOSSARY.md): you learn it once and it pays you back. `capacity`, `provider`,
  `satisfaction`, `coverage`, `terrain` are of that kind and are staying. A hard word that is merely
  a synonym choice does not earn anything: nobody learns from `InsufficientFunds` what
  `NotEnoughMoney` tells them for free, and `hysteresis` was retired in favour of `gap`.
  The bar is an elementary reading level, in English, for a reader who is not a native speaker.
  **If a word is not plainly elementary and not already in `GLOSSARY.md`, ask before inventing it.**
  This binds the names sketched here but not yet written — `NoFlatSpaceNear`,
  `RoadNetworkDisconnected` and the `Intent` variants are illustrations of the *shape* of a
  descriptive failure, not approved spellings.

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

### How far the tree has got

**[ROADMAP.md](ROADMAP.md) says, and it is the only file that does.** Do not restate the current
milestone here: this section used to, and it was four phases out of date, which is worse than saying
nothing. If you finish a phase, update `ROADMAP.md`.

### Two lessons that outrank any decision taken in the abstract

1. **Measure before you optimise, and measure the thing you are about to change.** Twice now the
   obvious hypothesis about where the cost lay has been wrong, and the measurement said so before it
   got expensive — once for step 3's invalidation ([A11](DECISIONS.md)), once for the multiplier A12
   was counting on as a discount ([A17](DECISIONS.md)). Doing the optimisation in the same phase you
   take the measurement in means not having the *before*.
2. **A balancing number that has to stand in a particular relation with another one is a validation
   check, not a comment.** This came out of coverage on a "first capacity taken, first served" basis
   making hunger an **absorbing** state ([A5](DECISIONS.md)). The fix was not counting capacity in
   another unit — that restates the same constraint — but making a producer's capacity consistent
   with what its output sustains, from which the invariant *a house covered by food always eats*
   follows. Generalise it: if you find yourself writing a comment explaining why two numbers must
   agree, write a check instead.

The same rule now covers the documents. If a document has to agree with the code, that agreement is a
check in `cargo xtask doc-check` — not a promise to remember.

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
- **Do not cite a document path from code.** Cite the stable id — `A12`, `D4` — and state the rule in
  full where you cite it. Paths rot when files move; ids do not.
- **Do not rewrite a file in `plan/`** to match what the code does now. Append a dated amendment.
- **Do not restate the current state of the tree** anywhere except `ROADMAP.md`, and do not write a
  balancing value into `RULES.md`.
