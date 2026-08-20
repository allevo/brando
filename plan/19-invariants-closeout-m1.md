---
id: 19
kind: phase
status: not-yet-built
opened: 2026-08-09
---

# Phase 19 — The invariant suite and closing M1

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

> **Amended 2026-08-15.** A17 is still open, but it is no longer an entry in `DECISIONS.md`: writing
> one for a question nobody had answered was itself the mistake, and A17 has been reduced to a stub
> pointing at [19.5](19.5-the-per-tick-recomputation-cost.md), which is where the question and the
> measurement phase 14 took now live. See
> A24. The mentions of A17
> below are left standing because they record what was believed when this file was written, which is
> the point of it.

**Goal:** the table at the top of `invariants.rs` covers the new invariants, the benchmark is
re-measured with M1's measures, and `CLAUDE.md` declares M1 complete.
**Depends on:** 18.
**Size:** M.
**Decisions involved:** the whole Testing section of `CLAUDE.md`, A12–A17.

## Why now

For the same reason as [phase 09](09-invariants-closeout.md): M1's invariants have been written phase
by phase, each next to the system it checks, and what is needed is one place where you can read *what
the project guarantees*. It is the document a contributor — or an LLM working on the repo — reads to
learn what must not be broken.

And for a reason phase 09 showed to be worth more than expected: it is **here** that you write down
how the decisions really went. A5 and A11 are the two most useful entries in
`DECISIONS.md` precisely because both of them record that the starting
hypothesis was wrong. Without this phase, M1's surprises stay in the head of whoever had them.

## What gets built

### The invariant table, updated

`sim-core/tests/invariants.rs` has a markdown table at the top listing every invariant and where it
lives. M1 adds nine and changes three.

| Invariant | Origin |
|---|---|
| `residents <= rules.max_residents(level)`, for every house and always | phase 13 |
| `population == Σ residents`, never negative | D5, replaces M0's form |
| Σ residents served by a provider `<= capacity(level)` | phase 14 |
| *A house covered by food always eats* — now a strict equality | A5, under A12 |
| If the population changed, the coverage is dirty | phase 14, it is A12's contract |
| Population conservation, an exact equality | phase 14 |
| `treasury×1000 + remainder == (starting − Σcosts)×1000 + collected` | phase 17 |
| With constant services, a house's level is monotone over 360 ticks | phase 13 |
| The coverage↔population loop damps: the oscillation does not grow | phase 15 |
| The number of RNG draws does not depend on the seed | phase 14 |
| `satisfaction <= max`, for every house and service | phase 12 |
| Incremental coverage ≡ from scratch, **on a zero-rate dataset** | phase 06, reformulated in phase 14 |

The last row is the one worth stopping on, and it is where M1 paid for
A12. `coverage_equivalence` did not survive intact: with the population moving
every tick, the end-of-tick comparison always diverges, by construction. The form is the same, but it
runs on a fixture with the demographics switched off, and what it checks has become **half** of what
it used to check — the forgotten invalidation after a command, not after a change of population. The
other half is the row above, which is a different test.

It has to be written down without sugaring it: the guard on step 3 is now split in two, and whoever
wants to optimise the coverage before M2 (A17) has to keep both green.

**The three that changed their outcome on purpose**, to be noted next to the test as was done with
`a_hungry_house_is_not_saved_by_a_second_farm`:

- `population_stays_consistent` — `population == house_count × 4` is false by construction from phase
  14.
- `rejected_commands_mutate_nothing` — reformulated, and stronger: *the state after a tick of nothing
  but rejections matches the one after an empty tick*. It also catches a rejection that consumes an
  RNG draw, which M0's form did not see.
- `two_empty_ticks_recompute_nothing` — with the monthly levelling up, two empty ticks can recompute.
  Rewritten as "no recomputation if no level has changed".

And `different_seeds_give_different_hashes`, which does not change its outcome but **is re-enabled**
from phase 14 and from there can never be `#[ignore]` again. It closes a test written in phase 08 to
be red, exactly as it was designed.

