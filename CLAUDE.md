Istruzioni per lo sviluppo di questo repository. Leggere interamente prima di scrivere codice.

## Cos'è questo progetto

Un city builder 2.5D ispirato a Zeus: Master of Olympus. Il giocatore costruisce una città
partendo da poche risorse e la fa prosperare, gestendo economia, cibo, commercio e sicurezza.
Il gioco supporta più civiltà (greca, egizia, ...), ognuna con regole di costruzione proprie.

Tre requisiti non funzionali guidano tutta l'architettura:

1. La logica core è testata in modo estensivo e deterministica.
2. Il gioco è pilotabile da un'AI (LLM che pianifica + bot che esegue + evaluator che giudica),
   per bilanciare automaticamente i parametri.
3. Il gioco gira headless, senza alcuna dipendenza grafica.

Linguaggio: Rust. Edition 2021+.

---

## Decisioni architetturali già prese

Queste decisioni sono state discusse e sono **vincolanti**. Non riaprirle senza discussione
esplicita. Se una implementazione sembra richiedere di violarle, fermarsi e chiedere.

### D1 — Il core non è un ECS e non dipende da Bevy

`sim-core` è un crate Rust puro. Lo stato è una struct concreta con `SlotMap`/`Vec`, non un
`World` ECS. Motivo: l'ordine di iterazione delle query di un ECS non è un contratto stabile,
Bevy fa release breaking frequenti, e per i run di bilanciamento un loop stretto su array densi
è ordini di grandezza più veloce.

Bevy è **solo** un client di rendering che legge snapshot ed emette comandi.
Headless significa eseguire soltanto `sim-core`, senza Bevy in memoria.

### D2 — Copertura aggregata, non walker di servizio

I servizi cittadini (acqua, cibo, cultura, sanità) funzionano per **copertura aggregata**:
ogni edificio serve le case entro un raggio calcolato come distanza percorsa sulla rete
stradale, non in linea d'aria. Il raggio e la capacità sono parametri letti da tabella dati e
crescono con il livello di evoluzione dell'edificio.

I walker "che camminano" visti dal giocatore per questi servizi sono **puramente decorativi**,
vivono nel renderer, sono derivati dal risultato della copertura e **non possono mai
influenzare lo stato del core**. Se un walker decorativo compare in `sim-core`, è un bug.

### D3 — Alcuni walker sono invece reali

Trasporto merci tra magazzini, carovane commerciali e immigranti sono entità simulate vere:
occupano tempo, possono essere bloccate, trasportano stato, vivono in `sim-core`.
La distinzione è per tipo di edificio e va documentata nelle tabelle dati.

### D4 — Determinismo tramite seed + log dei comandi

Un salvataggio è `seed + Vec<Command>`, non un dump dello stato. Il core è concettualmente
una funzione pura `step(&mut World, &[Command])`.

Regole non negoziabili:

- **Mai** `HashMap`/`HashSet` iterati. Usare `BTreeMap`, `IndexMap` o `Vec` indicizzati.
- **Mai** `rand::thread_rng()`. L'RNG vive nello stato ed è seedato (`rand_pcg::Pcg64`).
- RNG **separati per dominio** (eventi, migrazione, produzione). Così aggiungere una feature
  non sfasa le sequenze esistenti e non invalida tutti i replay golden.
- **Niente float nello stato.** Le quantità frazionarie usano il newtype `Milli(i32)`
  (millesimi) con operazioni checked. I float sono ammessi solo nel renderer.
- Nessun parallelismo nel core finché l'ordine di riduzione non è dimostrabilmente fissato.
- Nessun I/O, nessun accesso all'orologio di sistema dentro il core.

### D5 — Le case, non i cittadini

L'unità di simulazione della popolazione è la **casa** (contiene N abitanti, un livello di
evoluzione, lo stato di soddisfazione per ogni servizio). Non si simulano individui.
Target di scala: mappa 200×200 tile, ~15.000 abitanti.

