# Fase 09 — Suite invarianti e chiusura M0

**Goal:** gli invarianti elencati in `CLAUDE.md` sono property test verdi in un unico posto
identificabile, e `CLAUDE.md` dichiara M0 completato.
**Dipende da:** 08.
**Dimensione:** M.
**Decisioni coinvolte:** tutta la sezione Testing di `CLAUDE.md`.

## Perché adesso

Gli invarianti sono stati scritti fase per fase, ognuno accanto al sistema che verifica. Serve un
posto unico dove si legga *cosa il progetto garantisce*, indipendentemente da come è
organizzato il codice: è il documento che un contributore (o un LLM che lavora sul repo) legge
per capire cosa non deve rompere.

## Cosa si costruisce

### `sim-core/tests/invarianti.rs`

Un solo file, un generatore di sequenze di comandi condiviso, e un invariante per test —
raccogliendo quelli già scritti nelle fasi 04–07:

```rust
/// Invarianti del core (CLAUDE.md, Testing punto 1). Ognuno di questi ha
/// trovato o troverà un bug vero; nessuno va silenziato per far passare la CI.
///
/// Generatore condiviso: sequenze di Command arbitrari (inclusi invalidi)
/// su una griglia 32×32, poi N tick.
```

| Invariante | Origine |
|---|---|
| Popolazione mai negativa; abitanti coerenti con le case esistenti | D5 |
| Cibo conservato: `prodotto == consumato + perso + Σ stock` | fase 07 |
| Nessuna sovrapposizione di edifici, in entrambe le direzioni tile↔edificio | fase 04 |
| Tesoro coerente: `iniziale − Σ costi accettati` | fase 04 |
| Copertura incrementale ≡ copertura da zero | fase 06 |
| Etichettatura della rete indipendente dall'ordine di costruzione | fase 05 |
| Nessun panic su comandi arbitrari, inclusi malformati | Testing punto 3 |
| Ogni tile occupato risolve a un id vivo nello slotmap | fase 04 |

### Fuzzing dei comandi

`proptest` con 10.000 casi in release copre già il punto 3 di `CLAUDE.md`. Un target
`cargo-fuzz` vero (`fuzz/fuzz_targets/commands.rs`) è utile ma non è un blocco per M0: se il
tempo stringe, va annotato in questo file come debito con la motivazione, non fatto a metà.

Il generatore deve produrre comandi **cattivi** in proporzione significativa (coordinate fuori
mappa, `kind` inesistenti, demolizioni nel vuoto, footprint sovrapposti): un generatore che
produce solo comandi validi verifica un decimo di quello che sembra verificare.

### Tripwire di performance

```sh
cargo xtask bench-smoke     # 200×200, ~1000 edifici, 3650 tick (10 anni)
```

Stampa tick/s e il tempo del passo 3 (copertura). **Non impone una soglia** — non si ottimizza
prima del profiler. Serve a due cose: avere un numero di partenza registrato in questo file, e
accorgersi se in M1 diventa dieci volte peggiore. Registrare qui il valore misurato, con data e
macchina.

### Aggiornamento della documentazione

- `CLAUDE.md`, sezione **Stato attuale** → `Milestone: M0 — completato`, con una riga su cosa
  copre.
- Se durante M0 una decisione si è rivelata sbagliata o incompleta, è **questo** il momento di
  scriverlo in `CLAUDE.md`, non di ricordarselo.
- [decisioni-aperte.md](decisioni-aperte.md): segnare come chiuse quelle risolte e annotare
  come sono state risolte davvero (spesso diverge dalla raccomandazione, ed è l'informazione più
  utile per chi legge dopo).

## Fuori scope

Ottimizzazione. Scenari con obiettivi, bot euristico, evaluator: sono lo scenario test
(`CLAUDE.md`, Testing punto 4) e hanno bisogno di `sim-scenario`, che è M1.

## Test

Questa fase *è* test. Il criterio è che ogni riga della tabella sopra corrisponda a un
`#[test]` esistente, e che nessuno sia `#[ignore]` senza una riga di motivazione accanto.

## Verifica

```sh
cargo test --workspace
PROPTEST_CASES=10000 cargo test --workspace --release invarianti
cargo xtask regen-golden --check
cargo xtask bench-smoke
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

**Fatto quando:** tutti i comandi sono verdi, `bench-smoke` ha un numero registrato in questo
file, e `CLAUDE.md` dice M0 completato. A questo punto esiste un core simulabile, deterministico
e protetto da replay: le fondamenta su cui M1 può cambiare bilanciamento senza paura.
