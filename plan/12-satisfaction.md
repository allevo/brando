# Phase 12 — House satisfaction

**Goal:** a served house reaches maximum satisfaction in `max / step_up` ticks **computed from the
`DataSet`**; take the water away and it goes back to zero in `max / step_down`.
**Depends on:** 11.
**Size:** M.
**Decisions involved:** [A10](open-decisions.md), A9, D5, tick order step 6.

## Why now

Because it is the first half of step 6, which has been an empty function since phase 04, and it is
the quantity every later phase reads: the levels (14), the births (15), attractiveness (16).
Introducing it on its own gives a goal you can work out by hand from the tables — `max / step_up`
ticks, nothing more — and which can therefore only be got wrong in one way.

Keeping it together with the levels would make an L-sized phase whose failure would not say **which**
of the two halves is broken: if a house does not level up, is it because the satisfaction is not
rising or because the threshold is not tripping? Separated, the second question already has an
answer.

## What gets built

### The field, in `sim-core/src/world.rs`

```rust
pub struct House {
    pub origin: TilePos,
    pub level: u8,
    pub residents: u16,
    pub served: ServiceFlags,
    /// How long the service has been there, **not how much of it arrives** (A10).
    ///
    /// Goes up by `step_up` when the service is satisfied this tick, down by
    /// `step_down` when it is missing, saturating in `0..=max`. One slot per
    /// service, including the ones the current level does not require: if the
    /// house levels up, the time already accumulated on a service it was
    /// receiving anyway was not a lie.
    pub satisfaction: [u8; ServiceKind::COUNT],
}
```

`u8` and not `i16` as [A10](open-decisions.md) had supposed: the accumulator is clamped to `0..=max`
and never needs the sign or the range. `House` stays small, and there is nothing extra to defend —
the tight budget is `Tile`'s, not this one's.

**Why not a resource level.** Measuring *how much* food enters a house would break phase 07's "no
partial consumption", and it is that choice that makes conservation an exact equality instead of an
inequality. It would also be degenerate: with capacity/output consistency
([A5](open-decisions.md)) a covered house always receives 100%. The reasoning at length is in
[A10](open-decisions.md).

### Step 6.1, in `sim-core/src/tick.rs`

```rust
/// Step 6 — levelling up, decay and migration.
///
/// The internal order is **game semantics** as much as the order of the ten steps,
/// and the same rule applies: do not reorder without regenerating the recordings
/// and writing down why. In this phase only 6.1 exists; phases 13, 14 and 15 add
/// the later sub-steps **below** it, never above.
fn houses_and_migration(world: &mut World) {
    update_satisfaction(world); // 6.1
}
```

`update_satisfaction` walks the houses in `HouseId` order and, for each `ServiceKind` the current
level requires, applies `step_up` or `step_down` depending on `served.get(k)`, saturating.

**Why 6.1 is the first sub-step and not the last.** It reads only what steps 3 and 4 have written
*this tick*, and all the rest of step 6 reads it. If it came after levelling up, a house would level
up on the previous tick's data: correct on average, unreadable in a recording you are trying to
follow by hand.

**The saturation is game semantics**, not a shortcut against overflow — the same distinction as the
full granary in phase 07, and it has to be commented as such. Service time does not accumulate
forever: past the maximum, one more month of water buys nothing.

### The asymmetry in `served`, which this phase dissolves

[A9](open-decisions.md) points out that `House::served` means two things depending on the bit: for
water "it is covered" (written by step 3), for food "it ate" (rewritten by step 4). With the
invariant *covered ⇒ always eats* the two coincide, so today the difference exists only on paper —
but this phase is the first to **read** that field, and reading it with two meanings is the trap A9
announced.

**Decision: `served` means "covered", for both bits.** Step 4 stops rewriting the food bit and
merely consumes. A covered house that does not eat becomes an invariant violation — which is exactly
what it has to be.

