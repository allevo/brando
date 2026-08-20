# The rules of the game

**This file answers one question: what does the game do?**

Every sentence below states something that happens in the game, and names the parameter that tunes
it. **It carries no values, deliberately.** Numbers change at every rebalance; the rules they are
plugged into change far more rarely.

**The values live in `sim-data/data/*.ron` and nowhere else.** A balancing number in a `.rs` file is
a bug. The tables' own comments carry the derivations — why a rate is the rate it is — and are worth
reading before changing anything.

Where two numbers have to stand in a fixed relation, that relation is **a check and not a comment**.
[What the tables may not say](#what-the-tables-may-not-say) lists them; a table that breaks one is
refused at load with a full report.

---

## Time

One tick is one game day. The month and year lengths are fixed by `Calendar` — thirty ticks and twelve
months, respectively — not by a table. Scenario objectives are expressed in months and years, never in
ticks.

## The map and its terrain

The map is a grid of tiles. A `tile` carries one terrain, at most one occupant — a building or a
`house` — and a flag saying whether a `road` runs over it. **A tile holds one thing:** a road and an
occupant never share one.

| Terrain | `buildable` | `walkable` | What it is |
|---|---|---|---|
| `Plain` | yes | yes | open ground, and the only terrain a building stands on |
| `Rock` | no | yes | ground you can cross but not settle |
| `Water` | no | no | neither, and a stretch of it splits the city in two |

## Roads and distance

A `road` may be laid on a tile that is on the map, whose terrain is `walkable`, and that carries
neither an occupant nor a road already. It costs that terrain's `road_cost`.

A building's **entrance** is a road tile orthogonally touching any tile of its area. A building
without an entrance is off the network.

**Distance is walked along the roads, never measured in a straight line.** It counts road tiles and
nothing else, so the terrain decides what a road costs to lay and never what it costs to cross. Two
buildings facing the same road are no distance apart. Two places belong to the same network when a
road path joins them, and which roads make up a network does not depend on the order they were laid
in.

Demolishing a tile that carries a road takes the road away. Demolishing a tile with nothing on it is
refused.

## Buildings

A kind of building declares its `size` in tiles, its `cost`, how many `levels` it has, and its
`role`. A building that produces goods also declares its `output_per_tick` and its `max_stock`.

**A building says what it is: its `role` is declared, not worked out from what it leaves blank.**
There are two roles, and a building has exactly one. A building whose role is `house` declares the
`required_services` it needs. A building whose role is `provider` declares the `service` it supplies:
its `kind`, its `range_per_level` and its `capacity_per_level`, one value per level. Declaring a
service on a house, or none on a provider, is refused when the tables are loaded.

A `house`'s `required_services` is the union of what its levels require. The kinds the tables declare
today:

| Kind | Role | What it needs | What it gives |
|---|---|---|---|
| `house` | `house` | water and food, and nothing else | nothing |
| `well` | `provider` | nothing | water |
| `farm` | `provider` | nothing | food, and it produces that food into a stock of its own |

**Producing is not a role.** The `farm` is a `provider` that also produces, so what it grows is
declared beside its role rather than instead of it.

Which kinds exist is data, not code: a table that adds a kind adds it to the game.

A building's own level decides which entry of `range_per_level` and `capacity_per_level` it reads.
Only a `house` ever changes level.

**Placing.** A building may be placed where every tile of its area is on the map, is `buildable`, and
is free of a road, a building and a house alike. The whole area is checked before anything is
written, so a refused placement leaves nothing half built. A placement is refused, and says which of
these it was, when a tile lies off the map, when a tile is taken, when the terrain is not buildable,
when the treasury cannot pay the `cost`, or when the kind of building is not one the tables declare.

**Demolishing.** Demolishing returns nothing, and it takes more than the building with it. The
residents of a demolished `house` leave the city and are counted as lost; the `stock` of a demolished
`farm` goes with it.

## Houses

A `house` holds residents, up to its level's `max_residents`.

A newly built house stands at the first level, with `starting_residents_per_house` residents and at
zero satisfaction on every service. If it is born with anybody in it, a provider within reach covers
it the very day it is placed, so it begins gaining satisfaction at once. But it begins at zero and
nothing is granted in advance, so its first rise costs the whole climb.

A house with nobody left in it still stands and still holds its tile. No provider serves it, nobody
is born in it, and nobody in it dies — but immigration can still move somebody into it, whole rooms
and all, whatever its coverage. A profile that founds every house empty fills its city by immigration
alone.

## Coverage

A provider serves the houses within `range_per_level` of it, measured as walked distance along the
roads. Both ends have to sit on the network: a provider without an entrance serves nobody, and a
house without an entrance is out of reach at any distance.

**Capacity is counted in residents, not in houses.** A provider takes houses until
`capacity_per_level` runs out: a house of two residents takes two places, a house of eight takes
eight, and the same `well` therefore reaches fewer houses as the district fills up.

**A house with nobody in it is not served.** A service exists to reach residents, so a house with
none is no candidate for one, however close it stands. A house emptied by deaths therefore loses its
coverage, and gets it back when somebody moves in.

A house is served whole or left out whole. A house that does not fit is skipped and the next
candidate is considered, so one large house near a provider does not shut out everything behind it.
Candidates are taken nearest first; at equal distance the house whose tile comes first on the map
goes first, and since no two houses stand on the same tile that settles it.

**A contested house goes to the provider built first that still has room for it** — which is not the
same as the first one to reach it. A provider left with fewer places than the house needs leaves it
standing, and the house is still free when the next provider comes to it. A house already taken is
set aside before the next provider starts filling up, so it never eats places that provider could
have given to somebody else.

Providers of different services never compete. Contention is per service, and one house is held by a
`well` and a `farm` at the same time.

Coverage is built again from scratch whenever the roads change, a building or a house is placed or
demolished, or anybody was born, died or was evicted the tick before — because places are counted in
residents, and any of those makes yesterday's assignment untrue.

## Food

A `farm` adds `output_per_tick` to its `stock` every tick. The stock stops at `max_stock`: the
granary has a lid, and what would go past it is lost.

Every resident of a house covered by food eats `food_per_resident` per tick, in thousandths. A house
draws that amount times the number of residents living in it out of the `farm` covering it, and it
draws it whole or not at all: either everyone in the house is served that tick, or no one is.

The houses are served one at a time, in a fixed order that comes out the same on every run of the
same game — and which house is served first never changes the outcome, for the reason below.

**Everyone in a covered house eats, whatever that house's own level asks for.** Coverage does not
read the level's requirements: a first-level hut that asks for water only is still assigned to a
`farm` within reach, its residents still eat, and it still takes places that farm counts. That is
what lets it build up the food satisfaction the level above demands, and it is also what makes it
compete for the farm.

**A house a farm covers always eats.** A farm may not declare more capacity than its output feeds, so
it can never take on more residents than it can feed, and no stock runs short. Hunger is still
reachable, but only through **lack of coverage** — more houses than the providers have places for.
It cannot happen to a house that has a farm.

Food is conserved as an exact equality: what was produced equals what was eaten, plus what is in
stock, plus what was written off — lost to a full granary, or lost with a demolished farm.

## Satisfaction and mood

Satisfaction measures **how long a service has been arriving, not how much of it arrives**. A covered
house always receives everything it needs, so "how much" would be the same number for every house.

Every house carries one accumulator per service. While the service arrives, the accumulator rises by
`step_up` each tick; while it does not, it falls by `step_down`. It stops at `satisfaction.max` and
never goes below zero.

The accumulators move for **every** service the `house` declares in `required_services`, including
the ones its current level does not ask for. A level that introduces a new service is reachable
because of it: the accumulator has been moving all along.

Which services are **read** is a separate question, and there the answer is per level. A house's
**mood** is the worst of the services its own level requires. `mood_thresholds` cuts the range into
the bands the renderer draws — `Awful`, `Unhappy`, `Happy`, `Great`. The thresholds ascend and sit
above zero, so a house at zero satisfaction is `Awful`.

## Levels — the monthly review

Each entry of `house_levels` declares its `max_residents`, the `required_services` that level
demands, its `level_up_threshold`, its `decay_threshold` and its `taxable_per_resident`.

Houses are reviewed **on a month boundary and at no other time**, and move at most one level.

A house **falls** one level when, on any service its own level requires, its satisfaction is below
that level's `decay_threshold`. A house **rises** one level when, on every service the level above
requires, its satisfaction is at or above that level's `level_up_threshold`. Falling is decided
first, so a house on its way down cannot climb back on one leftover requirement in the same review.

The review reads the accumulators, not who is covered at that moment: a house that lost a service
days ago still rises if what it banked has not yet run down past the threshold.

**The first level never falls**: there is nothing below it. A house that empties out is the
demographics' business, not the review's.

Falling shrinks the house, and the residents who no longer fit under the new level's `max_residents`
are **evicted**: they leave the city, and they are counted as having left. Rising grants permission
and not people: it raises the ceiling, and the residents arrive by their own rules.

`decay_threshold` sits strictly below `level_up_threshold` on the same level, and the band between
them is the **gap**. Inside it nothing moves, whichever side the house came from. The gap and the
monthly cadence together are what keep a house sitting on a boundary from flipping at every review.

**Levelling up does not reset satisfaction, and it can lower the mood on the spot.** The accumulators
carry over untouched; what changes is the list that is read. A house never rises into a service it
has never received — the threshold has to be cleared on that service too — but the service it has
been receiving for the least time now counts towards its mood, and if it sits lower than everything
the old level asked for, the house is promoted and less happy at once.

## Births and deaths

Rates are expressed **per month and per thousand residents**. Nothing is lost in the conversion to
ticks: a rate that would mature less than one event a month keeps its fraction until it does, so a
small city still grows, slowly. The first residents come from building houses, not from births.

**Births** run at `births_per_thousand_per_month`, counted against the **eligible** residents only —
those in a house that still has somebody living in it, has room left under its level's
`max_residents`, and is at or above `birth_threshold` on the worst service its own level asks for.
That rate is then scaled by the city's average satisfaction, which is the same reading taken over the
whole city: every house contributes the worst service **its own** level requires, weighted by how
many people live in it, so a full house counts for more than a hut. A struggling city therefore has
few children even in the houses that are doing well, and a city whose houses are all full has nobody
eligible and stops growing.

**Deaths** run at `deaths_per_thousand_per_month` for the residents of a house that is not going
without, and at `deaths_per_thousand_per_month_when_unserved` for the residents of one that is.
**Going without** means the worst service the house's own level requires sits below
`unserved_threshold`. It is not read off food: a hut at the first level asks for water only, so it is
not going without whether or not a `farm` reaches it.

`jitter_per_thousand` varies each rate by a symmetric fraction, drawn once per flow per tick.

Which house an event falls on is drawn at random, and the two sets are not the same. A birth can only
land on a house that meets all three conditions, and that house drops out once it fills up. A death
can land on **any house with somebody left in it**, served or not: the rate is raised by the houses
going without, but the person it takes is drawn from the whole city, so a district that loses its
water kills people everywhere. A house that loses its last resident is abandoned, and drops out.

**Departures happen before arrivals** within a tick. A house that loses somebody can win them back
the same tick, and a house never holds more residents than its level allows at any moment you could
look at it.

## Migration

`attractiveness` is a single number, in thousandths, saying how much the city draws people in. It is
a weighted sum of two shares, weighted by `satisfaction_weight` and `free_places_weight`: the average
satisfaction across every resident, and the free places in the houses the coverage currently reaches.
A city with no residents yet reads both shares at their ceiling, so a city whose houses are all born
empty can still draw its first immigrants.

**Emigration** runs at `emigration_per_thousand_per_month_unhappy`, for the residents of a house below
`emigration_threshold` on the worst service its own level requires — the same reading
`unserved_threshold` takes, of a different threshold. It is the channel by which a city that decays
empties out even without deaths. `emigration_threshold` sits below `birth_threshold`, so a house cannot
be having children and losing residents to emigration at the same satisfaction.

**Immigration** fills the free places of every house that has one — served or not, occupied or empty.
`attractiveness` scales the rate: at the ceiling it runs at the full rate, and it falls away as the city
becomes less attractive. What the rate is counted *against* depends on how big the city already is, and
there are two regimes with a hard cutover between them at `founding_population_threshold`.

Below the threshold the city is **founding**, and the rate runs at
`founding_immigration_per_thousand_per_month` counted against the **free places**. A city whose houses
are all born empty has no residents for a rate to read, and none it could ever get, so the founding
regime counts what such a city does have: room.

At or above the threshold the city is **growing**, and the rate runs at
`immigration_per_thousand_per_month` counted against the **residents**. A bigger, equally attractive
city therefore draws people faster than a small one, which is the word-of-mouth model the founding
regime cannot express.

Zero free places is zero immigrants either way, because houses are the only container for population
and with none free there is nowhere for anybody to go. What differs is what the city does about it. In
the founding regime the rate is a multiple of the free places, so a city with none draws nobody at all.
In the growth regime the rate reads the residents, so the city still draws people and simply cannot
house them: everyone it could not house is **turned away** and counted. A player reads that count to
tell a city that is full from a city that is unwanted — the first needs houses, the second needs
services — and the two look identical in the population alone.

**A migrant turned away is lost, not queued.** Each day's arrivals are worked out afresh from that
day's own city, the same way the granary's lid works: what would go in beyond it is lost. Nobody waits
outside for room to appear.

**Immigration is not gated on coverage.** A house is chosen for its free places alone; whether a
provider reaches it decides how fast the city as a whole attracts people, through `attractiveness`, not
which house within it fills first. Gating the choice on coverage would leave an emptied, uncovered house
unable to ever be refilled — uncovered because empty, empty because uncovered.

`jitter_per_thousand` varies each of the two rates by a symmetric fraction, drawn once per flow per
tick, the same shape as the demographics' own jitter.

## The treasury

The city starts with `starting_treasury`. A `road` costs its terrain's `road_cost`, a building costs
its `cost`, and both come out of the treasury the moment the command is applied. A placement that
cannot be paid for is refused rather than allowed to run up a debt, and the treasury never goes
negative.

Nothing is ever paid back. Demolishing returns not a coin.

Each house level declares a `taxable_per_resident`. Taxes are not collected yet; the field is already
in the tables and in the dataset hash.

## Difficulty

A profile is chosen at the start of a game and **never changes afterwards**. It changes the
simulation, so it is state rather than a setting: it enters the state hash and travels in the
replay's header, and two games with the same seed and the same commands on different profiles are
different games.

A profile declares `starting_residents_per_house`, the residents of a newly built house. Everything
else about a new house is the same on every profile, which is why a district built all at once rises
all at once, a month later.

## The order of a day

What happens before what inside a tick is a rule of the game. What a player can see happens in this
order:

- The commands are applied. An invalid one is discarded and reported, and does not interrupt the tick
  or the commands that follow it.
- The services reach the houses.
- The farms grow food, and the covered houses eat.
- Satisfaction moves by one tick.
- On a month boundary, the houses are reviewed.
- Deaths, then emigration, then births, then immigration.

These orderings are observable. The houses eat after the coverage is settled, so a house
covered today eats today. Satisfaction moves before the review that reads it, so a month of service
counts at the review that closes it. Departures come before arrivals, so a house that loses somebody
can take them back the same day; deaths and emigration read this tick's own review before they act, and
immigration's `attractiveness` reads this tick's own deaths, emigration and births.

The full sequence — the ten steps of a tick, and which are still empty — is in
[ARCHITECTURE.md](ARCHITECTURE.md).

---

## What the tables may not say

Every relation below is checked when the tables are loaded. Breaking one is a load error with a full
report, not a subtle misbehaviour at run time.

| The relation | What breaking it would do |
|---|---|
| Some building declares the `house` role | no house levels up, eats or is taxed, and nothing says so |
| A house declares as many `levels` as `house_levels` has entries | the two tables describe different houses |
| A house's `required_services` is the union of what its levels require | a level asking for a service missing from the union is unreachable: nothing ever lifts that accumulator off zero |
| A `provider` declares a `service`, and a `house` declares none | a building that says one thing and is built as another |
| A level requires at least one service | the level is reached for free |
| `max_residents` grows with the level | levelling up shrinks the house |
| `decay_threshold` strictly below `level_up_threshold` — the gap | a house on the boundary flips at every review |
| No threshold beyond `satisfaction.max` | a level nothing can reach |
| Every service a level requires is provided by some building | a level nobody can ever satisfy |
| No level's `max_residents` beyond the largest `capacity_per_level` declared for that service | a house too big to be served in full stops growing, and nothing says why |
| A food provider's `capacity_per_level` within what its `output_per_tick` and `max_stock` sustain | houses covered by a farm stay hungry for ever |
| `starting_residents_per_house` within the first level's `max_residents` | a house born beyond its own capacity, a state the game cannot represent |
| `mood_thresholds` strictly ascending and above zero | the bands overlap, or a house at zero satisfaction is not `Awful` |
| `jitter_per_thousand` below the whole it is a fraction of | a jittered rate comes out negative |
| Neither `birth_threshold` nor `unserved_threshold` beyond `satisfaction.max` | a condition no city can meet |
| `births_per_thousand_per_month` beats `deaths_per_thousand_per_month` | a city at full satisfaction shrinks, so no growth scenario is winnable |
| `deaths_per_thousand_per_month_when_unserved` strictly worse than `deaths_per_thousand_per_month` | losing a service costs the city nothing |
| No lone zero rate in `demographics` | a table somebody half filled in |
| `emigration_threshold` no greater than `satisfaction.max` | a condition no house could ever meet |
| `emigration_threshold` strictly below `birth_threshold` | a house could be having children and losing residents to emigration at the same satisfaction, fighting each other on every tick |
| `satisfaction_weight` and `free_places_weight` add up to a whole thousand | `attractiveness` stops being a share expressed in thousandths |
| No lone zero rate in `migration` | a table somebody half filled in |
| `founding_population_threshold` at least one | no city is ever below it, so a city founded with nobody in it is rated on a population of nobody and can never draw a soul |

Some of these are asked only of a table that describes demographics at all: every rate at zero
switches births and deaths off, which is a configuration and not a mistake. And the comparison
between the two rates is the **best case, and only the best case** — a city at full satisfaction with
every house served. It guarantees that such a city grows. A city doing badly is meant to shrink.

## What the tables cannot change

Changing one of these changes what the game *is*, so there is nothing to tune.

| The rule | Where it lives |
|---|---|
| A building that requires services and provides none is a house | `sim-core/src/data.rs` |
| A service reaches a house by coverage over walked distance, never by a walker carrying it (D2). The walkers the player sees are decorative and live in the renderer | `sim-core/src/coverage.rs` |
| A house is served whole or left out whole, and its residents eat whole or not at all | `sim-core/src/coverage.rs`, `sim-core/src/production.rs` |
| Money has no fractions, and quantities do | `sim-core/src/units.rs` |
| An invalid command is discarded and reported, and does not interrupt the tick | `sim-core/src/tick.rs` |
| Which services exist at all (D6) — kinds of building, by contrast, **are** data | `sim-core/src/service.rs` |
| Which terrains exist at all, and what may be done on each — where a building may stand, where a road may be laid (D6). The table describes every terrain and gives it the one thing about it that is a number, its `road_cost` | `sim-core/src/grid.rs` |
| What happens before what inside a tick | `sim-core/src/tick.rs` |

A handful of numbers live in the code, and none of them is balancing: the largest map side a tile
index can address, the most levels a per-level table can hold, how many thousandths make a unit, the
version stamped into a recording, and how often a recording writes a hash down. Each is the
definition of a representation or of a file format, never of a rule.

If you find yourself wanting to tune something on this page, that is a design change worth a task of
its own in [ROADMAP.md](ROADMAP.md) — not an edit.

---

## Every parameter, and the rule it tunes

**`terrain.ron`** — one row per terrain, and every terrain has one. It gives a terrain the one thing
about it that is a number; what may be done on it is fixed in the code.

| Parameter | What it decides |
|---|---|
| `terrain` | which terrain the row describes |
| `road_cost` | what laying a road on it takes out of the treasury |

**`buildings.ron`** — one entry per kind of building.

| Parameter | What it decides |
|---|---|
| `size` | the rectangle of tiles the building stands on |
| `cost` | what placing it takes out of the treasury |
| `levels` | how many levels it has |
| `role` | what part it plays: `house` or `provider` |
| `required_services` | what a house needs — the union of what its levels require |
| `service` | the service a provider supplies |
| `kind` | which service that is |
| `range_per_level` | how far it reaches, walked along the roads — one value per level |
| `capacity_per_level` | how many residents it serves at once — one value per level |
| `output_per_tick` | what a producer adds to its stock each tick |
| `max_stock` | how much its granary holds before the rest is lost |

**`difficulty.ron`** — one entry per profile.

| Parameter | What it decides |
|---|---|
| `starting_residents_per_house` | the residents of a newly built house |

**`rules.ron`** — the rules the whole city runs on.

| Parameter | What it decides |
|---|---|
| `starting_treasury` | the city's money on its first day |
| `house_levels` | one entry per house level, holding the five below |
| `max_residents` | how many residents the level holds |
| `required_services` | what the level demands, read for the mood, the review and going without |
| `level_up_threshold` | the satisfaction each of the level's services needs before a house rises into it |
| `decay_threshold` | the satisfaction below which a house at that level falls |
| `taxable_per_resident` | what a resident is worth in tax |
| `satisfaction.max` | the ceiling on an accumulator |
| `step_up` | how much an accumulator gains per tick while the service arrives |
| `step_down` | how much it loses per tick while the service does not |
| `mood_thresholds` | where the mood bands begin |
| `food_per_resident` | thousandths of food one resident eats per tick |
| `demographics` | the rates below |
| `births_per_thousand_per_month` | the birth rate, before it is scaled by the city's satisfaction |
| `deaths_per_thousand_per_month` | the death rate in a house that is not going without |
| `deaths_per_thousand_per_month_when_unserved` | the death rate in a house that is |
| `unserved_threshold` | the satisfaction below which a house counts as going without |
| `birth_threshold` | the satisfaction a house needs before it can have children |
| `jitter_per_thousand` | the symmetric wobble drawn on each rate, once per flow per tick |
| `migration` | the rates and weights below |
| `satisfaction_weight` | how much of `attractiveness` is the city's average satisfaction, in thousandths |
| `free_places_weight` | how much of `attractiveness` is the free places in the houses the coverage reaches, in thousandths |
| `founding_immigration_per_thousand_per_month` | the inbound rate below the threshold, counted against the free places |
| `founding_population_threshold` | the population at which the founding rate hands over to the one below |
| `immigration_per_thousand_per_month` | the inbound rate at and above the threshold, counted against the residents |
| `emigration_per_thousand_per_month_unhappy` | the outbound rate for a house below `emigration_threshold` |
| `emigration_threshold` | the satisfaction below which a house's residents start to emigrate |
