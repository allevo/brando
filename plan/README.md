# Plan for the first stages of development — Brando

> **Status: index.**
>
> The index of the development record below. It is **not** the statement of how far the tree
> has got — [ROADMAP.md](../ROADMAP.md) is, and it is the only file that is.

This plan covers **M0 — Foundations** (phases 00–09, complete) and **M1 — A minimal game loop**
(phases 11–18) of the roadmap in `CLAUDE.md`, broken into small phases, plus **19**, which sits
between M1 and M2.

M1 was planned after M0 closed, not before: doing it earlier would have meant deciding on the
balancing without having watched a single tick run. [10-beyond-m0.md](10-beyond-m0.md) remains the
document that sketches M2 and M3 by the same rule. Phase 19 is the single exception, and it is an
exception about *structure* rather than balancing — see below.

## The guiding principle

Every phase has **one goal only**, closed by a command you run that gives a yes-or-no answer. If a
phase ends and you cannot say whether it worked, the phase is badly defined.

An operating rule: one phase = one commit (or one PR). You do not start phase N+1 with N red.

## The phases of M0 — Foundations

| #  | Phase | A verifiable goal in one line | Size |
|----|-------|-------------------------------|------|
| 00 | [Workspace and guardrails](00-workspace.md) | The workspace compiles and `CLAUDE.md`'s prohibitions (floats, `HashMap`, `thread_rng`, `unsafe`) are compiler/linter errors, not conventions | S |
| 01 | [Core types and the grid](01-core-types.md) | `Milli`, the newtype ids and the `Grid` exist; `size_of::<Tile>()` is within budget and the position↔index conversion round-trips over the whole map | M |
| 02 | [One RNG per domain](02-rng-determinism.md) | The same seed ⇒ the same sequence for every domain, and adding a new domain does not knock the existing ones out of phase | S |
| 03 | [sim-data: validated RON tables](03-sim-data.md) | A valid table loads, a broken one produces a report with **every** error; the dataset has a stable hash | M |
| 04 | [World, tick, commands](04-world-tick-commands.md) | `step()` advances the tick, applies valid commands, rejects invalid ones with a structured error, and does not panic on 10,000 random commands | M |
| 05 | [Roads and the road network](05-roads-network.md) | The network is rebuilt only if `dirty` and its labelling is independent of the build order | M |
| 06 | [Aggregate service coverage](06-service-coverage.md) | The well serves the houses within range **along the roads**; the incremental computation matches the from-scratch one | L |
| 07 | [The farm, food, the first loop](07-farm-food.md) | The food produced is conserved (produced = consumed + stock) and the houses go uncovered on an empty store | M |
| 08 | [Replay, the state hash, xtask](08-replay-expected.md) | `seed + Vec<Command>` replays to the same hash; `regen-expected` is idempotent | L |
| 09 | [The invariant suite and closing M0](09-invariants-closeout.md) | `CLAUDE.md`'s invariants are green property tests; `CLAUDE.md` says "M0 complete" | M |

Dependencies: the chain is sequential, with two exceptions.
**03** is independent of **02** (they can be done in parallel after 01).
**08** only needs *some* state that evolves: if need be it can be brought forward after **05** with a
grid of nothing but roads, and the recording enriched as you go.

Between M0 and M1 there is a batch of work with no phase file, documented in
[DECISIONS.md](../DECISIONS.md): the consistency between capacity and output (**A5**), the
clearing up of a since-deleted `dubbi.md` ("open questions", **A7–A10**) and the step 3
optimisations (**A11**).

## The phases of M1 — A minimal game loop

| #  | Phase | A verifiable goal in one line | Size |
|----|-------|-------------------------------|------|
| 11 | [Difficulty](11-difficulty.md) | The same seed and the same commands at different difficulties ⇒ different hashes; the difficulty is in the replay's header and in the hash | S |
| 12 | [Satisfaction](12-satisfaction.md) | A served house reaches the maximum in `max/step_up` ticks computed from the `DataSet`; take the water away and it goes back to zero in `max/step_down` | M |
| 13 | [House levels](13-house-levels.md) | A served house reaches level 2 in a number of ticks computable from the `DataSet`; take the water away and it goes back to level 1, and it does not oscillate | L |
| 14 | [Births and deaths](14-births-deaths.md) | A served city grows until it fills up, one that loses its services empties out; the seed-sensitivity test is re-enabled | L |
| 15 | [Immigration and emigration](15-migration.md) | Two cities identical except for their coverage receive different flows; and the coverage↔population loop **damps** | M |
| 16 | [Treasury and taxes](16-treasury-taxes.md) | The treasury invariant stays an **exact equality** with the income in it | M |
| 17 | [`sim-scenario`](17-sim-scenario.md) | "500 residents within 5 years" declares itself complete on the right tick, and not before | L |
| 18 | [Invariants and closing M1](18-invariants-closeout-m1.md) | The new invariants are green property tests, A12's cost is measured and attributed, `CLAUDE.md` says M1 complete | M |

