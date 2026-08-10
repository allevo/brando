# Plan for the first stages of development — Brando

This plan covers **M0 — Foundations** (phases 00–09, complete) and **M1 — A minimal game loop**
(phases 11–19) of the roadmap in `CLAUDE.md`, broken into small phases.

M1 was planned after M0 closed, not before: doing it earlier would have meant deciding on the
balancing without having watched a single tick run. [10-beyond-m0.md](10-beyond-m0.md) remains the
document that sketches M2 and M3 by the same rule.

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
[open-decisions.md](open-decisions.md): the consistency between capacity and output (**A5**), the
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

The decision all of M1 rests on is [A12](open-decisions.md): **the services chase the population**. A
provider's capacity is counted on the residents present, so the coverage is recomputed when anyone
moves — that is, on almost every tick. It is a gameplay choice paid for in computation time, taken in
full knowledge, and the debt it opens is [A17](open-decisions.md): **the project's only open
decision**, to be closed before M2. Both are worth reading before phase 13.

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

Every phase of M1 except 11 regenerates the recordings, and that is the moment the project's most
valuable test risks becoming a ritual. The message at the top of every `.hashes` already states the
rule — *if it changes without the balancing having changed, a source of non-determinism has been
introduced: stop and find it, do not regenerate* — but applying it takes a protocol, and every phase
file cites this one instead of repeating it.

1. **Green before you start.** `cargo xtask regen-expected --check` has to be green *before* you touch
   the code. If it is not, the tree is already dirty for other reasons and the signal is lost.
2. **Exactly the files you expected.** After the change, `--check` has to list the files the phase
   declares it regenerates, not one more. A `.ron` that changes in a phase that does not touch the
   header is already the clue.
3. **Idempotence.** `regen-expected` twice in a row: the second has to say "nothing to do". It is
   where non-determinism within a process shows up first.
4. **A separate process.** `cargo test -p sim-replay` catches what point 3 cannot: memory addresses,
   `RandomState`, the iteration order of hash collections.
5. **Look at the first diverging tick** in the `.hashes` diff. It is the check nobody does and it is
   worth more than the other four: if the new mechanic cannot act before tick 60 and the diff starts
   at 30, the cause is something else and has to be found before committing. The recording's textual
   format exists for this.
6. **One reason to regenerate per commit.** A commit that regenerates the recordings and changes two
   mechanics is no longer diffable.

## Decisions

Some choices are necessary in order to implement but are not fixed by `CLAUDE.md`: they are gathered
in [open-decisions.md](open-decisions.md) with a recommendation for each. Phases 01, 03 and 08 assume
the recommendation; if one gets overturned, it changes the content of that phase, not the order of the
plan.

The document carries on past M0: A7–A11 came out of clearing up that same `dubbi.md`, A12–A16 out of planning
M1. For each one, alongside the recommendation, there is **how it really went** — which is almost
always the more useful information, because it diverges.

## Implemented till
Everything is implemented till 11 included.