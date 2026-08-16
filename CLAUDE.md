Instructions for developing this repository. Read all of it before writing any code.

**This file answers one question: how do I work here?** What the project is, what the game does, where
the code lives and how far it has got are four other questions, and each has exactly one file that
answers it — the table below. Nothing here states the current state of the tree.

*Why* is not on that list, and that is deliberate: it is answered where it binds. A rule the code has
is stated in full in the comment on that code; a question still open is stated in its own task; and
the rules that bind code nobody has written yet are the **constitution**, `D1`–`D7`, which is the one
task holding rules instead of a record of work.

Every identifier, comment and document in this repository is in English. Any word you do not
recognise is defined in [GLOSSARY.md](GLOSSARY.md).

---

## The documents, and which one to update

Each document answers **exactly one question**, and nothing else may answer it. Two documents that
can both answer the same question will eventually disagree, and then neither can be trusted.

| Question | File | Lifecycle |
|---|---|---|
| What is this project, and what do I read first? | [README.md](README.md) | present tense, edited in place |
| Where does the code live, how does data flow? | [ARCHITECTURE.md](ARCHITECTURE.md) | present tense, edited in place |
| What does the game do? | [RULES.md](RULES.md) | present tense, **no values** — names of parameters only |
| What is next, what is undecided? | [ROADMAP.md](ROADMAP.md) | the only statement of how far the tree has got |
| What does this word mean? | [GLOSSARY.md](GLOSSARY.md) | edited in place |
| How do I work here? | this file | edited in place — **no statements of current state** |
| How do I open and close a task? | [the `task` skill](.claude/skills/task/SKILL.md) | edited in place |
| What happened, in order? | [plan/](plan/) | **frozen history** |

The last row but one is a **skill**, which is loaded when the job comes up rather than read every
session. Opening and closing a task is a procedure followed at two particular moments, not a rule to
carry around, and it answers that question in full — this file does not answer it at all.

**`plan/` is a record of moments in the past and is never edited to match the present.** When a task
document turns out to describe behaviour that later changed, you do **not** rewrite the sentence: you
append a dated amendment block below it. The original prediction next to what really happened is the
most useful thing in the whole record — rewriting it destroys the only evidence of how the design
moved.

**A link is not prose.** When a frozen document points at a file that has since been deleted or
renamed, repoint the link and leave the sentence exactly as it was written. A dead link destroys the
meaning of the sentence carrying it, where repointing preserves it: the freeze is on what a document
says, not on where its links land.

**A task written before the register was removed may cite an `A<n>`, and it resolves to nothing.**
Those were implementation decisions in a register that has been removed; the rules they held are now
stated in full at the places they bind, and the tasks that name them were left as written rather than
rewritten to hide it. Reading `A12` in one and finding no `A12` anywhere is expected, not a broken
link. **That includes plans not yet built**, several of which still open with a
`**Decisions involved:**` line, so a phase taken off the shelf has to be read with it in mind. A task
written now carries none. `D<n>` still resolves, and always will.

### What a task looks like

Every document under `plan/` is a task, and every task has **one goal only**, closed by a command you
run that gives a yes-or-no answer. If a task ends and you cannot say whether it worked, the task was
badly defined. One task is one commit, or one PR, and you do not start the next one with the last
still red.

A task's id is the number its file name opens with, and the file states that id again in front
matter — twice on purpose, once where a reader finds it and once where a link finds it. `kind` says
what the document is, `status` says how far the work got, and they are two fields rather than one
because they answer two different questions: one field answering both is how a document ends up
disagreeing with itself. `doc-check` reads all of it, along with the dates against `ROADMAP.md`'s row
and an open question against the roadmap slot that schedules it. Which fields exist, which values
they take, and which shape the body takes are in the skill, whose template is itself checked against
the code.

One id namespace, cited by id and never by path: `D1`–`D7`, the **constitution**, taken before any
code and binding on all of it. If an implementation seems to require breaking a `D`, stop and ask.
`doc-check` refuses a `D` the constitution does not define, and refuses any other id letter outright.

