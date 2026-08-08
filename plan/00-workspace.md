# Fase 00 — Workspace e guardrail

**Goal:** il workspace compila vuoto, e i divieti di `CLAUDE.md` sono applicati dal toolchain,
non dalla buona volontà di chi scrive.
**Dipende da:** niente.
**Dimensione:** S.
**Decisioni coinvolte:** D1 (core puro), D4 (determinismo), convenzioni di codice.

## Perché adesso

I divieti di D4 (`HashMap` iterati, `thread_rng`, float nello stato) sono facili da violare per
distrazione e costosi da scoprire dopo: si scoprono come un golden replay che cambia hash senza
motivo, tre fasi più tardi. Renderli errori di compilazione costa mezz'ora ora e azzera quella
classe di bug.

## Cosa si costruisce

**Manifest di workspace virtuale** alla radice (il `src/main.rs` corrente va rimosso: il runner
headless è `xtask`, non un binario alla radice).

```toml
[workspace]
resolver = "3"                     # richiesto da edition 2024
members = ["sim-core", "sim-data", "sim-replay", "xtask"]

[workspace.dependencies]
# versioni pinnate qui, i crate figli usano { workspace = true }
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
float_arithmetic = "deny"          # i float restano nel renderer
```

Attenzione a `rand` / `rand_pcg`: devono condividere la stessa major di `rand_core`, altrimenti
`Pcg64` non implementa il `RngCore` che il resto del codice si aspetta. Verificare con
`cargo tree -d` che `rand_core` compaia una sola volta.

**`clippy.toml`** alla radice — è qui che i divieti diventano meccanici:

```toml
disallowed-types = [
  { path = "std::collections::HashMap", reason = "D4: ordine di iterazione non deterministico, usare BTreeMap o IndexMap" },
  { path = "std::collections::HashSet", reason = "D4: idem" },
]
disallowed-methods = [
  { path = "rand::thread_rng", reason = "D4: l'RNG vive nello stato ed è seedato" },
  { path = "rand::rng",        reason = "D4: idem" },
  { path = "std::time::Instant::now",     reason = "D4: nessun accesso all'orologio nel core" },
  { path = "std::time::SystemTime::now",  reason = "D4: idem" },
]
```

**I quattro crate**, ognuno con `#![forbid(unsafe_code)]` in testa a `lib.rs` (esplicito come
chiede `CLAUDE.md`, anche se ridondante con la lint di workspace) e `lints.workspace = true`
nel proprio manifest.

- `sim-core` — libreria, dipendenze: `serde`, `thiserror`, `slotmap`, `rand`, `rand_pcg`, `blake3`
- `sim-data` — libreria, dipende da `sim-core` + `ron`, `serde`, `thiserror`
- `sim-replay` — libreria, dipende da `sim-core`, `sim-data` + `ron`, `blake3`
- `xtask` — binario, dipende da tutti e tre

Il grafo punta sempre verso `sim-core`. Vale la pena un commento nel manifest di `sim-core` che
dica che quella lista di dipendenze non cresce senza discussione.

## Fuori scope

`sim-civ`, `sim-scenario`, `agent-bot`, `agent-eval`, `agent-llm`, `game-bevy`. Nascono quando
c'è codice da metterci dentro (M1–M3).

## Test

Un test per crate che verifichi che il crate esiste è rumore. Quello che serve è verificare che
i guardrail **mordano**, e si fa una volta a mano (vedi Verifica), non con un test permanente.

Utile invece un test in `sim-core` che documenti l'intento:

```rust
/// D4: nessun tipo pubblico del core deve esporre float.
/// Sentinella minima, non una prova: la lint clippy::float_arithmetic è il vero vincolo.
#[test]
fn milli_non_e_un_float() {
    assert_eq!(core::mem::size_of::<crate::Milli>(), 4);
}
```

(da attivare nella fase 01, quando `Milli` esiste)

## Verifica

```sh
cargo build --workspace
cargo test  --workspace          # 0 test, verde
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo tree -d                    # nessun duplicato di rand_core
```

Poi la verifica che conta, da fare **una volta e annullare**: aggiungere in `sim-core/src/lib.rs`

```rust
fn prova() -> std::collections::HashMap<u8, u8> { std::collections::HashMap::new() }
fn prova2(a: f32) -> f32 { a * 2.0 }
```

`cargo clippy` deve fallire con i due messaggi configurati. Se passa, `clippy.toml` non viene
letto (di solito: file nel posto sbagliato, o clippy invocato da una sottodirectory).
Rimuovere le due funzioni e committare.

**Fatto quando:** i quattro comandi sono verdi e la violazione deliberata è stata vista
fallire.
