# Phase 13 — House levels

**Goal:** a house served with water and food reaches level 2 in a number of ticks you can compute
from the `DataSet`; take the water away and it goes back to level 1; and on the boundary it **does
not oscillate**.
**Depends on:** 12.
**Size:** L.
**Decisions involved:** [A12](open-decisions.md), A9, A10, A5, D5, D6.

## Why now

Because phase 12's satisfaction exists in order to be read by someone, and this is the first system
that reads it. And it is the moment when the house's capacity stops being a single number:
`Rules::max_residents` (phase 11) changes its body and becomes the per-level table.

One thing that does **not** happen here, because of [A12](open-decisions.md): levelling up does not
touch the coverage. A provider's capacity is consumed by the **residents present**, and going up a
level does not bring people in — it only brings permission to hold more of them. It is the
demographics (phase 14) that fill the new space, and that is where the coverage starts to chase.
This phase moves `residents` in one way only: eviction on decay.

## What gets built

### The tables, in `rules.ron`

`residents_per_house_level: Vec<u16>` becomes `house_levels: Vec<HouseLevelDef>`.

```rust
// sim-core/src/data.rs
pub struct HouseLevelDef {
    /// The most residents it can hold. It is the house's ceiling, **not** what
    /// the coverage counts a provider's capacity against: that is counted on the
    /// residents who actually live there (A12). A house that can hold 8 with two
    /// residents weighs two, not eight.
    pub max_residents: u16,
    /// The services needed to **rise to** this level and to **stay at** it.
    ///
    /// This is where `required_services` stops being declarative data read only
    /// by `is_house()` — the gap noted in A9.
    pub required_services: Vec<ServiceKind>,
    /// The minimum satisfaction on **each** of the required services, to rise here.
    pub level_up_threshold: u8,
    /// Below this, on any required service at all, it decays.
    pub decay_threshold: u8,
    /// The taxable base per resident (phase 16). Zero until then.
    pub taxable_per_resident: Milli,
}
```

**Why in `Rules` and not in `BuildingDef`.** `House` does not carry a `BuildingKindId`, and in M1
there is only one kind of house. Adding one so a per-kind table could be indexed would be the
invented abstraction D6 forbids before the second civilisation. When it is needed,
`Rules::max_residents` (phase 11) is once again the only place to change — that is why it is a
function.

`Rules::max_residents` changes its body, not its signature:

```rust
pub fn max_residents(&self, level: u8) -> Option<u16> {
    Some(self.house_levels.get(usize::from(level).checked_sub(1)?)?.max_residents)
}
```

The house in `buildings.ron` moves to `levels: 3` and keeps `required_services` as the **union** of
the per-level requirements — it is still needed by `is_house()`, and it is cross-validated against
the levels (see `Inconsistency::InconsistentRequirements`).

### Step 6, extended

```rust
fn houses_and_migration(world: &mut World) {
    update_satisfaction(world);             // 6.1, phase 12
    if is_monthly_review(world) {
        decay(world);                       // 6.2a
        level_up(world);                    // 6.2b
    }
}
```

**Why the review is monthly and not every tick.** Not for the cost — with
[A12](open-decisions.md) levelling up does not touch the coverage, so it is cheap. It is because
hysteresis on its own only protects against oscillation if the thresholds are far apart, whereas an
infrequent cadence makes it **structural**: thirty ticks pass between two decisions, and a house
cannot go up and down more than twelve times a year by construction.

Satisfaction keeps accumulating **every tick**: it is only the *decision* that is monthly. It
introduces no new concept — the month has existed since phase 03 ([A6](open-decisions.md)) — it is
Zeus-like, and it makes the recordings more readable: a level that only changes at multiples of 30
can be followed by eye.

**Why decay before levelling up.** A house on its way down must not be able to go up in the same
tick because of one leftover requirement. Evaluating the worse condition first makes the transition
monotone, and closes the one-tick flip-flop before it exists.

**One jump per review**, and levelling up is evaluated against the requirements of the
**destination** level: that way you never rise into a requirement that is not met.

**Decay evicts**: `residents = min(residents, max_residents(new_level))`, and the evicted count as
emigration in phase 14's totals. It is not a bookkeeping detail — it is a flow of population, and if
it is not counted, phase 14's conservation is not an equality. It is the same reason
`lost_to_demolition` exists in `FoodTotals`.

### What is **not** built, and why it matters

