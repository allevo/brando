# Fase 13 — Livelli delle case

**Goal:** una casa servita da acqua e cibo raggiunge il livello 2 in un numero di tick
calcolabile dal `DataSet`; togliendo l'acqua torna al livello 1; e sul confine **non oscilla**.
**Dipende da:** 12.
**Dimensione:** L.
**Decisioni coinvolte:** [A12](decisioni-aperte.md), A9, A10, A5, D5, D6.

## Perché adesso

Perché la soddisfazione della fase 12 esiste per essere letta da qualcuno, e questo è il primo
sistema che la legge. Ed è il momento in cui `capienza` smette di essere un numero solo:
`Rules::capienza` (fase 11) cambia corpo e diventa la tabella per livello.

Una cosa che **non** succede qui, per via di [A12](decisioni-aperte.md): l'evoluzione non tocca
la copertura. La capacità di un provider si consuma sugli **abitanti presenti**, e salire di
livello non porta gente — porta solo il permesso di ospitarne di più. È la demografia (fase 14)
a riempire lo spazio nuovo, ed è lì che la copertura comincia a rincorrere. Questa fase muove
`abitanti` in un modo solo: lo sfratto da degrado.

## Cosa si costruisce

### Le tabelle, in `rules.ron`

`abitanti_per_livello_casa: Vec<u16>` diventa `livelli_casa: Vec<LivelloCasaDef>`.

```rust
// sim-core/src/data.rs
pub struct LivelloCasaDef {
    /// Abitanti massimi. E' il tetto della casa, **non** cio' su cui la
    /// copertura conta la capacita' dei provider: quella si conta sugli
    /// abitanti che ci vivono davvero (A12). Una casa di capienza 8 con due
    /// abitanti pesa due, non otto.
    pub capienza: u16,
    /// Servizi necessari per **salire a** questo livello e per **restarci**.
    ///
    /// E' qui che `servizi_richiesti` smette di essere dato dichiarativo letto
    /// solo da `e_una_casa()` — il buco annotato in A9.
    pub servizi_richiesti: Vec<ServiceKind>,
    /// Soddisfazione minima su **ognuno** dei servizi richiesti, per salire qui.
    pub soglia_evoluzione: u8,
    /// Sotto questa, su un servizio richiesto qualunque, si degrada.
    pub soglia_degrado: u8,
    /// Base imponibile per abitante (fase 16). Zero fino ad allora.
    pub imponibile_per_abitante: Milli,
}
```

**Perché in `Rules` e non in `BuildingDef`.** `House` non porta un `BuildingKindId`, e in M1
c'è un solo tipo di casa. Aggiungerglielo per poter indicizzare una tabella per tipo sarebbe
l'astrazione inventata che D6 vieta prima della seconda civiltà. Quando servirà,
`Rules::capienza` (fase 11) è di nuovo l'unico punto da cambiare — è per questo che è una
funzione.

`Rules::capienza` cambia corpo, non firma:

```rust
pub fn capienza(&self, livello: u8) -> Option<u16> {
    Some(self.livelli_casa.get(usize::from(livello).checked_sub(1)?)?.capienza)
}
```

La casa in `buildings.ron` passa a `livelli: 3` e tiene `servizi_richiesti` come **unione** dei
requisiti per livello — serve ancora a `e_una_casa()`, ed è cross-validato contro i livelli
(vedi `Incoerenza::RequisitiIncoerenti`).

### Il passo 6, esteso

```rust
fn houses_and_migration(world: &mut World) {
    aggiorna_soddisfazione(world);          // 6.1, fase 12
    if e_valutazione_mensile(world) {
        degrada(world);                     // 6.2a
        evolvi(world);                      // 6.2b
    }
}
```

**Perché la valutazione è mensile e non a ogni tick.** Non è per il costo — con
[A12](decisioni-aperte.md) l'evoluzione non tocca la copertura, quindi costa poco. È perché
l'isteresi da sola protegge dall'oscillazione solo se le soglie sono lontane, mentre una
cadenza rada la rende **strutturale**: fra due decisioni passano trenta tick, e una casa non
può salire e scendere più di dodici volte l'anno per costruzione.

La soddisfazione continua ad accumularsi **ogni tick**: è solo la *decisione* a essere mensile.
Non introduce nessun concetto nuovo — il mese esiste dalla fase 03 ([A6](decisioni-aperte.md))
— è zeusiano, e rende i golden più leggibili: un livello che cambia solo ai multipli di 30 si
segue a occhio.

**Perché il degrado prima dell'evoluzione.** Una casa che sta scendendo non deve poter salire
nello stesso tick per via di un requisito residuo. Valutare prima la condizione peggiore rende
la transizione monotona, e chiude il flip-flop a un tick prima che esista.

**Un solo salto per valutazione**, e l'evoluzione si valuta sui requisiti del livello **di
destinazione**: così non si sale mai dentro un requisito non soddisfatto.

