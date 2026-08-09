# Fase 15 — Immigrazione ed emigrazione

**Goal:** due città identiche tranne per la copertura ricevono flussi migratori diversi; una
città senza posti liberi non riceve nessuno, e una che ne ha ma li ha lasciati scoperti li
riempie **e poi peggiora**.
**Dipende da:** 14.
**Dimensione:** M.
**Decisioni coinvolte:** [A15](decisioni-aperte.md), [A12](decisioni-aperte.md), A10, A14, D3, D5.

## Perché adesso

Perché chiude il quadrato demografico e perché è il primo uso di `RngDomain::Migration`, che
esiste dalla fase 02 con quel nome e non ha mai fatto niente.

Va **dopo** nascite e morti, non insieme: quelle sono locali alla casa, questa ha bisogno di un
indice cittadino e di una regola di distribuzione. Con i due domini RNG separati, scriverla non
sfasa la sequenza della 14 e il golden di quella fase resta com'era — che è esattamente il
motivo per cui i domini sono separati per nome e non per posizione.

Ed è la fase in cui il ciclo di [A12](decisioni-aperte.md) diventa osservabile per intero: fino
a qui la popolazione cresceva solo dall'interno, con un tetto naturale nelle capienze esistenti.
Con l'immigrazione la città può **superare i propri servizi** in fretta, che è la dinamica per
cui i servizi rincorrono la popolazione — e la prima in cui può andare storta.

## Cosa si costruisce

### L'attrattività

```rust
/// Quanto la citta' attrae, in millesimi.
///
/// Somma pesata di termini interi, tutti dallo stato: la soddisfazione media
/// pesata sugli abitanti e i posti liberi in case **servite**. La fase 16
/// aggiunge l'aliquota come terzo termine, e non prima: accoppiare le due fasi
/// significherebbe bilanciare la migrazione e le tasse insieme, che e' due
/// problemi in uno (A15).
///
/// Funzione **pura** sullo stato: nessun RNG qui dentro. Il jitter sta nel
/// flusso, non nell'indice — cosi' l'attrattivita' si puo' stampare nel dump e
/// confrontare fra due partite, e resta il numero che l'osservazione semantica
/// per l'LLM (M3) mostrera' al posto dello stato serializzato.
pub fn attrattivita(world: &World) -> i32;
```

I pesi in `rules.ron`, mai nel codice.

**La media pesata divide per la popolazione**, che può essere zero: una città senza abitanti è
uno stato normalissimo, e `nessun_panic_su_diecimila_comandi` ne produce di continuo. Il caso
va gestito esplicitamente — attrattività di una città vuota uguale a quella di una città
appena soddisfatta, così i primi immigranti arrivano — non con un `debug_assert`.

### Il passo 6, completo

```rust
/// Passo 6 — evoluzione, degrado e i quattro flussi demografici.
///
/// L'ordine interno e' **semantica di gioco** quanto l'ordine dei dieci passi,
/// e vale la stessa regola: non si riordina senza rigenerare i golden e
/// scrivere perche'.
fn houses_and_migration(world: &mut World) {
    aggiorna_soddisfazione(world);          // 6.1  fase 12
    if e_valutazione_mensile(world) {
        degrada(world);                     // 6.2a fase 13, include lo sfratto
        evolvi(world);                      // 6.2b fase 13
    }
    morti(world);                           // 6.3  fase 14
    emigrazione(world);                     // 6.4  questa fase
    nascite(world);                         // 6.5  fase 14
    immigrazione(world);                    // 6.6  questa fase
}
```

L'ordine completo e le sue ragioni, da scrivere una volta in `tick.rs`:

- **6.1 per prima**: legge solo ciò che i passi 3 e 4 hanno scritto in questo tick, e tutto il
  resto legge lei.
- **Degrado prima dell'evoluzione**: una casa che scende non deve poter salire nello stesso
  tick per un requisito residuo.
- **6.2 prima della demografia**: un livello nuovo cambia la capienza, e i posti che apre devono
  essere riempibili **nello stesso tick**. Altrimenti ogni salto costa un tick di crescita
  mancata: invisibile in tabella, visibilissimo nella curva.
- **Uscite prima delle entrate** (6.3–6.4 prima di 6.5–6.6): garantisce `abitanti <= capienza`
  in ogni istante osservabile, e rende la popolazione reattiva invece che a scatti.
- **Immigrazione ultima**: è il flusso residuo e dipende dai posti liberi *finali*. Metterla
  prima significherebbe calcolarli su uno stato che sta ancora cambiando.

L'unico riordino difendibile sarebbe 6.5 prima di 6.3 — nascite prima delle morti — che in
aggregato non cambia nulla ma impedirebbe a una casa piena di rimpiazzare un morto nello stesso
tick. Peggiore. L'ordine sopra è quello da fissare.

### Dove vanno gli immigranti, e dove **non** si mette un gate