### The shared generator, extended

The `a_game()` generator has to produce `SetTaxRate` too, including levels that do not exist in
significant proportion — phase 09's rule still holds: *a generator that only produces valid commands
checks a tenth of what it looks like it checks*.

And the generated games have to be **long enough** to let the demographics run: a sequence of twelve
ticks never sees a monthly review, so it would test nothing of what M1 added. A second generator of a
few hundred ticks, with fewer cases, is worth having.

### The benchmark, re-measured — and it is the phase's main deliverable

M1 bought a game dynamic with some computation time (A12), and this phase has to
say **how much**, so that whoever optimises before M2 knows where to look. A single number is not
enough: attribution is needed.

`cargo xtask bench` with three new measures alongside M0's A–G, on the same machine and with the same
method (`--release`, 40 repetitions, median):

- **H. an empty tick with the demographics switched off** (zero rates). It is the point of
  comparison: it has to match `A` at the end of M0, 248 µs. If it does not, the cost is not the
  coverage recomputation but step 6 itself, and that is another item to remove.
- **I. step 6 alone.** The analogue of M0's `G`, and it separates the demographic work from the
  recomputation it triggers. Without it, the cost can only be deduced by subtraction, and the
  difference is dominated by noise.
- **J. the fraction of ticks in which the population moved**, over a whole game. It is the real
  multiplier: A12's total cost is `J × G`, not `G`. In a full or empty city `J` is low and nothing is
  paid; in a growing one it is close to 1.

  > **Refuted — read the measurement below before using this bullet.** `J` was measured in phase 14
  > and it is **1**: 502 recomputations in 502 ticks at the reference scale, 97% at mid-game. A city
  > large enough has at least one birth or death maturing every single tick, so A12's cost is `G`,
  > not `J × G`. The discount this paragraph assumes **does not exist**, and no countermeasure that
  > relies on `J` being small is worth building.

The measures have to be **recorded in this file**, with the date and the machine, as in
[09](09-invariants-closeout.md). And they have to be read together: `A` was 248 µs at the end of M0
and it is the number A11 names as the one to watch, because it is paid on every
tick. With A12 it becomes `248 µs + J × 3.05 ms`, and the expectation is ~3.3 ms in a living city —
**~13×**.

That number is not a failure: it is A12's declared price, decided in full knowledge. But it is also
the input to A17, the open task to close **before M2**, and in this phase it has
to be written down without rounding it downwards.

> **Taken early, in phase 14** (2026-08-12, Apple M-series laptop, `--reps 100`, one sitting). The
> numbers are in A17; this file has to re-take them at the end of M1, when
> migration and taxes have added their own work to step 6, and compare.
>
> | | 100×100, 3,000 res. | 200×200, 15,000 res. |
> |---|---|---|
> | `H` empty tick, demographics off | 43 µs | **324 µs** |
> | `A` empty tick, real rates | 700 µs | **3.600 ms** |
> | `G` `compute_from_scratch` alone | 618 µs | **3.251 ms** |
> | `J` ticks where the population moved | 97% | **100%** |
> | `I` step 6 alone, derived | ~51 µs | **~25 µs** |
>
> Two of the three expectations written above turned out wrong, and the corrections belong here
> because this file is what re-measures them.
>
> **`J` is 1, not "low".** The sentence above — *in a full or empty city `J` is low and nothing is
> paid* — describes a city that does not exist at this scale: with 3,750 houses at least one birth
> or death matures on every tick, so the population moves on all 502 of them. A12's cost is `G` and
> not `J × G`. The multiplier this decision was counting on as a discount is not there.
>
> **`H` did not match M0's `A`**: 324 µs against ~280 µs on the same machine at the end of phase 13.
> By the rule stated three lines above, that makes it *another item to remove*, and a separate one —
> step 6 scans the houses three times even with the rates at zero, and that scan is nothing to do
> with the recomputation.
>
> **`I` was measured after all, by subtraction, and it is ~25 µs against `G`'s 3.25 ms.** The
> demographic work is not the cost by two orders of magnitude. With `J` at 1 the derivation is
> `A − H − G`, a difference of large numbers, so it is worth reading as "small" rather than as three
> significant figures.

