# Decisions

**This file answers one question: why is it this way?** It is the project's decision register and
the target of every `A<n>` tag in the code. Whoever is looking for "why does the code do it this way
and not that way" finds the answer here.

For each decision: the recommendation the phases assumed, the reasoning, what it would cost to decide
otherwise, and — added after implementation — how it really went. That last part is almost always the
more useful information, because it diverges.

**This file is append-only.** A decision is never rewritten to match what the code does now: that
would destroy the only record of why the code changed. It is *amended* with a dated block, or
superseded by a later decision that names the one it replaces.

**And it holds only decisions already taken.** An entry is never written ahead of the work that
settles it — not for a phase that is planned and unbuilt, not to stop something being forgotten. Being
append-only is what makes this strict: an early entry cannot be withdrawn, only amended, so the
cheapest repair costs more than never having written it. Numbers are not reserved ahead of time
either. Until the work is done the argument belongs in that phase's file under `plan/`, which says on
its face that it is a prediction. The rule and the two cases that look like exceptions are in
[CLAUDE.md](CLAUDE.md).

The architectural decisions `D1`–`D7` are the project's constitution and live in
[CLAUDE.md](CLAUDE.md). The `A` codes here are implementation decisions, taken as the work went on.
Code cites both by id and never by path — an id survives any reorganisation of these files.

> **Two decisions are open**, and neither is left here as prose to be forgotten. Each holds a
> numbered slot in [ROADMAP.md](ROADMAP.md) at the point where it has to be closed:
> **A18** at **14.5**, before phase 15, and **A17** at **18.5**, before M2.

## The state at the end of M0

| # | Outcome | Note |
|---|---|---|
| A1 | **Closed**, as recommended | `Milli(i32)` and `Coins(i32)` kept separate, no bare operators |
| A2 | **Closed, differently** | `Arc<DataSet>` in the `World`, but the *definitions* had to move into `sim-core` |
| A3 | **Closed**, as recommended | a hand-written hash, plus a test that checks it covers every field |
| A4 | **Closed**, as recommended | sides up to 256, recordings on 32×32, a tripwire at 200×200 |
| A5 | **Closed with a discovery**, then corrected | hunger turned out to be an absorbing state; dissolved after M0 by making capacity and output consistent: see below |
| A6 | **Closed**, as recommended | 30 ticks/month, 360/year, in `rules.ron` |

None stayed open. The two worth reading are A2 and A5, because both ended up somewhere other than
where they had been written.

## Decisions after M0

| # | Outcome | Note |
|---|---|---|
| A7 | **Closed**: the commands stay an enum | a trait would break `Copy`, serde and the recordings' format |
| A8 | **Closed**: the `Arc<DataSet>` stays | it serves `World: Clone`, not the borrow checker — **amended**, read the entry |
| A9 | **Closed**: water is coverage, not a resource | but `required_services` is declarative data M1 has to start reading |
| A10 | **Closed in principle**, to be implemented in M1 | satisfaction is an accumulator of time, not a resource level |
| A11 | **Closed: not doing it**, with the condition for reopening it | targeted invalidation of the coverage; the cost was elsewhere, and removing it gave 6.5× with no new state |

They came out of clearing up a since-deleted `dubbi.md` ("open questions", 2026-08-09): three were
design questions, and the answers were worth more than the question. A9 and A10 are prerequisites of
M1 phases 1–2. A11 is the only one that says **not** to do something, and it is the one with the most
useful story: the starting hypothesis was wrong, and the measurement said so before it got expensive.

## Decisions taken while planning M1

| # | Outcome | Note |
|---|---|---|
| A12 | **Taken**, phases 13–15 | **the services chase the population**: capacity is counted on the residents present, and the coverage is recomputed when anyone moves |
| A13 | **Taken**, phase 11 | game difficulty is a new axis: a table of profiles, an id in the `World` and in the replay's header |
| A14 | **Taken**, phase 14 — **done** | four-flow demographics, aggregated; a base rate with jitter from a seeded RNG |
| A15 | **Taken**, phases 15–16 | attractiveness = average satisfaction + free places, and the tax rate **only** from phase 16 |
| A16 | **Taken**, phase 17 | the objectives live in the `World`, `sim-scenario` builds them |
| A17 | **TO_BE_DECIDED** — [ROADMAP 18.5](ROADMAP.md), before M2 | the cost of recomputing the coverage every tick, which is A12's price — **measured in phase 14**, and two of its three expectations were wrong |

A12 is the one to read: it is a *gameplay* choice paid for in computation time and in test coverage,
and it was taken in full knowledge. A17 is the first genuinely open decision of the project since the
end of M0, and that is no accident: it is the debt A12 opens.

Of these, only A14 has been written up with how it really went. A12 and A13 are implemented but not
yet reviewed; A15 and A16 are still **predictions**. When M1 closes (phase 18) each gains the same
treatment — which for A2, A5 and A11 has been the most useful information in the document.

## Decisions from the bug hunt after phase 13

| # | Outcome | Note |
|---|---|---|
| A18 | **TO_BE_DECIDED** — [ROADMAP 14.5](ROADMAP.md), before phase 15 | an empty house consumes no provider capacity, so on `hard` one well serves unboundedly many |

It came out of a review of the whole tree on 2026-08-11 (see
[the report](plan/bug-hunt-2026-08-11.md)), which turned up six things. Five were bugs and were fixed
in the same batch; this one is not a bug, it is a question nobody had been asked, and it is the
second entry in this file to be genuinely open at the same time as another.

