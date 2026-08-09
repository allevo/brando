# Fase 11 — Difficoltà di gioco

**Goal:** stesso seed e stessi comandi, due difficoltà diverse ⇒ hash diversi; la difficoltà
viaggia nell'header del replay e un file registrato a una difficoltà non si rigioca a un'altra.
**Dipende da:** 09 (M0 chiuso).
**Dimensione:** S — è la prima fase di M1.
**Decisioni coinvolte:** [A13](decisioni-aperte.md), A2, A3, D6.

## Perché adesso

Perché tocca le tre cose che rigenerano i golden — `World::new`, l'`Header` del replay,
`hash_world` — e non ne tocca nessun'altra. Farla dopo significa rigenerarli due volte, e ogni
rigenerazione in più è un'occasione in meno di leggere un diff e capirlo.

È lo stesso argomento dei `DirtyFlags` in fase 04: la struttura si mette quando costa poco, non
quando serve. Qui la tabella nasce con **un solo knob** — gli abitanti di una casa appena
costruita — e le fasi 13, 14 e 16 ci appenderanno i loro senza toccare più né l'header né la
firma di `World::new`.

C'è anche una ragione di gioco per cui il knob è quello. Con la demografia (fase 14) la
popolazione arriva migrando; se una casa nasce già piena, il giocatore non vede mai la parte
difficile del gioco. Ma partire sempre da zero rende i primi anni lentissimi. La difficoltà è
la manopola che sceglie fra le due, e va decisa a inizio partita perché cambia la simulazione.

## Cosa si costruisce

### Un accessore che serve subito e servirà di più

```rust
// sim-core/src/data.rs
impl Rules {
    /// Abitanti massimi di una casa al livello dato (livello 1 = indice 0).
    ///
    /// `None` fuori range, come `ServiceDef::raggio` e `ServiceDef::capacita`:
    /// un livello che non esiste in tabella e' un errore di dato, non un panic.
    ///
    /// E' una funzione e non un accesso diretto al `Vec` perche' la fase 13
    /// ristruttura `abitanti_per_livello_casa` in una tabella per livello: li'
    /// cambia il **corpo**, non i chiamanti.
    pub fn capienza(&self, livello: u8) -> Option<u16> {
        self.abitanti_per_livello_casa
            .get(usize::from(livello).checked_sub(1)?)
            .copied()
    }
}
```

Serve già qui, alla validazione incrociata più sotto. Il campo resta
`abitanti_per_livello_casa`: rinominarlo sposterebbe l'hash del dataset, e la fase 13 lo
ristruttura comunque.

### La tabella, `sim-data/data/difficolta.ron`

Quarto file letto da `load_from_dir`.

```ron
// Profili di difficolta'. Nasce con un campo solo: le fasi 13, 14 e 16
// aggiungono qui i loro knob senza toccare nient'altro.
(
    profili: [
        // Abitanti di una casa appena costruita. A "difficile" e' zero: la
        // casa si popola solo migrando, e la citta' va meritata.
        (id: "facile",    abitanti_iniziali_casa: 4),
        (id: "normale",   abitanti_iniziali_casa: 2),
        (id: "difficile", abitanti_iniziali_casa: 0),
    ],
)
```

`facile` ha `abitanti_iniziali_casa == capienza(1)`, cioè il comportamento di oggi. È quello
che rende il diff degli `.hashes` di questa fase attribuibile **al solo header**: registrando i
golden esistenti a `facile`, l'unica cosa cambiata nello stato è il byte della difficoltà.

### Le definizioni, in `sim-core/src/data.rs`

```rust
/// Indice nella tabella dei profili.
///
/// Non un enum: la difficolta' e' dato (D6), e una civilta' o uno scenario
/// potranno dichiararne di propri senza toccare il codice.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct DifficoltaId(u8);

pub struct DifficoltaDef {
    pub id: String,
    /// Abitanti di una casa appena costruita. Zero e' legittimo: la casa si
    /// popola migrando (fase 15).
    pub abitanti_iniziali_casa: u16,
}

impl DataSet {
    pub fn difficolta(&self, d: DifficoltaId) -> Option<&DifficoltaDef>;
    pub fn difficolta_by_id(&self, s: &str) -> Option<DifficoltaId>;
}
```