### The definition of done for a phase

A phase is not finished until the tests are green, the recordings are accounted for, each of
`ARCHITECTURE.md`, `RULES.md`, `ROADMAP.md` and `GLOSSARY.md` has been updated or consciously
declared unaffected — say which, explicitly, because "I did not think about it" is the failure this
guards against — and the task file has its `## How it went`, including the ways the plan was wrong.
The full list is in the skill, together with the edits that close a task, because they are read at
the moment a phase closes and at no other.

The one of them that reaches further than the task file: any decision taken along the way is written
where it binds — see below — and any decision **deferred** gets a half-numbered **open question**
slot in `ROADMAP.md`, at the point where it has to be answered, and a task of its own holding the
argument.

### Recording a decision

**A decision is recorded where it binds, and nowhere else.** There is no register. There was one, and
it was removed because it answered a question the code already answered: across 176 citations in 38
source files, not one comment leaned on it — every one stated its rule in full and wore the id as a
decoration. A second copy of a rule that nothing keeps in agreement with the first is not a record,
it is a fork waiting to be noticed.

So, by case:

- **A rule the code has.** In the comment on that code, stated in full. This is the common case, and
  the naming rule below already required it: the sentence has to teach the rule to a reader who
  cannot look anything up.
- **A rule that binds code nobody has written yet.** The constitution, and only if it is genuinely of
  that kind. It is one task, and it has not grown since the first commit.
- **A question not yet settled.** A half-numbered **open question** slot in `ROADMAP.md`, at the
  point where it has to be answered, plus a task of its own holding the argument. The slot exists
  because such a question goes invisible otherwise, which is what happened twice while four phases
  were planned on top of it. `doc-check` fails if a slot is not half-numbered, names no task, or
  names one that has been built.
- **The reasoning for work that has not run yet.** In that task's file, which opens by saying it is a
  plan and may diverge.

The test to apply before writing any of it down: *has the work that settles this actually been done?*
If the answer is "no, but I am confident", it is a prediction and belongs in a task, saying so.
Confidence is not a decision.

### Regenerating the recordings without losing the signal

Almost every phase regenerates the recordings, and that is the moment the project's most valuable
test risks becoming a ritual. The header of every `.hashes` states the rule — *if it changes without
the balancing having changed, a source of non-determinism has been introduced: stop and find it, do
not regenerate* — but applying it takes a protocol:

1. **Green before you start.** `cargo xtask regen-expected --check` has to be green *before* you
   touch the code. If it is not, the tree is already dirty for other reasons and the signal is lost.
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

---

## Testing — mandatory, not optional

No PR adds a system to the core without the corresponding tests. In order of value:

**1. Property tests (`proptest`) on the invariants.**
Population never negative, goods conserved along the production chain, no overlap between buildings,
a treasury consistent with the transactions. These find the real bugs.

**2. Recorded replays.**
One `seed + Vec<Command>` file per scenario. Every N ticks a `blake3` hash is computed over a fixed
serialisation of the state, and compared against a reference file.
If the balancing changes, the hashes change on purpose and are regenerated with
`cargo xtask regen-expected`. **If they change when they should not, a source of non-determinism has
been introduced: stop and find it.** This is the project's most valuable test.

**3. Fuzzing the commands.**
Random sequences of commands, absurd or malformed ones included, must never cause a panic.
The LLM will produce invalid commands: the core rejects them with a structured error.

**4. Scenario tests.**
A heuristic bot has to complete scenario 1 within N months. It is the canary on the balancing: if it
fails after a parameter change, the difficulty curve is broken.

**Performance — not a test.** `cargo run --release -p xtask -- bench` measures the cost of `step()`
on a city at the reference scale, separating the application of the commands from the recomputations
that follow. It has no thresholds: absolute times depend on the machine, and the way to use it is to
run it before and after a change on the same machine. The one thing that has to stay the same in
absolute terms is the state hash it prints: if that moves without the rules or the tables having
changed, the optimisation has changed the semantics and it is a bug.

