---
id: 22
kind: phase
status: not-yet-built
opened: 2026-08-19
---

# Phase 22 — A building gains a level when the player pays for it

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).
>
> It is also, deliberately, a **sketch rather than a full plan**: it was written when the task was
> added to the tree, not when the phase was designed. What follows is the goal and the shape of the
> problem. The detail arrives when there is a second level worth buying, and phase 23 is the first
> thing in the tree that wants one.

**Goal:** the player can raise a provider's level with a command, and the per-level numbers the
tables have carried since the beginning take effect.
**Depends on:** nothing that is not already built — the machinery has been in place since phases 03
and 13 and has never been started.
**Size:** M (guessed, not estimated).

## Why it exists

`Building::level` is state. It is hashed, it travels in every recording, and it has been
`Level::FIRST` on every building since M0. Around it sits machinery that was built for it and has
never run: every provider declares `range_per_level` and `capacity_per_level`, every one of them
declares a vector of exactly one entry, and so `service.range(level)` and `service.capacity(level)`
are lookups into a table with nothing to look up.

A house has a way to grow — it stays satisfied long enough and the monthly review promotes it
(phase 13). A provider has none. The only answer the game can give to *this well serves too few
people* is **build another well**, and that is a thinner decision than a city builder is supposed to
offer: a second well is a copy, where a bigger well is a choice between money now and room later.

What makes it a phase rather than a gap worth noting is that phase 23 needs it. A farm that can be
given more land will produce more food than it can find mouths for, because the mouths it may serve
are the ones within its range, and nothing in the game moves a range.

## The shape it will take

Three things, and none of them adds work to a tick:

- **A command, positional like the rest.** It names the tile, and the core works out what stands
  there, the way `Demolish { at }` already does. This is the fourth variant the command set has taken
  since M0 and every addition is permanent, so it is worth arguing rather than assuming — but raising
  a level is not a case of placing anything, and folding it into `PlaceBuilding` would make one
  command mean two things.
- **A cost per level, in the tables.** The refusals follow from it: nothing stands there, it is
  already at the top level, there is not enough money. Each one carries the numbers the caller needs
  in order to act, in the shape the existing errors already use, because that message is the feedback
  the bot and the LLM get and it is the only feedback they get.
- **Marking the coverage as needing recomputation.** What a level *does* is already written —
  `coverage.rs` reads the range and the capacity out of the per-level vectors once per provider — so
  the whole of the effect is that step 3 has to run again. `DirtyFlags` already carries the means. The
  mistake to avoid is the one where a level moves and nothing recomputes until something unrelated
  happens to move as well, which shows up as a bug that comes and goes.

## The decisions it will produce, none of them taken

- **Whether a level can be lost, and how.** A house degrades on the monthly review when its
  satisfaction falls; a provider has no review to hang that on. A level that can only ever be bought
  is a rule tested in one direction, and one-directional rules are where this kind of mechanic has
  always gone wrong.
- **Whether the area grows with the level.** Houses stay 1×1 at every level and the tables say so in
  a comment, because otherwise the overlap invariant goes red in a way nobody can read. A provider
  that grows from 2×2 to 3×3 has to refuse when the extra ground is taken, and refuse without leaving
  half a building behind.
- **Whether the cost is per level or cumulative**, and whether a level takes effect on the tick it is
  bought or after a delay. The first is balancing and lives in a table; the second is a rule and does
  not.
- **Whether every provider gains levels at once**, or only those whose row declares more than one.
  The tables can already express either, and the load-time checks already run over every level a row
  declares.
- **Whether a house can be raised the same way**, overriding the review phase 13 built. It looks
  wrong. Looking wrong is not a decision.

## Out of scope, as things stand

- **House levels and the monthly review.** Phase 13 owns them and nothing here reaches into them.
- **Output that grows with the level.** A producer's output is one number, not a vector, and neither
  this phase nor phase 23 needs it to become one: phase 23 grows a farm's output from the land it
  works, not from its level.
- **A refund for coming back down**, which is one of the decisions above wearing a price tag.
- **Anything the renderer needs in order to show a level changing.** The event set gains a member and
  stops there.

## What its tests will have to prove

1. **A house outside a level-1 provider's range is covered after the upgrade, and was not before.**
   Both halves in one test, because only the first is ever written by accident. Every horizon computed
   from the `DataSet` and never written as a literal, which is the convention the test files in
   `sim-core` already keep.
2. **Every refusal leaves nothing behind**: no level moved, treasury untouched. The no-partial-
   mutation discipline on a new failure path, which is where it has always had to be proved.
3. **The stored coverage still equals the one computed from scratch** after an upgrade. That is the
   test that catches a dirty flag nobody set, and it exists already.
4. **The recordings diverge at the checkpoint after the upgrade** and not one before it. If the
   divergence starts earlier, something else changed and it has to be found before anything is
   committed.

## Verification

The protocol in [CLAUDE.md](../CLAUDE.md) applies unchanged. `bench` is worth a run but not an
argument: nothing here adds work to a tick, it only makes a recomputation that already exists happen
at a moment it did not happen before.
