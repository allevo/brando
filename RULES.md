# The rules of the game

**This file answers one question: what does the game actually do?**

It states the rules and the formulas, and it names the parameter that tunes each one. **It contains
no values, deliberately.** Numbers change at every rebalance; the rules they are plugged into change
far more rarely. A rules document that quoted its numbers would be stale the first time you touched a
table — this one stays true, and tells you exactly which knob to reach for.

**The values live in `sim-data/data/*.ron` and nowhere else.** A balancing number appearing in a
`.rs` file is a bug. The tables' own comments carry the derivations — why a rate is the rate it is —
and those comments are worth reading before changing anything.

Where a number must stand in a fixed relation to another number, that relation is **a validation
check and not a comment**: see `DataSet::inconsistencies`. Breaking one is a load error with a full
report, not a subtle misbehaviour at run time.

---

## Configurable — the rules driven by the tables

### Time

One tick is one game day. A month is `ticks_per_month` ticks; a year is `months_per_year` months.
Scenario objectives are expressed in months and years, never in ticks.

### Terrain and roads

Each terrain declares whether it is `buildable`, whether it is `walkable`, and its `road_cost` — the
cost of walking a road laid on it.

Distance in this game is **walked along the road network**. The road
network is rebuilt only when it is dirty, and its connected-component labelling does not depend on the
order the roads were built in.

### Buildings

A building kind declares its `id`, its `size` in tiles, its `cost`, how many `levels` it has, and the
`required_services` it needs. A building that provides a service declares `kind`, `range_per_level`
and `capacity_per_level`, one entry per level.

A building is classified as **a house** by its `required_services` being non-empty. A house's
`required_services` is the union of what each of its levels asks for, and it has to stay that way, or
the house stops being recognised as one. Checked, not commented.

Buildings may not overlap, and may only be placed on `buildable` terrain.

### Coverage

A provider serves the houses within `range_per_level` of it, measured as walked distance along the
roads. What it can serve at once is bounded by `capacity_per_level`, **counted in residents actually
present, not in houses** (A12) — a house that could hold many but holds few weighs only the few.

Because capacity is counted on the residents present, the coverage has to be recomputed whenever
anyone moves in or out. That is A12's price, and it is paid at the end of step 6.

A provider's capacity may not exceed what its output sustains. That relation is checked, and it is
what makes ***a house covered by food always eats*** true rather than hopeful (A5). Hunger is still
reachable — but only through **lack of coverage**, when houses outnumber the providers' capacity.