The pattern worth noticing is that it sits **between** two decisions rather than inside either:
[A12](#a12--the-services-chase-the-population) chose to count capacity in residents and
[A13](#a13--game-difficulty-is-an-axis-of-the-state) chose a profile that starts houses at zero of
them. Each is right on its own; the degenerate case is what neither was looking at. It is the third
time in this document that the interesting thing has been an interaction and not a choice — A2 and
A16 were both the dependency graph dictating a boundary — and it is the reason the register is worth
keeping.

## Decisions from the vocabulary review

| # | Outcome | Note |
|---|---|---|
| A19 | **Taken** (2026-08-12) | which hard words earn their place: the domain's own words stay, synonym choices go |

It came out of reading the whole tree against `GLOSSARY.md`'s opening promise — that no word sends
you to a dictionary — for the first time since M0. The rule it produced now lives in `CLAUDE.md`'s
code conventions; the reasoning is below, and it is the only entry in this file whose recommendation
was **overturned in full**.

---

## A1 — Money is not `Milli`

**Recommendation:** `Milli(i32)` stays for the fractional quantities (food, labour, wear). Money is a
separate whole-number newtype, `Coins(i32)`, because it has no in-game fractions and using
thousandths would halve the useful range for nothing.

`Milli(i32)` covers ±2,147,483 units. That is enough for food and population (a target of 15,000
residents, D5). The treasury over a long game can exceed it, so keeping it out of `Milli` is also
prudence about the range.

*If the opposite is decided:* a single type simplifies the signatures, but `Milli(i64)` is needed and
the impact on `size_of::<House>()` has to be checked.

---

## A2 — Where the `DataSet` lives

`CLAUDE.md` describes `step(&mut World, &[Command])`, but the systems need the tables.

**Recommendation:** `World` holds an `Arc<DataSet>` and a `data_hash: [u8; 32]` field. The signature
stays as declared, loading (I/O) stays outside the core, and the dataset's hash goes into the state
hash: if someone changes a balancing number, the recorded replay fails **immediately and for the
right reason**, instead of diverging ten ticks later through a side effect.

*Alternative:* pass `&DataSet` as a parameter to `step`. Purer, but it changes the declared signature
and propagates a parameter into every internal function.

**How it was really resolved (phase 04).** The recommendation holds, but it has a consequence phase
03 had not foreseen: if the `World` holds an `Arc<DataSet>`, then `DataSet` has to be visible from
`sim-core`, and `sim-core` cannot depend on `sim-data` without inverting the dependency graph.

The **definitions** (`DataSet`, `Rules`, `BuildingDef`, `ServiceDef`, `TerrainDef`) and the dataset
hash were therefore moved into `sim-core::data`: they are pure structs, with no I/O, and they belong
to the core's vocabulary. What stays in `sim-data` is the RON parsing, the validation and the I/O —
that is, exactly what D4 forbids the core. `sim-data` re-exports the types for the convenience of
whoever loads the tables, so callers do not have to know where they are defined.

The boundary that matters stays where it was: *no balancing number in the code*, and *no I/O in the
core*.

---

## A3 — The state hash is written by hand, not delegated to serde

**Recommendation:** `fn hash_world(&World) -> [u8; 32]` feeds a `blake3` hasher field by field in an
order made explicit in the code.

The reason: with `serde` the hash depends on the order the fields are declared in and on the format,
so a harmless refactor (moving a field) would invalidate every recording without anyone having
changed the semantics. It is exactly the false positive that makes the project's most valuable test
useless. The cost is that adding a field to the state requires adding it to the hash by hand —
mitigated by a coverage test (see phase 08).

---

## A4 — The size of the map in M0

**Recommendation:** `Grid` accepts a width and a height up to 256, with `TileIdx(u16)`
(D: a small `Tile`, `u16` indices). M0's test scenarios use 32×32 grids: a recorded replay you can
read by eye is worth more than a realistic one. A 200×200 case exists only as a performance tripwire
in phase 09.

---

## A5 — Food in M0 has no walkers

**Recommendation:** in M0 the farm is a **service provider** (like the well): it produces into a
local stock and covers the houses within range along the roads, consuming from that stock.

It is not the final production chain: warehouses and real logistics walkers are M3 (D3). It is
however the smallest version that closes an observable production→consumption loop, and it introduces
no concept that will have to be removed: the coverage stays valid, in M3 only *where* the goods come
from changes.

*To keep an eye on:* the risk is that the farm's range becomes a balancing number M1 gets built on,
and that M3 has to dismantle it. Accepted, and noted in the farm's RON table.

**What emerged while implementing it (phase 07).** The simplification has a consequence nobody had
foreseen while writing the plan: **the hungry state is absorbing for a house already assigned**. A
contest between providers is won by the first (phase 06) and the capacity is counted in *houses*, not
residents; so a house covered by a farm running a deficit occupies a place no other farm can take
over, and carries on not eating forever.

A second farm only helps the houses the first had left **outside** its own capacity. Both behaviours
have a test that pins them down (`a_second_farm_covers_the_houses_left_out`,
`a_hungry_house_is_not_saved_by_a_second_farm`), the second of which is written to **change its
outcome** when the capacity becomes "residents served" in M1: it will be the signal that the
simplification has been dissolved, not a regression.

An entry was also added to `FoodTotals`, `lost_to_demolition`: demolishing a full farm makes its
stock vanish from the world, and without recording it conservation stopped being an equality. The
same reason `lost_to_full_stock` exists.

**How it was dissolved (2026-08-08, a prerequisite of M1 phase 1).** Capacity is now counted in
**residents served**. It was done then and not inside M1 because house levels make the population
vary from house to house, and with a variable population a capacity in houses no longer says how many
people a provider can serve.

Changing the unit, on its own, **achieved nothing**: converting the values in the table by the number
of residents per house (well 8 → 32, farm 6 → 24) left the behaviour identical tile for tile —
verified by running the two scenarios over a game year and comparing the output. The absorbing state
was still there, expressed in another unit. It is why the two steps are separate commits: the first
demonstrates it.

What dissolved it is the second half: **a producer's capacity has to be consistent with what its
output sustains**. The farm produces 400 milli/tick and a resident eats 20, so it sustains 20 of them
— five houses, not six. From this follows an invariant that was not there before, and it is the real
gain: *a house covered by the food service always eats*
(`invariants.rs::covered_means_fed`). Hunger is still reachable, but it is a lack of **coverage** and
it is cured by building.

The consistency is a **validation check**, not a convention in the comments:
`DataSet::unsustainable_food_capacity` in `sim-core`, called from `sim-data`'s validation. It lives in
the core because that is where the definitions live (A2) and because it is also needed by
`sim-core`'s test fixture, which does not go through `sim-data` and could otherwise drift onto a
balancing that would be rejected in production. The arithmetic is for the worst case, an empty stock:
the smaller of one tick's output and what the granary can hold. **It has to go in M3** along with A5:
once the goods arrive from a warehouse via walkers (D3), capacity will stop depending on local output.

The filling rule was pinned down too, and it will become observable with mixed populations: no partial
assignment, and whoever does not fit is **skipped**, they do not act as a barrier. Stopping at the
first house that does not fit would keep distance as an absolute priority, but it would leave places
unused — and the capacity in the table would stop telling the truth, which would break precisely the
consistency with the output. The cost is that a house further away can go ahead of a nearer one that
does not fit; accepted. The rule is a pure function, `coverage::pick_within_capacity`, with a
table-driven test covering the mixed-population cases M0 cannot yet produce.

The test written in phase 07 to change its outcome has changed its outcome, and is now
`hunger_is_cured_by_building_a_second_farm`.

---

## A6 — The rhythm of time

`CLAUDE.md` fixes 1 tick = 1 day and the month as a fixed multiple, without giving the number.

**Recommendation:** 30 ticks = 1 month, 12 months = 1 year (360 ticks). The value lives in `sim-data`
(`rules.ron`), not in the code: scenario objectives are expressed in months and they are needed by M1.

---

## A7 — Commands stay an enum, not a trait

**Decision:** `Command` stays a closed enum, applied by a `match` that calls three free functions
(`tick.rs`). No `trait TryApply`, no `Box<dyn …>`.

The reason is not stylistic, it is that the trait would break things that exist:

- `Command` is `Copy` and `Serialize`/`Deserialize`, and a save file is `seed + Vec<Command>`
  serialised in RON with `struct_names(true)`. A trait object breaks `Copy`, serde's derive,
  `PartialEq`/`Eq` — and **the format of the recordings already committed**.
- D4 wants a **closed** vocabulary: a trait invites external implementations, i.e. commands the
  determinism log cannot represent.
- M3's LLM adapter needs an enumerable schema: `Intent → Vec<Command>` with a finite, inspectable set.

And there is nothing to save: the `match` is **five lines and three variants**, and the logic already
sits in three separate functions.

*If the irritation is something else* — `tick.rs` is the longest file in the core (434 lines) and
mixes dispatch, applying the commands, the ten steps and seven helpers — then the answer is to split
into **modules**, not into trait objects. They are two different problems with two different
solutions.

---

## A8 — `World` holds an `Arc<DataSet>`, and it is not for the borrow checker

A2 had decided on the `Arc` without saying why it was an `Arc` and not a value. The answer:

**`World` is `Clone` and gets cloned a great deal.** `rejected_commands_mutate_nothing` clones it
once per tick for every proptest case — thousands of times per run — and
`the_hash_covers_the_whole_state` once per perturbation. With the `Arc` the field costs a refcount
bump; without it, every clone would redo the allocations of `Vec<BuildingDef>`, of the ids' `String`s
and of the per-level `Vec<u16>`s. On top of that `sim-replay` and `xtask` build several `World`s from
the same dataset loaded once.

The alternatives are worse: `World<'a>` propagates a lifetime into `Scenario`, into the replay
signatures and into every test; passing `&DataSet` to `step` changes the signature declared in
`CLAUDE.md` (already considered and rejected in A2).

**`Arc` and not `Rc`** even though the core is single-threaded: `Arc` makes `World: Send`, and `xtask`
has batches of games for automatic balancing on its roadmap — exactly the case where you will want to
parallelise.

*A note on a false clue.* `production.rs` does `Arc::clone(&world.data)` and looks like it is using
the `Arc` for nothing. There it really is a way around the borrow checker: `take_from_stock(world, …)`
takes the whole `&mut World`, so a `&world.data` alive for the length of the function would be
incompatible. It could be removed by passing `take_from_stock` only the fields it touches, but that
costs an atomic increment per tick and would make the signature noisier. Left as it is, on purpose.

> **Amended 2026-08-14, when `World` stopped being `Clone`
> ([A22](#a22--world-is-not-clone-and-the-compiler-says-so)).** The first sentence of this entry —
> *`World` is `Clone` and gets cloned a great deal* — was the whole of the answer, and it is now
> false: the derive is gone in every configuration and both callers it named by name,
> `rejected_commands_mutate_nothing` and `the_hash_covers_the_whole_state`, build a second world
> instead of copying one.
>
> **The `Arc` stays, and the decision is not reversed — its load moves onto the parts of the
> argument that survive.** They were already written above and were merely the smaller half:
> `sim-replay` and `xtask` build several worlds from one dataset loaded once, and so, now, do the
> tests — `twins` hands one `Arc` to two worlds precisely so that they run on the same tables and
> not on equal ones. The alternatives are untouched by any of this: `World<'a>` still propagates a
> lifetime into every replay signature, and `&DataSet` as a parameter of `step` was already refused
> in A2. `Arc` and not `Rc` is likewise untouched — it buys `World: Send`, for the batched balancing
> on xtask's roadmap.
>
> Worth keeping for the shape of it: an entry whose stated reason dies while its conclusion holds.
> Had the reason been the only one, this would have been a reversal, and the way to tell the two
> apart was to go back and read what the entry actually claimed.

---

## A9 — Water is coverage, not a resource

**Decision:** the well declares neither `output_per_tick` nor `max_stock`, so step 4 never touches it.
Water has a range and a capacity, and that is all. It is Zeus's model, and it is the right one: a
production chain for water would be one more resource to balance with no gameplay payoff.

**The gap next to it, which has to be closed in M1.** `required_services` (`["water","food"]` on the
house) is read **only** by `BuildingDef::is_house()`: it serves to classify the kind of building, not
to decide anything in the simulation. The coverage assigns any reachable house without ever looking at
whether it actually needs that service. Today it is not observable — there is only one kind of house —
but it is the field M1 has to start reading in earnest, once the levels have different service
requirements.

**An asymmetry to dissolve at the same time.** `House::served` means two things depending on the bit:
for water "it is covered" (written by step 3), for food "it ate" (rewritten by step 4). With the
invariant *covered ⇒ always eats* the two always coincide, so today the difference exists only on
paper — but it is a trap for whoever reads that field in M1.

> **Closed in phase 12**, the first phase to read that field. Both bits now mean "covered" and step 3
> is the only writer; step 4 consumes and says nothing.
>
> The part that was not foreseen is what it cost. `covered_means_fed` was the invariant guarding
> *covered ⇒ eats*, and it worked by comparing the two bits against the coverage — a real question
> only because the sources were independent. Unifying them turned it into `x == x`: a test that would
> have stayed green for ever, on the very property the new code depends on. It was replaced by
> `FoodTotals::covered_but_unfed`, a counter step 4 raises when a house with a provider does not eat,
> asserted at zero over a whole run.
>
> **The gap A9 named is still open**, and phase 12 made it observable rather than closing it:
> coverage still assigns a service to a house that does not require it. Today that is harmless —
> satisfaction ignores the unrequired slot — and `dataset_where_a_house_requires` is the fixture that
> pins the behaviour down. It becomes a real decision in phase 13, when levels stop requiring the
> same things.

> **Amended 2026-08-13, reviewing the documents.** The gap now has an entry of its own,
> [A20](#a20--a-house-is-covered-by-services-its-level-does-not-require), and a slot in the roadmap.
> Phase 13 arrived and the prediction above held exactly: with the first rung asking for water only,
> the unrequired assignment stopped being harmless — a hut is fed, and it takes places on the farm.
> What the prediction got wrong is that phase 13 did not turn it into "a real decision", because
> nobody was looking at it while the levels were being built. It took a reader asking *does the food
> a house needs really depend on its residents* to find it, four phases later. A gap named inside a
> closed entry is a gap nobody is scheduled to close: that is why it is now an entry with a status
> and a number, which is the mechanism A17 and A18 already proved was needed.

---

## A10 — House satisfaction is an accumulator of time, not a resource level

It is needed by M1 phases 1–2 (levelling up, decay, migration) and has to be decided before writing
them.

**Not a "level of food that arrived".** Measuring *how much* food enters a house would break "no
partial consumption" (phase 07), and it is that choice that makes conservation an **exact equality**
instead of an inequality — that is, that makes the project's most valuable test able to find a bug
instead of merely reassuring. It would also be degenerate: with capacity/output consistency (A5) a
covered house always receives 100%.

**Decision:** one accumulator per service,
`House { satisfaction: [i16; ServiceKind::COUNT] }`, which rises when the service is there and falls
when it is missing. It does not measure how much arrives, it measures **how long** it has been
arriving — which is what "I leave / I stay / I grow" actually needs, and which is not degenerate even
today, because coverage does get lost (demolish a farm, break a road).

> **Amended while planning M1.** The type is `[u8; ServiceKind::COUNT]`, not `[i16; …]`: the
> accumulator is clamped to `0..=max` and never needs the sign or the range, and that way `House`
> stays small. The rest of the decision holds unchanged, and [phase 12](plan/12-satisfaction.md)
> implements it to the letter — with one addition it did not foresee, the `covered_but_unfed`
> counter that A9 above explains.

With **different thresholds for levelling up and decaying**: the hysteresis band stops the city
oscillating on the boundary every tick, and it is also what keeps the recordings stable. Every
threshold in RON (D6).

**On migration.** D5 says the unit is the house, not the individual, and immigrants as real walkers are
D3/M3. In M1 migration has to stay **aggregate**: a city-wide attractiveness index decides the net
flow, which fills the houses that have room. Then "migrants only go where life is good" is not a
separate mechanism — it is the same rule as levelling up (an unserved house does not accept new
residents and in the long run loses them). One mechanism fewer for the same behaviour.

**Sequence:** it has to be done together with the house levels, not before. It is the moment
`residents` stops being constant, and it is why the services' capacity has already been moved to
residents (A5). When it arrives, `RngDomain::Migration` is used for the first time and
`different_seeds_give_different_hashes`, today `#[ignore]`, is re-enabled and has to pass.

**Watch out for one mechanical detail:** every new field on `House` has to be added to `hash_world` by
hand (A3). If it is forgotten, `the_hash_covers_the_whole_state` fails — the net is there.

---

## A11 — Targeted invalidation of the coverage is not being done (for now)

Step 3 cost 19.83 ms per recomputation at the reference scale, and the obvious hypothesis was that the
problem was *how many times* the BFS gets redone: any accepted command invalidates all 1,219
providers. The obvious cure was to cache the reached tiles per provider — the BFS depends only on the
roads, the position and the range, **not on the houses**, so placing a house ought to invalidate
nothing.

**It was not the right hypothesis.** The cost was not the number of BFS runs, it was what each one
dragged along with it: three `BTreeMap`s in the inner loop and a scratch buffer the size of the grid
allocated per provider. Removing them gave **6.5×** without adding a byte of state
([09-invariants-closeout.md](plan/09-invariants-closeout.md) for the numbers).

**Decision:** with `G` at 3.05 ms and 2.5 µs per provider, the cache is not being built now.

- It would eliminate only the BFS traversal itself, i.e. part of the remaining 2.5 µs — not all of it:
  sorting the candidates, filling the capacity and the entrances all remain.
- It would do nothing for `F`, the case where a road gets laid: there the topology changes and every
  BFS has to be redone anyway. Today `F ≈ D`, so it would cover half the cases.
- In exchange it asks for the `World`'s first **persistent derived structure** (~0.5 MB, bigger than
  the grid), cloned on every `World::clone`, and an invalidation contract somebody has to remember to
  honour. It is also the first of these changes that can introduce a correctness bug instead of just a
  slowness one.

The other three were bit-identical rewrites of code that was already there. This is a different
category, and `CLAUDE.md` is explicit about both things: *do not optimise before the profiler*, and
*every new system has to be reachable and observable*.

**When to reopen it.** When a batch of M2 games shows the time dominated by the ticks where building
happens; or when M1 grows the number of providers a lot; or when a profiler shows `bfs_roads` as the
dominant entry in `compute_from_scratch`. Until then, the number to watch is not `D` but `A` — 248 µs
paid on **every** tick, against 3.35 ms paid only when the player builds.

**If it does get done, three things are already decided**, because they are the traps and not the
details:

1. The cache has to be indexed by the **whole key** (`slotmap::SecondaryMap<BuildingId, _>`), never by
   the slot index alone. A slot reused by a new building would silently inherit the old one's tiles,
   and if the range and the roads coincide the coverage would come out wrong **with no signal at
   all**. `SecondaryMap` compares the version too and treats a mismatch as absence: the miss is
   automatic.
2. `compute_from_scratch` has to stay **cache-free**. It is `coverage_equivalence`'s oracle: if it
   read the world's cache, phase 06's most important test would become a tautology. A test is needed
   to pin that down, poisoning the cache and checking that `compute_from_scratch` ignores it.
3. Store the **range** the tiles were computed with alongside them. That way the entry self-validates
   against the building's level (M1) instead of depending on who remembers to invalidate it. The
   origin stays unguarded: if a `Command::Move` ever arrives, it will have to invalidate the cache by
   hand.

---

## A12 — The services chase the population

A provider's capacity is consumed by the **residents present**, not by the house's capacity. A house
with eight places and two residents weighs two. It is the gameplay choice that decides whether the
city gets built ahead of the curve or behind it, and it is worth writing out in full because it has a
price that gets paid elsewhere.

### The problem it opens

In M0 `residents` is written once at construction and never changes again, so `coverage.rs` can count
the capacity against it with no consequences. With the demographics (phase 14) it changes every tick,
and the coverage computed at step 3 goes stale within the same tick. Two things suffer, in two
different ways.

**Growth breaks *a house covered by food always eats*** (A5). Five houses of four residents on one
farm are 400 milli/tick against 400 produced: an exact tie, by construction of the validation. At
eight residents the demand doubles, the stock holds for fifty-odd ticks, then `take_from_stock()`
returns `false`.

**Both growth and decline break `coverage_equivalence`.** Capacity 20, candidates `A4 B4 C4 D4 E4` all
assigned. If `A` rises to 8, `compute_from_scratch` assigns `A8 B4 C4 D4` and drops `E`; if `B` falls
to 2, the used amount drops to 18 and lets in an `F2` that the skip rule had left out. In both cases
the stored coverage and the from-scratch one diverge, on a test nobody got wrong.

### The decision, and what it costs

**The coverage is invalidated every time the population moves.** Step 6 finishes by marking the
providers dirty, and the next tick's step 3 redoes the assignment on the real population. Every
invariant stays standing, none has to be weakened for convenience.

The price is direct: the recomputation goes from "when the player builds" to "almost every tick". `G`
is 3.05 ms against the empty tick's 248 µs, so the typical tick heads towards ~3.3 ms — **~13×**. Ten
years of play at 200×200 go from ~0.9 s to ~12 s, and a batch of a hundred games for automatic
balancing from a minute and a half to twenty minutes. Requirement 2 of `CLAUDE.md` — the game is
drivable by an AI that runs many games — is the one that pays.

It does not get optimised inside M1: the cost has to be **measured and attributed** (phases 14 and 18)
and the countermeasures are [A17](#a17--the-per-tick-recomputation-cost), to
be closed before M2. It is A11's lesson applied beforehand instead of afterwards: the obvious
hypothesis about where the cost lies has already been wrong once.

There is also a cost that is not measured in microseconds. `coverage_equivalence` compares at the
**end of the tick**, i.e. after step 6 has moved the population: with the demographics running it
always diverges, by construction. It has to be split in two — the original form on a zero-rate
dataset, plus a direct test of the invalidation contract — and the two halves together check less than
the original did. The guard on step 3 gets weaker, and it is why A17 has to be closed before anyone
optimises on top of it.

### What it buys

The gameplay dynamic it exists for. The services get built **behind** the population, not ahead of it:
the city grows, outgrows its own wells, the coverage starts falling short for someone, satisfaction
drops, the player sees the problem and builds. The overshoot is the signal, not a fault — and it is
the loop that makes the game a game instead of a planner.

Two technical consequences, both favourable:

- **The food arithmetic becomes tight.** Capacity and consumption are now in the same unit, so
  `capacity × consumption ≤ output` is exact instead of an upper bound. No output wasted on half-empty
  houses, and the check on it does not change a line.
- **Levelling up does not touch the coverage.** Going up a level brings no people in, it brings
  permission to hold more of them: no gate on levelling up is needed, and phase 13 is simpler than it
  looked.

**How the second one really went** (phase 13, done). It held exactly as written, and it is measured
rather than asserted: `A` at the reference scale is 280 µs before and 282 µs after, and the recorded
recomputation count does not move across a review that promotes a house. The gate that an early draft
of phase 13 wanted was never written. What the phase did add is the **other** half of the same
statement, as a test: `population_stays_consistent` now also says that a promoted house gains
permission and not people, so if levelling up ever started bringing residents in, an invariant would
go red before the performance did.

The only place `residents` moves in phase 13 is eviction on decay, and that does invalidate the
coverage — the first application of the contract phase 14 generalises. It cannot fire yet under a
dataset that passes validation, which is written up in that phase's notes.

### The alternative that was rejected, and why it was looked at

Counting the capacity in **places** — the level's capacity, taken or not. The coverage would go back
to being a function of quantities that rarely change (the grid, the buildings, the levels), so
`coverage_equivalence` would stay intact with no changes, the invalidation would go back to being
rare, and the empty tick would stay at 248 µs. Technically it is the better option on every axis.

It was rejected because it turns the feel of the game inside out: with places, a well fills up with
empty houses and the player has to size the services **before** the people arrive. The game becomes an
exercise in planning ahead instead of one in reacting. Between a technical constraint and a gameplay
dynamic, the dynamic won, and the constraint became A17.

Worth bearing in mind if A17 one day fails to close: switching to places is
`rules.max_residents(house.level)` in place of `house.residents` in `coverage.rs`, i.e. one line. It
is the way out, and it costs exactly the dynamic that was wanted.

### Another thing that is not being done: sticky coverage

Giving priority to houses already served would reduce the churn — with capacity counted on residents,
the marginal house comes in and goes out every time the demand crosses the threshold. But it would
make the coverage depend on **history**, and `compute_from_scratch` has no history:
`coverage_equivalence` would fall over entirely instead of halfway. The churn is damped by the inertia
of the satisfaction (A10), which is already there.

---

## A13 — Game difficulty is an axis of the state

It exists to answer a question the demographics raise: **does a newly-built house have residents?** If
it is born full, the player never sees the hard part of the game; if it is always born empty, the
first years are painfully slow.

**Decision:** it depends on the difficulty. A table of profiles in
`sim-data/data/difficulty.ron`, a `DifficultyId` in the `World`, the **textual** id in the replay's
header, and the profile inside both hashes.

It is not an enum: difficulty is data (D6), and a scenario or a civilisation will be able to declare
its own without touching the code. The header gets the textual id and not the index, because a
recorded `.ron` saying `difficulty: 1` cannot be read and reordering the table would silently change
the meaning of every save file already written.

**It has to be done early** — [phase 11](plan/11-difficulty.md), right after A12 — for the same argument as
the `DirtyFlags`: it touches `World::new`, the `Header` and `hash_world`, i.e. the three things that
regenerate the recordings, and doing it late regenerates them twice. It is born with **one knob only**
(`starting_residents_per_house`) and phases 14, 15 and 17 hang theirs off it without touching the
header or `World::new`'s signature again.

*If it is decided otherwise* — difficulty as a parameter of the scenario and not of the state — the
replay does not carry it, and two games with the same `.ron` can diverge. That is exactly what D4
forbids.

---

## A14 — Four-flow demographics, aggregated, with limited randomness

D5 says the unit of simulation is the house and not the individual, and D3 says immigrants are real
walkers. The two together leave open *how* coarse M1's demographics have to be.

**Decision:** four flows — births tied to the city's overall wellbeing, deaths, immigration,
emigration — all **aggregated**. Immigrant walkers stay M3: what M3 will add is the **travel time**,
not the attractiveness rule. It is the same shape as A5 for the farm, and like that one it has to be
noted where whoever reads it in M3 will find it.

**On the randomness.** A computed base rate plus a jitter drawn from a seeded RNG, not a pure formula
and not a die per house. The replay's determinism stays absolute (D4): the same seed, the same result
bit for bit. The formulation to keep is ***the randomness is in the choice of seed, not in the
execution***.

Two draws per flow and not 3,750: the jitter on the rate, once per tick, and the choice of house the
event falls on. It is not for the cost — it is that "a random rate, a deterministic distribution" is
easier to explain, to balance and to read in a recording.

**Two RNG domains, not one**: `Demographics` for births and deaths, `Migration` — which has existed
since phase 02 and has never done anything — for the migratory flows. Separating them means writing
phase 16 does not knock phase 15's sequence out of phase. It is the use case `RngDomain` was designed
for, and in four phases of M0 it had never come up.

**The trap, which is worth more than the decision.** `rand::Rng::random_range` uses rejection
sampling: the number of `next_u64()` values consumed depends on the values drawn, and therefore on the
seed. Then `rng.draws(d)` stops being a function of the game state — that is, it stops being of use to
the state hash, which is why phase 02 put it there — and
`different_seeds_give_different_hashes` would pass **even if the demographics did absolutely nothing**.
A fixed-cost draw is needed, and the test that pins it down
(`the_number_of_draws_does_not_depend_on_the_seed`), both written *before* the births.

**How it really went (phase 14, done).** The decision held in every part that mattered, and the trap
above was the most valuable thing written in this entry: `Stream::below` and its test landed in a
commit of their own, before a single birth existed, and only then was
`different_seeds_give_different_hashes` switched back on. Four corrections.

**The two draw sites became two, conditionally.** "One draw per flow per tick" is true only of a flow
that has somebody eligible. A flow with none takes no jitter at all — it costs nothing to write and
it keeps `draws` a readable function of the city rather than of the calendar. It is also what lets
`commands.rs` go on asserting that an empty world touches no stream: the sentence is unchanged since
phase 04 and is now true for a reason instead of by absence.

**The test named above could not be written as named.** `the_number_of_draws_does_not_depend_on_the_seed`
describes something false: the choice of house is one draw *per event*, and how many events mature
depends on the jitter, i.e. on the seed. The property splits in two — `below` costs exactly one draw
whatever its argument, and a city whose rates are zero draws the same number of values under every
seed — and only together do they make the re-enabled hash test mean anything.

**The accumulator does not hold thousandths.** Dividing into thousandths once a tick truncates up to
a thousandth per flow per tick, which over five years is a systematic *downward* drift of a couple of
events: small, invisible, and precisely what the jitter test would otherwise have been blind to. It
holds the undivided numerator, and the modulo keeps it under the divisor.

**"Aggregated" turned out to have a second meaning.** The entry justified aggregation by cost and by
D5. The stronger reason emerged in the tests: with a rate counted against the **eligible** residents,
a city whose houses are full has nobody eligible and stops growing *by construction*. Counted against
the population it would also have stopped, because `max_residents` clamps every birth — but for a
reason no reader could point at. The plateau is a property of the rule, not an artefact of a clamp.

---

## A15 — Attractiveness: two terms, plus the tax rate one phase later

**Decision:** the city's attractiveness is the average satisfaction weighted by residents plus the
free places in **served** houses, and the tax rate enters as a third term **only in phase 17**, not in
the 16 that introduces migration.

The delay is deliberate: putting the tax rate into the attractiveness together with migration would
mean balancing two mechanics in one go, and when the curve does not come out right you do not know
which of the two to fix. The phases do not get coupled.

The gate on free places is **hard**: zero places in served houses ⇒ zero immigration, at any
attractiveness. From it follows the cheapest part of A10: *"migrants only go where life is good"* is
not a separate mechanism, it is the filter on the eligible set — the same houses levelling up
considers worthy, for the same reason. One mechanism fewer and one balancing number fewer.

The attractiveness is a **pure** function, with no RNG: the jitter is in the flow, not in the index.
That way it can be printed in the dump, compared between two games, and it is already in the form the
semantic observation for the LLM (M3) will want — an aggregate indicator, not serialised state.

*To keep an eye on:* the weighted average divides by the population, which can be zero. An empty city
is a perfectly normal state and `no_panic_on_ten_thousand_commands` produces them constantly.

---

## A16 — The objectives live in the `World`, `sim-scenario` builds them

`CLAUDE.md` puts checking the objectives at step 9 of the tick, inside `step`. But the dependencies
point towards `sim-core`, so `sim-scenario` depends on the core and the core cannot call it.

**Decision, and it is the same shape as A2:** the `Objective` enum, its evaluation and the progress
state live in `sim-core` — they are pure structs with no I/O and they belong to the core's vocabulary,
like `ServiceKind`. What stays in `sim-scenario` is the definition of the scenarios, loading from a
file and composing the mandatory and optional objectives.

It is the second time the dependency graph has dictated the boundary, and it is worth writing down:
whoever reads has to recognise the pattern instead of rediscovering it.

The corollary that matters: by keeping the objectives in the `World`, the **tick of completion goes
into the state hash**. "The scenario declares itself complete on the right tick" then becomes a
recording that diverges if the tick changes, instead of an `assert` somebody has to remember to write
— and it is what makes phase 18's scenario the canary on the balancing that `CLAUDE.md` asks for at
Testing point 4, in the version M1 can have before M2's bot.

*Rejected alternative:* passing the objectives to `step` as a parameter. It changes the signature
declared in D4 and leaves their state outside the hash, i.e. outside the recordings.

---

## A17 — The per-tick recomputation cost

**Status: TO_BE_DECIDED** — slot [18.5 in ROADMAP.md](ROADMAP.md), to close before M2.

The first decision in this project to stay open, and that is no accident: it is the debt
[A12](#a12--the-services-chase-the-population) opens on purpose. (It was the only one until
[A18](#a18--an-empty-house-consumes-no-capacity) joined it, which is
also A12's doing, from the other side.)

With the services chasing the population, the coverage is recomputed almost every tick instead of only
when the player builds. The typical tick goes from 248 µs to ~3.3 ms — **~13×** — and that cost is
paid on every game, and therefore on every batch of automatic balancing (`CLAUDE.md`, requirement 2).
It was accepted to buy a gameplay dynamic, not through carelessness.

**Why it stays open instead of being solved in M1.** Because A11 has just happened: the obvious
hypothesis about where step 3's cost lay was wrong, and it was the measurement that said so. Doing the
optimisation in the same phase you take the measurement in means not having the *before*. `CLAUDE.md`
is explicit — *do not optimise before the profiler* — and this time the profiler arrives at the end of
M1.

**What is needed to close it.** The numbers from phases 14 and 18, which have to be carried over here:

- `H` — the empty tick with the demographics switched off. It has to match the 248 µs at the end of
  M0; if it does not, the cost is not the recomputation but step 6 itself.
- `I` — step 6 alone, which separates the demographic work from the recomputation it triggers.
- `J` — the fraction of ticks in which the population moved, over a whole game. It is the real
  multiplier: A12's cost is `J × G`, not `G`. In a full city `J` is low.

### The measurement, taken in phase 14

`cargo xtask bench --reps 100`, with and without `--zero-demographics`, on one machine in one
sitting. At the reference scale (200×200, 15,000 residents):

| | | |
|---|---|---|
| `H` — empty tick, demographics off | **324 µs** | |
| `A` — empty tick, real rates | **3.600 ms** | |
| `G` — `compute_from_scratch` alone | **3.251 ms** | 2,667 ns per provider |
| `J` — ticks in which the population moved | **100%** | 502 recomputations in 502 ticks |
| `I` — step 6 alone, derived as `(A − H) − J × G` | **~25 µs** | |

**Two of the three expectations above were wrong, and that is the useful part.**

**`J` is not low. It is one.** The sentence "in a full city `J` is low" was the hope that A12's cost
would be amortised over the ticks where nobody moves. At the reference scale the population moves on
*every* tick — 502 out of 502, and 97% at the mid-game scale — because a city of 15,000 residents
has enough houses that at least one birth or death matures every single tick. A12's cost is
therefore `G`, not `J × G`, and no countermeasure that relies on `J` being small is worth building.
The multiplier was the discount this decision was quietly counting on, and it does not exist.

**`H` moved: 324 µs against the ~280 µs the same machine measures at the end of phase 13.** Step 6
now scans every house three times even with the rates at zero — twice to split the residents between
the served and the unserved, once more for the average satisfaction — and that scan is a fixed cost
the recomputation has nothing to do with. It is small next to `G` and it is real, and by the rule
written above it is *a different thing to optimise*: it lives in `demographics.rs`, not in step 3.

**What `I` says.** ~25 µs, against `G`'s 3.25 ms. The demographic work itself is not the problem by
two orders of magnitude: **essentially the whole of A12's price is the coverage recomputation it
triggers**, which is what the countermeasures below already assume. The derivation is arithmetic over
four measured terms, not a guess — but with `J` at 1 the subtraction is `A − H − G`, a difference of
large numbers, so ~25 µs should be read as "small" and not as a figure to three digits.

**The candidate countermeasures, in order of payoff-to-risk.**

1. **Targeted invalidation instead of global.** Today any change calls `mark_all_providers_dirty`. If
   a house's residents change, the only providers to revisit are the ones serving it and the ones that
   could serve it — a small set. The `DirtyFlags::coverage` list has existed since phase 04 for
   exactly this and **has never been read**: this is the moment it was written for.
2. **Recomputation on a threshold.** Do not invalidate for every single resident who moves, but when
   the accumulated drift at a provider passes a threshold. It changes the gameplay semantics — the
   coverage reacts with a delay — so it has to be decided as a rule, not as an optimisation, and put
   in a table.
3. **A11's candidate cache**, now with one more reason: the BFS depends on the roads, the position and
   the range, not on the population, so with A12 it becomes the part that is *entirely* recomputed for
   nothing. The three traps already written in A11 apply identically.
4. **Switching to places**, i.e. reversing A12. It is one line in `coverage.rs` and it brings the cost
   back to M0's, at the price of the gameplay dynamic. It is the way out, not the solution, and it
   should only be considered if the first three are not enough.

**Watch out for something A12 has weakened.** `coverage_equivalence` is no longer the complete oracle
it was: it runs on a zero-rate dataset, and the contract about the population is a separate test.
Whoever works on this decision has to keep **both** green, and it is worth asking whether it would not
be better to rebuild a single oracle first — for instance by comparing the coverage against
`compute_from_scratch` at the point in the tick where it has just been computed, instead of at the end
of the tick. That would be the first job to do, before touching any performance.

---

## A18 — An empty house consumes no capacity

**Status: TO_BE_DECIDED** — slot [14.5 in ROADMAP.md](ROADMAP.md), to close before phase 15.

`pick_within_capacity` (`sim-core/src/coverage.rs`) walks the candidates in priority order and
subtracts each one's residents from what is left:

```rust
left = left.checked_sub(residents)?;      // residents == 0 always succeeds
```

On [A13](#a13--game-difficulty-is-an-axis-of-the-state)'s `hard` profile every house is born with
`starting_residents_per_house: 0`, so the subtraction can never fail and **every house in range is
picked, whatever the capacity** — including by a provider whose capacity is itself zero. The
fixture's `small_well` declares four residents and serves seven empty houses in
`on_hard_an_empty_house_consumes_no_capacity`; seven is not a limit, it is how many the test builds.

### Why it is a decision and not a bug

Because it follows from [A12](#a12--the-services-chase-the-population) doing exactly what it says.
Capacity is counted in residents, an empty house has none, and it therefore weighs nothing. Read
that way the behaviour is not a slip, it is the rule.

What makes it a question anyway is that coverage does not only feed the food arithmetic — it feeds
**satisfaction**, and satisfaction feeds the ladder. So on the profile that exists to make the game
harder, one small well takes an entire district to the top rung for free, which is the opposite of
what the profile is for. `hard` is meant to be the setting where the services have to be earned;
today it is the setting where they are cheapest.

It sits between two decisions rather than inside either. A12 chose the unit; A13 chose a starting
value of zero for it. Neither was looking at the point where they meet, and that is the whole of
this entry.

### The candidate answers

1. **Charge a minimum of one per house** — `left.checked_sub(residents.max(1))`. An empty house
   still costs a place, so capacity stays meaningful at every population. Cheapest to write, and it
   moves no recording today (the recorded scenarios run `easy`, where houses are born with four).
   Its weakness is that it makes the unit no longer purely residents, which is a small lie in a
   number the tables describe honestly.
2. **Cap the candidate count as well as the residents** — a second bound, in houses, alongside the
   one in residents. Honest about being two constraints, and it needs a second column in the table
   and therefore a balancing pass.
3. **Accept it and say so** — write the behaviour into `pick_within_capacity`'s doc comment as
   intended, and let `hard` be a profile that starts generous and gets harder as it fills. It is
   defensible, and it is the answer that costs nothing; it should be chosen deliberately rather than
   by not choosing.

### What is needed to close it

**Phase 15's numbers.** Today the unbounded assignment is free, because nobody living in those
houses eats or drinks. When migration lands, every one of them becomes a real claim on a provider
sized for four, and the question stops being about satisfaction alone. Settle it before phase 15
moves the recordings rather than after: the same change is one line now and a regeneration to
attribute later.

> **Amended while planning phase 14 (2026-08-12), and the amendment is the interesting part.**
> Everything above was written about `hard`, the profile where a house is **born** empty. Deaths
> make zero residents reachable on **every** profile, `easy` included — which is to say inside the
> two committed recordings. Two things follow, and the decision taken was to defer anyway:
>
> - the sentence "it moves no recording today" expires with phase 14. Closing A18 before the
>   demographics land costs nothing and can be proved with `regen-expected --check`; closing it
>   afterwards is a regeneration somebody has to attribute. That cost was accepted knowingly, which
>   is the only way it is worth paying.
> - an emptied house keeps its coverage, so its satisfaction climbs with nobody in it and it can be
>   promoted at the monthly review. Harmless while it lasts — a promotion grants permission and not
>   people (A12) — but it is exactly the state phase 15's immigration will fill, so whichever answer
>   wins has to be checked against a house that is empty, served and at a level it never earned by
>   housing anyone.
>
> Phase 14 leaves a test stating the behaviour as it stands, written to **change its outcome** rather
> than break when this is closed ([14, test 15](plan/14-births-deaths.md)). It is the third use of that
> device, and it is what keeps a deferred decision visible in the suite and not only here.

**And something to measure it with, which does not exist.** Nothing in the property suite exercises
`hard` at all — every proptest builds its world with the bare `world()`, i.e. `easy`. Worse,
`population_is_consistent` (`sim-core/tests/invariants.rs`) hardcodes
`house_count * RESIDENTS_PER_HOUSE`, which *is* the `easy` constant, so the suite could not be
pointed at `hard` even if someone wanted to: the invariant would fail on the profile rather than on
a bug. Generalising it to read the profile off the `DataSet` is the prerequisite, and it belongs to
phase 14, where the population starts moving for reasons other than the difficulty.

*Watch out for one thing when it is closed.* Whichever answer wins, the fix lands in the same
function that `CapacityBeyondOutput` depends on for *a house covered by food always eats*. Charging
a minimum of one makes a provider serve **fewer** houses, never more, so the food invariant can only
get safer — but the reasoning has to be redone rather than assumed, because it is the second time
that function has turned out to carry a rule nobody had written down.

---

## A19 — Plain words, and which hard words earn their place

**Recommendation:** rename the load-bearing nouns that sit above an elementary reading level. An
inventory of all four crates found about forty such words, and the four heaviest were `satisfaction`
(149 uses), `coverage` (144), `capacity` (127) and `terrain` (126). The recommendation was to
translate them — `comfort`, `reach`, `serves`/`max_residents`, `ground` — on the grounds that a word
a ten-year-old cannot read is a word that fails the glossary's own promise.

**How it really went: all four were kept, and that is what produced the rule.**

The recommendation was wrong, and it was wrong in an interesting way. It treated "hard to read" as
one property, when it is two. `terrain` is hard *once*: you meet it, you learn it, and it is then the
word every other city builder and every map format also uses, so learning it pays you back outside
this repository too. `InsufficientFunds` is hard *every time*, and teaches you nothing, because
`NotEnoughMoney` was available and says exactly the same thing.

So the rule is not about difficulty, it is about **whether the word is doing work**:

> A hard word earns its place when it is the domain's own word, and then it is defined in
> `GLOSSARY.md`. A hard word that is merely a synonym choice does not.

What that decided, in one pass:

- **Kept**, because they are the domain's: `satisfaction`, `coverage`, `capacity`, `terrain`,
  `provider`, `residents`, `treasury`, `occupant`, `Demolish`, `stock`, `range`, `entrance`,
  `decay`, `review`, `threshold`, `sustainable`, `Milli`, `Inconsistency`, `ComponentId`,
  `propagate_coverage`, `invalidate_coverage`, `evicted`, `HouseEvolved`/`HouseDegraded`.
- **Retired**, because a plainer word said the same thing: `hysteresis` → `gap`; `rung`/`ladder` →
  `level` and `rules.house_levels`; `canary` → `every_field`; `perturbation` → *change*;
  `InsufficientFunds` → `NotEnoughMoney`; `UnsuitableTerrain` → `WrongTerrain`; `OutOfBounds` →
  `OutsideMap`; `Mood::Desperate`/`Thriving` → `Awful`/`Great`; `RngDomain` → `RngKind`;
  the `DOMAIN` hash-prefix constants → `PREFIX`.

`rung`/`ladder` is the case worth keeping. The metaphor was not translated, it was **deleted**: the
table it named is already called `house_levels` and the type is already `Level`, so "ladder" was a
third name for a thing that had two. The glossary got shorter rather than differently worded, which is
the outcome to prefer whenever it is available.

### Two limits, chosen deliberately

**The RON keys did not move**, and neither did the Rust structs that mirror them — `Rules`,
`HouseLevelDef`, `ServiceDef`, `DifficultyDef`, all of `sim-data/src/raw.rs`, with no
`#[serde(rename)]` anywhere. So `Economy::treasury` and `Rules::starting_treasury` still agree, but a
future rename on one side has to be spelled the same on the other or the seam opens. The alternative
considered was renaming the keys too — hash-safe, since `data_hash.rs` feeds values and id strings and
never key names — and it was declined to keep the files and the code reading alike.

**Prose was left out**, except where a retired word survived in it. `monotone`, `cadence`,
`equidistant`, `materialised`, `orthogonal`, `naivety`, `pedantry` are all still in the doc comments,
and the messages still say "the output sustains" and "unreachable by construction". That is the
larger half of the problem — the words a reader actually *meets* are mostly in prose, not in
identifiers — and it is deliberately a separate pass on the same rule.

### Why it cost nothing to do

Names are never hashed. `sim-core/src/data_hash.rs` feeds field *values* and id *strings*;
`sim-replay/src/hash.rs` destructures every struct but hashes only contents. Neither touches a field
or a type name, so the whole pass came out with `regen-expected --check` green, both recordings
byte-identical, and both benchmark state hashes unmoved — which is the proof, not a hope. Any naming
pass after this one can be checked the same way, and if a hash moves, the rename was not a rename.

### The audit found three glossary rows that were false

Worth recording separately, because none of them was about vocabulary:

1. **`mood`** claimed "the number itself never leaves the core". `House::satisfaction` is `pub`,
   re-exported, and `xtask` averages it into the `sat.` column.
2. **`capacity`** defined only the provider's sense; the code uses the word for a house's resident
   ceiling just as often, and the row now admits both.
3. **`FORMAT_VERSION`** was listed as frozen at `1`. It has been `2` since phase 11.

Plus two frozen values that were load-bearing and unlisted — the declaration order of `RngKind`
(hashed positionally, exactly the `Terrain` hazard) and the difficulty ids — and two words defined for
phases that are not written yet, `jitter` and `attractiveness`, which are now marked as such instead
of reading like stale entries.

The general lesson, and it is A5's again in a third place: **a document that is checked by nobody
drifts.** The glossary had no test. It still has none, but it now has a rule that says what belongs in
it, which is the cheapest available substitute.

---

## A20 — A house is covered by services its level does not require

**Status: TO_BE_DECIDED** — slot [14.6 in ROADMAP.md](ROADMAP.md), to close before phase 15.

`compute_from_scratch` (`sim-core/src/coverage.rs`) offers **every** house within range as a
candidate, whatever that house's level asks for; the level is never read there. `production` then
feeds every house that came out of it with a food provider attached. So a house on the first rung —
which since phase 13 is a hut requiring water only — is assigned to a farm, eats
`food_per_resident` per resident per tick, and occupies places the farm counts against its capacity.

This is [A9](#a9--water-is-coverage-not-a-resource)'s gap, named in M0, made observable by phase 12,
and made to matter by phase 13. It reached this entry because a reviewer asked whether the food a
house needs really scales with its residents; the answer is yes, and the interesting half is *for
every house, at every level*.

### Why it is a decision and not a bug

Because the same assignment does two jobs at once, and only one of them is questionable.

- **It feeds the hut**, which costs the farm stock and places it could have given to a house that
  actually requires food. That is the part that looks wrong.
- **It is also the only way the hut ever gets promoted.** Satisfaction moves on the union of what
  every level requires, precisely so that a rung introducing a new service is reachable; and the rung
  above the hut asks for food at `level_up_threshold`. Stop assigning food to the hut and its food
  accumulator sits at zero for ever: nothing rises past the first level again.

So "assign only what the level requires" is not a one-line fix, it is a change to how the ladder is
climbed. That is what makes this a decision.

### Why it has to close before phase 15

The same reason [A18](#a18--an-empty-house-consumes-no-capacity) does, and it is the same function.
Today the number of huts is bounded by how many houses the player builds. Migration fills them, and
every one it fills becomes a real claim on a farm sized for the rungs above. Both questions are about
who consumes a provider's places, both are answered inside `pick_within_capacity` and its caller, and
answering them in one pass costs one regeneration instead of two.

### The candidate answers

1. **Leave it, and write it down as intended.** The hut eats because it is being brought up to the
   rung above; a farm feeding the district it will serve is not a leak, it is the ramp. Costs
   nothing, and it is defensible — but it should be chosen, not inherited.
2. **Cover for satisfaction, consume only what the level requires.** Splits the assignment's two
   jobs: the hut keeps building its food satisfaction and stops eating. It sounds like the best of
   both and it has a real cost — the coverage stops being one relation and becomes two, and
   *a house covered by food always eats* stops being true as stated, which is a sentence three
   documents and one validation check rest on.
3. **Assign only what the level requires, and make the next rung reachable another way** — a rung is
   entered on the services *below* it plus a wait, or satisfaction starts at a value instead of zero
   when a service first arrives. Honest about what it costs: it moves the level rules, not the
   coverage.

### What it will need

A test that pins today's behaviour first, in the form that **changes its outcome** rather than breaks
when this closes — the device phase 14 used for A18. And the numbers phase 15 produces: how much of a
farm's capacity huts really take once migration is filling them is a measurement, not a guess, and it
does not exist yet.

*Watch out for the same thing A18 warns about.* Whichever answer wins lands in the function
`CapacityBeyondOutput` depends on for *a house covered by food always eats*. Answer 2 in particular
changes what that sentence means, so the check has to be re-argued rather than assumed — for the
third time in that function's life.

---

## A21 — A building declares its role, and the roster stays data

**Status: decided**, phase 14.4.

`BuildingDef::is_house()` classified a building by **two absences** — no service, and a non-empty
`required_services`. It worked, and keeping it working cost three validation checks and a warning
comment in `buildings.ron`. The tell was `InconsistentRequirements`: the building's
`required_services` was doing two unrelated jobs at once — saying *which satisfaction accumulators
move*, which `satisfaction::update` needs, and saying *this is a house*, which is why the field could
never be simplified.

The building now declares a `role`, and the field does one job.

### Why a sum type rather than a tag beside the old fields

The flat eight-field struct was a union in disguise. A house carried `service: None`,
`output_per_tick: None`, `max_stock: None`; a well carried an empty `required_services`. Every
illegal combination was representable, and validation was what stood between the table and a state
the rest of the game had no meaning for. Adding a tag to that struct would have declared the role and
left all of it standing.

`BuildingDef` is now `{ id, size, cost, levels, role, production }`, with
`BuildingRole::House { required_services } | Provider { service }`. Grouping the two production
fields into a `Production` where both are required means `ProducerWithoutStock` and
`StockWithoutOutput` still fire against the **raw** table — they are properties of one row of one
file — but past validation the broken combination has no shape to be in.

### Why the variants are roles and not buildings

The proposal on the table was an enum per kind of building — house, well, farm — with a matching
enum for the live data. It was rejected on three counts, and the reasons are worth keeping because
the idea is a natural one:

- **It contradicts D6.** The roster in code means a civilisation cannot add a building from data and
  a balancing run cannot vary it, which bears on the project's second non-functional requirement.
  `ids.rs` already said so: *"Not an enum: building kinds are data, not code (D6)."*
- **It unshares what a prototype exists to share.** `size`, `cost` and `levels` are identical in
  shape for every building, and a variant per building duplicates them into every arm — a match at
  every shared read, growing with each building the game gains. Roles are two, and stay two.
- **It costs memory rather than saving it, which was its stated purpose.** An enum is as large as its
  largest variant plus a tag. Unioning the instances would take a house from 8 bytes to 12, and the
  day a variant carries anything larger every house in the city grows to match it. The instance side
  of the proposal was also already in the tree: `Building` and `House` are two structs in two
  `SlotMap`s, and `Building.kind` is the reference to the prototype.

A related proposal — a raw `*const BuildingDef` on each instance — was refused on
`#![forbid(unsafe_code)]`, and fatally on the state hash: `sim-replay/src/hash.rs` hashes
`kind.get()`, and an address is per-process. The regeneration protocol names *memory addresses* as
the non-determinism a separate process exists to catch. **The index is the more self-contained of the
two**: a pointer that cannot dangle, and one that can be hashed, compared and written to a file.

### Producing is not a role

The `farm` supplies food **and** grows it, so no single variant could hold it. Production is a
separate `Option<Production>`. Putting it inside `Provider` would have ruled out a producer that
supplies no service — M3's warehouse — which is the widening D6 warns against inventing before
something needs it.

### What survived, and why

- `NoHouse` **stays**, restated as *no row declares the house role*. Without one,
  `satisfaction::update` and `remove_house` silently do nothing. It did get harder to trip by
  accident, which is the check earning its keep rather than a reason to drop it: producing a
  house-less table now takes rows that are all valid providers, so the test became a fixture file
  instead of a one-word replacement.
- `LevelCountMismatch` is untouched. It was never about the heuristic.
- `InconsistentRequirements` **stays with its body and a new message**. Its old reason — *or
  `is_house()` stops recognising it* — died with the heuristic. Its real reason outlived it, and was
  already written down twice in the code: a service required by a level but missing from the union
  has an accumulator nothing ever moves, so **that level is unreachable by construction**.
- `required_services` **stays declared** rather than derived from `house_levels`.
  [A20](#a20--a-house-is-covered-by-services-its-level-does-not-require) is still open, and deriving
  the union would fix the answer to a question A20 has not been asked. It is a later phase and a
  smaller one if it still looks right once A20 closes.

### What this did not fix

A `House` still carries no `BuildingKindId`, so `DataSet::house_def()` is still a scan over the
definitions recovering what the house did not record, and a civilisation with two kinds of house
still does not work. Declaring the role did not lift that limit — it lives in the instance, not in
the definition — and lifting it adds a hashed field to `House`, which is a second reason to
regenerate and therefore a phase of its own.

### The raw table stayed flat

`RawBuildingDef` keeps its flat fields and gains `role: String`, required. A RON enum would make an
unknown role a serde *"unknown variant"* error instead of a validation error naming the row it is in,
which is the rule `raw.rs` opens with. Turning the flat row into the sum is what validation does, and
the contradictions found on the way — a house declaring a service, a provider declaring none — are
reported against the field to blame.

---

## A22 — `World` is not `Clone`, and the compiler says so

**Status: decided**, 2026-08-14.

Phase 14.4 put the derive behind `test-util` and wrote that the compiler enforced it now instead of
a convention. It enforced half of it: the production path could not copy a world, and the tests
still could. The derive is now gone in every configuration.

**Why it is worth removing rather than tolerating.** A save is `seed + Vec<Command>` and never a
dump of the state (D4). A copy of a world is therefore a *second way to reach a state*, and the
whole determinism story rests on there being one. Every use it had was a test saying "nothing
changed" by comparing a state against a photograph of itself — which is a weaker claim than it
looks, and the six sites were all of that shape.

### The replacement says more than the copy did

A test that wants a `before` builds a **second world and plays it**: same tables, same grid, same
seed, same commands, one of them taking the tick empty where the other took the rejected one. Two
things follow that a copy could not give:

- **The comparison can cover everything.** `World::first_difference` is an exhaustive destructure in
  the manner of `every_field`, so a field added to the state stops it compiling. A `before`/`after`
  pair could only ever compare the fields a tick is *forbidden* to move; two worlds each played once
  can be compared on `roads`, `coverage`, `dirty`, `food` and `population` as well. That is not a
  technicality: `Coverage` and `RoadNetwork` carry their recompute and rebuild counters, so a
  rejected command that dirtied a flag which step 2 or step 3 then consumed **inside the same tick**
  is now visible. It left no trace at all in the final state, so no `before`/`after` test could have
  seen it.
- **It is the formulation phase 16 had already asked for**, in its test 5: *the state after
  `step(w, cmds)` with every command rejected matches the one after an empty tick*. It was written
  there as a change phase 16 would have to make once taxes moved the economy every tick. It is
  simply how the test is written now, and phase 16 inherits it.

The comparison **checks the tick first**, and that ordering is the point rather than housekeeping.
The one failure mode this technique has that a copy does not is a twin somebody forgets to play:
two worlds out of step still agree about an empty grid and an untouched treasury, so the test would
pass while comparing two unrelated games. Comparing the tick first turns that into a named failure.

Nothing is asserted on the branch where a command was accepted. Two identical worlds given identical
commands stay identical because `step` is a function, so an assertion there would be `x == x` — the
trap `covered_means_fed` fell into in phase 12, recorded under A9.

### Why a compile-time check and not a comment

`#[derive(Clone)]` is one word, its absence is invisible, and nothing else in the tree would fail if
it came back — the exact case CLAUDE.md's rule covers: if you are writing a comment to explain why
two things must hold, write a check instead. `not_clone` in `world.rs` asks the compiler the real
question, through an inherent associated constant being chosen ahead of a trait's of the same name.
It therefore answers for a hand-written `impl Clone for World` too, which a check on the *spelling*
of the derive — in `doc-check`, say — would walk straight past.

It carries a **positive control**: a local type that does derive `Clone`, asserted to read as such.
Without it a change that broke the resolution would leave the guard answering "not `Clone`" to
everything, which is green for the wrong reason. That is A5's lesson in a third place, after
`dataset_with_a_farm_that_grows_nothing`: give the check something it has to catch, or it will
eventually be checking nothing.

### What it cost

Nothing measurable, and one API member traded for a much larger one. `test-util` loses `Clone` and
gains `first_difference`, which is **read-only** — the feature's other members are mutation hooks.
`sim-core`'s tests build a second world per case instead of a deep copy per tick, which is cheaper
than what it replaces. `the_hash_covers_the_whole_state` replays the `minimal` recording seventeen
times instead of cloning a baseline, and the `sim-replay` suite stayed at three hundredths of a
second. No recording moved: nothing here touches a rule or a table.

The one behaviour that could have changed did not. A rejected command was already a no-op down to
the RNG position and the derived counters — every handler in `tick.rs` validates in full before its
first write, and `charge` writes the treasury only on success — so the strengthened test was green
the first time it ran. That is the answer to a question nobody had asked before, and it is worth
having as an answer rather than as an assumption.
