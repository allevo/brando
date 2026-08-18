# Roadmap

**What is done, what is next, and what is not decided.** This is the only file that states how far the
tree has got — if another document tells you what is built, it is wrong and should point here.

> **Implemented through phase 14.9.9.** M0 is complete; M1 is in progress.

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

## To do — the rest of M1

| # | What | Verified by | Blocks |
|---|---|---|---|
| 14.9.9.1 | [How well served the city is becomes a knob](plan/14.9.9.1-how-well-served-the-city-is-becomes-a-knob.md) | `--places 100` prints today's state hash unchanged, and 70 and 150 move the served share the way they say | 18.5 |
| **14.6** | **Open question — [A house is covered by services its level does not require](plan/14.6-a-house-is-covered-by-services-its-level-does-not-require.md):** is a house covered by, and fed by, a service its own level does not require? | an answer written into this task, plus a test that changes its outcome rather than breaking | phase 15 |
| 15 | [Immigration and emigration](plan/15-migration.md) | two cities identical except for their coverage receive different flows, and the coverage↔population loop **damps** | |
| 16 | [Treasury and taxes](plan/16-treasury-taxes.md) | the treasury invariant stays an **exact equality** with the income in it | |
| 17 | [`sim-scenario`](plan/17-sim-scenario.md) | "500 residents within 5 years" declares itself complete on the right tick, and not before | |
| 18 | [Invariants and closing M1](plan/18-invariants-closeout-m1.md) | the new invariants are green property tests, and the cost of the coverage chasing the population is measured and attributed | |
| **18.5** | **Open question — [The per-tick recomputation cost](plan/18.5-the-per-tick-recomputation-cost.md):** what to do about the per-tick recomputation cost | an answer written into this task, and a measurement that survives it | M2 |

## What next

Beyond M1, and none of it planned in detail: a milestone is planned only once the one before it has
closed, so the balancing is decided after watching real ticks run rather than before.

- **[Terrain: relief and cost](plan/19-terrain.md)** — the map stops being flat: tiles gain a ground
  height, a building pays to flatten what it stands on and is refused on a cliff, and a map becomes a
  file with a hash. **Comes before M2**, because the renderer has to be built against the map model it
  will really draw.
- **[Things on the ground, and the cost of clearing them](plan/20-ground-clearing.md)** — the other
  half of *the site you chose is part of what the building costs*. Waits on the terrain.
- **[Bridges](plan/21-bridges.md)** — a bridge makes two bank-side networks one region, and demolishing
  it splits them again. Waits on the terrain.
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