---

## Code conventions

- Errors with `thiserror` in the libraries. No `unwrap()`/`expect()` in the core, except for provably
  impossible invariants, and in that case with a comment explaining why.
- No `async` in the core: it is a tick-based simulation, it has nothing to wait for.
- Newtypes for the ids (`BuildingId`, `HouseId`, `TileIndex`), never a bare `usize` in a signature.
- Balancing numbers **never** live in the code: they go in `sim-data` (D6).
  If a numeric game constant shows up in a `.rs`, it is a bug.
- `#![forbid(unsafe_code)]` in every `sim-*` crate.
- Comments in English, like the rest of the repository. Doc comments on the public traits and on
  every non-obvious invariant.
- **A comment states its rule in full**, with no tag standing in for a sentence. A reader with no
  access to any document should still learn the rule from the comment alone. This used to be phrased
  as "state the rule *and* tag it with the id", and the tags are gone precisely because the sentences
  were already carrying everything. The one id that survives is a `D`, and it is still a label on a
  full sentence and never a substitute for one. Where a rule resolves to a code item, link it there
  (`/// [`DifficultyDef`]: crate::data::DifficultyDef`): rustdoc then checks the link for you.
- **Never cite a document by path from code.** Cite the stable id and state the rule where you cite
  it. Paths rot when files move; ids do not. `doc-check` enforces this for `plan/`.
- **Naming — plain words, and which hard words earn their place.**
  A hard word earns its place when it is the domain's own word, and then it is defined in
  [GLOSSARY.md](GLOSSARY.md): you learn it once and it pays you back. `capacity`, `provider`,
  `satisfaction`, `coverage`, `terrain` are of that kind and are staying. A hard word that is merely
  a synonym choice does not earn anything: nobody learns from `InsufficientFunds` what
  `NotEnoughMoney` tells them for free, and `hysteresis` was retired in favour of `gap`.
  The bar is an elementary reading level, in English, for a reader who is not a native speaker.
  **If a word is not plainly elementary and not already in `GLOSSARY.md`, ask before inventing it.**
  This binds names that are sketched but not yet written, wherever they are sketched: the shapes in
  `ROADMAP.md` are illustrations of what a descriptive failure looks like, not approved spellings.

---

## Scope, and two lessons that outrank any decision taken in the abstract

**Do not expand the scope.** A city builder has an enormous number of interconnected systems and it is
extremely easy to spend months on mechanics nobody ever plays. Every new system has to be reachable
and observable in an existing scenario. The same rule applies to the workspace itself: a crate comes
into being in the phase that fills it, because **empty crates scaffolded in advance are surface that
invites you to fill it**.

1. **Measure before you optimise, and measure the thing you are about to change.** Twice now the
   obvious hypothesis about where the cost lay has been wrong, and the measurement said so before it
   got expensive. Once for step 3: invalidating the coverage only where it changed was the obvious
   win, and the measurement found the condition true on **100% of ticks at the reference scale**, so
   there was nothing to skip and the optimisation was not built. Once for the multiplier that
   counting capacity on the residents present was expected to earn back as a discount. Doing the
   optimisation in the same phase you take the measurement in means not having the *before*.
2. **A balancing number that has to stand in a particular relation with another one is a validation
   check, not a comment.** This came out of coverage on a "first capacity taken, first served" basis
   making hunger an **absorbing** state. The fix was not counting capacity in
   another unit — that restates the same constraint — but making a producer's capacity consistent
   with what its output sustains, from which the invariant *a house covered by food always eats*
   follows. Generalise it: if you find yourself writing a comment explaining why two numbers must
   agree, write a check instead.

The same rule now covers the documents. If a document has to agree with the code, that agreement is a
check in `cargo xtask doc-check` — not a promise to remember.
