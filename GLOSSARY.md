# Glossary

This file exists for one reason: so that no word in the code sends you to a dictionary.

Sections:

1. **[Words worth a definition](#words-worth-a-definition)** — the words used in identifiers that
   are not everyday vocabulary, each with a plain explanation.
2. **[Words deliberately avoided](#words-deliberately-avoided)** — the common jargon this repository
   does *not* use, and the plain word it uses instead.
3. **[Retired words](#retired-words)** — words this repository used to use. They survive in older
   documents under `plan/`, so they are still explained here.
4. **[Frozen strings and values](#frozen-strings-and-values)** — what is load-bearing, and why
   touching it would break something.

The rule that decides which words get in at all is the naming rule in
[CLAUDE.md](CLAUDE.md): a hard word earns its place when it is the domain's
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
| **role** | What part a building plays: a **house**, which needs services, or a **provider**, which supplies one. Every building declares its role in the table, and has exactly one. It is not the same as its **kind**, which is *which* building it is — `house`, `well`, `farm` — and stays data the tables are free to add to. Producing is neither: the farm is a provider that also produces, so what it grows is declared beside its role. |
| **stock** | How much food a farm currently holds. A granary that fills up and stops. |
| **granary** | A store of food that has a lid: once it is full, what would go in is lost rather than queued. It is where `stock` lives and what `max_stock` bounds. |
| **entrance** | A road tile touching a building. A building with no entrance is cut off from the network and serves nobody. |
| **occupant** | What is standing on a tile — a building or a house. Roads are not occupants: a tile carries its road in a flag, so a road and an occupant are counted separately. |
| **walkable** | Whether a road can be laid on this terrain. Rock is walkable, water is not. It is a fact about the terrain, fixed in the code, and not a number in a table. |
| **buildable** | Whether a building can be placed on this terrain. Fixed in the code, like walkability: only plain ground is buildable. |
| **sustainable** / **unsustainable** | Whether a farm's declared capacity matches what it actually grows. An unsustainable capacity means houses that stay hungry forever, so it is a validation error. |
| **conservation** | Nothing appears and nothing vanishes without being counted: food produced equals food eaten plus food stored plus food explicitly written off. It is checked as an exact equality, not as "close enough". |
| **thousandths** | How fractional amounts are held without floats: `Milli(i32)` counts thousandths of a unit, so 1500 means one and a half. Floats are banned in the state (D4) because they make two machines disagree; thousandths never do. |
| **checkpoint** | A save point in a recorded game: every 30 ticks, the state's hash is written down so a later run can be compared against it. |
| **invariant** | Something that has to be true after *every* possible sequence of moves — "population is never negative". If one breaks, there is a real bug. |
| **absorbing state** | A situation you can get into and never get out of, no matter what you build. Hunger used to be one; it was a bug, and it was fixed by making a producer's declared capacity consistent with what its output sustains. |
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
| **demographics** | Births and deaths, counted for the city as a whole rather than person by person. Nobody has an age or a name: a rate says how many are born and how many die, and which house it happens in is drawn. |
| **flow** | One of the four ways the population moves — births, deaths, immigration, emigration. Each keeps its own running total, and the four together are what makes conservation an exact equality. |
| **jitter** | A small random wobble added to a rate, so two games with different seeds do not play out identically. Symmetric, so it does not move the average. |
| **eligible** | Whoever a rate is counted against. Births are counted against the residents of houses with room to spare and a satisfaction above the threshold — *not* against the whole population, which is what makes a full city stop growing on its own. |
| **attractiveness** | One number saying how much the city draws new people in. It is what decides whether anybody moves there, and it is read for the whole city rather than for one house. |
| **ground height** | How high the ground is on one tile, counted in **steps** rather than in any unit of length. The grid's own `height` is its size in tiles, which is a different thing — hence the longer name. A step is a unit of gameplay: the renderer multiplies it by a scale of its own to get pixels. |
| **slope** | The difference between the highest and the lowest ground height across the tiles a building sits on. Zero means level ground. It is worked out when it is needed and never stored, so it cannot disagree with the heights it comes from. |
| **task** | One document under `plan/`, and one piece of work: a single goal, closed by a command that answers yes or no. It is the word for the *document*, not a second word for **phase** — a phase is one kind a task can be, alongside an open question and the constitution. |
| **front matter** | The block of `key: value` lines between two `---` lines at the very top of a task, holding its id, kind, status and dates. It is the machine-readable half of a task; everything below it is prose for the reader. |
| **constitution** | The rules `D1`–`D7`, settled before the first line of code and binding on all of it. They are the only rules that live in a document rather than in the comment on the code they bind, because they bind code that does not exist yet. |
| **calendar** | The fixed shape of game time: how many ticks make a month, and how many months make a year. Not a balancing knob — nobody tunes what a month **is** the way the rest of the tables get tuned, so it is a Rust constant (`Calendar`) rather than a row in a table. |

Some words above are defined here but do not exist in the code yet, because the phases that introduce
them are not written: **attractiveness** arrives with phase 15, **ground height** and **slope** with
phase 19. Until then, finding one in `plan/` and not in a `.rs` is expected, not a stale entry.
**jitter** was in this note until phase 14, and is out of it because it is in `demographics.rs`.

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

The phase documents also name a few types by their old spelling — `RngDomain`, `InsufficientFunds`,
`OutOfBounds`, `UnsuitableTerrain`. The vocabulary review of 2026-08-12 renamed them and nothing but
the name changed in any of them. Those documents are frozen records and are not corrected, so the old
spellings stay: where one would mislead, an amendment says so under the sentence that uses it.

---

## Frozen strings and values

These are load-bearing. Renaming the constant that holds one is fine; changing the **value** rewrites
history and breaks the recorded replays.

| What | Value | Why it is frozen |
|---|---|---|
| `RngKind::salt()` | `"brando/rng/v1/events"`, `.../migration`, `.../production`, `.../demographics` | Each random-number stream is seeded from its kind's *name*. Change the text and every recorded game shifts. |
| Declaration order of `RngKind` | `Events, Migration, Production, Demographics` | `sim-replay/src/hash.rs` iterates `RngKind::ALL` and hashes each stream's position. Reordering silently changes every state hash — the same hazard as `Terrain` below. |
| The dataset hash prefix | `b"brando/dataset/v1"` | Keeps this hash apart from every other hash in the project. |
| The state hash prefix | `b"brando/world/v1"` | Same, for the state. |
| Declaration order of `Flow` and `Flow::index()` | `Births, Deaths, Immigration, Emigration` | `Demographics::remainder` is indexed by it and the whole array goes into the state hash, so reordering silently reassigns every accumulated fraction to a different flow. The same hazard as `RngKind` above. |
| Declaration order of `Terrain` | `Plain, Water, Rock` | The hash stores the position, not the name. Reordering silently changes every hash. |
| Declaration order of `ServiceKind` and `ServiceKind::index()` | `Water = 0, Food = 1` | Same. |
| Building ids in `buildings.ron` | `"house"`, `"well"`, `"farm"` | `sim-core/src/data_hash.rs` feeds each id string into blake3, so renaming one moves every recorded hash. |
| Difficulty ids in `difficulty.ron` | `"easy"`, `"normal"`, `"hard"` | Hashed the same way as the building ids, and the recording's header stores the id by name. |
| `FORMAT_VERSION` | `2` | The shape of a save file. It went to `2` in phase 11, when the header started carrying the difficulty. It goes up when the shape changes, not when a name does. |
| `CHECKPOINT_EVERY` | `30` | How far apart the committed checkpoints are. |
| `Calendar::TICKS_PER_MONTH`, `Calendar::MONTHS_PER_YEAR` | `30`, `12` | Not hashed directly — they left `Rules`/`DataSet::hash` for a Rust constant (D6, amended) — but changing either still moves every recorded replay, through the tick's own behaviour (when step 6.2's review fires) rather than through being fed into a hash byte for byte. `regen-expected --check` catches a change here exactly as it catches a reordered tick step. |
| Benchmark labels `A.`–`G.` | — | `plan/09-invariants-closeout.md` records the end-of-M0 timings against those letters, and `xtask/src/bench.rs` carries them as its reference figures. `H`–`J` were added by phase 14 and are measured with `--zero-demographics`. |
| Design codes `D1`–`D7` | — | The constitution: defined in the one task whose `kind` is `constitution`, and cited by id from the `.rs` sources — **never by path**, so that moving that file breaks nothing. `doc-check` enforces both halves, and refuses any other id letter: there used to be an `A<n>` namespace for implementation decisions, and it was removed because every rule in it was already stated in full at the place it binds. |
| Task ids `NN` and half numbers `NN.N`, `NN.N.N` | — | A task's id is the number its file name opens with, stated again in its front matter and checked against the name. A half number is work falling between two whole phases — `doc-check` compares the parts one at a time, so `14.5.5` sits between `14.5` and `14.6`. The first part is written with two digits and every part after it with one, so that a listing, which compares bytes rather than numbers, runs in the same order: the slot above `14.9` is `14.9.5` and never `14.10`. How far the numbers have got is `ROADMAP.md`'s answer and no other file's. |