Niente `Default` per `DifficoltaId`. Sembra una scomodità e non lo è: i chiamanti di
`World::new` sono quattro, e un default è esattamente il meccanismo con cui uno dei quattro
resterebbe indietro senza che niente lo segnali.

### Lo stato, in `sim-core/src/world.rs`

```rust
pub struct World {
    // ...
    /// Scelta a inizio partita, mai piu' modificabile: cambia la simulazione,
    /// quindi entra nell'hash e viaggia nell'header del replay.
    pub(crate) difficolta: DifficoltaId,
}

impl World {
    pub fn new(grid: Grid, data: Arc<DataSet>, seed: u64, difficolta: DifficoltaId) -> Self;
    pub const fn difficolta(&self) -> DifficoltaId;
}
```

I quattro chiamanti da aggiornare: `sim-replay/src/replay.rs::mondo_iniziale`,
`xtask/src/scenario.rs::Scenario::mondo`, `xtask/src/bench.rs`,
`sim-core/tests/comune/mod.rs::mondo_con`.

`place_building` legge `abitanti_iniziali_casa` dal profilo invece che da
`rules.abitanti_per_livello_casa.first()`.

### Il replay, in `sim-replay/src/recording.rs`

```rust
pub const FORMAT_VERSION: u16 = 2;

pub struct Header {
    pub format_version: u16,
    pub seed: u64,
    /// L'**id testuale** del profilo, non l'indice.
    ///
    /// Un golden con `difficolta: 1` non si legge, e riordinare la tabella
    /// cambierebbe in silenzio il significato di ogni salvataggio gia'
    /// scritto. Il costo e' una risoluzione all'apertura; il ricavo e' che il
    /// file resta quello che i golden devono essere, cioe' leggibile.
    pub difficolta: String,
    pub grid: GridSpec,
    pub dataset_hash: String,
}
```

```rust
/// Il profilo dell'header non esiste in tabella.
///
/// Errore distinto da `DatasetMismatch` perche' la causa e' diversa: li' il
/// bilanciamento e' cambiato, qui un profilo e' stato rimosso o rinominato.
#[error("profilo di difficolta' sconosciuto: {trovato:?} (noti: {noti})")]
DifficoltaSconosciuta { trovato: String, noti: String },
```

### I due hash

`sim-replay/src/hash.rs`, subito dopo `w.data().hash`:

```rust
h.update(&[w.difficolta().get()]);
```

`sim-core/src/data_hash.rs`, blocco dei profili in coda, con la lunghezza prima del contenuto
come per tutti gli altri.

### Il canarino di compilazione — la cosa più importante della fase

M1 aggiunge sette campi allo stato in sei fasi, e [A3](decisioni-aperte.md) dice che ognuno va
messo a mano in `hash_world`. La rete di sicurezza è `l_hash_copre_tutto_lo_stato`, ma quel test
**non fallisce da solo** quando arriva un campo nuovo: fallisce solo se qualcuno scrive anche la
perturbazione corrispondente. Cioè la rete c'è solo se qualcuno si ricorda di tenderla.

```rust
// sim-core/src/world.rs, dietro `test-util`
/// Canarino di **compilazione** per l'hash canonico (A3).
///
/// Non fa niente a runtime. Esiste perche' il `let World { .. }` esaustivo
/// smette di compilare nel momento in cui si aggiunge un campo allo stato: il
/// promemoria arriva mentre scrivi il campo, non quando un test fallisce — e
/// arriva anche se nessuno ha aggiunto la perturbazione corrispondente in
/// `l_hash_copre_tutto_lo_stato`, che oggi e' l'unico modo in cui quel test si
/// accorge di qualcosa.
///
/// Se stai leggendo questo perche' non compila: aggiungi il campo qui, poi
/// decidi se va in `hash_world` (stato) o no (derivato/diagnostico), e in ogni
/// caso scrivi quale dei due nel doc comment del campo.
#[cfg(feature = "test-util")]
pub fn canarino_dei_campi(&self) {
    let World {
        tick: _, grid: _, buildings: _, houses: _, walkers: _, economy: _,
        rng: _, dirty: _, roads: _, coverage: _, food: _,
        edifici_per_origine: _, case_per_origine: _, data: _, difficolta: _,
    } = self;
}
```