### D6 — Civiltà = dati + regole Rust

Tutto ciò che è numerico (costi, catene produttive, raggi, requisiti) sta in tabelle RON
caricate e validate all'avvio. Le regole spaziali e strutturali (es. gli Egizi seppelliscono a
ovest, la città è divisa est/ovest) stanno in implementazioni Rust del trait
`CivilizationRules`.

Gli hook del trait devono essere **funzioni pure** sullo stato: niente I/O, niente RNG proprio.
Tenere il trait minimo. Non allargarlo su speculazione: con una sola implementazione non è
possibile sapere quale sia l'astrazione giusta. Si allarga quando la seconda civiltà lo
richiede davvero.

### D7 — Scenari e sandbox

Il gioco supporta entrambi, ma **il giocatore AI gioca solo scenari** con obiettivi espliciti
(obbligatori + opzionali), stile campagna. Lo score dell'evaluator si basa su quelli.

---

## Struttura del workspace

```
sim-core/      stato, tick, comandi. Zero dipendenze oltre serde/rand_pcg
sim-data/      tabelle RON + validazione al caricamento
sim-civ/       trait CivilizationRules + implementazioni
sim-scenario/  scenari, obiettivi, condizioni di vittoria
sim-replay/    seed+log, save/load, hashing dello stato
agent-bot/     compilatore Intent -> Command primitivi
agent-eval/    metriche e scoring di una partita
agent-llm/     adattatore: osservazione semantica, schema dei tool
game-bevy/     renderer, consuma snapshot ed eventi
xtask/         runner headless, batch di partite, rigenerazione golden
```

Le dipendenze puntano sempre verso `sim-core`, mai il contrario.
Se `game-bevy` compare tra le dipendenze di un altro crate, è un errore architetturale.

---

## Modello dello stato

```rust
pub struct World {
    tick: u32,
    grid: Grid,                          // Vec<Tile>, indice y*W+x
    buildings: SlotMap<BuildingId, Building>,
    houses: SlotMap<HouseId, House>,
    walkers: Vec<Walker>,                // solo logistici reali (D3)
    economy: Economy,
    rng: RngSet,                         // RNG separati per dominio
    dirty: DirtyFlags,
}
```

`Tile` deve restare piccolo: usare indici `u16`, non puntatori né `Option<Box<...>>`.
40.000 tile devono stare in cache il più possibile.

---

## Ordine del tick

Questo ordine è **semantica di gioco**, non dettaglio implementativo. Non riordinare senza
rigenerare i golden replay e documentare il motivo.

1. Applica i comandi in arrivo
2. Ricostruisci la rete stradale se dirty
3. Propaga la copertura dei servizi (BFS su strade dai provider dirty)
4. Produzione e consumo delle catene
5. Step dei walker logistici reali
6. Evoluzione / degrado case, migrazione
7. Finanza e tasse
8. Eventi casuali (incendi, malattie, invasioni)
9. Verifica obiettivi di scenario
10. Emissione eventi per il renderer

Il passo 3 è l'hot path. Implementarlo in modo ingenuo all'inizio va bene, ma i `DirtyFlags`
devono esistere fin da subito: retrofittarli dopo è doloroso.

Unità di tempo: **1 tick = 1 giorno di gioco**, il mese è un multiplo fisso. Gli obiettivi di
scenario si esprimono in mesi/anni.

---

## Testing — obbligatorio, non opzionale

Nessuna PR aggiunge un sistema al core senza i test corrispondenti. In ordine di valore:

**1. Property test (`proptest`) sugli invarianti.**
Popolazione mai negativa, merci conservate lungo la catena produttiva, nessuna sovrapposizione
di edifici, tesoro coerente con le transazioni. Questi trovano i bug veri.