Zero posti liberi ⇒ zero immigrazione, a qualunque attrattività. Non un tasso ridotto: zero. Le
case sono l'unico contenitore della popolazione (D5), e senza contenitore non c'è flusso.

Dentro le case con posto, l'insieme eleggibile è quello delle case **servite**. È la parte più
economica di [A10](decisioni-aperte.md): *"i migranti vanno solo dove si sta bene"* non è un
meccanismo separato, è il filtro — le stesse case che l'evoluzione considera degne, per la
stessa ragione.

**Il gate che non si mette**, ed è la decisione da scrivere: l'immigrazione **non** è
subordinata alla capacità residua dei provider. Sarebbe facile e sembrerebbe prudente — non far
entrare nessuno se il pozzo è al limite — ma è esattamente il contrario di
[A12](decisioni-aperte.md). Con quel gate i servizi tornerebbero a precedere la popolazione, e
la città si fermerebbe da sola senza che il giocatore debba accorgersi di niente.

Senza, succede quello che deve: la città si riempie, supera i propri servizi, la copertura
comincia a mancare a qualcuno, la soddisfazione cala, l'emigrazione si accende. **Il giocatore
vede il problema e costruisce.** L'overshoot è il segnale, non un bug.

Un'avvertenza che vale la pena scrivere accanto: con la capacità sugli abitanti, una casa
**vuota** costa zero e risulta quindi sempre servita. Alla difficoltà `difficile`, dove tutte
le case nascono vuote, il filtro "solo case servite" all'inizio non filtra niente. È corretto —
una casa vuota non ha bisogno d'acqua — ma significa che la prima ondata di immigranti si
spalma ovunque e il deficit si manifesta tutto insieme qualche tick dopo. È il momento più
delicato della curva e va guardato nel dump.

### Il rischio vero di questa fase: il ciclo può non smorzare

Con i servizi che rincorrono, il sistema ha una retroazione negativa naturale — troppa gente ⇒
copertura che manca ⇒ emigrazione ⇒ meno gente ⇒ copertura che torna. Se i guadagni sono
tarati male, non converge: oscilla, e la città pulsa per sempre invece di stabilizzarsi.

Tre cose la smorzano, e sono già tutte in piedi per altri motivi: la soddisfazione è un
accumulatore lento (A10), quindi un tick di copertura mancata non fa scappare nessuno; la
valutazione dei livelli è mensile (fase 13); e l'emigrazione ha una soglia, non è proporzionale
allo scarto.

Quello che manca è la **prova che basti**, ed è il test 9. Se non smorza, il rimedio è
bilanciamento — soglia di emigrazione più bassa, accumulatore più lento — non un meccanismo
nuovo.

Una cosa che **non** va fatta per smorzare: rendere la copertura "appiccicosa", cioè dare
priorità alle case già servite. Ridurrebbe il churn ma renderebbe la copertura dipendente dalla
storia, e `calcola_da_zero` non ha storia — `equivalenza_copertura` cadrebbe, e con essa il
presidio di ogni ottimizzazione futura del passo 3.

### L'emigrazione

Le case sotto una soglia di soddisfazione perdono abitanti, con un tasso in tabella. È il
canale per cui una città che si degrada si svuota **anche senza morti** — la differenza fra
"la gente muore di fame" e "la gente se ne va", che sono due cose diverse e vanno lette diverse
nel dump.

Gli sfrattati da degrado (fase 13) confluiscono qui nel ledger: sono emigrazione, non un quinto
flusso.

### L'emigrazione

Le case sotto una soglia di soddisfazione perdono abitanti, con un tasso in tabella. È il
canale per cui una città che si degrada si svuota **anche senza morti** — la differenza fra
"la gente muore di fame" e "la gente se ne va", che sono due cose diverse e vanno lette diverse
nel dump.

Gli sfrattati da degrado (fase 13) confluiscono qui nel ledger: sono emigrazione, non un quinto
flusso.

### La tabella e i controlli

```ron
migrazione: (
    // Pesi dell'attrattivita', in millesimi.
    peso_soddisfazione: 700,
    peso_posti_liberi: 300,
    // Flusso massimo in ingresso, per mille abitanti al mese, ad attrattivita' piena.
    immigrazione_per_mille_al_mese: 30,
    emigrazione_per_mille_al_mese_scontenti: 40,
    soglia_emigrazione: 25,
    jitter_per_mille: 200,
),
```

**Controllo incrociato**: `soglia_emigrazione < soglia_nascite`, e sotto la soglia di degrado
del livello 1. Se una casa emigrasse a una soddisfazione a cui fa ancora figli, i due flussi si
combatterebbero su ogni tick e la popolazione oscillerebbe senza che niente lo segnali — è la
stessa forma di `Incoerenza::IsteresiAssente`.

Il `Sommario` guadagna `immigrati`, `emigrati` e `attrattivita`.

## Fuori scope

