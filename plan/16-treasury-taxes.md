# Phase 16 — Treasury and taxes

> **Status: not yet built.**
>
> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

**Goal:** phase 09's treasury invariant extends to the income and stays an **exact equality**; the
tax rate is a real lever, not free money.
**Depends on:** 15.
**Size:** M.
**Decisions involved:** [A15](../DECISIONS.md), A1, D4, tick order step 7.

## Why now

Because step 7 has been empty since phase 04, and because it is the last mechanic needed before a
scenario can have non-trivial objectives: with no income the treasury can only go down and every
scenario ends by running out.

And because there is now something to tax. Taxes on a constant population would be a linear function
of time; on a population that grows with wellbeing they close the ring — services → wellbeing →
population → revenue → more services. It is the ring that makes the game a game, and it has to be
tried out before an objective is put on top of it (phase 17).

The tax rate enters here **also** as the third term of the attractiveness, which phase 15
deliberately left out so as not to have to balance two things at once
([A15](../DECISIONS.md)).

## What gets built

### The command — the first since M0

```rust
pub enum Command {
    PlaceRoad { at: TilePos },
    PlaceBuilding { kind: BuildingKindId, origin: TilePos },
    Demolish { at: TilePos },
    /// A tax rate level, as an index into the `tax_rates` table.
    ///
    /// A discrete level and not a free per-thousand, for two reasons: it is the
    /// shape `Intent::AdjustTax { delta }` knows how to produce (CLAUDE.md,
    /// AI interface), and it keeps the numbers in the table instead of in the
    /// command — so rebalancing the rates does not invalidate the command logs
    /// already recorded.
    SetTaxRate { level: u8 },
}

#[error("unknown tax rate: level {level}, the maximum is {max}")]
UnknownTaxRate { level: u8, max: u8 },
```

Adding a variant to `Command` does not break the existing recordings: RON is additive and the `.ron`
files already written do not contain it. `FORMAT_VERSION` does **not** change — what changes is the
shape of the state, not that of the file. It is the distinction `FORMAT_VERSION`'s doc comment
already makes.

And the matching event, because the tax rate is state the renderer has to be able to show:

```rust
/// Emitted only when the rate **really changes**: a `SetTaxRate` that puts back
/// the level that was already there produces nothing. It is the same rule as
/// `ServiceCoverageChanged` — a delta, not an echo of the command.
Event::TaxRateChanged { level: u8 },
```

The generator in `invariants.rs::any_command()` has to produce `SetTaxRate` too, **including levels
that do not exist** in significant proportion: a generator that only produces valid commands checks
a tenth of what it looks like it checks (phase 09).

### The state

```rust
pub struct Economy {
    pub treasury: Coins,
    /// The current level, an index into `rules.tax_rates`.
    pub tax_rate: u8,
    /// Thousandths of a coin not yet rounded into a whole coin.
    ///
    /// Taxes are collected **every tick** and are worth fractions of a coin:
    /// truncating on every tick would lose most of a small city's revenue — the
    /// same cliff the demographics' `remainder` avoids for the births (phase 14).
    /// With the remainder, `treasury*1000 + remainder` is an exact equality with
    /// the running total, and it is what makes the treasury invariant a test able
    /// to find a bug instead of merely reassuring.
    pub remainder: Milli,
}

/// Running totals, for the invariant and for M2's evaluator. Diagnostics: they
/// influence no game decision and stay outside the hash, like `FoodTotals`.
pub struct EconomyTotals { pub spent: i64, pub collected: i64 }
```

`tax_rate` and `remainder` go into the hash; the totals do not.

### Step 7

```rust
/// Step 7 — finance. Continuous collection, not monthly.
///
/// Continuous so the treasury's curve stays smooth: a spike every thirty ticks
/// would be a step to explain to M2's bot and would make the average treasury
/// depend on *when* you look. The monthly number — the one the player reasons in
/// — is reconstructed by the renderer from the summary, which is its job.
fn finance(world: &mut World) { .. }
```

The arithmetic: `Σ_houses taxable_per_resident(level) × residents`, accumulated in **`i64`**, then
multiplied by the rate's per-thousand and added to `remainder`; the whole coins move into `treasury`.

