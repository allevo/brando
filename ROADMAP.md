# Roadmap

**What is done, what is next, and what is not decided.** This is the only file that states how far the
tree has got — if another document tells you what is built, it is wrong and should point here.

> **Implemented through phase 16.5.** M0 is complete; M1 is in progress.

## Done

| # | What | When |
|---|---|---|
| 14.5.5 | [The constitution](plan/14.5.5-the-constitution.md) — D1–D7, settled before the first line of code and binding on all of it | 2026-08-08 |
| 00–09 | [M0 — foundations](plan/00-workspace.md): workspace, core types, RNG, `sim-data`, world and tick and commands, roads, coverage, farm and food, replay hashing, invariants | 2026-08-08 |
| 11 | [Difficulty](plan/11-difficulty.md) | 2026-08-10 |
| 12 | [Satisfaction](plan/12-satisfaction.md) | 2026-08-10 |
| 13 | [House levels](plan/13-house-levels.md) | 2026-08-10 |
| — | Bug hunt — six latent findings; five were bugs and were fixed, the sixth is the open question at 14.5 | 2026-08-11 |
| — | Vocabulary review — renames only, no hash moved; it is where the naming rule in `CLAUDE.md` comes from | 2026-08-12 |
| 14 | [Births and deaths](plan/14-births-deaths.md) | 2026-08-12 |
| 14.4 | [A building says what it is](plan/14.4-building-kind.md) | 2026-08-14 |
| — | `World` stopped being `Clone` in any configuration, and a compile-time check says so | 2026-08-14 |
| 14.7 | [A half number is read, not cut short](plan/14.7-half-numbers.md) | 2026-08-14 |
| — | Decisions review — the register was cut down to the decisions actually taken, one task before it was removed altogether | 2026-08-15 |
| 14.8 | [Every task says what it is](plan/14.8-every-task-says-what-it-is.md) — front matter, checked; the four unnumbered documents leave | 2026-08-15 |
| 14.9 | [The decisions live where they are cited](plan/14.9-decisions-live-where-they-are-cited.md) — the register is gone; every rule is stated where it binds | 2026-08-15 |
| 14.9.5 | [Writing a task is a procedure](plan/14.9.5-writing-a-task-is-a-procedure.md) — the task mechanics leave `CLAUDE.md` for a skill, whose template is checked against the code | 2026-08-15 |
| 14.9.6 | [An id sorts the same way everywhere](plan/14.9.6-an-id-sorts-the-same-way-everywhere.md) — a number after the first is a single digit, so a listing runs the way the numbers do | 2026-08-15 |
| 14.9.7 | [A terrain says what it allows](plan/14.9.7-a-terrain-says-what-it-allows.md) — what may be built on and walked on leaves the tables for the enum; a new terrain kind is now a compile error | 2026-08-16 |
| 14.9.8 | [A tile answers for itself](plan/14.9.8-a-tile-answers-for-itself.md) — a tile exposes no field and `Grid` hands out no mutable one; outside `sim-core` a tile changes through setup or a command, and by no third way | 2026-08-16 |
| 14.5 | [An empty house consumes no capacity](plan/14.5-an-empty-house-consumes-no-capacity.md) — answered: a house with nobody in it is not served, so it gathers no satisfaction and cannot be promoted on one | 2026-08-17 |
| 14.9.9 | [A provider stops looking once it is full](plan/14.9.9-a-provider-stops-looking-once-it-is-full.md) — the walk ends where the capacity does, and step 3 costs half what it did | 2026-08-17 |
| 14.9.9.2 | [A tick cannot be mistaken for any other number](plan/14.9.9.2-a-tick-cannot-be-mistaken-for-any-other-number.md) — the simulation's current tick is a `Tick`, not a bare `u32`; every recorded replay hash comes out byte-identical | 2026-08-18 |
| 14.9.9.3 | [Nobody tunes what a month is](plan/14.9.9.3-nobody-tunes-what-a-month-is.md) — `ticks_per_month`/`months_per_year` leave `rules.ron` for a hardcoded `Calendar`; `D6`'s first amendment | 2026-08-18 |
| 14.6 | [A house is covered by services its level does not require](plan/14.6-a-house-is-covered-by-services-its-level-does-not-require.md) — answered: yes, and by design; stopping it would strand every house on the level it was built at | 2026-08-18 |
| 14.9.9.1 | [How well served the city is becomes a knob](plan/14.9.9.1-how-well-served-the-city-is-becomes-a-knob.md) — `--places` per kind of provider and `--tables <dir>`; the header says how well served the city it built is, and the coverage becomes a curve | 2026-08-18 |
| 14.9.9.4 | [A city is founded at the levels you ask for](plan/14.9.9.4-a-city-is-founded-at-the-levels-you-ask-for.md) — `--houses 1=2000,2=800`; the provider stop turns out never to fire at all in a city of level-3 houses | 2026-08-18 |
| 14.9.9.5 | [The streets stop being perfect](plan/14.9.9.5-the-streets-stop-being-perfect.md) — `--seed <n>` builds a ragged, provably connected map; a tick on one costs 40% more than on the lattice every figure was measured on | 2026-08-18 |
| 15 | [Immigration and emigration](plan/15-migration.md) — the destination is never gated on coverage (slot 14.5's answer generalised), immigration is counted per free place and not per resident, and the coverage↔population loop damps: a `hard` city, born with nobody in it, reaches a living population within five years by migration alone | 2026-08-19 |
| 15.5 | [A full city turns people away, and the player can see it](plan/15.5-a-full-city-turns-people-away-and-the-player-can-see-it.md) — immigration draws on the residents a city already has once it is past a founding threshold, and below it on the free places, so a city born empty still starts; everyone a full city cannot house is counted in `turned_away`, outside the conservation equality because they never became a resident | 2026-08-20 |
| 16 | [Terrain: relief and cost](plan/16-terrain.md) — a building's footprint spans a `slope`, never stored; a step of it beyond zero costs `flatten_cost_per_step`, and past `max_build_slope` the placement is refused with `TooSteep`; a map is loaded from `sim-data/maps/<id>.ron` through the same raw/validate/`Def` pipeline as the balancing tables, refusing a bad row, an unclaimed character, a height out of range or a severed walkable region | 2026-08-21 |
| 16.5 | [A seed draws a map, and a person commits it](plan/16.5-a-seed-draws-a-map-and-a-person-commits-it.md) — `cargo xtask gen-map` draws one in the new `map-gen` crate and `sim-data` writes it, refusing anything the loader would not accept; the format now reads and writes through one owner, and `doc-check` refuses a workspace in which any crate the simulation loads names the generator | 2026-08-21 |

## To do

| # | What | Verified by | Blocks |
|---|---|---|---|
| 17 | [Treasury and taxes](plan/17-treasury-taxes.md) | the treasury invariant stays an **exact equality** with the income in it | |
| 18 | [`sim-scenario`](plan/18-sim-scenario.md) | "500 residents within 5 years" declares itself complete on the right tick, and not before | |
| 19 | [Invariants and closing M1](plan/19-invariants-closeout-m1.md) | the new invariants are green property tests, and the cost of the coverage chasing the population is measured and attributed | |
| **19.5** | **Open question — [The per-tick recomputation cost](plan/19.5-the-per-tick-recomputation-cost.md):** what to do about the per-tick recomputation cost | an answer written into this task, and a measurement that survives it | M2 |

## What next

Beyond M1, and none of it planned in detail: a milestone is planned only once the one before it has
closed, so the balancing is decided after watching real ticks run rather than before.

- **[Things on the ground, and the cost of clearing them](plan/20-ground-clearing.md)** — the other
  half of *the site you chose is part of what the building costs*. Waits on the terrain.
- **[Bridges](plan/21-bridges.md)** — a bridge makes two bank-side networks one region, and demolishing
  it splits them again. Waits on the terrain.
- **[A building gains a level when the player pays for it](plan/22-a-building-gains-a-level-when-the-player-pays-for-it.md)** —
  a provider's level is hashed state that has never moved, and the per-level tables around it have
  never been read past their first entry. This gives a building a way to grow that is not *build
  another one*.
- **[A farm works the land around it](plan/23-a-farm-works-the-land-around-it.md)** — a farm's output
  and the mouths it can feed grow with the land the player attaches to it, and a fourth terrain
  decides what a parcel of it is worth. Waits on the terrain, and on 22.
- **M2 — the two clients, in parallel.** `game-bevy` renders M1's state isometrically with placeholder
  assets, the point being to validate the snapshot/event boundary rather than the graphics. Alongside
  it: a heuristic bot that completes scenario 1, and an evaluator.
- **M3 — depth.** Production chains with real logistics walkers; the `CivilizationRules` trait arrives
  **together with** the second civilisation and not before; an LLM adapter on top of a bot that works.

### How the AI is meant to drive it

Neither milestone is planned in detail, but four constraints on the AI player are settled and shape
both. They live here, and not in a plan file, because the phases that build them are not written.

The loop is: **the LLM produces `Intent`s → the bot compiles them into `Command`s → the core runs →
the evaluator judges the saved state → feedback to the LLM.**

- **The LLM never reasons in absolute coordinates.** It is bad at it and produces unplayable plans. It
  works in intents shaped like `BuildDistrict { kind, near, size }`, `EnsureService { service, area }`,
  `SetupProduction { chain, target_rate }` and `AdjustTax { delta }` — illustrations of the *shape* of
  an intent, not approved spellings. The naming rule in `CLAUDE.md` binds the real names when they
  are written.
- **A failure is descriptive, and the failure is the feedback.** The bot returns a `Vec<Command>` or a
  failure that says what was wrong and with which numbers — how much money was needed and how much
  there was. That message goes back to the LLM and closes the loop.
- **The observation is a summary, never the serialised state.** Aggregate indicators, a list of
  problems ordered by severity, and an ASCII map downsampled to about 40×40 with one symbol per zone.
  Far more effective than any detailed JSON.
- **The evaluator produces a vector of metrics, not a scalar** — mandatory objectives completed,
  optional ones, ticks taken, stability as the variance of the treasury, resilience as collapses in
  population. The scalar is derived afterwards, so the formula can change without redoing the runs.

The AI player only ever plays scenarios, with explicit objectives, and the evaluator scores those
(D7). The determinism log records the primitive `Command`s (D4); the `Intent`s are kept as metadata
for analysis.