A sequential chain, no exceptions: every phase reads the state the previous one introduces.
Merges are acceptable if eight phases feel like too many: **14+15** if the demographics turn out
smaller than expected. Never 12+13, never 16 with anything, and never 14 with nothing — it is the one
that touches the hot path.

The decision all of M1 rests on is [A12](../DECISIONS.md): **the services chase the population**. A
provider's capacity is counted on the residents present, so the coverage is recomputed when anyone
moves — that is, on almost every tick. It is a gameplay choice paid for in computation time, taken in
full knowledge, and the debt it opens is [A17](../DECISIONS.md), to be closed before M2. Both are
worth reading before phase 13.

Between **13** and **14**, as between M0 and M1, there is a batch of work with no phase file: a review
of the whole tree on 2026-08-11, kept in [bug-hunt-2026-08-11.md](bug-hunt-2026-08-11.md). Six
findings, all of them latent — the suite was green throughout. Five were bugs and were fixed; the
sixth was a question nobody had asked, and it became [A18](../DECISIONS.md), the second decision
open at the same time as A17.

Three of the six were done **there** rather than later for the same reason phase 11 comes early: they
touch what the recordings see, and each of the two phases after 13 makes one of them more expensive.
The one to read is the first, because its lesson is A5's in a new place — the check that guarantees
*a house covered by food always eats* had a hole in the guard itself.

## The phase between M1 and M2

| #  | Phase | A verifiable goal in one line | Size |
|----|-------|-------------------------------|------|
| 19 | [Terrain: relief and cost](19-terrain.md) | The same building costs more on a hillside than on the flat and is refused on a cliff; the map is loaded from a file, and the dumps do not move when the tile is re-packed | L |

It follows **18** and closes before M2 starts. It is the one phase in this index planned before its
milestone, and [ROADMAP.md](../ROADMAP.md) gives the reason: M2's renderer has to be built against the
map model it will actually draw. Its structure is fixed; its numbers are not, which is the same
division M1 kept while M0 was still open.

## What gets built and what does not

Of the nine crates planned in `CLAUDE.md`, M0 brings four into being — `sim-core`, `sim-data`,
`sim-replay`, `xtask` — and M1 a fifth, `sim-scenario`, in phase 18. `game-bevy` and `agent-*` arrive
with M2, `sim-civ` with M3 and the second civilisation (D6). Empty crates scaffolded in advance are
surface that invites you to fill it.

## How a phase gets verified

Every phase file closes with a **Verification** section: the commands to run and what has to happen.
The baseline that has to stay green in every phase is:

```sh
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Every identifier in this repository is in English. Any word you do not recognise is defined in
[GLOSSARY.md](../GLOSSARY.md).

## Regenerating the recordings without losing the signal

**Moved to [CLAUDE.md](../CLAUDE.md), under the definition of done.** Every phase file cites this
protocol, and it is a rule about how to work rather than a record of what was planned — so it lives
with the other working rules and is kept true, instead of ageing here.

## Decisions

Some choices are necessary in order to implement but are not fixed by `CLAUDE.md`: they are gathered
in [DECISIONS.md](../DECISIONS.md) with a recommendation for each. Phases 01, 03 and 08 assume
the recommendation; if one gets overturned, it changes the content of that phase, not the order of the
plan.

The document carries on past M0: A7–A11 came out of clearing up that same `dubbi.md`, A12–A16 out of planning
M1, A18 out of the bug hunt after phase 13. For each one, alongside the recommendation, there is
**how it really went** — which is almost always the more useful information, because it diverges.

## How far the tree has got

**[ROADMAP.md](../ROADMAP.md) says, and it is the only file that does.** This section used to answer
the same question as `CLAUDE.md`'s "Current state", and the two disagreed — which is how a reader
ends up trusting neither.

What belongs here, because it is about the record and not about the tree: the bug hunt follows phase
13, and the vocabulary review follows that
([naming-review-2026-08-12.md](naming-review-2026-08-12.md), which produced [A19](../DECISIONS.md)
and the naming rule in `CLAUDE.md`). The review renamed identifiers only and moved no hash: it is the
one batch so far whose correctness proof is that nothing changed.