The `i64` is not generic caution: 15,000 residents times a taxable base times a per-thousand leaves
`i32`, and `Milli` is `i32` ([A1](../DECISIONS.md)). The same rule as `FoodTotals`, which is `i64`
for the same reason — city-wide aggregates accumulate wide and are converted exactly once.

`taxable_per_resident` lives in `HouseLevelDef` (phase 13), where it was already planned at zero: the
better houses pay more, and it is why the player wants to make them level up.

### The tax rate in the attractiveness

The third term of `attractiveness()` (phase 15), with its weight in the table. It makes the lever a
real choice instead of free money: raising the rate increases the revenue per resident and reduces
the residents.

**A cross-table check** (`Inconsistency::TaxRateWithoutAChoice`): between the lowest and the highest
rate, the total steady-state revenue has to have an interior maximum, not be monotone. If revenue
always grew with the rate, the game would have an obvious answer and the lever would not be a lever.
It is a balancing check like `UnsustainableDemographics`, and for the same reason: a dataset that
makes a choice fake produces no visible error.

### The table

```ron
tax_rates: [
    (id: "none",   per_thousand: 0),
    (id: "low",    per_thousand: 60),
    (id: "medium", per_thousand: 100),
    (id: "high",   per_thousand: 160),
],
// Thousandths of attractiveness lost per per-thousand of tax rate.
tax_rate_weight: 4,
starting_tax_rate: 2,
```

The starting rate can move into the difficulty profile, if you want `hard` to start with less room.

## Out of scope

**Building upkeep and a negative treasury.** With no recurring outgoings the treasury can only rise
and the invariant stays simple; with them you need bankruptcy, decay of unpaid buildings and a losing
condition. It is the thing that will want to come in first in M2, and it is right that it comes in
there: it is reachable and observable in a scenario only once there is a bot playing against it.

Variable construction costs, subsidies, loans, trade. Trade is M3 with the caravans (D3).

## Tests

1. **The treasury invariant** (property test, *the goal*):
   `treasury×1000 + remainder == (starting − Σ accepted costs)×1000 + collected`, an exact equality
   after any sequence of commands. It is the extension of `the_treasury_adds_up`, and it stays an
   equality — which is what [plan/10](10-beyond-m0.md) asked for.
2. **No thousandth lost**: over ten thousand ticks with a small city, the running total matches the
   sum of the per-tick revenues. It is the test that justifies `remainder`; without the field it
   fails conspicuously.
3. **A tax rate out of range is rejected** with `UnknownTaxRate`, and the state does not change — the
   normal case, not a failure (phase 04).
4. **The lever bites**: two games identical apart from the tax rate ⇒ the one on the high rate has
   more revenue per resident and **fewer residents**. If the second effect is not visible, the weight
   in the attractiveness is too low and that is balancing, not code.
5. **`rejected_commands_mutate_nothing` changes its outcome on purpose**: with continuous taxes the
   economy changes every tick, even with every command rejected. The reformulation is **stronger**
   than the original — *the state after `step(w, cmds)` with every command rejected matches the one
   after `step(w.clone(), &[])`* — and it catches more: today it would not notice a rejection that
   consumes an RNG draw, with the new form it would.
6. **The treasury does not go negative**: `charge` keeps rejecting for insufficient funds, and there
   are no recurring outgoings. The test already exists in `no_panic_on_ten_thousand_commands`, and
   should be extended to the fact that `remainder` is never negative.
7. **A zero tax rate**: zero revenue, `remainder` still, no division by zero anywhere.
8. **The hash covers the tax rate and the remainder**: two perturbations in
   `the_hash_covers_the_whole_state`.

## Verification

```sh
cargo test -p sim-core taxes
PROPTEST_CASES=2000 cargo test -p sim-core --release the_treasury_adds_up
cargo xtask run --ticks 1800 --dump-every 90
cargo xtask regen-expected
```

The dump closes the phase: the treasury column has to rise smoothly, with no monthly steps, and the
city has to be able to finance its own growth. If the treasury explodes, the rate or the taxable base
is high; if it is never enough to build a well, phase 17 will not have a winnable scenario — and that
has to be sorted out now.

**Done when:** test 1 passes as an exact equality with 2000 cases, and test 4 shows both effects of
the tax rate.
