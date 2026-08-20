---
id: 18
kind: phase
status: not-yet-built
opened: 2026-08-09
---

# Phase 18 — `sim-scenario` and objectives

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

**Goal:** the scenario "500 residents within 5 years" declares itself complete on the right tick, and
not before.
**Depends on:** 17.
**Size:** L.
**Decisions involved:** A16, A2, A6, D7.

## Why now

Because it is the last phase that adds a mechanic, and because an objective only makes sense once
there is a game underneath: before phase 17 the city could not finance its own growth, and "500
residents within 5 years" would have been won or lost by the balancing rather than by the player.

It is also the first new crate since M0. It comes into being now and not earlier because
[plan/README](../CLAUDE.md) is explicit: *empty crates scaffolded in advance are surface that invites
you to fill it*.

> **Amended 2026-08-15.** `plan/README.md` no longer exists. The rule it was quoted for is a working
> rule rather than a record, so it moved to `CLAUDE.md`, under "Scope", and the link points there.

## What gets built

### Where what lives — the dependency graph decides

`CLAUDE.md` puts checking the objectives at **step 9 of the tick**, inside `step`. But the
dependencies point towards `sim-core`, so `sim-scenario` depends on the core and the core cannot call
it. The split is forced, and it is the same one as A2:

- In **`sim-core`**: the `Objective` enum, its evaluation, and the progress state. They are pure
  structs with no I/O, and they belong to the core's vocabulary like `ServiceKind`.
- In **`sim-scenario`**: the definition of the scenarios (initial map, commands, difficulty,
  mandatory and optional objectives), loading from a file, composition.

It is the second time the graph has dictated the boundary, and it is worth writing down: the first
was the `DataSet`, and whoever reads has to be able to recognise the pattern instead of rediscovering
it.

### The objectives live in the `World`

```rust
// sim-core
pub enum Objective {
    /// A minimum population by a deadline. The deadline is in **months**, never
    /// in ticks (A6): it is the unit the player reasons in and the one D7 wants
    /// the objectives in.
    Population { at_least: u32, within_months: Option<u32> },
    Treasury { at_least: Coins, within_months: Option<u32> },
    /// This many houses at the given level or above.
    HouseLevel { level: Level, how_many: u32, within_months: Option<u32> },
}

pub struct Objectives {
    pub mandatory: Vec<Objective>,
    pub optional: Vec<Objective>,
}

/// Progress, one entry per objective, in the order they are declared.
pub struct ObjectiveState {
    /// The tick the objective was completed on, or `None`.
    ///
    /// Once completed it **stays** completed: an objective that un-completes
    /// because the population has fallen would make victory reversible, and no
    /// campaign scenario works like that.
    completed: Vec<Option<u32>>,
    outcome: Outcome,
}

pub enum Outcome { InProgress, Won, Lost }
```

**Why in the `World` and not passed to `step`.** Keeping them in the state preserves the
`step(&mut World, &[Command])` signature D4 declares, and — above all — puts the **tick of
completion inside the state hash**. It is the only thing that makes a recording able to check this
phase's goal: "it declares itself complete on the right tick" becomes a hash that diverges if the
tick changes, instead of an `assert` in a test somebody has to remember to write.

`World::new` gains the objectives, or a `World::with_objectives`. They are validated data like the
`DataSet`, and like it they arrive from outside already checked.

### Step 9

```rust
/// Step 9 — scenario objectives.
///
/// After the finance and before the events are emitted: it evaluates the state
/// at the end of the tick, which is what the player sees.
fn check_objectives(world: &mut World, r: &mut StepReport) { .. }
```

An objective is completed when the condition is true and it is not already completed; the deadline is
evaluated in `tick / ticks_per_month`. The scenario is **won** when every mandatory objective is
completed, **lost** when one of them has a deadline that has passed.

```rust
Event::ObjectiveCompleted { index: u16, mandatory: bool },
Event::ScenarioEnded { outcome: Outcome },
```

