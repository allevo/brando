---
id: 23
kind: phase
status: not-yet-built
opened: 2026-08-19
---

# Phase 23 — A farm works the land around it

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).
>
> It is also, deliberately, a **sketch rather than a full plan**: it was written when the task was
> added to the tree, not when the phase was designed. What follows is the goal and the shape of the
> problem. The detail arrives when phase 16 has closed and a map can really draw good ground, and
> when phase 22 has given a farm a way to reach further.

**Goal:** a farm's output and the number of mouths it can feed grow with the land the player attaches
to it, and the ground under each parcel decides what that parcel is worth.
**Depends on:** 16 — good ground cannot be drawn, recorded or replayed while a map is one uniform
terrain; and 22 — without a level to buy, a farm's reach never moves and the land beyond it feeds
nobody.
**Size:** L (guessed, not estimated).

## Why it exists

A farm is a fixed number. It grows the same amount wherever it stands, feeds the same number of
residents, and takes the same four tiles doing it. The city's food supply is therefore a **count of
farms**, and the map — which is the whole of what a city builder asks a player to read — has no say
in the one system every other system depends on. A farm on a river bend and a farm on bare ground are
the same building.

There is a second reason, and it is the one that makes this a phase rather than a wish. The farm is
already the odd row in the table: the only building that is a provider **and** a producer, which is
why production is a field rather than a role — no single role could hold both. That row is where the
game's economy is going to be built, and today it holds one number.

## The shape it will take

- **A farmfield**: a new kind of building, one tile, legal on any ground a building may stand on. It
  is a kind and therefore data (D6), so it arrives as a row rather than as code.
- **A new command that names two tiles** — the field's, and the tile the farm stands on. Positional,
  like every command the game has, because a save is a seed and a list of commands and an id from a
  slot map has no business inside one. Naming the farm outright is what lets two farms interleave
  their land on one good patch, which no rule inferring the owner from distance or from what a field
  touches can allow.
- **A fourth terrain, good ground, added last** so that the three that exist keep the positions the
  hash stores them at, and the frozen-order row in [GLOSSARY.md](../GLOSSARY.md) gains a name rather
  than being reordered. It answers **yes** to both questions a terrain answers: a building may stand
  on it and a road may be laid on it. It is ordinary ground that happens to grow more, so it also
  needs a road cost of its own and a row in [RULES.md](../RULES.md), which is compared against the
  code on every build.
- **Both of a farm's numbers move together.** A field adds a fixed amount to the output *and* a fixed
  number of residents to the capacity, and how much depends on the ground under it — two tiers, plain
  and good. This is not a preference. A field that raised only the output would raise nothing anybody
  could see, because the surplus goes straight past the granary's lid and is counted as lost; a field
  that raised only the capacity would put houses back on a farm that cannot feed them and that no
  other farm can take over, which is the absorbing state the balancing was deliberately built to
  remove.
- **The load-time check moves from the total to the increment.** Today a farm's declared capacity is
  refused unless its output sustains it. It becomes: one field's capacity within one field's output,
  per tier. Because both sums are linear in the number of fields, the farm's total follows, and the
  rule *a house a farm covers always eats* survives without being weakened. Keeping that rule is the
  point of the whole check, and a phase that quietly turns it into a comment has broken the game.
- **A farm's capacity stops being a constant in the table.** This is the one change that reaches into
  the tick's hot path: step 3 reads a provider's capacity once per provider per recomputation, and it
  would read a total computed from the land instead of a number looked up by level.
- **A reach of its own**, per level, saying how far a farm's land may lie from it. Not the service
  range: a designer should be able to build a farm that works a lot of land and feeds a small corner
  of the city, or the reverse, and one number cannot say both. The spellings are not chosen here; the
  naming rule in [CLAUDE.md](../CLAUDE.md) binds them, and every limit is a parameter in `sim-data`.

## The decisions it will produce, none of them taken