**2. Replay golden.**
Un file `seed + Vec<Command>` per scenario. Ogni N tick si calcola un hash `blake3` su una
serializzazione canonica dello stato, confrontato con un file di riferimento.
Se cambia il bilanciamento, gli hash cambiano di proposito e si rigenerano con
`cargo xtask regen-golden`. **Se cambiano quando non dovevano, è stata introdotta una fonte di
non-determinismo: fermarsi e trovarla.** Questo è il test più prezioso del progetto.

**3. Fuzzing dei comandi.**
Sequenze casuali di comandi, anche assurdi o malformati, non devono mai causare panic.
L'LLM produrrà comandi invalidi: il core li rifiuta con un errore strutturato.

**4. Scenario test.**
Un bot euristico deve completare lo scenario 1 entro N mesi. È il canarino sul bilanciamento:
se fallisce dopo un cambio di parametri, la curva di difficoltà è rotta.

**Prestazioni — non è un test.** `cargo run --release -p xtask -- bench` misura il costo di
`step()` su una città alla scala di riferimento, separando l'applicazione dei comandi dai
ricalcoli che la seguono. Non ha soglie: i tempi assoluti dipendono dalla macchina, e il modo
di usarlo è eseguirlo prima e dopo una modifica sulla stessa macchina. L'unica cosa che deve
restare uguale in assoluto è l'hash dello stato che stampa: se si sposta senza che siano
cambiate le regole o le tabelle, l'ottimizzazione ha cambiato la semantica ed è un bug.

---

## Interfaccia AI

Il ciclo è: **LLM produce `Intent` → bot li compila in `Command` → core esegue → evaluator
giudica lo stato salvato → feedback all'LLM.**

L'LLM non ragiona mai in coordinate assolute: è pessimo a farlo e produce piani ingiocabili.

```rust
enum Intent {
    BuildDistrict { kind: DistrictKind, near: Landmark, size: u8 },
    EnsureService { service: ServiceKind, area: AreaRef },
    SetupProduction { chain: ChainId, target_rate: u32 },
    AdjustTax { delta: i8 },
}
```

Il bot restituisce `Result<Vec<Command>, IntentFailure>`, dove il fallimento è **descrittivo**
(`NoFlatSpaceNear`, `InsufficientFunds { needed, available }`, `RoadNetworkDisconnected`).
Quel messaggio torna all'LLM come feedback e chiude il loop.

Nel log di determinismo si registrano i `Command` primitivi; gli `Intent` si conservano come
metadato per l'analisi.

**Osservazione per l'LLM**: mai lo stato serializzato. Serve un riassunto — indicatori
aggregati, lista dei problemi ordinata per gravità, e una mappa ASCII downsampled a ~40×40 con
simboli per zona. Molto più efficace di qualsiasi JSON dettagliato.

**Evaluator**: produce un *vettore* di metriche, non uno scalare (obiettivi obbligatori
completati, opzionali, tick impiegati, stabilità come varianza del tesoro, resilienza come
crolli di popolazione). Lo scalare si deriva dopo, così la formula può cambiare senza rifare
i run.

---

## Confine core ↔ renderer

Bevy riceve uno **snapshot completo al primo frame** e poi **eventi delta**
(`HouseEvolved`, `BuildingPlaced`, `WalkerSpawned`, `ServiceCoverageChanged`).
Ricostruire 40.000 entità ogni tick è inaccettabile.

Il renderer non chiama mai metodi che mutano il core. L'unico canale in scrittura è la coda
dei `Command`.

---

## Convenzioni di codice

- Errori con `thiserror` nelle librerie. Niente `unwrap()`/`expect()` nel core, eccetto
  invarianti dimostrabilmente impossibili, e in quel caso con un commento che spiega perché.
- Niente `async` nel core: è una simulazione a tick, non ha nulla da attendere.
- Newtype per gli id (`BuildingId`, `HouseId`, `TileIdx`), mai `usize` nudo nelle firme.
- I numeri di bilanciamento non stanno **mai** nel codice: vanno in `sim-data`.
  Se compare una costante numerica di gioco in un `.rs`, è un bug.