> **Correction, made while implementing.** The sentence that followed here said the violation "is
> already guarded by `covered_means_fed`". It is not, and the mistake is worth keeping written down
> because it is the shape of mistake this whole phase is about. That invariant compared
> `coverage().is_served(id, Food)` against `House::served`'s food bit — a real question precisely
> because the two had **independent sources**, step 3 and step 4. Make both bits come from step 3 and
> the comparison becomes `x == x`: the project would have lost a real check at the exact moment it
> started depending on it, and nothing would have gone red.
>
> What replaces it is a counter in `FoodTotals`, `covered_but_unfed`, incremented by step 4 when a
> house with a provider assigned fails to eat, and asserted at zero. It is diagnostic, outside the
> hash, free on the happy path — and it is a **better** question than the one it replaces, because it
> holds over the whole history instead of at the moment somebody looks. The 1,000-tick fuzz test
> checks it once at the end and thereby covers all 1,000 ticks.
>
> The general lesson, next to phase 09's: when two fields are made to agree by construction, the test
> that compared them does not become redundant — it becomes **vacuous**, which looks the same in CI
> and is the opposite thing.

The cost: if one day the invariant fell over (M3, when the goods come from a warehouse),
satisfaction would rise for a house that did not eat. That has to be written **now** in the field's
doc comment, because now is when it is understood; in M3 it would show up as an inexplicable
balancing bug. `covered_but_unfed` is what will say out loud that it has happened.

### The table, in `rules.ron`

```ron
satisfaction: (
    max: 100,
    // 25 ticks, a little under a month, from zero to the maximum.
    step_up: 4,
    // It is lost faster than it is gained: losing the water is an event,
    // getting it back is an investment.
    step_down: 8,
    // Written as a tuple, not a list: see below.
    mood_thresholds: (25, 50, 75),
),
```

Validation of shape: the three values greater than zero, `step_up` and `step_down` no greater than
`max`. The **cross-table** check against the level thresholds arrives with phase 13, which
introduces them.

The bands live here too, and they are a `[u8; Mood::COUNT - 1]` rather than a `Vec`: the arity is
decided by the enum, so a table with two thresholds or four is a **deserialisation** error and needs
no check anybody has to remember to write. The price is that RON spells a fixed-length array as a
tuple, `(25, 50, 75)` — which is what `size: (2, 2)` in `buildings.ron` already does, and for the
same reason.

Two order checks that are not decoration: strictly ascending, and the first one above zero. The
second is what makes "satisfaction 0 ⇒ `Desperate`" true **by construction**, and the event's
contract for a newly-built house rests on it.

### The event

Not one per house per tick: that would be 40,000 events per tick, i.e. exactly what the boundary
with the renderer forbids. The renderer needs a mood, not a number.

```rust
/// A house's satisfaction band. It is what the renderer draws, and it is
/// deliberately coarse: the event is emitted on a change of **band**, not of value.
pub enum Mood { Desperate, Unhappy, Happy, Thriving }

Event::HouseMoodChanged { house: HouseId, mood: Mood }
```

The band boundaries live in `rules.ron`, not in the code. The mood is the minimum across the
services the level requires: a house with water and no food is desperate, not half happy.

Two things settled while implementing. The mood is **not** a field of `House`: it is derived from the
satisfaction, and a second copy in the state would be one more thing to hash, to keep in step and to
get wrong. It is computed twice per house per tick — once into the start-of-tick snapshot step 10
already builds for the coverage deltas, once when comparing — which is a handful of `u8` comparisons
against a pass that was being made anyway.

And a **newly-built house emits nothing**: it is born at zero, which the validation above makes
always `Desperate`, so the snapshot's default for a house that did not exist yet is the right answer
rather than a placeholder. `Desperate` is the mood the renderer has to assume from `HousePlaced`, and
that belongs in the event's doc comment, not here.

### The hash

`h.update(&c.satisfaction);` in `hash_world`'s loop over the houses. Phase 11's canary is a reminder
on its own.

## Out of scope

Any **consequence** of satisfaction: levelling up, decay, migration, births. This phase computes it
and nothing more, just as phase 06 recorded who was served without drawing consequences. The
thresholds, the hysteresis and the monthly evaluation are phase 13.

## Tests

1. **Going up** (*the goal*): a house served with water and food; after `max / step_up` ticks —
   **read from the `DataSet`, never written in the test** — the satisfaction is at maximum on both
   services. It is the lesson of phase 07's test 3: a hardcoded number breaks on every rebalancing
   without signalling anything real.
2. **Coming down**: with the well demolished, after `max / step_down` ticks the water satisfaction
   is zero and **the food one is still at maximum**. The second half is the one that counts: it pins
   down that the accumulators are independent.
