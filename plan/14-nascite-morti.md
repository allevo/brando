# Fase 14 — Nascite e morti

**Goal:** una città servita cresce fino a saturare le capienze, una che perde i servizi si
spopola; e `seed_diversi_danno_hash_diversi`, `#[ignore]` dalla fase 08, si riattiva e passa.
**Dipende da:** 13.
**Dimensione:** L — è la fase più rischiosa di M1.
**Decisioni coinvolte:** [A12](decisioni-aperte.md), [A14](decisioni-aperte.md), A10, D4, D5.

## Perché adesso

Perché è il **primo uso reale dell'RNG** in tutto il progetto. Dalla fase 02 esistono tre
stream seedati per dominio, la loro posizione entra nell'hash canonico, e nessuno li ha mai
usati: `comandi.rs` contiene ancora `assert_eq!(w.rng(), prima.rng(), "nessun sistema estrae
dall'RNG in M0")`. Questa fase chiude quel cerchio, e con esso il test 7 della fase 08.

Ed è la fase in cui **la popolazione comincia a muoversi ogni tick**, che è la scelta di
[A12](decisioni-aperte.md) e il suo prezzo. Tutto ciò che segue in questo documento discende da
lì.

Va separata dalla migrazione (fase 15) per due motivi. Uno di leggibilità: nascite e morti sono
locali alla casa, la migrazione ha bisogno di un indice cittadino e di una regola di
distribuzione — modi di fallire diversi, test diversi. Uno concreto: con **due domini RNG
distinti**, scrivere la 15 non sfasa la sequenza di questa e non rigenera un golden che non
aveva motivo di cambiare. È letteralmente il caso d'uso per cui `RngDomain` è stato scritto in
fase 02, e finora non si era mai presentato.

## Il contratto: la copertura rincorre la popolazione

Va scritto per primo perché è il punto in cui M1 tocca l'hot path, ed è la parte che si può
sbagliare in silenzio.

La capacità di un provider si consuma sugli **abitanti presenti** ([A12](decisioni-aperte.md)):
i servizi rincorrono la popolazione, non la precedono. Finora `abitanti` cambiava solo per
comando — e ogni comando invalida già la copertura — o per sfratto (fase 13). Da qui cambia
**a ogni tick**, e servono tre cose.

**1. Il passo 6 invalida la copertura se ha mosso qualcuno.** Una riga, e senza di essa il
passo 3 del tick successivo lavora su una popolazione vecchia: le case cresciute consumerebbero
più di quanto il provider ha riservato, e *una casa coperta dal cibo mangia sempre* cadrebbe. È
la stessa forma dell'invalidazione dimenticata che `equivalenza_copertura` coglie dalla fase 06.

**2. `coperta ⇒ mangia` regge, ed è più stretto di prima.** Dentro un tick l'ordine è: passo 3
assegna con la popolazione corrente, passo 4 consuma con la stessa, passo 6 la cambia. Le due
letture che contano avvengono nello stesso istante logico. E poiché ora capacità e consumo si
contano nella stessa unità, il conto di `capacita_cibo_insostenibile` diventa un'uguaglianza
esatta invece di un maggiorante: `Σ abitanti serviti ≤ capacita`, e
`capacita × consumo ≤ produzione`. Nessuna produzione sprecata su case mezze vuote.

**3. `equivalenza_copertura` va riformulato, e va fatto con cura.** Oggi confronta la copertura
memorizzata con `calcola_da_zero` **a fine tick** — cioè dopo che il passo 6 ha già mosso la
popolazione. Con la demografia attiva le due divergono sempre, e non perché ci sia un bug: la
copertura è stata calcolata al passo 3 e sarà rifatta al passo 3 successivo.

La riformulazione sbagliata è "confronta solo se non è dirty": in una città che cresce è dirty
ogni tick, e il test più prezioso del progetto smetterebbe di girare senza diventare rosso.

La riformulazione giusta sono **due test che dicono due cose diverse**:

- `equivalenza_copertura` resta **identico nella forma**, ma gira su un dataset con i tassi
  demografici a zero. Continua a verificare ciò che ha sempre verificato — l'invalidazione
  dimenticata dopo un comando — e resta l'oracolo che protegge ogni futura ottimizzazione del
  passo 3.
- `la_demografia_invalida_la_copertura`, nuovo e diretto: se in un tick la popolazione è
  cambiata, a fine tick `dirty.coverage_da_rivedere()` è vero. Verifica esattamente il
  contratto nuovo, e niente altro.

