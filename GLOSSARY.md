# Glossary

This file exists for one reason: so that no word in the code sends you to a dictionary.

Four sections:

1. **[Words worth a definition](#words-worth-a-definition)** — the words used in identifiers that
   are not everyday vocabulary, each with a plain explanation.
2. **[Words deliberately avoided](#words-deliberately-avoided)** — the common jargon this repository
   does *not* use, and the plain word it uses instead.
3. **[Retired words](#retired-words)** — words this repository used to use. They survive in older
   documents under `plan/`, so they are still explained here.
4. **[Frozen strings and values](#frozen-strings-and-values)** — what is load-bearing, and why
   touching it would break something.

The rule that decides which words get in at all is **A19** in
[plan/open-decisions.md](plan/open-decisions.md): a hard word earns its place when it is the domain's
own word, and then it is defined here. A hard word that is merely a synonym choice does not.

---

## Words worth a definition

The rule is **plain words over jargon**.

| Word | What it means here |
|---|---|
| **treasury** | The city's money — the single pot the player spends from. `Economy.treasury`. |
| **residents** | The people living in *one house*. The city-wide total is `population`. |
| **range** | How far a well or a farm reaches, counted in tiles **walked along roads**, never in a straight line. One value per level (`range_per_level`), read with the building's own level. |
| **capacity** | A ceiling on residents, and it names **two** of them. A provider's capacity is how many residents it can serve at once (`capacity_per_level`) — not how many houses. A house's capacity is how many residents it can hold at its level (`max_residents`). Which one is meant is always clear from what it belongs to, and no third meaning exists. |
| **provider** | A building that supplies a service to nearby houses — the well provides water, the farm provides food. |
| **stock** | How much food a farm currently holds. A granary that fills up and stops. |
| **granary** | A store of food that has a lid: once it is full, what would go in is lost rather than queued. It is where `stock` lives and what `max_stock` bounds. |
| **entrance** | A road tile touching a building. A building with no entrance is cut off from the network and serves nobody. |
| **occupant** | What is standing on a tile — a building or a house. Roads are not occupants: a tile carries its road in a flag, so a road and an occupant are counted separately. |
| **walkable** | Whether a road can be laid on this terrain. Rock is walkable, water is not. |
| **buildable** | Whether a building can be placed on this terrain. |
| **sustainable** / **unsustainable** | Whether a farm's declared capacity matches what it actually grows. An unsustainable capacity means houses that stay hungry forever, so it is a validation error. |
| **conservation** | Nothing appears and nothing vanishes without being counted: food produced equals food eaten plus food stored plus food explicitly written off. It is checked as an exact equality, not as "close enough". |
| **thousandths** | How fractional amounts are held without floats: `Milli(i32)` counts thousandths of a unit, so 1500 means one and a half. Floats are banned in the state (D4) because they make two machines disagree; thousandths never do. |
| **checkpoint** | A save point in a recorded game: every 30 ticks, the state's hash is written down so a later run can be compared against it. |
| **invariant** | Something that has to be true after *every* possible sequence of moves — "population is never negative". If one breaks, there is a real bug. |
| **absorbing state** | A situation you can get into and never get out of, no matter what you build. Hunger used to be one; it was a bug, and fixing it is written up in A5. |
| **dirty (flag)** | A note saying "this has to be recomputed next tick". Not "unclean" — the opposite of "up to date". |
| **scratch (buffer)** | A piece of scrap memory reused between calls so it does not have to be allocated again each time. |
| **seed** | The one number a whole game is generated from. Same seed + same commands ⇒ exactly the same game, down to the bit. |
| **salt** | A fixed piece of text mixed into a seed so two different systems drawing random numbers never produce the same sequence. |
| **hot path** | The code that runs most often and therefore costs the most. Here it is step 3 of the tick. |
| **satisfaction** | How long a service has been reaching a house, not how much of it arrives. It climbs while the service is there and falls when it is not. |
| **mood** | The coarse band a house's satisfaction falls into — awful, unhappy, happy, great. It is what the renderer draws, and it is all the renderer is sent: the event carries the band, never the number, because one event per house per tick is what that boundary exists to avoid. The number itself is `House::satisfaction`, and `xtask` does read it, to average it into the `sat.` column. |
| **gap** | Two thresholds deliberately kept apart — one to go up a level, a lower one to come back down — so a house sitting on the boundary does not flip every review. A level whose two thresholds meet is a validation error (`Inconsistency::NoGap`), because the city would flicker. |
| **level** | How far a house has come up: a better house, holding more people and demanding more services. Buildings have levels too, on the same `Level` type, which counts from 1 the way the tables do and indexes them from 0 — so no caller ever writes the `±1` itself. |
| **review** | The monthly moment when houses are looked at and decide whether to go up a level or come down. Between two reviews a level cannot change. |
| **decay** | A house coming *down* a level because a service it depends on has been missing too long. The opposite of levelling up. |
| **eviction** | Residents a house has to send away because decay shrank it below the number living there. |
| **inconsistency** | A relation between two tables that does not hold — a level nobody can ever reach, a house born beyond its own capacity. Reported by `DataSet::inconsistencies` and refused at load time. |

Two words are defined here but do not exist in the code yet, because the phases that introduce them
are not written: **jitter** (a small random wobble added to a rate, so two games with different seeds
do not play out identically) and **attractiveness** (one number saying how much the city draws new
people in). They arrive with phases 14–16. Until then, finding them in `plan/` and not in a `.rs` is
expected, not a stale entry.

---

## Words deliberately avoided

If you read about Rust or game development elsewhere you will meet these. They mean the same as the
plain word this repository uses instead.

| Common jargon | Used here instead | Why |
|---|---|---|
| **footprint** | `size` | The rectangle of tiles a building sits on. `size: (2, 2)` is a 2×2 farm. |
| **golden** (golden test, golden file) | `expected` | A recorded game plus its committed hashes, re-run every test to prove nothing drifted. `tests/expected/`, `cargo xtask regen-expected`. |
| **ledger** | `FoodTotals`, `PopulationTotals` | Running totals kept only so conservation can be checked as an exact equality. |
| **canonical** (canonical hash) | *the state hash*, *the dataset hash*, *a fixed order* | It only ever meant "the one true hash, computed in a fixed order", which the surrounding comments already explain. |
| **hysteresis** | `gap` | See the entry above. The idea is load-bearing; the word was not. |
| **domain** (RNG domain, domain separation) | `RngKind`, *hash prefix* | One meant "which system is drawing", the other "keep this hash apart from that one". Two plain phrases beat one borrowed word doing both jobs. |

---

## Retired words

These were used in this repository and are not any more. They still appear in the phase documents
under `plan/`, which are a record of decisions as they were taken and are not rewritten afterwards.

| Retired word | What it meant | Used now |
|---|---|---|
| **rung** | One entry of the table of house levels. | `level` |
| **ladder** | The table of house levels itself. | `rules.house_levels`, iterated with `Rules::all_levels` |
| **hysteresis** | The deliberate distance between the level-up and decay thresholds. | `gap` |
| **canary** (`field_canary`) | The exhaustive `let World { .. }` that stops compiling when a field is added. | `World::every_field` |
| **perturbation** | A test's deliberate change to one field of the state. | *change* |
| **desperate** / **thriving** | The bottom and top bands of a house's mood. | `Mood::Awful`, `Mood::Great` |

The phase documents also name a few types by their old spelling — `RngDomain` (17 times),
`InsufficientFunds`, `OutOfBounds`, `UnsuitableTerrain`. A19 lists every rename with its replacement,
and nothing but the name changed in any of them.

---

## Frozen strings and values

These are load-bearing. Renaming the constant that holds one is fine; changing the **value** rewrites
history and breaks the recorded replays.

| What | Value | Why it is frozen |
|---|---|---|
| `RngKind::salt()` | `"brando/rng/v1/events"`, `.../migration`, `.../production` | Each random-number stream is seeded from its kind's *name*. Change the text and every recorded game shifts. |
| Declaration order of `RngKind` | `Events, Migration, Production` | `sim-replay/src/hash.rs` iterates `RngKind::ALL` and hashes each stream's position. Reordering silently changes every state hash — the same hazard as `Terrain` below. |
| The dataset hash prefix | `b"brando/dataset/v1"` | Keeps this hash apart from every other hash in the project. |
| The state hash prefix | `b"brando/world/v1"` | Same, for the state. |
| Declaration order of `Terrain` | `Plain, Water, Rock` | The hash stores the position, not the name. Reordering silently changes every hash. |
| Declaration order of `ServiceKind` and `ServiceKind::index()` | `Water = 0, Food = 1` | Same. |
| Building ids in `buildings.ron` | `"house"`, `"well"`, `"farm"` | `sim-core/src/data_hash.rs` feeds each id string into blake3, so renaming one moves every recorded hash. |
| Difficulty ids in `difficulty.ron` | `"easy"`, `"normal"`, `"hard"` | Hashed the same way as the building ids, and the recording's header stores the id by name. |
| `FORMAT_VERSION` | `2` | The shape of a save file. It went to `2` in phase 11, when the header started carrying the difficulty. It goes up when the shape changes, not when a name does. |
| `CHECKPOINT_EVERY` | `30` | How far apart the committed checkpoints are. |
| Benchmark labels `A.`–`G.` | — | `plan/09-invariants-closeout.md` records timings against those letters. `plan/18-invariants-closeout-m1.md` proposes three more, `H`–`J`, which `bench.rs` does not have yet. |
| Design codes `D1`–`D7` | — | Defined in `CLAUDE.md` and cited 48 times from the `.rs` sources. |
| Decision codes `A1`–`A19`, phase numbers `00`–`18` | — | The `A` codes alone are cited 69 times from the `.rs` sources, before counting `plan/`. |