3. **Saturation at both ends** (property test): after any sequence of commands and any number of
   ticks, `satisfaction[k] <= max` for every house and service. Never negative is guaranteed by the
   type, and it is half the reason it is a `u8`.
4. **A new house starts at zero**, not at maximum: it is born uncovered (step 3 covers it in the
   same tick, but the accumulator starts from zero anyway) and its first levelling up costs the full
   time.
5. **Services not required**: a service outside the level's `required_services` is not touched.
   Today the house requires both, so the test is written on a fixture with a house that requires
   only one — and it is the case phase 13 will make real.

   In practice that has to be a **second dataset**, not a second kind of house in the same one:
   `House` does not record its `BuildingKindId`, so two house kinds are indistinguishable once
   placed. `common::dataset_where_a_house_requires(&[Water])` is the fixture, and the case it
   produces is sharper than expected — coverage does not read `required_services` either (the gap A9
   names), so the farm assigns itself to that house anyway, `served` says food is there, and the
   accumulator still has to stay at zero. It is not "the service is absent", it is "the service is
   present and irrelevant", which is the one phase 13 needs.
6. **An event on the band, not on the value**: a house going from satisfaction 40 to 44 within the
   same band emits nothing; one that crosses a boundary emits exactly once. It is phase 07's test 7
   rewritten for the mood, and for the same reason.
7. **The hash covers the satisfaction**: a perturbation in `the_hash_covers_the_whole_state`.

## Verification

```sh
cargo test -p sim-core satisfaction
PROPTEST_CASES=2000 cargo test -p sim-core --release satisfaction
cargo xtask run --ticks 120 --dump-every 30     # the new column has to move readably
cargo xtask regen-expected                      # a deliberate regeneration: a new field in the state
cargo xtask bench                               # measure A: step 6 had never cost anything
```

The `xtask run` dump gains a column with the average satisfaction — of the **worst** required
service, not the mean of the two, so the column and the mood say the same thing. It serves to close
the phase by eye: if it rises too fast or too slowly, that is balancing (`sim-data`), not code — but
it has to be looked at now, because `regen-expected` freezes it into a recording. At `--dump-every
30` the climb is already over by the first row; `--ticks 40 --dump-every 4` is what actually shows
the curve.

`bench` is worth looking at in this phase more than in the others: step 6 has always cost zero, and
from here it costs one pass over every house on every tick. The measure to compare is `A`, the empty
tick, which [A11](open-decisions.md) names as the number to keep an eye on.

Two notes from doing it.

**The filter has to match, and cargo filters on the test's name, not on its file.** None of this
phase's six tests naturally contains the word "satisfaction", so `cargo test -p sim-core
satisfaction` ran zero of them while looking green. They live in a `mod satisfaction { … }` inside
`tests/satisfaction.rs` so the names carry the phase's word — cheaper than bending six names, and it
is the same reason the unit tests in `src/satisfaction.rs` were matched all along.

**Reading the `.hashes` diff, this phase, says nothing.** Protocol point 5 asks for the first
diverging tick, and here both movers act at tick 0 — the new field in the state and the new block in
the dataset hash — so divergence at the first checkpoint is what you would see whatever happened.
What rules out a third cause is the phase-11 corroboration, and here it is unusually strong, because
this phase is defined by having **no consequences**: `cargo xtask run --ticks 360` on both scenarios,
from a worktree at the previous commit and from this one, has to print the same table row for row.
It does, the new column aside.

**Done when:** tests 1 and 2 pass with the ticks computed from the `DataSet`, and the dump at 120
ticks can be read.

## How it went

The curve behaves as designed: the houses go up at tick 2, the satisfaction is at 100 at tick 27 —
25 ticks, `max / step_up`, a little under a month. Nothing to rebalance.

`A` at the reference scale went from ~254 µs to ~285 µs, +12%, about 8 ns per house per tick for
step 6.1 plus the mood in the snapshot and in the comparison. At mid game the difference is inside
the machine's noise. Both figures are medians of two runs at `--reps 100` on the same machine; a
single run at the default reps had `A` at mid game apparently rising 2.3×, which was noise, and
that is worth remembering the next time this number is used to judge something.

`G` did not move, as expected: nothing in this phase touches step 3.
