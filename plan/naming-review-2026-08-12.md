# Vocabulary review — brando, 2026-08-12

> **Status: closed.**
>
> A dated vocabulary audit. It produced [A19](../DECISIONS.md) and the naming rule now in
> `CLAUDE.md`.

The decision this produced is [A19](../DECISIONS.md#a19--plain-words-and-which-hard-words-earn-their-place),
and the rule it produced lives in `CLAUDE.md`'s code conventions. Neither of those is repeated here.
What is here is the **inventory**: every word that was looked at, what was decided, and — the part
worth keeping — the work-list the review deliberately did not do.

## Baseline and method

Tree at `f6fb03d` (phase 13 plus the bug hunt that follows it). Whole suite green,
`regen-expected --check` green before anything was touched, both benchmark state hashes recorded
first: `28e0f9cb2d467289` (mid game) and `f5bbb2e83aedeaa9` (late game).

The method was an inventory of every identifier, data key, error message and doc comment in the four
crates, scored against one question: would a ten-year-old reading English as a second language know
this word? That produced about forty candidates. Every one was then put to the same test — is this the
domain's own word, or is it a synonym choice? — which is the rule A19 states.

Two things found on the way that were not about vocabulary at all are recorded in A19: three
`GLOSSARY.md` rows that had become false, and two frozen values that were load-bearing and unlisted.

## The verdict, word by word

Counts are occurrences in the four crates before the pass.

### Kept — the domain's own words

| Word | Uses | Why it stays |
|---|---|---|
| `satisfaction` | 149 | the mechanic's name; `comfort` and `happiness` both collide with `Mood` |
| `coverage` | 144 | D2 is built on the phrase "aggregate coverage" |
| `capacity` | 127 | common enough (a bus has one); the glossary row now admits both its senses |
| `terrain` | 126 | what every map-based game and map format calls it |
| `provider` | 94 | one word for "the building that supplies the service" |
| `Milli` | 74 | "milli" is a prefix a child already owns, from millimetres |
| `Inconsistency` | 59 | names a relation between two tables, which `Mismatch` blurs |
| `occupant` | 57 | precise, and now defined in the glossary rather than assumed |
| `treasury` | 35 | and it agrees with `Rules::starting_treasury` in the table |
| `ComponentId` | 23 | `IslandId` was more vivid and less true — components are not islands |
| `decay`, `review`, `threshold`, `stock`, `range`, `entrance` | — | all glossary-defined, all the domain's |
| `sustainable` | 15 | carries the whole of A5 in one word |
| `Demolish` | — | what every city builder calls the bulldozer |
| `residents` | — | distinguishes a house's people from `population`, the city's |
| `propagate_coverage`, `invalidate_coverage`, `evicted`, `HouseEvolved`/`HouseDegraded`, `TilePos::manhattan`, `Visited::epochs` | — | kept on the same rule |

### Retired — a plainer word said the same thing

| From | To | Sites |
|---|---|---|
| `rung` / `ladder` | `level`, and `rules.house_levels` iterated with `Rules::all_levels` | ~120 |
| `hysteresis` | `gap`; `Inconsistency::NoHysteresis` → `NoGap` | 17 |
| `InsufficientFunds` | `NotEnoughMoney` (+ the message) | 11 |
| `UnsuitableTerrain` | `WrongTerrain` (+ the message) | 8 |
| `OutOfBounds` | `OutsideMap` | 10 |
| `Mood::Desperate` / `Thriving` | `Mood::Awful` / `Great` | 46 |
| `RngDomain` | `RngKind`; the `DOMAIN` hash prefixes → `PREFIX` | 60 |
| `field_canary` | `every_field` | 5 |
| `Perturbation` (test alias) | `Change` | 8 |
| `PROFILES` / `fn profile` in `bench.rs` | `PRESETS` / `fn preset` | 4 |
| `canonical` | *a fixed order* — the one leak against the "words avoided" table | 1 |

`rung`/`ladder` is the one to read, and it is in A19: the metaphor was deleted rather than translated,
because the table it named is already `house_levels` and the type is already `Level`.

Three renames were forced off their planned spelling by collisions, which is worth knowing before the
next pass invents a name in the abstract:

- `Rules::house_ladder()` → `all_levels()`, **not** `house_levels()`, which would shadow the field.
- `validate.rs`'s `fn rung(level, field)` → `level_path`, because `fn level(level: Level, …)` reads as
  a mistake.
- `satisfaction.rs`'s test builder `ladder()` → `two_levels()`, because that module already had a
  `fn rules()`.

## What was left out — the work-list for the next pass

Prose was out of scope except where a retired word survived in it. **This is the larger half of the
problem**: the hard words a reader actually meets are mostly here, not in the identifiers. Two of them
are in strings the LLM and the player will read.

| Where | Words still there |
|---|---|
| messages, `sim-core/src/data.rs` | `CapacityBeyondOutput` prints "the output **sustains**"; `UnreachableThreshold` prints "**unreachable** by construction" |
| messages, `sim-data/src/validate.rs` | "strictly **ascending**", "a **producer** must have max_stock > 0" |
| `sim-core/src/levels.rs` | `monotone`, `cadence`, `pathology`, `pedantry`, `demographics`, `convergence`, `predicates` |
| `sim-core/src/coverage.rs` | `materialised`, `equidistant`, `contention`/`contested`, `prefix sum`, `counting sort`, `naivety` |
| `sim-core/src/grid.rs`, `world.rs` | `orthogonal`, `wraparound`, `sentinel`, `Euclidean`, `hygiene`, `Zeus rule` (an unexplained proper noun) |
| `sim-core/src/rng.rs` | `entropy`, `XOF mode`, `divergent generators` |
| `xtask/src/bench.rs` | `lattice`, `pitch`, `Bresenham`, `preemption`, `tripwire`, `marginal cost`, `synthetic` |
| everywhere | `BFS` — an acronym that is never expanded once |

Two smaller things also left standing, both deliberate:

1. **The RON keys and the `Raw*` structs that mirror them.** Hash-safe to rename (`data_hash.rs`
   feeds values and id strings, never key names), and declined so that the files and the code keep
   reading alike. The seam this leaves is that a future rename has to be spelled the same on both
   sides or they drift apart silently.
2. **`plan/` is not rewritten.** The phase documents record decisions as they were taken. `rung`,
   `ladder` and `hysteresis` still appear there, which is why `GLOSSARY.md` now has a **Retired words**
   section: the historical documents stay readable without being edited.

## Verification

The pass is provably semantics-free, and this is the check to repeat on any future naming pass:

- `cargo xtask regen-expected --check` → `recordings up to date, nothing to do`, run **without** ever
  calling `regen-expected`.
- `git diff sim-replay/tests/expected/` → empty. Both recordings byte-identical, `.hashes` included.
- `cargo run --release -p xtask -- bench` → both state hashes unchanged from the baseline above.
- The two `insta` snapshots in `sim-data/tests/snapshots/` unchanged, which is what says no message
  moved beyond the three that were meant to.

If a hash moves during a rename, the rename was not a rename. Names are never hashed: `data_hash.rs`
feeds field values and id strings, and `hash.rs` destructures every struct but hashes only contents.

One incidental finding: **`cargo fmt --all --check` was already red at `f6fb03d`**, at
`sim-core/src/rng.rs:347` and `sim-core/tests/coverage.rs:258` — almost certainly a rustfmt version
newer than the one the last commit was made with. Both are fixed in this batch, so two hunks of its
diff belong to that and not to the rename.
