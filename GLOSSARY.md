# Glossary

This file exists for one reason: so that no word in the code sends you to a dictionary.

Three sections:

1. **[Words worth a definition](#words-worth-a-definition)** — the words used in identifiers that
   are not everyday vocabulary, each with a plain explanation.
2. **[Words deliberately avoided](#words-deliberately-avoided)** — the common jargon this repository
   does *not* use, and the plain word it uses instead.
3. **[Frozen strings and values](#frozen-strings-and-values)** — what is load-bearing, and why
   touching it would break something.

---

## Words worth a definition

The rule is **plain words over jargon**.

| Word | What it means here |
|---|---|
| **treasury** | The city's money — the single pot the player spends from. `Economy.treasury`. |
| **residents** | The people living in *one house*. The city-wide total is `population`. |
| **range** | How far a well or a farm reaches, counted in tiles **walked along roads**, never in a straight line. |
| **capacity** | How many residents a provider can serve at once. Not how many houses. |
| **provider** | A building that supplies a service to nearby houses — the well provides water, the farm provides food. |
| **stock** | How much food a farm currently holds. A granary that fills up and stops. |
| **entrance** | A road tile touching a building. A building with no entrance is cut off from the network and serves nobody. |
| **walkable** | Whether a road can be laid on this terrain. Rock is walkable, water is not. |
| **buildable** | Whether a building can be placed on this terrain. |
| **sustainable** / **unsustainable** | Whether a farm's declared capacity matches what it actually grows. An unsustainable capacity means houses that stay hungry forever, so it is a validation error. |
| **checkpoint** | A save point in a recorded game: every 30 ticks, the state's hash is written down so a later run can be compared against it. |
| **invariant** | Something that has to be true after *every* possible sequence of moves — "population is never negative". If one breaks, there is a real bug. |
| **absorbing state** | A situation you can get into and never get out of, no matter what you build. Hunger used to be one; it was a bug, and fixing it is written up in A5. |
| **dirty (flag)** | A note saying "this has to be recomputed next tick". Not "unclean" — the opposite of "up to date". |
| **scratch (buffer)** | A piece of scrap memory reused between calls so it does not have to be allocated again each time. |
| **seed** | The one number a whole game is generated from. Same seed + same commands ⇒ exactly the same game, down to the bit. |
| **salt** | A fixed piece of text mixed into a seed so two different systems drawing random numbers never produce the same sequence. |
| **hot path** | The code that runs most often and therefore costs the most. Here it is step 3 of the tick. |
| **jitter** | A small random wobble added to a rate, so two games with different seeds do not play out identically. |
| **hysteresis** | Deliberately using two different thresholds — one to go up, a lower one to come down — so a value sitting on the boundary does not flicker back and forth. |
| **attractiveness** | One number saying how much the city draws new people in. |
| **satisfaction** | How long a service has been reaching a house, not how much of it arrives. It climbs while the service is there and falls when it is not. |
| **mood** | The coarse band a house's satisfaction falls into — desperate, unhappy, happy, thriving. It is what the renderer draws; the number itself never leaves the core. |
| **level** / **rung** / **ladder** | A house's level is how far it has come up: a better house, holding more people and demanding more services. The **ladder** is the table of levels, and one entry in it is a **rung**. In the code it is the `Level` type, which counts from 1 the way the tables do and indexes them from 0 — so no caller ever writes the `±1` itself. |
| **review** | The monthly moment when houses are looked at and decide whether to go up a level or come down. Between two reviews a level cannot change. |
| **decay** | A house coming *down* a level because a service it depends on has been missing too long. The opposite of levelling up. |
| **eviction** | Residents a house has to send away because decay shrank it below the number living there. |
| **inconsistency** | A relation between two tables that does not hold — a level nobody can ever reach, a house born beyond its own capacity. Reported by `DataSet::inconsistencies` and refused at load time. |

---

## Words deliberately avoided

If you read about Rust or game development elsewhere you will meet these. They mean the same as the
plain word this repository uses instead.

| Common jargon | Used here instead | Why |
|---|---|---|
| **footprint** | `size` | The rectangle of tiles a building sits on. `size: (2, 2)` is a 2×2 farm. |
| **golden** (golden test, golden file) | `expected` | A recorded game plus its committed hashes, re-run every test to prove nothing drifted. `tests/expected/`, `cargo xtask regen-expected`. |
| **ledger** | `FoodTotals`, `PopulationTotals` | Running totals kept only so conservation can be checked as an exact equality. |
| **canonical** (canonical hash) | *the state hash*, *the dataset hash* | It only ever meant "the one true hash, computed in a fixed order", which the surrounding comments already explain. |

---

## Frozen strings and values

These are load-bearing. Renaming the constant that holds one is fine; changing the **value** rewrites
history and breaks the recorded replays.

| What | Value | Why it is frozen |
|---|---|---|
| `RngDomain::salt()` | `"brando/rng/v1/events"`, `.../migration`, `.../production` | Each random-number stream is seeded from its domain's *name*. Change the text and every recorded game shifts. |
| The dataset hash prefix | `b"brando/dataset/v1"` | Keeps this hash apart from every other hash in the project. |
| The state hash prefix | `b"brando/world/v1"` | Same, for the state. |
| Declaration order of `Terrain` | `Plain, Water, Rock` | The hash stores the position, not the name. Reordering silently changes every hash. |
| Declaration order of `ServiceKind` and `ServiceKind::index()` | `Water = 0, Food = 1` | Same. |
| Building ids in `buildings.ron` | `"house"`, `"well"`, `"farm"` | `sim-core/src/data_hash.rs` feeds each id string into blake3, so renaming one moves every recorded hash. |
| `FORMAT_VERSION` | `1` | The shape of a save file. It goes up when the shape changes, not when a name does. |
| `CHECKPOINT_EVERY` | `30` | How far apart the committed checkpoints are. |
| Benchmark labels `A.`–`G.` | — | `plan/09-invariants-closeout.md` and `plan/18` record timings against those letters. |
| Decision codes `A1`–`A18`, phase numbers `00`–`18` | — | Referenced from about forty places. |