Both are emitted exactly once: they are changes of state, not polling.

### `sim-scenario`

A new workspace member, `#![forbid(unsafe_code)]`, `lints.workspace = true`, dependencies `sim-core`
+ `serde`/`ron`/`thiserror`. It contains the description of a scenario — the initial grid, terrains,
proposed difficulty, starting commands, objectives — with loading from RON and validation with a
complete report, exactly like `sim-data`.

**An objective has to be validated against the `DataSet`**: "every house at level 4" with `levels: 3`
in the table is an unwinnable scenario, and it should be rejected at load time instead of discovered
after five years of simulated play. The same spirit as `DataSet::inconsistencies()`.

### The name `Scenario` is already taken

`xtask/src/scenario.rs` has a `Scenario` struct that is a different thing: a situation built by hand
for the runner and to give the recordings some content, with no objectives — its doc comment already
says so. It has to be renamed (`Situation`, or `TestCase`) **in this phase**, before there are three
of them and before somebody imports the wrong one.

### The scenario's recording

A new scenario that **reaches** its objective, with its `.ron` and its `.hashes`. It is the canary on
the balancing that `CLAUDE.md` asks for at Testing point 4, in the version M1 can have: the heuristic
bot is M2, but a recording that reaches the objective at tick N fails as soon as the balancing moves,
and the diff says **by how much**.

**The list of scenarios is duplicated by hand** between `xtask/src/scenario.rs::NAMES` and
`sim-replay/tests/expected.rs::SCENARIOS`, and with a new scenario forgetting one is almost certain —
and it shows up as "the new recording is never checked", i.e. silence instead of red. A zero-cost
remedy, to be done here: `expected.rs` lists the `*.ron` files in `expected_dir()` instead of having
a constant, and **fails if a `.ron` has no matching `.hashes`**. It removes the duplication and adds
a check that does not exist today.

## Out of scope

The heuristic bot and the evaluator: they are M2, and the evaluator will produce a *vector* of
metrics of which `ObjectiveState` is only the first element. Campaigns, chained scenarios, unlocks.
Objectives that depend on events (fires put out, invasions repelled): the random events are outside
M1.

## Tests

1. **The goal**: the scenario declares itself won on the expected tick, and `Outcome::InProgress` on
   the previous one. The two halves count equally — "not before" is half the goal.
2. **A missed deadline**: the same scenario with a tight `within_months` ⇒ `Outcome::Lost` on the
   deadline's tick, not later.
3. **A completed objective stays completed** even if the population later falls. It pins down the
   choice, which is arbitrary and has to be documented.
4. **Optional ones**: not completing them does not prevent victory; completing them shows up in
   `ObjectiveState`. It is the data M2's evaluator will build its score on.
5. **An unwinnable objective is rejected** at load time: a level beyond `levels` in the table ⇒ a
   validation error with the right path.
6. **The scenario's recording**: the completion tick is in the hash, so the recording protects it.
   Change one balancing number and the recording diverges — and that is the signal the canary works.
7. **`ObjectiveCompleted` emitted exactly once**, not on every tick the condition stays true. The
   same rule as `ServiceCoverageChanged` (phase 07).
8. **`expected.rs` discovers the files itself**: a `.ron` with no `.hashes` makes the test fail
   instead of going unnoticed.

## Verification

```sh
cargo test -p sim-scenario
cargo test -p sim-replay
cargo xtask run --scenario growth --ticks 1800 --dump-every 90
cargo xtask regen-expected --check
```

The new scenario's dump is the proof that closes the phase: the line where the objective completes
has to be **readable** and to fall somewhere sensible — not at tick 30 and not at 1799. If it comes
too early the scenario is trivial, if it never comes it is unwinnable; either way it is balancing
(`sim-data`), not code, and it has to be sorted out before the recording freezes it.

**Done when:** test 1 passes in both its halves and the scenario's recording is committed.