**No gate on levelling up.** An early draft of this phase had a
`places_to_level_up(world, h, level)` that stopped a house from rising if its providers had no
capacity for the extra space. With [A12](open-decisions.md) it is not needed, and the reason is
worth writing down: **going up a level consumes no capacity**, because capacity is counted on the
residents present and levelling up brings no people in. It brings permission to hold more of them,
which is a different thing.

The pressure comes later, when the demographics fill the space: then demand grows, the provider
fills up, and some house falls out of the coverage. **That is not a pathology to switch off, it is
the game loop** — the city outgrows its own services, quality drops, the player builds. With the
gate it would never happen, and the city would stop on its own without the player having to notice
anything.

The flip side is that this loop has to be shown to **damp** rather than diverge, and it is phase 14
that has to demonstrate it — here there is not yet any demographics to produce it.

**No coverage invalidation on a level change**, for the same reason: the house's capacity does not
enter `pick_within_capacity`'s arithmetic. `coverage_equivalence` stays green without this phase
doing anything, and that is the cheapest way of checking it — if it failed, it would mean something
still reads the house capacity where it should not.

### `DataSet::inconsistencies()` — M1's second structural defence

`unsustainable_food_capacity` becomes one case of a function that gathers **every** cross-table
check:

```rust
/// Every inconsistency *between* tables, in one place.
///
/// It lives in the core because that is where the definitions live (A2) and
/// because it is also needed by the fixture in `sim-core/tests/common/mod.rs`,
/// which does not go through `sim-data`. Adding a check here automatically makes
/// it active on the fixture too: it is the generalisation of A5's lesson — a
/// number that has to stand in a relation with another is a check, not a comment.
pub fn inconsistencies(&self) -> Vec<Inconsistency>;

pub enum Inconsistency {
    CapacityBeyondOutput { building: usize, level: u8, capacity: u16, sustainable: u16 },
    /// `max_residents(l+1) <= max_residents(l)`: levelling up would shrink the house.
    CapacityNotIncreasing { level: u8 },
    /// `decay_threshold(l) >= level_up_threshold(l+1)`: no hysteresis band,
    /// and the city oscillates at every review. Hysteresis is a **check**,
    /// not a comment (A10).
    NoHysteresis { level: u8 },
    /// A threshold beyond `satisfaction.max`: a level unreachable by construction,
    /// and nothing in the game would flag it.
    UnreachableThreshold { level: u8, threshold: u8, max: u8 },
    /// A level requires a service no building provides.
    ServiceWithoutProvider { level: u8, service: ServiceKind },
    /// No provider of that service has capacity for a **full** house of that size.
    /// With A12 it is not a blocker — the house is servable as long as it stays
    /// half empty — but it is a dataset in which a level can never be served in
    /// full, and the city plugs up without saying why.
    CapacityBeyondEveryProvider { level: u8, service: ServiceKind, max_residents: u16 },
    /// `buildings["house"].required_services` differs from the per-level union.
    InconsistentRequirements,
    /// `buildings["house"].levels != rules.house_levels.len()`.
    LevelCountMismatch { declared: u8, in_table: usize },
    /// From phase 11.
    StartingResidentsBeyondCapacity { difficulty: usize, starting: u16, max_residents: u16 },
}
```

`sim-data/src/validate.rs:119` calls `inconsistencies()` instead of the single check, and
`the_fixture_keeps_capacity_and_output_consistent` becomes `the_fixture_has_no_inconsistencies` with
`assert_eq!(d.inconsistencies(), vec![])`. From that moment **every new check protects `sim-core`'s
fixture for free** — which today does not go through `sim-data` and is the real gap.

### `is_house()`, which has to be guarded

It is a heuristic: `service.is_none() && !required_services.is_empty()`. If `required_services` moves
per level and the top-level line disappears, **the house stops being a house** and half the game
changes behaviour silently. The remedies: keep it as the union,
`Inconsistency::InconsistentRequirements`, and a validation saying "at least one building has to be
a house".

### Events

```rust
Event::HouseEvolved  { house: HouseId, from: u8, to: u8 },
Event::HouseDegraded { house: HouseId, from: u8, to: u8 },
```

Two variants and not one with `to < from`: the renderer hangs two different animations off them, and
discriminating on a numeric comparison is the kind of thing you get wrong once but forever.

### The house's area stays 1×1 at every level

It has to be written down, not left implicit. Houses that merge by taking over their neighbour's
tiles are a whole mechanic (Zeus does it) and they are M3. `no_overlap` contains
`expected += 1; // houses are 1x1 in M0`: without that line, someone will slip a size into
`house_levels` and that test will mysteriously go red.