**I walker immigranti.** D3 dice che gli immigranti sono entità simulate vere, e lo saranno:
in M3, insieme ai walker logistici. In M1 la migrazione è un numero
([A14](decisioni-aperte.md)), e ciò che M3 aggiungerà è il **tempo di viaggio**, non la regola
di attrattività. Va scritto qui, perché è la semplificazione di questa fase e sarà chi legge in
M3 a doverla smontare — stessa forma di [A5](decisioni-aperte.md) per la fattoria.

Emigrazione verso una destinazione, città rivali, immigranti con mestieri o ricchezza: niente
di tutto questo esiste, e nessuno scenario lo chiede.

## Test

1. **Il goal**: due città identiche tranne per la copertura ⇒ flussi diversi, quella servita
   cresce più in fretta. Con lo stesso seed, così la differenza è solo la copertura.
2. **Il gate duro**: attrattività al massimo, zero posti liberi ⇒ zero immigrati, per cento
   tick. E aggiungendo una casa servita, il flusso riparte nello stesso tick.
3. **Solo case servite**: una casa scoperta con posti liberi non riceve nessuno, anche se è
   l'unica con posto in tutta la città. Il caso va costruito con una casa **già abitata** e
   scoperta: una vuota è servita a costo zero e non distinguerebbe niente.
4. **Emigrazione**: demolito il pozzo, le case si svuotano per emigrazione **e** per morti, e
   il ledger distingue i due. Il test è sul ledger, non sulla popolazione: è la distinzione a
   dover essere verificata.
5. **La conservazione della popolazione resta esatta** con i due flussi nuovi dentro. Il
   property test della fase 14 non cambia forma, e questo è il segno che il ledger era stato
   scritto giusto.
6. **Città vuota**: attrattività calcolata su zero abitanti non panica e non è assurda. Coperto
   anche da `nessun_panic_su_diecimila_comandi`, ma vale un test dedicato che dica il caso.
7. **Il golden della fase 14 non cambia**: `Migration` è un dominio distinto da `Demografia`, e
   uno scenario senza posti liberi non estrae nulla dal primo. Verificabile a scenario
   costruito apposta — ed è la prova pratica della separazione dei domini.
8. **Cento seed**: la popolazione a cinque anni sta in una banda stretta, mai due volte la
   stessa. Il test della fase 14 esteso, ora che il flusso dominante è la migrazione.
9. **Il ciclo smorza** (property test, *il test che definisce la fase*): città che supera i
   propri servizi e viene lasciata a sé stessa per cinque anni ⇒ la popolazione converge a una
   banda e **l'ampiezza dell'oscillazione non cresce**. È la verifica che la retroazione di A12
   è negativa e non un pendolo. Se fallisce, è bilanciamento — soglia di emigrazione più bassa
   o accumulatore più lento — non un meccanismo nuovo.
10. **La copertura non è appiccicosa**: due case equidistanti che si contendono l'ultimo posto
    ⇒ la vincitrice è sempre la stessa per `(distanza, TileIdx, HouseId)`, non "quella che ce
    l'aveva ieri". Fissa la scelta e impedisce che qualcuno introduca stickiness per ridurre il
    churn, che romperebbe `equivalenza_copertura`.
11. **L'overshoot è raggiungibile**: uno scenario in cui l'immigrazione porta la città oltre la
    capacità dei provider, e la copertura comincia davvero a mancare. È il ciclo di gioco di
    A12: se non si riesce a costruirlo, il gate implicito è più stretto di quanto si crede.

## Verifica

```sh
cargo test -p sim-core migrazione
PROPTEST_CASES=2000 cargo test -p sim-core --release conservazione_popolazione
cargo xtask run --ticks 1800 --dump-every 90
cargo xtask run --difficolta difficile --ticks 1800 --dump-every 90
cargo xtask regen-golden
```

Le due esecuzioni a difficoltà diverse sono la prova a occhio che chiude la fase: a `difficile`
le case nascono vuote, quindi **tutta** la popolazione arriva migrando, e la curva deve partire
più piano ma arrivare. Se a `difficile` la città non decolla mai, la soglia dell'attrattività è
tarata male — ed è meglio scoprirlo adesso che nella fase 17, quando uno scenario dovrà
dichiararsi vinto.

Nel dump vanno guardate **due colonne insieme**: la popolazione e la frazione di abitanti
coperti. Con i servizi che rincorrono, la seconda deve scendere quando la prima accelera e
risalire quando il giocatore costruisce. Se la copertura resta piatta al 100%, il ciclo di A12
non si sta innescando e lo scenario è troppo generoso; se crolla e non risale, è troppo severo.

`bench` va rieseguito e confrontato con i tre numeri della fase 14: l'immigrazione muove
popolazione ogni tick come le nascite, quindi il costo del ricalcolo non cresce, ma la
**frequenza** dei tick in cui la popolazione si muove sì. Il conteggio dei ricalcoli è il
numero da guardare, e va aggiunto ai dati di [A17](decisioni-aperte.md).

**Fatto quando:** passano il test 5 (conservazione ancora esatta) e il test 9 (il ciclo
smorza), e la partita a `difficile` arriva a una città viva in cinque anni.