Il terzo, implicito nei due sopra, è già coperto da `invarianti_coperta_significa_sfamata`, che
confronta due grandezze **entrambe** scritte fra il passo 3 e il passo 4 e quindi resta valido
senza modifiche.

**Che cosa costa.** Il ricalcolo della copertura passa da "quando il giocatore costruisce" a
"quasi ogni tick": `G` vale 3,05 ms contro i 248 µs del tick a vuoto, quindi il tick tipico va
verso ~3,3 ms — **~13×**. Dieci anni di gioco a 200×200 passano da ~0,9 s a ~12 s. È il prezzo
esplicito di A12, accettato, e **non si ottimizza in M1**: la misura e le contromisure sono un
compito aperto da chiudere prima di M2 ([A17](decisioni-aperte.md)). Ciò che questa fase deve
fare è **misurarlo bene**, non ridurlo — vedi la sezione Verifica.

Da qui in poi i `DirtyFlags` non evitano quasi più nulla in una città viva. Restano il
meccanismo giusto — sono loro che rendono possibile l'invalidazione mirata quando arriverà — ma
il commento in `DirtyFlags::coverage` che dice "oggi nessuno ne legge il contenuto" va
aggiornato con il perché adesso conta di più.

## Cosa si costruisce

### Il modulo, `sim-core/src/demografia.rs`

```rust
/// I quattro flussi di popolazione, aggregati: non si simulano individui (D5).
///
/// I `residuo` sono **stato** — decidono i tick successivi — e vanno
/// nell'hash. I contatori cumulativi sono diagnostica come il `FoodLedger` e
/// restano fuori, per la stessa ragione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Demografia {
    /// Frazioni di evento non ancora maturate, una per flusso, in millesimi di
    /// evento.
    ///
    /// Senza l'accumulo, un tasso che vale 0,4 nascite per tick verrebbe
    /// troncato a zero **ogni** tick e una citta' piccola non crescerebbe mai:
    /// il troncamento non e' un errore di arrotondamento, e' un baratro. E' lo
    /// stesso motivo per cui il tesoro avra' un residuo (fase 16).
    pub(crate) residuo: [i64; Flusso::COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DemografiaLedger {
    pub nati: u64,
    pub morti: u64,
    pub immigrati: u64,
    pub emigrati: u64,
    /// Sfrattati da un degrado di livello (fase 13).
    pub sfrattati: u64,
    /// Abitanti spariti con la casa demolita.
    ///
    /// Esiste per la stessa ragione di `FoodLedger::perso_per_demolizione`:
    /// senza, la conservazione della popolazione smette di essere
    /// un'uguaglianza e il test piu' importante della fase segnalerebbe un bug
    /// che non c'e'. E' la seconda volta che serve questo termine — la prima e'
    /// stata la giacenza di una fattoria demolita, in fase 07.
    pub persi_per_demolizione: u64,
}

pub enum Flusso { Nascite, Morti, Immigrazione, Emigrazione }
```

### Il passo 6, esteso

```rust
fn houses_and_migration(world: &mut World) {
    aggiorna_soddisfazione(world);          // 6.1, fase 12
    if e_valutazione_mensile(world) {
        degrada(world);                     // 6.2a, fase 13
        evolvi(world);                      // 6.2b, fase 13
    }
    morti(world);                           // 6.3
    nascite(world);                         // 6.5

    // La copertura si conta sugli abitanti presenti (A12): se qualcuno si e'
    // mosso, l'assegnazione di ieri non vale piu' e il passo 3 del prossimo
    // tick deve rifarla. Senza questa riga le case cresciute consumerebbero
    // piu' di quanto il provider ha riservato, e *coperta ⇒ mangia* cade.
    if world.demografia_ha_mosso_qualcuno() {
        segna_tutti_i_provider(world);
    }
}
```

6.4 e 6.6 (emigrazione e immigrazione) arrivano con la fase 15 e si infilano **fra** questi,
non in coda: l'ordine finale è documentato in `tick.rs` fin da ora, così chi legge sa dove
andranno. L'invalidazione resta l'ultima cosa del passo 6 anche allora.

L'invalidazione è condizionata e non incondizionata di proposito: in una città satura o
disabitata la popolazione non si muove, il ricalcolo non serve, e il tick torna a costare 248 µs.
È anche ciò che permette di far girare `equivalenza_copertura` nella sua forma originale su un
dataset a tassi zero.