Stesso trucco, gratis, dove i campi sono già `pub`: `hash_world` può destrutturare `Building`,
`House` ed `Economy` nei suoi cicli invece di accedere ai campi uno a uno, e `canonical_hash`
può destrutturare `Rules`, `BuildingDef` e `TerrainDef`. Una riga per struct, e il rischio noto
di A3 smette di dipendere dalla memoria di chi scrive.

### Validazione

Un controllo di forma — `profili` non vuoto, id unici e non vuoti — e uno **incrociato**, che
tocca due tabelle e quindi va in `DataSet` come `capacita_cibo_insostenibile`:

```
abitanti_iniziali_casa <= rules.capienza(1)
```

Una casa che nasce oltre la propria capienza è uno stato che il resto del gioco non sa
rappresentare: la fase 13 assume `abitanti <= capienza(level)` ovunque.

### `xtask`

`run --difficolta <id>`, `Scenario.difficolta`, e la difficoltà nell'intestazione del dump
accanto al seed e all'hash del dataset.

Per `regen-golden`: i golden esistenti si registrano a `facile`. La matrice completa
(ogni scenario × ogni profilo) moltiplica i file per tre e non aggiunge copertura — il
determinismo è lo stesso. Un solo golden a difficoltà diversa arriva quando serve, cioè quando
un profilo avrà knob che cambiano davvero la simulazione (fase 14).

**Il bench deve scegliere il profilo esplicitamente**, non prendere il primo o un default: se
ribilanciare il default spostasse i numeri registrati in [09](09-invarianti-chiusura.md), la
tripwire smetterebbe di essere confrontabile. Il profilo usato va stampato nell'intestazione,
accanto all'hash del dataset e per lo stesso motivo — un numero che si sposta dev'essere
attribuibile.

## Fuori scope

Knob diversi da `abitanti_iniziali_casa`: arrivano con le fasi che li usano. Difficoltà
modificabile in corsa (sarebbe un `Command`, e cambiare le regole a metà partita è un problema
di bilanciamento che nessuno scenario chiede). Difficoltà per scenario: è la fase 17, e sarà lo
scenario a proporre un profilo, non a definirne uno nuovo.

## Test

1. **Il goal**: stesso seed, stessi comandi, `facile` e `difficile` ⇒ hash diversi al primo
   tick in cui si costruisce una casa, e **uguali prima**. La seconda metà conta quanto la
   prima: dice che la difficoltà agisce dove deve e non altrove.
2. **`facile` riproduce M0**: con `abitanti_iniziali_casa == capienza(1)`, la popolazione dopo
   una partita è la stessa di prima della fase. Fissa che il knob è l'unico effetto.
3. **Round-trip dell'header**: un `Recording` salvato e riletto conserva l'id testuale; un
   header con un profilo inesistente dà `DifficoltaSconosciuta`, non un panic né un default
   silenzioso.
4. **Versione del formato**: un header con `format_version: 1` è rifiutato con
   `FormatoNonSupportato` (il test esiste già, va aggiornato il numero).
5. **Validazione incrociata**: fixture rotta con `abitanti_iniziali_casa` oltre la capienza ⇒
   un errore col path giusto. Come le altre sette in `sim-data/tests/fixtures/rotte/`.
6. **L'hash copre la difficoltà**: una perturbazione in `l_hash_copre_tutto_lo_stato`.
7. **Il canarino compila**, ed è tutto ciò che deve fare. Vale la prova manuale che chiude la
   fase.

## Verifica

```sh
cargo test --workspace
cargo xtask regen-golden          # rigenerazione **voluta**: header v2 + difficoltà
cargo xtask regen-golden --check  # e poi idempotente
cargo xtask run --difficolta difficile --ticks 360
```

Il diff degli `.hashes` va **letto**, non solo committato: deve divergere dal primo checkpoint,
perché il byte della difficoltà entra nell'hash dal tick 0. Se divergesse più tardi, la
difficoltà non è nell'hash dove si crede.

Poi la prova manuale che chiude la fase: **aggiungere un campo qualsiasi a `World` e verificare
che il progetto non compili** finché non lo si aggiunge al canarino. Rimuoverlo e committare. È
lo stesso rito della fase 02 con la variante in testa a `RngDomain`, e per lo stesso motivo: la
rete di sicurezza va vista scattare una volta, o non si sa se c'è.

**Fatto quando:** il test 1 passa in entrambe le sue metà, e il canarino è stato visto rompere
la compilazione.
