---
id: 00
kind: phase
status: implemented
opened: 2026-08-08
closed: 2026-08-08
---

# Phase 00 — Workspace and guardrails

> It records how the phase was planned and how it went, frozen as it was written. It is
> **not** a description of the tree today: for that see [ARCHITECTURE.md](../ARCHITECTURE.md)
> and [RULES.md](../RULES.md).

**Goal:** the workspace compiles empty, and the prohibitions in `CLAUDE.md` are enforced by the
toolchain, not by the good intentions of whoever is writing.
**Depends on:** nothing.
**Size:** S.
**Decisions involved:** D1 (a pure core), D4 (determinism), code conventions.

## Why now

D4's prohibitions (iterated `HashMap`s, `thread_rng`, floats in the state) are easy to break by
accident and expensive to discover later: you discover them as a recorded replay whose hash
changes for no reason, three phases further on. Turning them into compile errors costs half an
hour now and wipes out that whole class of bug.

## What gets built

**A virtual workspace manifest** at the root (the current `src/main.rs` has to go: the headless
runner is `xtask`, not a binary at the root).

```toml
[workspace]
resolver = "3"                     # required by edition 2024
members = ["sim-core", "sim-data", "sim-replay", "xtask"]

[workspace.dependencies]
# versions pinned here, the child crates use { workspace = true }
serde     = { version = "1", features = ["derive"] }
thiserror = "2"
slotmap   = { version = "1", features = ["serde"] }
rand      = { version = "0.9", default-features = false }
rand_pcg  = "0.9"
ron       = "0.10"
blake3    = "1"
proptest  = "1"
insta     = "1"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
float_arithmetic = "deny"          # floats stay in the renderer
```

Watch out for `rand` / `rand_pcg`: they have to share the same major of `rand_core`, otherwise
`Pcg64` does not implement the `RngCore` the rest of the code expects. Check with
`cargo tree -d` that `rand_core` shows up exactly once.

**The four crates**, each with `#![forbid(unsafe_code)]` at the top of `lib.rs` (explicit as
`CLAUDE.md` asks, even though it is redundant with the workspace lint) and
`lints.workspace = true` in its own manifest.

- `sim-core` — a library, dependencies: `serde`, `thiserror`, `slotmap`, `rand`, `rand_pcg`, `blake3`
- `sim-data` — a library, depends on `sim-core` + `ron`, `serde`, `thiserror`
- `sim-replay` — a library, depends on `sim-core`, `sim-data` + `ron`, `blake3`
- `xtask` — a binary, depends on all three

The graph always points towards `sim-core`. It is worth a comment in `sim-core`'s manifest saying
that its dependency list does not grow without discussion.

**`clippy.toml`** at the root — this is where the prohibitions become mechanical:

```toml
disallowed-types = [
  { path = "std::collections::HashMap", reason = "D4: non-deterministic iteration order, use BTreeMap or IndexMap" },
  { path = "std::collections::HashSet", reason = "D4: same" },
]
disallowed-methods = [
  { path = "rand::thread_rng", reason = "D4: the RNG lives in the state and is seeded" },
  { path = "rand::rng",        reason = "D4: same" },
  { path = "std::time::Instant::now",     reason = "D4: no access to the clock in the core" },
  { path = "std::time::SystemTime::now",  reason = "D4: same" },
]
```

## Out of scope

`sim-civ`, `sim-scenario`, `agent-bot`, `agent-eval`, `agent-llm`, `game-bevy`. They come into
being when there is code to put in them (M1–M3).

## Tests

One test per crate checking that the crate exists is noise. What is needed is checking that the
guardrails **bite**, and that is done once by hand (see Verification), not with a permanent test.

What is useful instead is a test in `sim-core` that documents the intent:

```rust
/// D4: no public type of the core may expose a float.
/// A minimal sentinel, not a proof: the clippy::float_arithmetic lint is the real constraint.
#[test]
fn milli_is_not_a_float() {
    assert_eq!(core::mem::size_of::<crate::Milli>(), 4);
}
```

(to be switched on in phase 01, once `Milli` exists)

## Verification

```sh
cargo build --workspace
cargo test  --workspace          # 0 tests, green
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo tree -d                    # no duplicate rand_core
```

Then the check that matters, to be done **once and undone**: add to `sim-core/src/lib.rs`

```rust
fn probe() -> std::collections::HashMap<u8, u8> { std::collections::HashMap::new() }
fn probe2(a: f32) -> f32 { a * 2.0 }
```

`cargo clippy` has to fail with the two configured messages. If it passes, `clippy.toml` is not
being read (usually: the file is in the wrong place, or clippy was invoked from a subdirectory).
Remove the two functions and commit.

**Done when:** the four commands are green and the deliberate violation has been seen to fail.