An open question sits here: an empty house consumes no capacity, because it has no residents to count
([A18](DECISIONS.md#a18--an-empty-house-consumes-no-capacity), slot 14.5 in [ROADMAP.md](ROADMAP.md)).

### Food

A farm produces `output_per_tick` into its own local stock, which is capped at `max_stock`. Each
resident of a covered house eats `food_per_resident` per tick, in thousandths.

Food is conserved as an **exact equality**: what was produced equals what was consumed plus what is in
stock. The capacity a farm may declare follows from this — `output_per_tick / food_per_resident` is
how many residents it can sustain.

### Satisfaction

Satisfaction measures **how long a service has been arriving, not how much of it** (A10). With
capacity and output consistent, a covered house always receives everything it needs, so "how much"
would be a constant.

It is kept per service. While the service arrives, satisfaction rises by `step_up` each tick; while it
does not, it falls by `step_down`. It is clamped to `satisfaction.max` and never goes below zero. It
is lost faster than it is gained, because losing a service is an event and getting it back is an
investment.

`mood_thresholds` cuts the range into the bands the renderer draws — Awful, Unhappy, Happy, Great.
The thresholds ascend and sit above zero, so a house at zero satisfaction is always Awful.

### House levels

Each entry of `house_levels` declares `max_residents`, the `required_services` that level demands,
its `level_up_threshold`, its `decay_threshold` and its `taxable_per_resident`.

The review runs **only on a month boundary**. A house rises one level when it has every service the
next level requires and its satisfaction is at or above that level's `level_up_threshold`. It falls
when satisfaction drops below its `decay_threshold`. A house that falls below the first level is
evicted.

`decay_threshold` sits strictly below `level_up_threshold` on the same level, and the band between
them is the **gap**. The gap plus the monthly cadence is what makes the absence of oscillation
*structural* rather than a lucky consequence of the numbers chosen. Both relations are checked: the
gap must be strictly positive, `max_residents` must grow with the level, and no threshold may exceed
`satisfaction.max`.

The first level's two thresholds are read by nobody — nothing rises into the first level and there is
no level below it to fall to. They exist so the table does not claim a rule that does not exist.

A promotion grants **permission, not people**: it raises the ceiling, and the residents arrive by
their own rules.

### Births and deaths

Rates are expressed **per month and per thousand residents**, because that is the form you read and
reason in. The conversion to ticks loses nothing: whatever does not mature this tick stays in an
accumulator and matures later.

The rule the module implements is ***the rate is random, the distribution is deterministic***. A rate
takes one jittered draw per tick; which house an event lands on is one draw per event. A city of
fifteen thousand residents costs a handful of draws, not one per resident. Nothing is drawn when there
is nothing to draw for.

- **Births** run at `births_per_thousand_per_month`, counted against the **eligible** residents only —
  those in a house that has room and satisfaction at or above `birth_threshold`. Counting against the
  whole population instead would make the plateau an accident of the numbers; this way a city whose
  houses are all full has nobody eligible, the rate is zero, and the population stops for a reason you
  can point at.
- **Deaths** run at `deaths_per_thousand_per_month` for a house that has every service its own level
  asks for, and at `deaths_per_thousand_per_month_when_unserved` for a house going without.
- **Going without** is read off the worst service **the house's own level** requires, compared against
  `unserved_threshold` — not off food. Since the first level is a hut that asks for water only, reading
  hunger off food regardless of level would make the opening of every game a slow bleed.
- `jitter_per_thousand` varies each rate by a symmetric fraction, drawn once per flow per tick. The
  symmetry is checked: an asymmetric jitter would shift the whole balancing without showing up
  anywhere.

Two relations are checked: births must beat deaths at maximum satisfaction, or a perfect city shrinks
and no growth scenario is winnable; and neither threshold may sit above `satisfaction.max`, or it is a
condition no city can ever meet.

**Departures happen before arrivals** within a tick. Technically, freeing a place first keeps
`residents <= max_residents` true at every observable instant rather than only at the end of the step.
For gameplay, a house that loses somebody can win them back the same tick, which makes the population
responsive instead of jerky.

### Difficulty

A profile is chosen at the start of a game and **never changes afterwards**. Because it changes the
simulation it is state: it enters the state hash and travels in the replay's header (A13).

Each profile declares `starting_residents_per_house`, the residents of a newly-built house. Validation
refuses any value beyond the first level's `max_residents` — a house born beyond its own capacity is a
state the game cannot represent.

### Treasury

The city starts with `starting_treasury`. Each house level declares `taxable_per_resident`. Taxes are
not yet collected — step 7 is empty until phase 16 — but the field is already in the tables and in the
dataset hash.

---

## Hardcoded — the rules that are not tunable, and why

These are in Rust because they are **structural**: changing one is a change to what the game *is*, or
to what the engine can represent, not a rebalance.

| Rule | Where | Why it is not data |
|---|---|---|
| The order of the ten tick steps, and step 6's sub-order | `sim-core/src/tick.rs` | Game semantics. See [ARCHITECTURE.md](ARCHITECTURE.md). |
| The order of the demographic draws | `sim-core/src/demographics.rs` | A determinism contract: the RNG sequence depends on it. |
| The maximum map side | `sim-core/src/grid.rs` (`MAX_SIDE`, A4) | `TileIdx` is a `u16` and cannot address more. A representational limit. |
| `Milli` is thousandths | `sim-core/src/units.rs` | The definition of the unit, not a quantity. Floats never enter the state (D4). |
| Money is `Coins`, never `Milli` | `sim-core/src/units.rs` (A1) | Money has no in-game fractions; thousandths would halve the useful range for nothing. |
| The service kinds themselves | `sim-core/src/service.rs` (D6) | Their *effects* are rules, and the core indexes them into fixed-size arrays. Building kinds, by contrast, **are** data. |
| Which RNG kinds exist, and their salts | `sim-core/src/rng.rs` | One stream per kind so adding a feature does not knock existing sequences out of phase. The salt is derived from the kind's **name**. |
| Hash prefixes, the recording format version, the checkpoint cadence | `sim-core/src/data_hash.rs`, `sim-replay/` | File and hash format, not gameplay. |
| A house is a house if its `required_services` is non-empty | `sim-core/src/data.rs` | A classification rule, not a quantity. |
| Coverage is aggregate, never service walkers | `sim-core/src/coverage.rs` (D2) | An architectural decision. The walkers the player sees are decorative and live in the renderer. |
| Rejected commands change nothing | `sim-core/src/tick.rs` | An invariant: an invalid command is discarded and reported, and does not interrupt the tick. |

If you find yourself wanting to tune something in this table, that is a design change worth an entry
in [DECISIONS.md](DECISIONS.md) — not an edit.