**Il degrado sfratta**: `abitanti = min(abitanti, capienza(nuovo_livello))`, e gli sfrattati
contano come emigrazione nel ledger della fase 14. Non è un dettaglio contabile — è un flusso
di popolazione, e se non lo si conta la conservazione della fase 14 non è un'uguaglianza. È lo
stesso motivo per cui esiste `perso_per_demolizione` nel `FoodLedger`.

### Che cosa **non** si costruisce, e perché conta

**Nessun gate sull'evoluzione.** Una prima stesura di questa fase aveva un
`posti_per_salire(world, h, livello)` che impediva a una casa di salire se i suoi provider non
avevano capacità per la capienza in più. Con [A12](decisioni-aperte.md) non serve, e la ragione
vale la pena scriverla: **salire di livello non consuma capacità**, perché la capacità si conta
sugli abitanti presenti e l'evoluzione non porta gente. Porta il permesso di ospitarne di più,
che è un'altra cosa.

La pressione arriva dopo, quando la demografia riempie lo spazio: allora la domanda cresce, il
provider si satura, e qualche casa esce dalla copertura. **Quella non è una patologia da
spegnere, è il ciclo di gioco** — la città supera i propri servizi, la qualità cala, il
giocatore costruisce. Con il gate non succederebbe mai, e la città si fermerebbe da sola senza
che il giocatore debba accorgersi di niente.

Il rovescio è che va verificato che quel ciclo **smorzi** invece di divergere, ed è la fase 14
a doverlo dimostrare — qui non c'è ancora demografia che lo produca.

**Nessuna invalidazione della copertura al cambio di livello**, per lo stesso motivo: la
capienza non entra nel conto di `scelte_entro_capacita`. `equivalenza_copertura` resta verde
senza che questa fase faccia niente, ed è il modo più economico di verificarlo — se fallisse,
vorrebbe dire che qualcosa legge ancora la capienza dove non deve.

### `DataSet::incoerenze()` — la seconda difesa strutturale di M1

`capacita_cibo_insostenibile` diventa un caso di una funzione che raccoglie **tutti** i
controlli incrociati:

```rust
/// Tutte le incoerenze *fra* tabelle, in un posto solo.
///
/// Vive nel core perche' e' li' che vivono le definizioni (A2) e perche' serve
/// anche alla fixture di `sim-core/tests/comune/mod.rs`, che non passa per
/// `sim-data`. Aggiungere un controllo qui lo rende automaticamente attivo
/// anche sulla fixture: e' la generalizzazione della lezione di A5 — un numero
/// che deve stare in relazione con un altro e' un controllo, non un commento.
pub fn incoerenze(&self) -> Vec<Incoerenza>;

pub enum Incoerenza {
    CapacitaOltreLaProduzione { building: usize, livello: u8, capacita: u16, sostenibili: u16 },
    /// `capienza(l+1) <= capienza(l)`: evolvere rimpicciolirebbe la casa.
    CapienzaNonCrescente { livello: u8 },
    /// `soglia_degrado(l) >= soglia_evoluzione(l+1)`: niente banda d'isteresi,
    /// e la citta' oscilla a ogni valutazione. L'isteresi e' un **controllo**,
    /// non un commento (A10).
    IsteresiAssente { livello: u8 },
    /// Una soglia oltre `soddisfazione.massimo`: livello irraggiungibile per
    /// costruzione, e niente nel gioco lo segnalerebbe.
    SogliaIrraggiungibile { livello: u8, soglia: u8, massimo: u8 },
    /// Un livello richiede un servizio che nessun edificio fornisce.
    ServizioSenzaProvider { livello: u8, servizio: ServiceKind },
    /// Nessun provider di quel servizio ha capacita' per una casa **piena** di
    /// quella capienza. Con A12 non e' un blocco — la casa e' servibile finche'
    /// resta mezza vuota — ma e' un dataset in cui un livello non puo' mai
    /// essere servito a pieno, e la citta' si tappa senza dire perche'.
    CapienzaOltreOgniProvider { livello: u8, servizio: ServiceKind, capienza: u16 },
    /// `buildings["casa"].servizi_richiesti` diverso dall'unione per livello.
    RequisitiIncoerenti,
    /// `buildings["casa"].livelli != rules.livelli_casa.len()`.
    LivelliDisallineati { dichiarati: u8, in_tabella: usize },
    /// Dalla fase 11.
    AbitantiInizialiOltreLaCapienza { difficolta: usize, iniziali: u16, capienza: u16 },
}
```

`sim-data/src/validate.rs:119` chiama `incoerenze()` invece del singolo controllo, e
`la_fixture_rispetta_la_coerenza_fra_capacita_e_produzione` diventa `la_fixture_non_ha_incoerenze`
con `assert_eq!(d.incoerenze(), vec![])`. Da quel momento **ogni controllo nuovo protegge
gratis anche la fixture di `sim-core`** — che oggi non passa da `sim-data` ed è la buca vera.

### `e_una_casa()`, che va presidiato

È un'euristica: `servizio.is_none() && !servizi_richiesti.is_empty()`. Se `servizi_richiesti`
si sposta per livello e la riga top-level sparisce, **la casa smette di essere una casa** e
mezzo gioco cambia comportamento in silenzio. Rimedi: tenerla come unione,
`Incoerenza::RequisitiIncoerenti`, e una validazione "almeno un edificio dev'essere una casa".

