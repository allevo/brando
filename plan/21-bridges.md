# Phase 21 — Bridges

> **Status: not yet built.**
>
> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).
>
> It is also, deliberately, a **sketch rather than a full plan**: it was written when the task was
> added to the tree, not when the phase was designed. What follows is the goal and the shape of the
> problem. The detail arrives when phase 19 has closed and there is a real river to cross.

**Goal:** the player can carry a road across water, within limits and at a much higher cost, and the
road network crosses it as one region.
**Depends on:** 19 — water is only a real obstacle once a map file can draw a river.
**Size:** L (guessed, not estimated).

## Why it exists

Phase 19 names it out of scope in one sentence and stands a rule in for it: a map whose walkable tiles
are not one connected region fails to load. That rule is a constraint on the map author standing in
for a missing player ability, and it was accepted as a deliberate trade. This phase is the other side
of the trade — once a bridge can be built, a river may sever a map, because the player can answer it.

It is also the first mechanic that lets the player change what the network can reach. Everything laid
so far follows the terrain; a bridge overrules it, at a price, and that is a different kind of
decision from any the game has offered.

## The shape it will take

Three places, and the middle one is the one to be careful with:

- **`place_road`** — or a command of its own; that is the first decision below. It has to refuse more
  than it accepts: a span longer than the limit, a crossing that does not land on walkable ground at
  both ends, a bridge that starts in the middle of water.
- **`bfs_roads`, step 2 of the tick.** The network labelling is what makes a bridge mean anything: two
  components on either bank become one. Step 2 is rebuilt only when dirty, so the cost is bounded, but
  it is still the tick and the change belongs under a measurement.
- **The tile.** A road on water needs to be distinguishable from a road on land — for the renderer, for
  demolition, and for the loader's connectivity check. Phase 19 left three spare bits and named a
  bridge flag as one of the things they were left for.

Demolition is where this kind of mechanic usually goes wrong: taking a bridge away has to split the
network back into two, and a rule that only ever adds connections is a rule that has been tested in
one direction.

## The decisions it will produce, none of them taken

- **Its own command, or a case inside `PlaceRoad`.** A separate command is honest about the cost and
  gives the bot and the LLM something to aim at; a case inside `PlaceRoad` keeps one way to lay a road
  and lets the rules decide what it costs. The command set has grown by one item since M0 and each
  addition is permanent, so this is worth arguing rather than assuming.
- **What a bridge is allowed to cross.** A straight span up to some length is the simple answer.
  Whether it may turn, whether it may run along the water instead of across it, and whether deep water
  refuses outright are all questions the simple answer leaves open, and the last one implies a notion
  of depth that does not exist yet.
- **What happens to phase 19's fourth loader refusal** — walkable tiles must form one region. It stays,
  it weakens, or it changes into "one region once every crossable span is counted". A severed map that
  the player *can* join is now a legitimate map, and possibly an interesting one; a severed map they
  cannot join is still a broken asset.
- **Whether a bridge can be built on and covered across.** A house cannot stand on it — that much is
  clear. Whether service coverage walks over it follows from step 3 treating it as road, which it will
  by default, and "by default" is not the same as decided.

The failure names this phase will need are descriptive of what went wrong and carry the numbers the
caller needs to act — the span it found against the span allowed, in the shape phase 19's `TooSteep`
uses. The spellings are not chosen here; the naming rule in [CLAUDE.md](../CLAUDE.md) applies, and
every limit is a parameter in `sim-data`.

## Out of scope, as things stand

- **Boats, ferries and anything that moves on water.** Water stays scenery with a road over it.
- **Bridges over anything but water** — over a cliff, over a road. Different rule, different phase.
- **Levels of bridge.** One kind first. A footbridge and a road bridge is a distinction to make once
  there is traffic that tells them apart, which is M3 at the earliest.
- **Aqueducts.** Water as a service is coverage (D2), not a thing that flows along a structure, and
  nothing about this phase changes that.
- **Water depth**, unless the decision above forces it.

## What its tests will have to prove

1. **A bridge joins two components into one**, and demolishing it splits them back. Both directions,
   in the same test file, because only one of them is ever written by accident.
2. **Every refusal leaves nothing behind**: no tile mutated, treasury untouched. The no-partial-
   mutation discipline on a new failure path, which is where it has always had to be proved.
3. **A city on a two-bank map** reaches the same population band as one on a single bank once the
   bridge is up, and cannot before it. That is the test that says the mechanic is reachable and
   observable in a scenario, rather than merely implemented.
4. **The recordings.** A bridge changes step 2's output, so the divergence has to start at the
   checkpoint after the bridge is laid and not before it. If it starts earlier, something else changed.

## Verification

The protocol in [CLAUDE.md](../CLAUDE.md) applies unchanged, and `bench` matters more here than in
most phases: this is the first change to step 2 since phase 05. Run it before and after on the same
machine.