- **What a farmfield is, structurally.** A third role gets placement, demolition, occupancy and the
  overlap invariant for nothing, and puts hundreds of buildings that serve nobody into the collection
  step 3 walks on every recomputation. A collection of its own keeps that walk clean and costs a new
  hashed structure, a new kind of id, and a bit in a tile whose four-byte budget phase 16 and phase 20
  have both already spent. Measure before choosing: the obvious answer about where the cost lies has
  been wrong twice in this tree.
- **What the farm stores.** A count per tier keeps the numbers in the tables, where a rebalance can
  move a farm's output without touching a single save. A running total is one addition instead of a
  lookup, and a second copy of a number that nothing keeps in agreement with the first.
- **What a demolition does at both ends** — a farm taken away with its land still attached, and a
  field taken away from a farm still standing. Both have to keep the food conservation an exact
  equality, and the write-off for what a demolition destroys already exists for the farm's stock.
- **The name.** *Field* is the word every document in this tree uses for a member of a struct, so the
  bare word cannot be borrowed without making half the prose ambiguous. *Farmfield* is the compound
  proposed. Whichever wins, it earns an entry in [GLOSSARY.md](../GLOSSARY.md) when the phase is
  built, because a hard word without one is forbidden.
- **Whether a field needs to touch a road.** Everything placed so far is either reachable or useless.
  A field is neither: nothing walks to it, and the food appears in the farm's stock however far away
  it grew. That follows from aggregate coverage (D2) rather than contradicting it, but *follows from*
  is not the same as decided.
- **Whether the ground under a field is read once, when it is placed, or on every tick.** Terrain
  never changes today, and phase 16 does not make it change either, so the two answers cannot be told
  apart yet — which is exactly when a choice gets made by accident.

## Out of scope, as things stand

- **Levels of farm.** Phase 22, which this depends on.
- **Yield that varies with anything but the ground** — weather, a season, a delay before new land
  starts paying. Each is a mechanic of its own and none of them is reachable in a scenario yet.
- **Ore, quarries and every other deposit.** Phase 19 names good ground and ore together and sends
  both to M3's production chains, on the argument that a yield modifier would mean rebalancing food
  while the scenario is calibrated on flat ground. This phase takes back exactly one of the two, and
  it owes phase 16 a dated amendment saying which and why — appended below the sentence, never an
  edit to it.
- **Workers, and anything that walks between the field and the barn.** Nobody is employed and no
  goods move; the food appears in the farm's stock. Aggregate coverage (D2) stands and this phase does
  not reopen it.
- **A second kind of food**, and land for any building other than a farm. The warehouse and the real
  logistics walkers are M3 and they replace where the goods come from, not the coverage.

## What its tests will have to prove

1. **Food stays an exact equality** with land in play — what was produced, minus what was eaten, minus
   what the granary lid threw away, minus what demolition destroyed, equals what is in stock. A
   demolished field is part of that sum and is the half most likely to be forgotten.
2. **A city built entirely of the largest farms the rules allow still never has a covered house go
   hungry.** That is the guarantee the increment check is bought to protect, and it has to be proved
   on the totals rather than on a table row, because the row is what the check reads and a test that
   reads the same thing proves nothing.
3. **Good ground actually pays.** Two cities identical but for the ground under their land reach
   different populations. This is the test that says the mechanic is reachable and observable in a
   scenario rather than merely implemented.
4. **The stored coverage still equals the one computed from scratch** once a capacity is per building
   rather than per level, and the walk that stops when a provider is full still agrees with the walk
   that goes to the end.
5. **Every refusal leaves nothing behind**: no tile mutated, no farm changed, treasury untouched.
6. **The recordings diverge at the checkpoint after the first field is placed**, and not before.

## Verification

The protocol in [CLAUDE.md](../CLAUDE.md) applies unchanged, and `bench` matters more here than in
most phases: this changes the inner loop of step 3, whose cost is already the open question at slot
18.5. Run it before and after on the same machine, and read the answer against that question rather
than on its own.
