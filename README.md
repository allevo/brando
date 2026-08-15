# Brando

A 2.5D city builder inspired by Zeus: Master of Olympus. You start with a few resources and make a
city prosper, managing the economy, food, trade and safety. Several civilisations, each with its own
building rules.

Three requirements drive the whole architecture: the core logic is **deterministic and extensively
tested**, the game is **drivable by an AI** so the balancing can be tuned automatically, and it
**runs headless** with no graphics dependency at all.

Written in Rust. The simulation has no renderer in memory when it runs headless.

## Documentation references

Read these in order.

1. **[ARCHITECTURE.md](ARCHITECTURE.md)** — where the code lives and how a tick flows through it.
2. **[RULES.md](RULES.md)** — what the game actually does, and which parameter tunes each rule.
3. **[ROADMAP.md](ROADMAP.md)** — how far the tree has got, and what comes next.

Then, as you need them: **[DECISIONS.md](DECISIONS.md)** when you want to know *why* something is the
way it is, and **[GLOSSARY.md](GLOSSARY.md)** when a word is unfamiliar — no word in this codebase
should send you to a dictionary.

## Which file answers which question

Each document answers exactly one question, and nothing else may answer it. Two documents that can
both answer the same question will eventually disagree — that is the whole reason for this table.

| Question | File | Update it |
|---|---|---|
| Where does the code live, how does data flow? | [ARCHITECTURE.md](ARCHITECTURE.md) | in place, in the same commit |
| What does the game do? | [RULES.md](RULES.md) | in place, in the same commit |
| Why is it this way? | [DECISIONS.md](DECISIONS.md) | **append only** — never rewrite an entry |
| What is next, what is undecided? | [ROADMAP.md](ROADMAP.md) | in place, freely |
| What does this word mean? | [GLOSSARY.md](GLOSSARY.md) | in place |
| How do I work in this repo? | [CLAUDE.md](CLAUDE.md) | in place |
| What happened, in order? | [plan/](plan/README.md) | **frozen** — amend with a dated block, never rewrite |

The distinction that matters: **`plan/` describes moments in the past and is never edited to match the
present.** Everything at the root describes the present and is kept true. A phase document that
contradicts the code is not a bug in the document — it is history, and it carries a status header
saying so.

## Running it

```sh
cargo test --workspace                       # the whole suite, including the doc checks
cargo xtask run --scenario minimal --ticks 360   # play a scenario headless
cargo xtask bench                            # where the time in step() goes
cargo xtask doc-check                        # the documents against the code
cargo xtask regen-expected --check           # do the recorded replays still hash the same?
```

`regen-expected --check` is the one to watch. **If a hash moves when the rules and the tables have not
changed, stop and find out why** — a new source of non-determinism has been introduced, and it is the
most valuable signal this project has.

Before committing:

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Layout

```
sim-core/      state, tick, commands
sim-data/      RON tables + validation at load time
sim-replay/    seed+log, save/load, hashing the state
xtask/         headless runner, recordings, benchmarks, doc checks
plan/          the development record, phase by phase — history, not reference
```

Dependencies always point towards `sim-core`. Six further crates are planned and deliberately not
scaffolded yet; [ARCHITECTURE.md](ARCHITECTURE.md) lists them and [ROADMAP.md](ROADMAP.md) says when
they arrive.