Three corrections to the bench, which the phases have accumulated:

1. `Layout::new` sizes the city on the residents there will actually be: with
   `starting_residents_per_house` depending on the difficulty, "15,000 residents" would become "3,750
   houses of zero residents". Print `houses`, `total capacity` and `population` separately, because
   with A12 the providers' load depends on the third and not on the second.
2. The count of recomputations has to be read before and after **every** measure and printed as a
   delta. It is what makes `H` and `J` interpretable, and what tells "the tick costs because it
   recomputes" apart from "the tick costs and nobody knows why".
3. The difficulty profile has to be chosen **explicitly** and printed in the header, next to the
   dataset hash and for the same reason: a number that moves has to be attributable.

### The documentation

- `CLAUDE.md`, **Current state** → `Milestone: M1 — complete`, with one line per crate on what it
  covers and a list of what M1 deliberately does not have.
- M1's "things learned while implementing", in the same form as M0's two. The candidates, to be
  confirmed or refuted by what will actually have happened: that a green test can check nothing at
  all, if the number of RNG draws depends on the seed (phase 14); and that a *gameplay* choice — the
  services chase the population — bought a quantifiable computation cost and a loss of test coverage,
  both accepted with eyes open (A12).
- `DECISIONS.md`: A12–A16 marked as closed, **with how they really went**.
  For A12 in particular: the expected cost was ~13× on the empty tick — say what it turned out to be,
  and whether the game dynamic it was buying really did show up in the games.

  > **Amended 2026-08-15.** There is no `DECISIONS.md` to update: it was removed, and a decision is
  > now recorded in the comment on the code it binds. The instruction still holds, at the new
  > address — say what the cost turned out to be, in the comment where the coverage is recomputed,
  > and in this task's own "How it went".
- **A17 stays open**, and it is the only one. It has to be updated with the
  numbers measured here, which are its input: it is the task to close before M2.
- [10-beyond-m0.md](10-beyond-m0.md): the questions M1 answered have to be struck through with the
  answer, as was done with M0's. Two are already there waiting: *does 30 ticks/month give a playable
  curve?* — phase 18's scenario says — and *is a 4-byte `Tile` enough?*

## Out of scope

**Optimising the per-tick recomputation**, which is A17's subject and has to be
done before M2, not here. This phase **measures** it and gives it the numbers; separating the two is
deliberate, and it is A11's lesson: the obvious hypothesis about where the cost was turned out to be
wrong, and it was the measurement that said so. Optimising in the same phase you measure in means not
having the before.

The heuristic bot and the evaluator: they are M2, and they are the real canary on the balancing
(`CLAUDE.md`, Testing point 4). The `cargo-fuzz` target, still debt from phase 09: M1 widened the
command space by a single variant, so the condition phase 09 set for picking it up again — "when a
new mechanic widens the command space" — has not really tripped yet. With `Intent` and the bot
(M2/M3) it will.

## Tests

This phase *is* tests. The criterion is the same as 09's: every row of the table corresponds to an
existing `#[test]`, and none is `#[ignore]` without a line of reasoning next to it — and in M1 the
list of legitimate `#[ignore]`s has to be **empty**, because the only one there was has been
re-enabled.

## Verification

```sh
cargo test --workspace
PROPTEST_CASES=10000 cargo test --workspace --release invariants
cargo xtask regen-expected --check
cargo xtask bench
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

**Done when:** every command is green, measures H, I and J are recorded in this file and carried over
into A17, and `CLAUDE.md` says M1 is complete. At that point there is a game: a
city that grows if you serve it, empties out if you neglect it, pays its taxes and can say when you
have won — deterministic, measured and protected by recordings. It is what M2 can rest a renderer and
a bot on without touching the core.
