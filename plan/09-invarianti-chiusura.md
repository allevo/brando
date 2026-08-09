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

**Deciso: debito, non fatto.** Oltre ai property test c'è un fuzz deterministico
(`nessun_panic_su_diecimila_comandi`): 10.000 comandi generati da un PRNG locale con seed fisso
su 1.000 tick, che verifica a ogni tick gli invarianti strutturali. Riproducibile come un
golden, e non richiede un toolchain nightly né un'esecuzione separata in CI. Da riprendere
quando una meccanica nuova allargherà lo spazio dei comandi — con `Intent` e bot (M2/M3) è
probabilmente il momento giusto.

Il generatore deve produrre comandi **cattivi** in proporzione significativa (coordinate fuori
mappa, `kind` inesistenti, demolizioni nel vuoto, footprint sovrapposti): un generatore che
produce solo comandi validi verifica un decimo di quello che sembra verificare.

### Tripwire di performance

```sh
cargo xtask bench          # due profili: 100×100/3.000 ab. e 200×200/15.000 ab.
```

**Non impone una soglia** — non si ottimizza prima del profiler. Serve a due cose: avere un
numero di partenza registrato qui, e accorgersi se in M1 diventa dieci volte peggiore.

#### Misure di partenza

`2026-08-08`, Apple M2 Pro, `--release`, 40 ripetizioni, mediana:

| Misura | 100×100, 3.000 ab. | 200×200, 15.000 ab. |
|---|---|---|
| A. tick a vuoto, niente di sporco | 38 µs | 256 µs |
| B. tick, 1 comando rifiutato | 36 µs | 263 µs |
| C. tick, 10.000 comandi rifiutati | 61 µs | 291 µs → **2 ns/comando** |
| D. tick, 1 comando accettato | 3,3 ms | **19,1 ms** |
| E. tick, 50 comandi accettati | 3,5 ms | 20,5 ms → **1,07× D** |

Città di riferimento a fine partita: 3.750 case, 1.219 provider, 6.541 tile strada.

#### Due misure aggiunte dopo (`2026-08-09`)

Le misure sopra dicevano *quanto* costa un tick con un comando, non *dove* va il tempo. `F` e
`G` rispondono a quella domanda, e sono state aggiunte prima di ottimizzare il passo 3.

| Misura | 100×100 | 200×200 |
|---|---|---|
| F. tick, 1 strada posata o tolta | 3,48 ms | 20,20 ms |
| G. solo `calcola_da_zero` (passo 3) | 3,43 ms → 14,0 µs/provider | **19,83 ms → 16,3 µs/provider** |

Cosa dicono, e perché servivano entrambe:

- **`G` chiude la questione dell'attribuzione.** 19,83 ms su 20,05 di `D`: il ricalcolo della
  copertura **è** il costo del tick, non una sua componente. Tutto il resto — validazione del
  comando, produzione su 3.750 case, emissione degli eventi — sta nei 250 µs di `A`.
- **Il costo è per provider, non per mappa.** 14,0 µs contro 16,3 µs a fronte di 4× i tile:
  quasi piatto. Quindi il totale scala col numero di provider, e ciò che va tolto è il costo
  *dentro* il ciclo per provider.
- **`F` è il caso che nessuna cache potrà coprire.** Posare una strada invalida la topologia:
  la rete va rietichettata e ogni BFS va rifatto. Che `F ≈ D` significa che anche il caso
  frequente (costruire un edificio) oggi paga il prezzo pieno del caso peggiore. Un'ottimizzazione
  che fa scendere `D` e lascia `F` dov'è ha risolto metà del problema, e senza `F` nessuno se ne
  accorgerebbe.

#### Dopo le ottimizzazioni del passo 3 (`2026-08-09`)

Stessa macchina, stessi profili. Tre interventi, tutti a risultato bit-identico — gli hash
stampati dal bench non si sono mai mossi e `regen-golden --check` è rimasto verde a ogni passo.

| Misura | 200×200 prima | 200×200 dopo | |
|---|---|---|---|
| A. tick a vuoto | 256 µs | 248 µs | invariata, come doveva |
| B. 1 comando rifiutato | 263 µs | 248 µs | invariata |
| C. 10.000 rifiutati | 291 µs | 274 µs | invariata, 2 ns/comando |
| D. 1 comando accettato | 20,05 ms | **3,35 ms** | **6,0×** |
| E. 50 comandi accettati | 20,19 ms | 3,36 ms | 1,00× D |
| F. 1 strada | 20,20 ms | 3,42 ms | 5,9× |
| G. solo `calcola_da_zero` | 19,83 ms | **3,05 ms** | **6,5×** — 16,3 → 2,5 µs/provider |

I tre interventi, in ordine di resa:

1. `case_per_tile` da `BTreeMap<TileIdx, Vec<HouseId>>` a CSR indicizzato: 19,83 → 12,38 ms.
2. Le altre due `BTreeMap` del ciclo per provider — la ricerca del minimo fra le candidate (che
   non serviva: il BFS visita per distanza crescente) e il controllo di contesa, che veniva
   fatto su *tutte* le candidate e non solo su quelle che entrano in capacità: 12,38 → 4,85 ms.
3. Lo scratch del BFS, allocato una volta per provider invece che una per ricalcolo:
   4,85 → 3,05 ms.

**La cosa più informativa non è il 6,5×, è che il costo per provider è diventato piatto**:
2517 ns a 100×100 contro 2500 a 200×200, dove prima erano 14,0 e 16,3 µs. La dipendenza dalla
dimensione della mappa è sparita del tutto, ed era tutta nell'intervento 3.

`A` resta 248 µs ed è ora il numero da guardare, perché si paga a **ogni** tick mentre `D` solo
quando il giocatore costruisce. Dentro `A`, il candidato più probabile è `production`, che fa
una `coverage.provider(h, Cibo)` per casa — 3.750 lookup per tick nella stessa `BTreeMap` che
l'intervento 2 ha tolto dal passo 3, ma sul percorso caldo di ogni tick. Non misurato ancora.

**Cosa dicono questi numeri.** Rispondono alla prima domanda aperta di
[10-oltre-m0.md](10-oltre-m0.md) — *il passo 3 è davvero l'hot path, o lo è il rebuild della
rete?*

1. **Il passo 3 domina, e di molto.** Un tick che accetta un comando costa ~75× un tick a
   vuoto (19,1 ms contro 256 µs). Non è il costo del comando: applicarne 10.000 rifiutati
   costa 2 ns l'uno. È il ricalcolo della copertura, che oggi è ingenuo — *qualunque*
   costruzione invalida tutti i 1.219 provider, e ognuno rifà il proprio BFS.
2. **Il costo è per tick, non per comando.** Cinquanta comandi accettati nello stesso tick
   costano 1,07× un comando solo. Un bot che costruisce in blocco paga quanto uno che
   costruisce un pezzo alla volta — informazione utile all'agente di M2.
3. **Il tick a vuoto è già la maggioranza del tempo di una partita.** 3.650 tick (10 anni) a
   256 µs fanno ~0,9 s se il giocatore non costruisce mai: accettabile, ma è il numero che va
   guardato quando M1 aggiungerà evoluzione, migrazione e tasse al passo 6 e 7.

Nessuno di questi va ottimizzato adesso. Il punto 1 ha già la sua rete di sicurezza: il test di
equivalenza incrementale/da-zero della fase 06 è scritto perché *qualunque* furbizia futura
sull'incrementalità sia coperta senza riscriverlo.

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