**Perché le uscite prima delle entrate.** Due motivi. Uno tecnico: dopo il degrado può esserci
una casa piena al limite, e liberare prima garantisce che `abitanti <= capienza` valga in ogni
istante osservabile, non solo a fine passo. Uno di gioco: una casa che perde un abitante può
riguadagnarlo nello stesso tick, il che rende la popolazione reattiva invece che a scatti.

**Le nascite hanno tre condizioni**: `abitanti > 0` (serve gente per fare gente),
`abitanti < capienza(livello)`, e soddisfazione sopra una soglia. Il tasso base è scalato dalla
soddisfazione media della città — è la parte "legata al benessere complessivo" di
[A14](decisioni-aperte.md): una città che sta bene fa figli, una che stenta no, anche nelle case
che stanno bene.

**Le morti** hanno un tasso base e uno maggiorato per le case sotto la soglia di fame. È il
canale attraverso cui una città che perde il cibo si spopola davvero, invece di limitarsi a
smettere di crescere.

### La tabella, in `rules.ron`

Tassi **al mese e per mille abitanti**, perché è la forma che si legge e su cui si ragiona. La
conversione a tick divide per `tick_per_mese` senza perdita, perché il resto si accumula in
`residuo`.

```ron
demografia: (
    nascite_per_mille_al_mese: 12,        // a soddisfazione massima
    morti_per_mille_al_mese: 6,           // a soddisfazione massima
    morti_per_mille_al_mese_affamati: 60,
    soglia_nascite: 60,                   // soddisfazione minima per fare figli
    // +/- 20% sul tasso, dall'RNG. Puo' passare nel profilo di difficolta' se
    // si vorra' che la difficolta' alta sia anche piu' volatile.
    jitter_per_mille: 200,
),
```

**Controllo incrociato** (`Incoerenza::DemografiaNonSostenibile`): a soddisfazione massima le
nascite devono superare le morti. Altrimenti una città perfetta si spopola e **nessuno scenario
di crescita è vincibile** — un bilanciamento che il gioco non segnalerebbe mai, e che si
scoprirebbe solo con il bot euristico di M2.

### L'RNG: un dominio nuovo, e una trappola seria

```rust
pub enum RngDomain { Events, Migration, Production, Demografia }
```

Quattro righe (`TUTTI`, `salt`, `index` con lo **slot nuovo in coda**, `from_index`), e per
costruzione non sfasa i golden esistenti — `sequenze_riproducibili_con_valori_attesi` lo
verifica. `RngDomain::TUTTI` cresce, quindi `hash_world` cambia comunque: la rigenerazione di
questa fase è attesa.

Due punti di estrazione, e due soli:

1. **Il jitter sul tasso**, una estrazione per flusso per tick:
   `tasso_effettivo = tasso_base × (1000 + jitter) / 1000`, con `jitter ∈ [−J, +J]` simmetrico.
   Il risultato si accumula in `residuo` e matura in eventi interi.
2. **La scelta della casa** su cui l'evento cade, una estrazione per evento, dall'insieme
   eleggibile costruito in ordine di `HouseId`.

**Niente un dado per casa** (3.750 estrazioni per tick). Non è per il costo: D5 chiede una
simulazione grossolana, e la regola "il tasso è casuale, la distribuzione è deterministica" è
più facile da spiegare, da bilanciare e da leggere in un golden.

#### La trappola, da chiudere prima di scrivere le nascite

`rand::Rng::random_range` usa **campionamento a rigetto**: il numero di `next_u64()` consumati
dipende dai valori estratti, quindi dal seed. Le conseguenze sono due, e la seconda è grave:

- `rng.draws(d)` smette di essere funzione dello stato di gioco e diventa rumore, proprio nel
  campo che [la fase 02](02-rng-determinismo.md) ha messo nell'hash per rendere attribuibile
  una divergenza.
- `seed_diversi_danno_hash_diversi` passerebbe **anche se la demografia non facesse
  assolutamente nulla**. Un test verde che non verifica niente è peggio di un test rosso.

Serve un'estrazione a costo fisso nel core:

```rust
impl Stream {
    /// Un intero in `0..n`, con **una sola** estrazione, sempre.
    ///
    /// Moltiplicazione allargata invece del campionamento a rigetto di
    /// `random_range`: il rigetto consuma un numero di valori che dipende dal
    /// seed, e allora `draws()` smette di essere funzione dello stato di gioco
    /// — cioe' smette di servire all'hash canonico, e fa passare
    /// `seed_diversi_danno_hash_diversi` senza che il gioco sia cambiato.
    ///
    /// Il bias e' 2^-64 relativo: irrilevante, e comunque preferibile a un
    /// costo variabile in un core che dev'essere deterministico nel numero di
    /// estrazioni oltre che nei valori.
    pub fn sotto(&mut self, n: u64) -> u64 {
        if n == 0 { return 0; }
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }
}
```

Va scritto **prima** delle nascite, non dopo, con il test che lo fissa (test 6).

#### Come si concilia con D4

Non c'è nulla da conciliare: il seed sta nell'header, l'RNG nello stato, e stesso seed ⇒ stesso
risultato bit a bit. Ciò che il giocatore percepisce come casualità è che non conosce il seed.
La formulazione da tenere: ***la randomicità è nella scelta del seed, non nell'esecuzione.***

### Lo `StepReport` guadagna un sommario

Nessun evento per casa per tick — sarebbe il polling che il confine col renderer vieta. Al suo
posto un aggregato del tick, che serve anche a `xtask run` e all'evaluator di M2:

```rust
pub struct StepReport {
    pub rejected: Vec<(usize, CommandError)>,
    pub events: Vec<Event>,
    /// Aggregati del tick. **Non** eventi delta: e' la fotografia che il
    /// renderer usa per le barre in alto e l'evaluator per le metriche. Come
    /// eventi sarebbero uno per casa per tick.
    pub sommario: Sommario,
}

pub struct Sommario {
    pub popolazione: u32,
    pub posti: u32,
    pub nati: u16,
    pub morti: u16,
    pub soddisfazione_media: u8,
    // fase 15: immigrati, emigrati, attrattivita
    // fase 16: incasso
}
```

Restano eventi solo i cambi di stato rari: `HouseAbandoned { house }` quando gli abitanti
arrivano a zero, `HouseRepopulated { house }` quando risalgono.

### L'hash

`residuo` (4 × `i64`) nel blocco dello stato. Il ledger resta fuori, come il `FoodLedger` e per
la stessa ragione: è diagnostica, non decide niente.

## Fuori scope

Immigrazione ed emigrazione (fase 15). Malattie ed epidemie: sono eventi casuali, passo 8, e
sono fuori da M1. Età, famiglie, mestieri — D5 dice che l'unità è la casa.

## Test

1. **Crescita**: città servita, la popolazione sale e **si ferma** a saturazione delle
   capienze. Il plateau conta quanto la salita: è la prova che il vincolo `abitanti <= capienza`
   morde.
2. **Spopolamento**: demolita la fattoria, la popolazione scende. Con il tasso da fame la
   discesa è calcolabile dal `DataSet`.
3. **Conservazione della popolazione** (property test, *il goal contabile della fase*):
   `Σ abitanti == nati + immigrati − morti − emigrati − sfrattati − persi_per_demolizione`,
   uguaglianza **esatta** dopo qualunque sequenza di comandi. È l'analogo della conservazione
   del cibo, e come quella trova il flusso che qualcuno ha dimenticato di contare.
4. **`invarianti_popolazione` cambia esito di proposito**: `popolazione == n_case × 4` è falso
   per costruzione da qui. Diventa due invarianti più forti — `popolazione == Σ abitanti` e
   `abitanti <= capienza(level)` per ogni casa. Il secondo è quello che regge
   *coperta ⇒ mangia*, quindi non è cosmesi.
5. **`seed_diversi_danno_hash_diversi` si riattiva**, e da qui non può più essere `#[ignore]`.
6. **Il numero di estrazioni non dipende dal seed** — il test che rende `draws` un'informazione
   e non un caso: stessa partita con dieci seed diversi, `draws(d)` identico per ogni dominio a
   ogni tick. Senza questo, il test 5 è verde e vuoto.
7. **Il jitter non sposta la media**: su diecimila tick il numero di nascite sta entro l'1% del
   tasso base. Coglie il jitter asimmetrico, che sposterebbe tutto il bilanciamento in modo
   invisibile.
8. **Cento seed danno traiettorie diverse ma tutte plausibili**: la popolazione a cinque anni
   sta in una banda stretta e non è mai due volte la stessa. È la definizione operativa di
   "un minimo di randomicità ma non due partite identiche" — senza la prima metà il
   bilanciamento è una lotteria, senza la seconda l'RNG non serve a niente.