- `#![forbid(unsafe_code)]` in tutti i crate `sim-*`.
- Commenti in italiano o inglese, ma coerenti all'interno del file. Doc comment sui trait
  pubblici e su ogni invariante non ovvio.

---

## Roadmap

**M0 — Fondamenta.** `World` con grid e strade, tre tipi di edificio (casa, pozzo, fattoria),
loop dei tick, `Command` primitivi, replay con hash. Nessuna grafica.
Test: property + un golden replay.

**M1 — Loop di gioco minimo.** Le case evolvono con acqua e cibo, degradano altrimenti.
Migrazione in ingresso e uscita. Tesoro e tasse. Uno scenario con obiettivo
("500 abitanti entro 5 anni").

**M2 — I due client, in parallelo.** Bevy renderizza lo stato di M1 in isometrico con asset
placeholder (l'obiettivo è validare il confine snapshot/evento, non la grafica). In parallelo:
bot euristico che completa lo scenario + evaluator.

**M3 — Profondità.** Catene produttive con walker logistici reali. Trait `CivilizationRules`
introdotto **insieme alla seconda civiltà**, non prima. Adattatore LLM sopra il bot funzionante.

### Stato attuale

> Milestone: **M0 — completato** (2026-08-08)
>
> Aggiornare questa sezione a ogni milestone completato.

Cosa copre M0, in una riga per crate:

- `sim-core` — `World` con griglia 256×256 max, strade con componenti connesse, copertura dei
  servizi su distanza percorsa, produzione e consumo di cibo, tick a dieci passi (quattro
  pieni, sei vuoti in attesa di M1/M3), comandi primitivi con errori strutturati, RNG per
  dominio. `Tile` sta in 4 byte.
- `sim-data` — tre tabelle RON validate con report completo degli errori e hash canonico del
  dataset.
- `sim-replay` — salvataggio come `seed + Vec<Command>`, hash canonico dello stato, due golden
  replay con checkpoint ogni 30 tick.
- `xtask` — `run`, `record`, `regen-golden [--check]`, `bench`.

Quello che M0 **non** ha, di proposito: evoluzione delle case, migrazione, tasse, obiettivi di
scenario, eventi casuali, walker logistici reali, renderer, agenti. Sono M1–M3.

Due cose imparate implementando, che valgono più delle decisioni prese a tavolino:

1. Il passo 3 (copertura) è l'hot path, confermato da misura e non da ragionamento: un tick che
   accetta un comando costa ~75× un tick a vuoto, perché l'invalidazione è ingenua. Il costo è
   **per tick, non per comando**. Numeri in [plan/09-invarianti-chiusura.md](plan/09-invarianti-chiusura.md).
2. La copertura "prima capacità occupata, primo servito" rende lo stato di fame **assorbente**
   per una casa già assegnata. È una conseguenza reale della semplificazione A5, ha un test che
   la fissa, e va sciolta in M1 quando la capacità diventerà "abitanti serviti".

---

## Cosa NON fare

- Non spostare la simulazione dentro il `World` di Bevy, per nessun motivo.
- Non introdurre float, `HashMap` iterati o `thread_rng` nel core.
- Non scrivere numeri di bilanciamento nel codice.
- Non implementare `CivilizationRules` prima di M3: con una sola civiltà l'astrazione sarebbe
  inventata.
- Non aggiungere sistemi di gioco senza il golden replay corrispondente.
- Non ottimizzare prima del profiler, ma non rimandare i `DirtyFlags`.
- Non espandere lo scopo: un city builder ha una quantità enorme di sistemi interconnessi ed è
  facilissimo passare mesi su meccaniche mai giocate. Ogni sistema nuovo deve essere
  raggiungibile e osservabile in uno scenario esistente.
