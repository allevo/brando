# Roadmap

**This file answers one question: what is next?** It holds the future and the undecided, and it is
the **only** place that states how far the tree has got. If another document tells you what is
implemented, that document is wrong and should be pointed here instead.

> ## Implemented through phase 14
>
> Everything up to and including [phase 14](plan/14-births-deaths.md) — births and deaths — plus the
> bug hunt that follows phase 13 and the vocabulary review after that. **M0 is complete; M1 is in
> progress.**

What runs today: a grid with roads and connected components, aggregate service coverage over walked
distance, food production and consumption, difficulty profiles, house satisfaction, house levels with
a monthly review, and aggregated births and deaths. Six of the ten tick steps are live —
see [ARCHITECTURE.md](ARCHITECTURE.md) for which, and [RULES.md](RULES.md) for what they do.

## How to read the numbering

Phases are whole numbers and come from [plan/README.md](plan/README.md). **Half numbers are decisions
that have to be closed before the next whole phase can start.** They are marked `TO_BE_DECIDED` and
each one names its entry in [DECISIONS.md](DECISIONS.md).

A half number is not optional work you may skip. It exists because an open decision that lives only
as prose in a register is an open decision nobody sees until it bites — which is exactly what
happened to A17 and A18, both of which stayed invisible while four phases were planned on top of
them.

## The rest of M1 — a minimal game loop

| # | What | Verified by | Blocks |
|---|---|---|---|
| **14.5** | **TO_BE_DECIDED — [A18](DECISIONS.md#a18--an-empty-house-consumes-no-capacity):** does an empty house consume provider capacity? | a decision written into A18, plus whichever test the answer implies | phase 15 |
| 15 | [Immigration and emigration](plan/15-migration.md) | two cities identical except for their coverage receive different flows, and the coverage↔population loop **damps** | |
| 16 | [Treasury and taxes](plan/16-treasury-taxes.md) | the treasury invariant stays an **exact equality** with the income in it | |
| 17 | [`sim-scenario`](plan/17-sim-scenario.md) | "500 residents within 5 years" declares itself complete on the right tick, and not before | |
| 18 | [Invariants and closing M1](plan/18-invariants-closeout-m1.md) | the new invariants are green property tests, and A12's cost is measured and attributed | |
| **18.5** | **TO_BE_DECIDED — [A17](DECISIONS.md#a17--the-per-tick-recomputation-cost):** what to do about the per-tick recomputation cost | a decision written into A17, and a measurement that survives it | M2 |

Why 14.5 comes first: A18 is one line to change **now** and a regeneration somebody has to attribute
**later**. Phase 14 made zero residents reachable on every difficulty profile, including inside the
two committed recordings, so the window in which closing it is free has already begun to shut.

Why 18.5 blocks M2: A12's cost is paid on every game and therefore on every batch of automatic
balancing, which is the project's second non-functional requirement. Phase 14 measured it and found
that two of the three expectations A17 was resting on were wrong — in particular the multiplier `J`,
hoped to be small, is **1**. No countermeasure that relies on `J` being small is worth building.

## Before M2 — the map stops being flat

| # | What | Verified by | Blocks |
|---|---|---|---|
| 19 | [Terrain: relief and cost](plan/19-terrain.md) | the same building costs more on a hillside than on the flat, and the treasury invariant stays an **exact equality** | M2 |

One phase sits between M1 and M2, and it is a deliberate exception to the rule below. Every map in the
tree today is one terrain repeated: tiles gain a ground height, a building pays for the ground it has
to flatten and is refused on a cliff, and a map becomes a file with a hash rather than a single value
in the recording's header.

It is planned ahead of its milestone because **M2's renderer depends on its shape**. A 2.5D renderer
built for a flat map is a different renderer from one that draws relief, and building it flat and then
reworking it puts the rework on the part of the tree with the fewest tests. What is fixed now is only
the structure — the packing, the map format, where the rule sits in `place_building`. The numbers, what
a step of slope costs and where the refusal falls, are set when the phase runs.

The phase file proposes that it add **no new `Terrain` variant** — with a ground height on the tile, a
mountain is high `Rock` and a hill is high `Plain`, so the frozen order need never move. That, and
whether a map is an asset or a seed, are the two decisions the phase turns on. **Neither is taken.**
They are closed *inside* phase 19 and written into [DECISIONS.md](DECISIONS.md) then, which is why
neither holds a half-numbered slot here: nothing is planned on top of them, so neither can go
invisible the way A17 and A18 did.

## Beyond M1

Sketched, not planned. The rule this project follows is that a milestone is planned only once the
previous one has closed, so that the balancing is decided after watching real ticks run rather than
before. [plan/10-beyond-m0.md](plan/10-beyond-m0.md) is the historical sketch of M2 and M3, kept
unrewritten so it can be compared with what the real plan turned out to be.

**M2 — the two clients, in parallel.** `game-bevy` renders M1's state isometrically with placeholder
assets, the point being to validate the snapshot/event boundary rather than the graphics. In
parallel: a heuristic bot that completes scenario 1, and an evaluator.

**M3 — depth.** Production chains with real logistics walkers. The `CivilizationRules` trait arrives
**together with** the second civilisation and not before. An LLM adapter on top of a bot that already
works.

Six of the ten crates in [ARCHITECTURE.md](ARCHITECTURE.md) do not exist yet, deliberately: empty
crates scaffolded in advance are surface that invites you to fill them.