9. **Aggiungere un dominio non sfasa gli altri**: `sequenze_riproducibili_con_valori_attesi`
   resta verde con `Demografia` in tabella. È la prova, quattro fasi dopo, che la fase 02 aveva
   ragione.
10. **Demolire una casa abitata** conta gli abitanti in `persi_per_demolizione`. Stesso test di
    `demolire_una_fattoria_registra_la_giacenza_persa`, e per lo stesso motivo.
11. **`comandi.rs:25` cambia esito**: `assert_eq!(w.rng(), prima.rng())` diventa falso appena
    la demografia gira. Da riscrivere come "un tick a vuoto consuma un numero **noto** di
    estrazioni", che è più forte.
12. **`la_demografia_invalida_la_copertura`** (nuovo, il contratto di A12): se in un tick la
    popolazione è cambiata, a fine tick `dirty.coverage_da_rivedere()` è vero. Ed è vero anche
    l'inverso — popolazione ferma, copertura pulita — perché è quello a dire che
    l'invalidazione è condizionata e non un `segna_tutti_i_provider` incondizionato.
13. **`equivalenza_copertura` gira ancora, su un dataset a tassi zero**, e resta identico nella
    forma. Va aggiunto il commento che spiega *perché* la fixture ha i tassi a zero: senza,
    qualcuno la "aggiusterà" mettendoci i valori veri e il test smetterà di girare senza
    diventare rosso.
14. **La copertura si adegua alla crescita**: casa servita che cresce oltre ciò che il provider
    può servire ⇒ al tick dopo esce dalla copertura, e *coperta ⇒ mangia* resta verde. È il
    ciclo di gioco di A12 osservato al minimo: la città supera i suoi servizi.

## Verifica

```sh
cargo test -p sim-core demografia
PROPTEST_CASES=2000 cargo test -p sim-core --release conservazione_popolazione
cargo test -p sim-replay                       # seed_diversi_danno_hash_diversi non più ignorato
cargo xtask run --ticks 1800 --dump-every 90   # cinque anni: la curva deve essere plausibile
cargo xtask regen-golden
cargo xtask bench
```

I 1800 tick sono la prova a occhio che chiude la fase, e vanno guardati con lo stesso sospetto
del dump della fase 07: se la curva esplode o si spegne, è bilanciamento (`sim-data`), non
codice — ma va sistemato adesso, perché `regen-golden` la congela.

### Misurare il prezzo di A12, che è il vero compito di questa fase

Il costo va **attribuito**, non solo constatato, altrimenti prima di M2 nessuno saprà da dove
cominciare a toglierlo. Servono tre esecuzioni sulla stessa macchina:

```sh
cargo xtask bench --difficolta normale                    # tassi veri
cargo xtask bench --difficolta normale --demografia-zero  # tassi a zero
```

- **`A` a tassi zero** deve restare dov'era a fine fase 13. Se si è mossa, il costo non è il
  ricalcolo della copertura ma il passo 6 in sé, ed è un'altra cosa da ottimizzare.
- **`A` a tassi veri** è il numero che A12 costa. L'attesa è ~3,3 ms contro 248 µs; se fosse
  molto peggio, c'è dell'altro e va trovato adesso.
- **La misura `I`** (solo il passo 6) separa il lavoro demografico dal ricalcolo che innesca.
  Senza, il costo della demografia si può solo dedurre per differenza, e la differenza è
  dominata dal rumore.

Va aggiunto anche il **conteggio dei ricalcoli**: `coverage().ricalcoli()` prima e dopo ogni
misura, con il delta stampato. Un `A` che paga millisecondi con zero ricalcoli sarebbe una
diagnosi completamente diversa.

I tre numeri vanno registrati in [18](18-invarianti-chiusura-m1.md) e sono l'input di
[A17](decisioni-aperte.md), il compito aperto sulle prestazioni da chiudere prima di M2.
**In questa fase non si ottimizza niente**: `CLAUDE.md` dice di non ottimizzare prima del
profiler, e A11 è la storia di cosa succede quando si indovina invece di misurare.

**Fatto quando:** passano il test 3 (conservazione esatta), il test 6 (estrazioni indipendenti
dal seed) e il test 12 (il contratto di invalidazione), e i tre numeri di cui sopra sono
registrati. Il 6 va **prima** del 5 in ordine di scrittura: senza, il 5 non significa niente.