### Eventi

```rust
Event::HouseEvolved  { house: HouseId, da: u8, a: u8 },
Event::HouseDegraded { house: HouseId, da: u8, a: u8 },
```

Due varianti e non una con `a < da`: il renderer ci attacca due animazioni diverse, e discriminare
su un confronto numerico è il tipo di cosa che si sbaglia una volta sola ma per sempre.

### Il footprint della casa resta 1×1 a tutti i livelli

Va scritto, non lasciato implicito. Case che si fondono occupando i tile del vicino sono una
meccanica intera (Zeus la fa) e sono M3. `nessuna_sovrapposizione` contiene
`attesi += 1; // le case sono 1x1 in M0`: senza questa riga, qualcuno infilerà un footprint in
`livelli_casa` e quel test diventerà misteriosamente rosso.

## Fuori scope

Nascite, morti, migrazione (fasi 14–15): `abitanti` cambia qui **solo** per sfratto da degrado.
Tipi di casa diversi. Livelli per gli edifici che non sono case — `Building::level` resta 1, e
`raggio_per_livello`/`capacita_per_livello` restano vettori di un elemento.

## Test

1. **Evoluzione** (*il goal*): casa servita, il livello 2 arriva alla prima valutazione mensile
   dopo che la soddisfazione ha superato `soglia_evoluzione` — numero di tick **calcolato dal
   `DataSet`**, mai hardcodato.
2. **Degrado**: demolito il pozzo, la casa torna al livello 1 alla prima valutazione dopo che
   la soddisfazione è scesa sotto `soglia_degrado`.
3. **Nessuna oscillazione** (property test, *il test che definisce la fase*): con servizi
   costanti, su 360 tick il livello di ogni casa è **monotono**. È la prova che l'isteresi
   funziona davvero, non solo che la validazione la impone — servono entrambe le cose e
   nessuna sostituisce l'altra.
4. **Evolvere non tocca la copertura**: una casa che sale di livello **non** cambia
   `Coverage::assegnazioni()` né incrementa `ricalcoli`, perché non ha portato abitanti. È il
   test che fissa A12 dove è più facile sbagliarsi — chi implementa la capienza per livello ha
   la tentazione di usarla anche in `coverage.rs`.
5. **`equivalenza_copertura` resta verde senza modifiche**, ora che i livelli cambiano davvero.
   Se fallisce, qualcosa legge la capienza dove dovrebbe leggere gli abitanti.
6. **Lo sfratto è contato**: casa al livello 2 con 8 abitanti che degrada a capienza 4 ⇒ 4
   abitanti e 4 sfrattati nel ledger. Fissa il termine prima che la fase 14 ne dipenda.
7. **Lo sfratto invalida la copertura**: è l'unico punto di questa fase in cui `abitanti`
   cambia, quindi è l'unico che deve segnare la copertura come sporca — la prima applicazione
   del contratto che la fase 14 generalizza.
8. **Un solo salto per valutazione**: una casa a soddisfazione massima non salta dal livello 1
   al 3 nello stesso mese.
9. **Le incoerenze**: una fixture rotta per ciascun caso nuovo di `Incoerenza`, e
   `la_fixture_non_ha_incoerenze` verde.
10. **`due_tick_a_vuoto_non_ricalcolano_niente`** cambia esito di proposito, ma solo per lo
    sfratto: due tick a vuoto ricalcolano se in mezzo cade un degrado che manda via qualcuno. Va
    riscritto in "nessun ricalcolo se nessuna popolazione è cambiata" — che è già la forma che
    la fase 14 vorrà.

## Verifica

```sh
cargo test -p sim-core livelli
PROPTEST_CASES=2000 cargo test -p sim-core --release oscillazione
cargo test -p sim-core copertura              # equivalenza_copertura deve restare verde
cargo test -p sim-data
cargo xtask run --ticks 720 --dump-every 30   # due anni: i livelli devono salire e fermarsi
cargo xtask regen-golden
cargo xtask bench                             # A non deve muoversi: qui la copertura non si ricalcola
```

Il dump a 720 tick è la prova a occhio che chiude la fase: la distribuzione dei livelli deve
salire e poi **fermarsi**, non oscillare e non crescere per sempre. Se oscilla con la
validazione verde, il bug è nell'implementazione e il test 3 deve coglierlo — se non lo coglie,
è il test a essere debole.

Il tick a vuoto (`A`) va guardato e **deve restare dov'era**: senza demografia niente cambia la
popolazione, quindi la copertura non si ricalcola. Se `A` si muove qui, qualcosa invalida la
copertura che non dovrebbe, e conviene trovarlo adesso — nella fase 14 sarebbe indistinguibile
dal costo previsto.

**Fatto quando:** passano il test 3 (nessuna oscillazione) e i test 4–5 (evolvere non tocca la
copertura). Sono i tre che definiscono la fase; gli altri sono correttezza di contorno.
