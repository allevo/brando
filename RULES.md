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

Distance in this game is **walked along the road network**, never measured in a straight line. Two
places belong to the same network when a road path joins them, and which roads make up one network
does not depend on the order they were built in.

### Buildings

A building kind declares its `id`, its `size` in tiles, its `cost`, how many `levels` it has, and the
`required_services` it needs. A building that provides a service declares `kind`, `range_per_level`
and `capacity_per_level`, one entry per level.

**A building that requires services without providing any is a house.** That is how the game tells
the two kinds apart, which is why the list on a house is never empty.

**Which services a house needs is declared per level**, in `house_levels` — see *House levels* below.
The list on the building itself is the union of what every level asks for, and it has to stay the
union, or the building stops being read as a house at all. That the two agree is checked when the
tables are loaded, not left to a comment.

Buildings may not overlap, and may only be placed on `buildable` terrain.

### Coverage

A provider serves the houses within `range_per_level` of it, measured as walked distance along the
roads. What it can serve at once is bounded by `capacity_per_level`, **counted in the residents
living in those houses and not in the houses themselves**: a house of two residents takes two places,
a house of eight takes eight, and the same well therefore reaches fewer houses as the district fills
up. A house with nobody in it takes no places at all.

A house is taken whole or left out whole — it is never half served — and a house that does not fit is
skipped rather than blocking the ones behind it. Priority is by walked distance, the nearest first.

Because places are counted in residents, an assignment stops being true the moment anyone is born,
dies or is evicted. That is why the coverage is recomputed at the end of step 6, and it is what makes
step 3 the most expensive part of the tick.

A provider may not declare more capacity than its own output feeds, and that relation is checked when
the tables are loaded. It is what turns ***a house covered by food always eats*** from a hope into a
rule: a farm can never take on more residents than it can feed. Hunger is still reachable, but only
through **lack of coverage**: more houses than the providers have places for. It cannot happen to a
house that has a farm.

### Food

A farm produces `output_per_tick` into its own stock, which stops at `max_stock`; what would go past
it is lost. Every resident of a house covered by food eats `food_per_resident` per tick, in
thousandths — so what a house takes is that amount times the number of residents living in it, and it
takes it whole or not at all. The capacity a farm may declare follows from this: its output divided
by what one resident eats is how many residents it can feed.

**Every covered house eats, whatever its own level asks for.** The coverage does not read the level's
requirements, so a first-rung hut that asks for water only is still assigned to a farm within reach,
still eats, and still takes up places that farm counts. That is what lets it build up the food
satisfaction the rung above demands — and it is also what makes it compete for the farm.

Food is conserved as an **exact equality**: what was produced equals what was eaten, plus what is in
stock, plus what was written off — food lost because the granary was full, and food lost along with a
demolished farm.

### Satisfaction

Satisfaction measures **how long a service has been arriving, not how much of it arrives**. With a
provider's capacity and its output kept consistent, a covered house always receives everything it
needs, so "how much" would be the same number for every house and would say nothing.

It is kept per service, and it moves for **every** service the building declares in
`required_services`, including the ones the house's current level does not ask for. It has to: a
level that introduces a new service would otherwise be out of reach for ever, its accumulator sitting
at zero with nothing to move it. While the service arrives, satisfaction rises by `step_up` each
tick; while it does not, it falls by `step_down`. It stops at `satisfaction.max` and never goes below
zero. It is lost faster than it is gained, because losing a service is an event and getting it back
is an investment.

Which services are **read** is a separate question, and there the answer is per level: a house's mood
is the worst of the services **its own level** requires. `mood_thresholds` cuts the range into the
bands the renderer draws — Awful, Unhappy, Happy, Great. The thresholds ascend and sit above zero, so
a house at zero satisfaction is always Awful.

**Levelling up does not reset satisfaction, and it can lower the mood on the spot.** The accumulators
carry over untouched; what changes is the list that is read. A house promoted into a level that asks
for a service it has never received reads that empty accumulator from the same tick, so it can be
promoted and unhappy at once. That is the signal working, not a glitch: the new rung is harder to
keep than the one below it.

### House levels

Each entry of `house_levels` declares `max_residents`, the `required_services` that level demands,
its `level_up_threshold`, its `decay_threshold` and its `taxable_per_resident`.

The review runs **only on a month boundary**. A house rises one level when it has every service the
next level requires and its satisfaction is at or above that level's `level_up_threshold` on each of
them. It falls one level when its satisfaction drops below its own level's `decay_threshold` on any
one of them. Falling is decided first, so a house on its way down cannot climb back on one leftover
requirement in the same review.

**The first level never falls**: there is nothing below it to fall to. A house that empties out is
the demographics' business, not the review's.

Falling shrinks the house, and the residents who no longer fit under the new level's `max_residents`
are **evicted**: they leave the city, and they are counted as having left.

`decay_threshold` sits strictly below `level_up_threshold` on the same level, and the band between
them is the **gap**. The gap plus the monthly cadence is what makes the absence of oscillation
*structural* rather than a lucky consequence of the numbers chosen. Both relations are checked: the
gap must be strictly positive, `max_residents` must grow with the level, and no threshold may exceed
`satisfaction.max`.

