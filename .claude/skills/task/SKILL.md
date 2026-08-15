---
name: task
description: Open a new task under the plan directory — a phase or an open question — or close one that is finished. Use when the user asks to start or plan the next phase, write a task file, add a roadmap slot, record an open question, amend a frozen task, or mark a phase done.
allowed-tools: Read, Write, Edit, Bash, Grep
---

# Task

Every document under `plan/` is a task. Writing one always touches two files at once — the task file
and `ROADMAP.md` — and `cargo xtask doc-check` is the authority on whether they agree. This document
covers the part the checker cannot see: which shape the body takes, and in what order the two files
are edited.

Run `cargo xtask doc-check` after every step below. It is fast, it reads both files, and its message
names the rule it is enforcing. Do not memorise its rules from here; get them from it.

## Which job

- **Opening** a task that does not exist yet → *Opening*, below.
- **Closing** one whose work is finished → *Closing*.
- **Correcting** a task that is already closed → *Amending*. Never edit frozen prose in place.

---

## Opening

### 1. Choose the id

Read `ROADMAP.md` first: it is the only file that states how far the tree has got, and it is where
the free numbers are visible.

- A **whole number** for work that reads the state the phase before it introduced. It goes at the end
  of the chain.
- A **half number** for work that falls between two whole phases and claims nothing about the state
  before it — a fix, a piece of tooling, an open question.

Numbers compare one component at a time as whole numbers, never as decimals. So `14.10` comes *after*
`14.9`, the slots between two phases do not run out at nine, and `14.5.5` is legal. Nothing is ever
added to an id or averaged with it: it is an ordering device, not a quantity.

### 2. Name the file

`plan/<id>-<title-in-kebab-case>.md`. The name has to **open with the id**, because the file states
that id again in its front matter and `doc-check` compares the two — stated twice on purpose, once
where a reader finds it and once where a link finds it.

The title is a plain-English statement, not a label: *every task says what it is*, *a half number is
read, not cut short*, *an empty house consumes no capacity*. The naming rule in `CLAUDE.md` binds it.
If a word is not plainly elementary and not already in `GLOSSARY.md`, ask before inventing it.

### 3. Write the front matter

It is the first thing in the file, and it is the whole of what the checker reads:

```yaml
---
id: 14.4
kind: phase
status: implemented
opened: 2026-08-13
closed: 2026-08-14
---
```

- `kind` is one of `phase`, `open-question`, `constitution`. It says what the document is.
- `status` is one of `implemented`, `not-yet-built`, `superseded`. It says how far the work got.

They are two fields rather than one because they answer two different questions, and one field
answering both is how a document ends up disagreeing with itself.

A task you are opening is `status: not-yet-built` and carries **no** `closed` date — work that is not
finished cannot say when it finished, and a date on it would be a guess presented as a record. Dates
are `YYYY-MM-DD` and nothing else. Get today's from `date +%F` rather than assuming it.

There are no other fields. `size` in particular is prose, not front matter; the checker rejects it.

### 4. Write the body

Three shapes exist in the tree, and the one to copy depends on what you are opening. Read the file
named beside each as the model before writing.

**A full plan** — a phase you are about to build. Model: `plan/15-migration.md`.

```markdown
# Phase 15 — Immigration and emigration

> Nothing in it is implemented. It is a plan, and the tree may well diverge from it once the
> work is really done — see [ROADMAP.md](../ROADMAP.md).

**Goal:** one sentence, and one goal only, closed by a command that gives a yes-or-no answer.
**Depends on:** 14.
**Size:** M.
```

then `## Why now` → `## What gets built` → `## Out of scope` → `## Tests` → `## Verification`, which
closes with a bold **Done when:** line. If a task ends and you cannot say whether it worked, the task
was badly defined.

**A sketch** — a phase added to the tree long before it is designed. Model: `plan/21-bridges.md`.
Same blockquote, with a second paragraph saying it is deliberately a sketch and naming what has to
close before the detail can be written. Its sections read forward rather than back: `## Why it
exists` → `## The shape it will take` → `## The decisions it will produce, none of them taken` →
`## Out of scope, as things stand` → `## What its tests will have to prove` → `## Verification`.

**An open question** — a decision you are deferring. Model: `plan/14.5-an-empty-house-consumes-no-capacity.md`.
No blockquote, no `**Goal:**` block, and the H1 is the problem stated in plain prose with no
`Phase N —` prefix. Exactly two sections:

- `## What the code does` — name the function and the file it lives in, and quote the line that
  causes the question.
- `## Why it could be a problem` — the argument, ending in what makes it urgent now.

An open question has **no `## Verification` section**. It closes with an answer, not a command, and
all three in the tree are written this way.

