---
id: 20
kind: phase
status: not-yet-built
opened: 2026-08-13
---

# Phase 20 — Things on the ground, and the cost of clearing them

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).
>
> It is also, deliberately, a **sketch rather than a full plan**: it was written when the task was
> added to the tree, not when the phase was designed. What follows is the goal and the shape of the
> problem. The detail arrives when phase 19 has closed and there is a real map to look at.

**Goal:** the same building costs more on ground with trees on it than on bare ground, the clearing is
paid for as part of placing the building, and the map is what says where the trees are.
**Depends on:** 19 — the map file, and the room phase 19 leaves in the tile.
**Size:** M (guessed, not estimated).

## Why it exists

Phase 19 makes the ground uneven; this makes it occupied. They are the two halves of the same idea —
*the site you chose is part of what the building costs* — and they are separate phases because each
one alone is a complete rule with its own regeneration.

The gameplay it buys is siting: a flat, empty patch is worth more than a flat, wooded one, so land
stops being interchangeable and clearing becomes a small early-game decision. Without it, terrain
affects the player only through slope and the shape of the coast, which is thin.

## The shape it will take

The rule lands where phase 19's slope rule lands: in `place_building`, after the loop that validates
the footprint and before the treasury is charged. The site cost becomes a sum of terms — the base
cost, the slope term, the clearing term — rather than a single number, and the moment there are two
terms the sum wants a name and a single function that computes it.

A tile has to say what is standing on it. Phase 19 leaves three spare bits for exactly this kind of
thing, and whether a handful of kinds fits in them or forces a layout change is the first question the
phase has to answer, before anything else, because the answer decides whether the recordings move for
one reason or two.

The map file gains a third block beside `terrain` and `ground`, and the loader gains the refusals that
go with it, in the same `raw` → `validate` → `Def` pipeline: an unknown character names its row and
column.

## The decisions it will produce, none of them taken

- **What can stand on a tile.** Trees only, or trees and loose rock and ruins. One kind is honest
  while there is one map; more than one is the speculative widening that phase 19 refused for
  `Terrain`, and the same argument probably applies here.
- **Whether the tile carries it at all**, or whether it lives in a side table of positions. A field on
  the tile is fast and costs bits that are hard to get back; a side table costs a lookup in a path
  that is not the hot one.
- **Whether clearing yields anything.** Wood from a cleared wood is the obvious idea and it is
  probably wrong here: there is no production chain for it to feed until M3, and a resource with no
  sink is a number the player watches go up. Naming it as a decision so that it is refused on purpose
  rather than forgotten.
- **Whether roads pay too.** A road on wooded ground is one tile of clearing, which is cheap to say
  and puts a second rule into `place_road`. Phase 19 left roads alone for a reason worth re-reading
  before deciding differently.

Names: the parameter names sketched anywhere in this file are illustrations of a shape, not approved
spellings. Every balancing number lives in `sim-data`, never in a `.rs`.

## Out of scope, as things stand

- **Regrowth.** Cleared ground stays cleared. A tile that changes on its own belongs to a tick step,
  and there is no step that wants it.
- **Wood as a resource**, per the decision above.
- **Things that block coverage or sight.** Coverage is walked distance along roads; a tree beside the
  road does not change it, and making it do so puts terrain into step 3, the hot path — which is
  exactly what phase 19 pushed out of itself.
- **Decorative variety.** How many kinds of tree the renderer draws is M2's business and has no state.

## What its tests will have to prove

Two of them are already known, because they are the same two that define phase 19:

1. **The treasury stays an exact equality.** `sim-core/tests/invariants.rs` reimplements what a
   command costs, independently and on purpose. A clearing term added in one place only turns
   `the_treasury_adds_up` red, and that is the test working, not failing.
2. **The dumps move only where the game moved.** Every map in the tree before this phase has nothing
   standing on it, so the first regeneration has to be explainable tile by tile.

Plus: the loader refuses a bad character and says where; a footprint costs base + slope + clearing and
the three terms are separable in the test; and clearing an already-clear tile costs nothing rather
than costing zero by accident.

## Verification

The protocol in [CLAUDE.md](../CLAUDE.md) applies unchanged, and the *before* has to be captured
before the first line is written.