## Out of scope

Births, deaths, migration (phases 14–15): `residents` changes here **only** through eviction on
decay. Different kinds of house. Levels for the buildings that are not houses — `Building::level`
stays 1, and `range_per_level`/`capacity_per_level` stay one-element vectors.

## Tests

1. **Levelling up** (*the goal*): a served house reaches level 2 at the first monthly review after
   the satisfaction has passed `level_up_threshold` — a number of ticks **computed from the
   `DataSet`**, never hardcoded.
2. **Decay**: with the well demolished, the house goes back to level 1 at the first review after the
   satisfaction has fallen below `decay_threshold`.
3. **No oscillation** (property test, *the test that defines the phase*): with constant services,
   over 360 ticks every house's level is **monotone**. It is the proof that the hysteresis really
   works, not just that validation imposes it — both things are needed and neither replaces the
   other.
4. **Levelling up does not touch the coverage**: a house that goes up a level does **not** change
   `Coverage::assignments()` and does not increment `recomputes`, because it brought no residents.
   It is the test that pins A12 down where it is easiest to get wrong — whoever implements the
   per-level capacity will be tempted to use it in `coverage.rs` too.
5. **`coverage_equivalence` stays green with no changes**, now that the levels really do change. If
   it fails, something reads the house capacity where it should be reading the residents.
6. **Eviction is counted**: a level-2 house with 8 residents decaying to a capacity of 4 ⇒ 4
   residents and 4 evicted in the totals. It pins the term down before phase 14 depends on it.
7. **Eviction invalidates the coverage**: it is the only point in this phase where `residents`
   changes, so it is the only one that has to mark the coverage dirty — the first application of the
   contract phase 14 generalises.
8. **One jump per review**: a house at maximum satisfaction does not jump from level 1 to 3 in the
   same month.
9. **The inconsistencies**: one broken fixture per new case of `Inconsistency`, and
   `the_fixture_has_no_inconsistencies` green.
10. **`two_empty_ticks_recompute_nothing`** changes its outcome on purpose, but only because of
    eviction: two empty ticks do recompute if a decay in between sends someone away. It should be
    rewritten as "no recomputation if no population has changed" — which is already the shape phase
    14 will want.

## Verification

```sh
cargo test -p sim-core levels
PROPTEST_CASES=2000 cargo test -p sim-core --release oscillation
cargo test -p sim-core coverage               # coverage_equivalence has to stay green
cargo test -p sim-data
cargo xtask run --ticks 720 --dump-every 30   # two years: the levels have to rise and then stop
cargo xtask regen-expected
cargo xtask bench --reps 100                  # A must not move: the coverage is not recomputed here
```

The filter on the second line has to **match**: check the count of tests run, not the colour. It did
not match when this file was written — see point 6 below — which is phase 12's lesson recurring in the
document that recorded it.

The dump at 720 ticks is the by-eye proof that closes the phase: the distribution of levels has to
rise and then **stop**, not oscillate and not grow forever. If it oscillates with validation green,
the bug is in the implementation and test 3 has to catch it — if it does not, the test is the weak
one.

The empty tick (`A`) has to be looked at and **has to stay where it was**: with no demographics
nothing changes the population, so the coverage is not recomputed. If `A` moves here, something is
invalidating the coverage that should not be, and it is better found now — in phase 14 it would be
indistinguishable from the expected cost.

**Done when:** test 3 (no oscillation) and tests 4–5 (levelling up does not touch the coverage) pass.
Those are the three that define the phase; the rest is correctness around the edges.

## How it went

The three defining tests are green and `A` did not move: 280 µs before, 282 µs after, medians of two
runs at `--reps 100` on the same machine, which is inside the noise this measure has shown before.
`G` did not move either. The dump at 720 ticks does what it had to: `minimal` goes `4/0/0` → `0/4/0`
→ `0/0/4` and stops, `hunger` settles at `3/0/5` — the five houses the farm covers climb to the top,
the three it does not stay at level 1 for ever. The `.ron` diff of the recordings is **one line**, the
dataset hash: not a single command moved, which is the cheapest possible confirmation that the
regeneration is about the tables and not about the log.

Six things came out differently from the plan above, and the reasons are worth more than the
decisions.