Whatever the shape: no balancing number goes in a `.rs` (D6), no `A<n>` goes anywhere at all — it
names nothing and `doc-check` fails on it — and no `**Decisions involved:**` line, which older files
still carry from a register that no longer exists.

### 5. Add the row to `ROADMAP.md`

Rows are ordered by number, so an open question sits immediately before the phase it blocks.

A phase goes in `## To do`, four columns:

```markdown
| 15 | [Immigration and emigration](plan/15-migration.md) | two cities identical except for their coverage receive different flows | |
```

An open question goes in the same table, **bold in both the `#` cell and the `What` cell**, and the
`What` cell has to open with the words `**Open question` and carry a link to the task holding the
argument:

```markdown
| **14.5** | **Open question — [An empty house consumes no capacity](plan/14.5-an-empty-house-consumes-no-capacity.md):** does an empty house consume provider capacity? | an answer written into this task, plus whichever test the answer implies | phase 15 |
```

The checker enforces all of it: the number has to be a half number, the link has to name a task that
exists, and that task has to say `kind: open-question` and `status: not-yet-built`.

Work with no place in the chain yet goes in `## What next` as a bullet, not a row.

### 6. Check

```sh
cargo xtask doc-check
```

---

## Closing

A phase is not finished until all seven of these are true.

1. `cargo test --workspace` is green, and the new behaviour has the tests its task file promised.
2. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean.
3. `cargo xtask regen-expected --check` is green, **or** the recordings were regenerated on purpose
   and the reason is written down. A hash that moves without the rules or tables changing is a
   source of non-determinism: stop and find it. The protocol for doing it without losing the signal
   is in `CLAUDE.md`.
4. `cargo xtask doc-check` is green.
5. Each of `ARCHITECTURE.md`, `RULES.md`, `ROADMAP.md` and `GLOSSARY.md` has either been updated or
   consciously declared unaffected. Say which, explicitly; "I did not think about it" is the failure
   mode this list exists to prevent.
6. The task file gets its `## How it went` section — see below — and its front matter is brought up
   to date.
7. Any decision taken along the way is written where it binds; any decision **deferred** gets an open
   question, opened by the steps above. `CLAUDE.md` says which case goes where.

Then the four edits, which have to agree with each other:

**The front matter.** `status: implemented`, and a `closed` date that is today and not before
`opened`. A phase abandoned rather than built is `superseded`, which also carries a `closed` date.

**The blockquote at the top** is replaced — the plan warning goes, the pointer to the outcome
arrives:

```markdown
> See [How it went](#how-it-went) at the end, which is where the shape it really took is written
> down. Everything above that section is the plan as it was written, including the parts the work
> proved wrong — the prediction next to the outcome is the point.
```

**`## How it went`**, appended as the last section, and **the ways the plan was wrong go in it**.
That is the point of the section, not an aside: the original prediction next to what really happened
is the most useful thing in the whole record. Paragraphs open with a bold lead-in sentence. Nothing
above this section is rewritten to match the outcome.

**`ROADMAP.md`.** Move the row out of `## To do` and into `## Done`, whose three columns are
`| # | What | When |`, and whose `When` has to be the same date as the task's `closed` — the checker
compares them. Then update the status line:

```markdown
> **Implemented through phase 14.9.** M0 is complete; M1 is in progress.
```

It has to name the **highest** implemented id, which is what `doc-check` reads it as. A row whose `#`
is a range covers every task inside it, and they must all carry that row's date.

One task is one commit, or one PR, and you do not start the next one with the last still red.

---

## Amending

A task is a record of a moment in the past and is never edited to match the present. When one
describes behaviour that later changed, append a dated block below the sentence rather than rewriting
it:

```markdown
> **Amended 2026-08-15.** *Task* is now in use, and this paragraph is why it took a real distinction
> to earn it rather than a preference.
```

The exception is a link. When a frozen document points at a file that has been deleted or renamed,
repoint the link and leave the sentence exactly as written: the freeze is on what a document says,
not on where its links land.

## Notes

- Never write the literal plan directory path in a `.rs`, `.ron` or `.toml`. Paths rot when files
  move; ids do not. Cite `D4` and state its rule in full where you cite it.
- A comment that names `phase 22` or `slot 22.1` fails the build until that task exists. The checker
  reads those two words followed by a number as a citation.
- The template above is checked against the code: `doc-check` fails if its fields, kinds or statuses
  drift from what `xtask/src/doc_check.rs` accepts. If you change one, change both, and let the
  check tell you which.
- Do not expand the scope of a task while writing it. Every new system has to be reachable and
  observable in an existing scenario.
