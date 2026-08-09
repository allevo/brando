# Fase 17 — `sim-scenario` e obiettivi

**Goal:** lo scenario "500 abitanti entro 5 anni" si dichiara completato al tick giusto, e non
prima.
**Dipende da:** 16.
**Dimensione:** L.
**Decisioni coinvolte:** [A16](decisioni-aperte.md), A2, A6, D7.

## Perché adesso

Perché è l'ultima fase che aggiunge una meccanica, e perché un obiettivo ha senso solo quando
c'è un gioco sotto: prima della fase 16 la città non poteva finanziare la propria crescita, e
"500 abitanti entro 5 anni" sarebbe stato vinto o perso dal bilanciamento invece che dal
giocatore.

È anche il primo crate nuovo dopo M0. Nasce adesso e non prima perché
[plan/README](README.md) è esplicito: *crate vuoti scaffoldati in anticipo sono superficie che
invita a riempirla*.

## Cosa si costruisce

### Dove vive cosa — il grafo delle dipendenze decide

`CLAUDE.md` mette la verifica degli obiettivi come **passo 9 del tick**, dentro `step`. Ma le
dipendenze puntano verso `sim-core`, quindi `sim-scenario` dipende dal core e il core non può
chiamarlo. La divisione è obbligata, ed è la stessa di [A2](decisioni-aperte.md):

- In **`sim-core`**: l'enum `Objective`, la sua valutazione, e lo stato di avanzamento. Sono
  struct pure senza I/O, e appartengono al vocabolario del core come `ServiceKind`.
- In **`sim-scenario`**: la definizione degli scenari (mappa iniziale, comandi, difficoltà,
  obiettivi obbligatori e opzionali), il caricamento da file, la composizione.

È la seconda volta che il grafo detta il confine, e vale la pena scriverlo: la prima è stata
il `DataSet`, e chi legge deve poter riconoscere il pattern invece di riscoprirlo.

### Gli obiettivi vivono nel `World`

```rust
// sim-core
pub enum Objective {
    /// Popolazione minima entro una scadenza. La scadenza e' in **mesi**, mai
    /// in tick (A6): e' l'unita' in cui il giocatore ragiona e in cui D7 vuole
    /// gli obiettivi.
    Popolazione { almeno: u32, entro_mesi: Option<u32> },
    Tesoro { almeno: Coins, entro_mesi: Option<u32> },
    /// Tutte le case al livello dato o superiore.
    LivelloCase { livello: u8, quante: u32, entro_mesi: Option<u32> },
}

pub struct Obiettivi {
    pub obbligatori: Vec<Objective>,
    pub opzionali: Vec<Objective>,
}

/// Avanzamento, uno per obiettivo, nell'ordine in cui sono dichiarati.
pub struct StatoObiettivi {
    /// Tick in cui l'obiettivo e' stato completato, o `None`.
    ///
    /// Una volta completato **resta** completato: un obiettivo che si
    /// scompleta perche' la popolazione e' scesa renderebbe la vittoria
    /// reversibile, e nessuno scenario di campagna funziona cosi'.
    completati: Vec<Option<u32>>,
    esito: Esito,
}

pub enum Esito { InCorso, Vinto, Perso }
```

**Perché nel `World` e non passati a `step`.** Tenerli nello stato mantiene la firma
`step(&mut World, &[Command])` che D4 dichiara, e — soprattutto — mette il **tick di
completamento dentro l'hash canonico**. È l'unica cosa che rende un golden capace di verificare
il goal di questa fase: "si dichiara completato al tick giusto" diventa un hash che diverge se
il tick cambia, invece di un `assert` in un test che qualcuno deve ricordarsi di scrivere.

`World::new` guadagna gli obiettivi, o un `World::con_obiettivi`. Sono dato validato come il
`DataSet`, e come quello arrivano dall'esterno già controllati.

### Il passo 9

```rust
/// Passo 9 — obiettivi di scenario.
///
/// Dopo la finanza e prima dell'emissione degli eventi: valuta lo stato di
/// fine tick, che e' quello che il giocatore vede.
fn check_objectives(world: &mut World, r: &mut StepReport) { .. }
```

Un obiettivo si completa quando la condizione è vera e non è già completato; la scadenza si
valuta in `tick / tick_per_mese`. Lo scenario è **vinto** quando tutti gli obbligatori sono
completati, **perso** quando uno di essi ha una scadenza superata.

```rust
Event::ObjectiveCompleted { indice: u16, obbligatorio: bool },
Event::ScenarioEnded { esito: Esito },
```