The first level's two thresholds are read by nobody: nothing rises into the first level, and it never
falls. They exist so the table does not claim a rule that does not exist.

A promotion grants **permission, not people**: it raises the ceiling, and the residents arrive by
their own rules.

### Births and deaths

Rates are expressed **per month and per thousand residents**, because that is the form you read and
reason in. Nothing is lost in the conversion to ticks: a rate that would mature less than one event a
month keeps its fraction until it does. A small city therefore still grows — slowly, and that is the
shape of the early game. The first residents come from building houses, not from births.

- **Births** run at `births_per_thousand_per_month`, counted against the **eligible** residents only
  — those in a house that has room left and satisfaction at or above `birth_threshold` — and the rate
  is then scaled by the city's average satisfaction, weighted by residents. A city that is struggling
  has few children even in the houses that are doing well. Counting against the whole population
  instead would make the plateau an accident of the numbers; this way a city whose houses are all
  full has nobody eligible, the rate is zero, and the population stops for a reason you can point at.
- **Deaths** run at `deaths_per_thousand_per_month` for the residents of a house that has every
  service its own level asks for, and at `deaths_per_thousand_per_month_when_unserved` for the
  residents of a house going without.
- **Going without** is read off the worst service **the house's own level** requires, compared against
  `unserved_threshold` — not off food. Since the first rung is a hut that asks for water only, reading
  hunger off food whatever the level would make the opening of every game a slow bleed.
- `jitter_per_thousand` varies each rate by a symmetric fraction. The symmetry is checked: an
  asymmetric jitter would shift the whole balancing without showing up anywhere.

Which house a birth or a death falls on is drawn at random among the houses eligible for it.

**Departures happen before arrivals** within a tick. A house that loses somebody can win them back
the same tick, which makes the population move smoothly instead of in jumps — and a house never holds
more residents than its level allows at any moment you could look at it.

Two relations are checked when the tables are loaded. Neither threshold may sit above
`satisfaction.max`, or it is a condition no city can ever meet. And `births_per_thousand_per_month`
has to beat `deaths_per_thousand_per_month` — but that comparison is the **best case, and only the
best case**: a city at full satisfaction with every house served. All it guarantees is that such a
city grows. It says nothing about a city doing badly, which is meant to shrink: the birth rate falls
away with the average satisfaction, while the raised death rate applies to more and more houses.

### Difficulty

A profile is chosen at the start of a game and **never changes afterwards**. Because it changes the
simulation it is state, not a setting: it enters the state hash and travels in the replay's header,
so two games with the same seed and the same commands on different profiles are different games and
cannot be confused for one another.

Each profile declares `starting_residents_per_house`, the residents of a newly-built house. Validation
refuses any value beyond the first level's `max_residents` — a house born beyond its own capacity is a
state the game cannot represent.

### Treasury

The city starts with `starting_treasury`. Each house level declares `taxable_per_resident`. Taxes are
not yet collected — step 7 is empty until phase 16 — but the field is already in the tables and in the
dataset hash.

---

## In the code — what is not in the tables, and why

**No rate, and no gameplay minimum or maximum, is hardcoded.** A balancing number in a `.rs` file is
a bug, not an exception to this section. What does live in the code is of two kinds, and they answer
different questions.

### Rules of the game that are not numbers

Changing one of these changes what the game *is*, so there is nothing to tune.

| Rule | Where |
|---|---|
| A building that requires services without providing any is a house | `sim-core/src/data.rs` |
| A service reaches a house by coverage over walked distance, never by a walker carrying it (D2). The walkers the player sees are decorative and live in the renderer | `sim-core/src/coverage.rs` |
| A house is served whole or left out whole, and it eats whole or not at all | `sim-core/src/coverage.rs`, `sim-core/src/production.rs` |
| Money has no fractions, and quantities do (A1) | `sim-core/src/units.rs` |
| An invalid command is discarded and reported, and does not interrupt the tick | `sim-core/src/tick.rs` |
| Which services exist at all (D6) — building kinds, by contrast, **are** data | `sim-core/src/service.rs` |
| What happens before what inside a tick: coverage before eating, satisfaction before the review that reads it, departures before arrivals | `sim-core/src/tick.rs` |

The last row looks like plumbing and is not. Departures before arrivals is why a house that loses
somebody can take them back the same day; satisfaction before the review is why a month of service
counts at the review that closes it. Reorder them and the game plays differently.

### Contracts that keep two runs identical

These decide nothing about the game and everything about whether the same seed and the same commands
replay to the same city, down to the bit. **[ARCHITECTURE.md](ARCHITECTURE.md) is the file that
describes them** — the frozen declaration orders, one random stream per kind with its salt taken from
the kind's name, the fixed order of the demographic draws, and the hash and recording formats.

Three numbers do live in the code, and none of them is balancing: the largest map side a tile index
can address, how often a recording writes a hash down, and how many thousandths make a unit. Each is
the definition of a representation or of a file format. A game where one of them differs is not a
rebalanced game, it is an unreadable file.

If you find yourself wanting to tune something on this page, that is a design change worth an entry
in [DECISIONS.md](DECISIONS.md) — not an edit.
