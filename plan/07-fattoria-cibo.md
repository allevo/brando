# Fase 07 — Fattoria, cibo e primo loop osservabile

**Goal:** il cibo è conservato (prodotto = consumato + giacenza, sempre) e le case restano
scoperte quando la giacenza è vuota.
**Dipende da:** 06.
**Dimensione:** M.
**Decisioni coinvolte:** D2, D4 (`Milli`), A5, ordine del tick passi 4 e 10.

## Perché adesso

Fino alla fase 06 il mondo cambia solo quando arriva un comando. Con questa fase il mondo
**evolve da sé**: c'è una grandezza che sale, una che scende, e un accoppiamento tra le due. È
il primo stato interessante da mettere sotto golden replay (fase 08) — un replay su uno stato
inerte verifica poco.

## Cosa si costruisce

Passo 4 del tick, `production(world)`:

1. Ogni fattoria aggiunge `produzione_per_tick` alla propria `stock`, saturando a
   `giacenza_max`. La saturazione è semantica di gioco (il granaio è pieno, il resto si perde) e
   va commentata come tale, altrimenti sembra una scorciatoia contro l'overflow.
2. Ogni casa servita per il cibo consuma `consumo_per_abitante * abitanti` dalla `stock` del suo
   provider. Le case si servono in ordine di `HouseId` — ordine deterministico, da documentare
   come la regola della fase 06.
3. Se la giacenza non copre il consumo di una casa, quella casa **non consuma nulla** (niente
   consumo parziale) e viene marcata non servita per il cibo in questo tick.

La scelta "niente consumo parziale" è deliberata: rende la conservazione verificabile con
un'uguaglianza esatta (test 1) invece che con una disuguaglianza, e in M1 dà un segnale binario
pulito a "la casa ha mangiato questo mese".

`consumo_per_abitante` e `produzione_per_tick` vivono in `sim-data` (fase 03).

### Contabilità per il test di conservazione

```rust
/// Totali cumulativi, solo per verificare la conservazione (test 1) e per
/// l'evaluator di M2. Non influenzano nessuna decisione di gioco.
pub struct FoodLedger { prodotto: Milli, consumato: Milli, perso_per_giacenza_piena: Milli }
```

Invariante: `prodotto == consumato + perso + Σ stock delle fattorie`. Senza il termine `perso`
la saturazione del punto 1 romperebbe l'uguaglianza — motivo per cui il campo esiste.

### Eventi (passo 10)

Versione minima, perché serve alla fase 08 e a M2:

```rust
pub enum Event {
    BuildingPlaced { id: BuildingId, kind: BuildingKindId, origin: TilePos },
    BuildingRemoved { id: BuildingId },
    ServiceCoverageChanged { house: HouseId, service: ServiceKind, served: bool },
}
```

`ServiceCoverageChanged` si emette solo sui **cambi di stato**, non ogni tick: è un delta, non un
polling. È il confine con il renderer (M2) e la scelta sbagliata qui costa 40.000 eventi per
tick.

## Fuori scope

Magazzini, walker logistici, catene a più stadi (M3, D3). Evoluzione o degrado delle case per
fame: la casa registra di non aver mangiato, non ne subisce conseguenze (M1). Nessun tipo di
cibo differenziato.

## Test

1. **Conservazione** (property test, il goal della fase): dopo qualunque sequenza di comandi e
   qualunque numero di tick, `prodotto == consumato + perso + Σ stock`. Uguaglianza esatta.
2. **Nessuna giacenza negativa** (property test): per ogni fattoria, in ogni tick, `stock >= 0`
   e `stock <= giacenza_max`.
3. **Fame**: fattoria con `produzione_per_tick` inferiore al consumo delle case che copre ⇒ dopo
   N tick calcolabili a mano la giacenza si esaurisce e le case risultano non servite. Il numero
   di tick va calcolato nel test dai valori del `DataSet`, **non** hardcodato: altrimenti il test
   si rompe a ogni ribilanciamento senza segnalare nulla di reale.
4. **Ripresa**: aggiunta una seconda fattoria, le case tornano servite. Verifica che lo stato
   "affamato" non sia assorbente.
5. **Saturazione**: fattoria senza case coperte ⇒ `stock` cresce fino a `giacenza_max` e si
   ferma; `perso_per_giacenza_piena` cresce di conseguenza.
6. **Ordine deterministico**: giacenza sufficiente per due case su tre ⇒ mangiano sempre le
   stesse due (le `HouseId` minori), su 100 esecuzioni ripetute.
7. **Eventi**: `ServiceCoverageChanged` emesso una sola volta al passaggio servita→non servita,
   non a ogni tick di fame.

## Verifica

```sh
cargo test -p sim-core cibo
PROPTEST_CASES=2000 cargo test -p sim-core --release conservazione
```

Verifica manuale utile a chiudere la fase — un piccolo dump testuale in `xtask`:

```sh
cargo xtask run --scenario minimo --ticks 120 --dump-every 30
```

Deve mostrare giacenza e case servite che si muovono in modo leggibile. Se i numeri sembrano
assurdi, è bilanciamento (`sim-data`), non codice — ma va guardato adesso, perché la fase 08
congela questi numeri in un golden.

**Fatto quando:** i property test 1 e 2 passano con 2000 casi e il dump a 120 tick è
interpretabile.