Entrambi si emettono una volta sola: sono cambi di stato, non polling.

### `sim-scenario`

Nuovo membro del workspace, `#![forbid(unsafe_code)]`, `lints.workspace = true`, dipendenze
`sim-core` + `serde`/`ron`/`thiserror`. Contiene la descrizione di uno scenario — griglia
iniziale, terreni, difficoltà proposta, comandi di partenza, obiettivi — con caricamento da RON
e validazione con report completo, esattamente come `sim-data`.

**Un obiettivo va validato contro il `DataSet`**: "tutte le case al livello 4" con `livelli: 3`
in tabella è uno scenario invincibile, e va rifiutato al caricamento invece che scoperto dopo
cinque anni di gioco simulato. Stesso spirito di `DataSet::incoerenze()`.

### Il nome `Scenario` è già preso

`xtask/src/scenario.rs` ha una struct `Scenario` che è un'altra cosa: una situazione costruita a
mano per il runner e per dare contenuto ai golden, senza obiettivi — il suo doc comment lo dice
già. Va rinominata (`Situazione`, o `Prova`) **in questa fase**, prima che diventino tre e prima
che qualcuno importi la sbagliata.

### Il golden dello scenario

Uno scenario nuovo che **arriva** all'obiettivo, con il suo `.ron` e i suoi `.hashes`. È il
canarino sul bilanciamento che `CLAUDE.md` chiede al punto 4 del Testing, nella versione che M1
può avere: il bot euristico è M2, ma un golden che raggiunge l'obiettivo al tick N fallisce
appena il bilanciamento si muove, e il diff dice **di quanto**.

**La lista degli scenari è duplicata a mano** fra `xtask/src/scenario.rs::NOMI` e
`sim-replay/tests/golden.rs::SCENARI`, e con uno scenario nuovo la dimenticanza è quasi certa —
e si manifesta come "il golden nuovo non viene mai verificato", cioè silenzio invece che rosso.
Rimedio a costo zero, da fare qui: `golden.rs` elenca i `*.ron` di `dir_golden()` invece di
avere una costante, e **fallisce se un `.ron` non ha il suo `.hashes`**. Toglie la duplicazione
e aggiunge un controllo che oggi non c'è.

## Fuori scope

Il bot euristico e l'evaluator: sono M2, e l'evaluator produrrà un *vettore* di metriche di cui
`StatoObiettivi` è solo il primo elemento. Campagne, scenari concatenati, sblocchi. Obiettivi
che dipendono da eventi (incendi spenti, invasioni respinte): gli eventi casuali sono fuori da
M1.

## Test

1. **Il goal**: lo scenario si dichiara vinto al tick atteso, e `Esito::InCorso` al tick
   precedente. Le due metà valgono uguale — "non prima" è metà del goal.
2. **Scadenza mancata**: stesso scenario con `entro_mesi` stretto ⇒ `Esito::Perso` al tick
   della scadenza, non dopo.
3. **Un obiettivo completato resta completato** anche se la popolazione poi scende. Fissa la
   scelta, che è arbitraria e va documentata.
4. **Opzionali**: non completarli non impedisce la vittoria; completarli si vede in
   `StatoObiettivi`. È il dato su cui l'evaluator di M2 costruirà lo score.
5. **Obiettivo invincibile rifiutato** al caricamento: livello oltre `livelli` in tabella ⇒
   errore di validazione con il path giusto.
6. **Golden dello scenario**: il tick di completamento è nell'hash, quindi il golden lo
   protegge. Cambiando un numero di bilanciamento, il golden diverge — ed è il segnale che il
   canarino funziona.
7. **`ObjectiveCompleted` emesso una volta sola**, non a ogni tick in cui la condizione resta
   vera. Stessa regola di `ServiceCoverageChanged` (fase 07).
8. **`golden.rs` scopre i file da solo**: un `.ron` senza `.hashes` fa fallire il test invece di
   passare inosservato.

## Verifica

```sh
cargo test -p sim-scenario
cargo test -p sim-replay
cargo xtask run --scenario crescita --ticks 1800 --dump-every 90
cargo xtask regen-golden --check
```

Il dump dello scenario nuovo è la prova che chiude la fase: la riga in cui l'obiettivo si
completa dev'essere **leggibile** e cadere dove ha senso — non al tick 30 e non al 1799. Se
arriva troppo presto lo scenario è banale, se non arriva è invincibile; in entrambi i casi è
bilanciamento (`sim-data`), non codice, e va sistemato prima che il golden lo congeli.

**Fatto quando:** il test 1 passa in entrambe le metà e il golden dello scenario è committato.