**1. `NoHysteresis` compares two thresholds of the *same* rung.** The formula written above,
`decay_threshold(l) >= level_up_threshold(l+1)`, mixes a 0-based index with a 1-based level and the
two happen to name the same entry. The relation that actually matters is *within* one rung: a house
rises to L at `up(L)` and falls from L below `decay(L)`, so the two rules overlap exactly when
`decay(L) > up(L)`. The band `decay(L)..up(L)` is the hysteresis, and it is checked for every level
from 2 up. The corollary is that **level 1's two thresholds are read by nobody** — nothing rises into
level 1 and there is no level 0 — so they are written as zero and a test in `sim-data` pins that down,
because a number in a table that decides nothing is the sort of thing somebody later balances.

**2. Step 6.1 keeps stepping the accumulators for the building's union, not the level's list.** This
looks like the wrong list and is the right one. With per-level requirements, a rung that introduces a
service the rung below does not ask for would be **unreachable**: nothing would ever move that
accumulator off zero, so its threshold could never be met, and the level would be dead without a
single test going red. It is also exactly what `House::satisfaction`'s doc comment promised in phase
12 — *the time already accumulated on a service it was receiving anyway was not a lie*. Which
services are **read** is a separate question, and there the answer is per level: the mood and the
decay check use the house's own rung, the level-up check the destination's.

**3. Decay and levelling up are one pass, not two.** With two passes, "a house on its way down must
not go back up in the same review" holds only *because* the hysteresis band is validated: the service
that failed `decay(L)` is also below `up(L)`, so the second pass cannot promote it. True today, and a
dependency that goes quiet the moment a table changes. One pass with at most one jump per house makes
it structural, and costs nothing.

**4. Eviction cannot happen yet, and that is provable.** Residents only ever arrive at construction,
capped at `max_residents(1)`, and `CapacityNotIncreasing` keeps the ladder from ever shrinking — so
the capacity a house falls back to is never below what it was born with. The branch is written and
tested all the same, because phase 14's conservation depends on the term, but the only honest way to
reach it is to put the residents there by hand. That is what the self dev-dependency on `sim-core`
with `features = ["test-util"]` is for: a crate depending on itself is how you turn one of your own
features on for your own tests.

The consequence for **test 10**: `two_empty_ticks_recompute_nothing` does *not* change its outcome as
this phase predicted, because no eviction can fire in it. It was rewritten anyway, into the form phase
14 will want — *nothing is recomputed while the population does not move* — and lengthened to cross
two reviews, so it now also witnesses that **levelling up invalidates nothing**.

**5. `Inconsistency` collapsed `ValidationErrorKind` rather than extending it.** The plan had
`sim-data` map each new case onto a variant of its own, which is the same enum written twice in two
crates. Instead `ValidationErrorKind::Inconsistent(Inconsistency)` is `#[error(transparent)]` and the
message lives in `sim-core`, next to the check; `sim-data` supplies only the RON path to blame. Adding
a check is now one place. Two variants were added beyond the list above: `NoHouse`, because it is the
only one of the three `is_house()` remedies that catches the building's list disappearing (with no
house, `InconsistentRequirements` has nothing to compare), and — in `sim-data`, since it is a property
of one field of one table — a rung that requires *nothing*, which `all()` over an empty list would
otherwise climb for free.

Only one broken fixture needed touching: the cross-table checks run after the per-table ones and only
if those are clean, so the six fixtures that fail on shape never reach them. `unsustainable_capacity.ron`
does reach them, and a buildings table of nothing but farms now produces four extra errors — no house,
and no provider of water at any rung — so it gained a house and a well at the **end** of the list,
where they do not shift the indices the expected report names.

**6. The verification command in this file had the phase-12 bug in it.** `cargo test -p sim-core
--release oscillation` matched zero tests, because the test was called `..._does_not_oscillate...`.
Green, and checking nothing. The test is now `the_level_shows_no_oscillation_with_constant_services`.
While fixing it, the property itself turned out to be weak for a second reason: a generated layout can
contain no served house at all, and monotone is trivially true of a city where nothing moves. It now
seeds a well, a farm and a house in a corner of the map the generator cannot reach, and asserts that
corner house climbs all the way in a year — so every one of the 2,000 cases witnesses real movement.

The balancing choice worth recording: **level 1 asks for water only**, levels 2 and 3 for water and
food. Three identical rungs would have left the per-level requirements doing nothing the building's
own list did not already do, and the mechanism would have gone untested in the production dataset. It
also gives `hunger` a split distribution instead of a flat one, which is the more legible recording.
