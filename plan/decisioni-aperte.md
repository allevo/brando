# Decisioni aperte

Scelte necessarie per implementare M0 che `CLAUDE.md` non fissa. Per ognuna: la
raccomandazione che le fasi assumono, e cosa cambia se si decide diversamente.

Da A7 in poi il file continua oltre M0: sono decisioni prese dopo la chiusura, con la stessa
forma. Chi cerca "perché il codice fa così e non cosà" trova qui la risposta.

> **Aperta davvero, in questo momento, ce n'è una sola:**
> [A17 — il costo del ricalcolo per tick](#a17--il-costo-del-ricalcolo-per-tick--aperta-da-chiudere-prima-di-m2),
> da chiudere prima di M2. È il debito che [A12](#a12--i-servizi-rincorrono-la-popolazione)
> apre di proposito.

## Stato a fine M0

| # | Esito | Nota |
|---|---|---|
| A1 | **Chiusa**, come raccomandato | `Milli(i32)` e `Coins(i32)` separati, nessun operatore nudo |
| A2 | **Chiusa, diversamente** | `Arc<DataSet>` nel `World`, ma le *definizioni* sono dovute passare in `sim-core` |
| A3 | **Chiusa**, come raccomandato | hash a mano, più un test che verifica che copra ogni campo |
| A4 | **Chiusa**, come raccomandato | lato fino a 256, golden su 32×32, tripwire a 200×200 |
| A5 | **Chiusa con una scoperta**, poi corretta | la fame risultava uno stato assorbente; sciolta dopo M0 rendendo coerenti capacità e produzione: vedi sotto |
| A6 | **Chiusa**, come raccomandato | 30 tick/mese, 360/anno, in `rules.ron` |

Nessuna è rimasta aperta. Le due che meritano di essere lette sono A2 e A5, perché entrambe
sono finite altrove rispetto a come erano state scritte.

## Decisioni dopo M0

| # | Esito | Nota |
|---|---|---|
| A7 | **Chiusa**: i comandi restano un enum | un trait romperebbe `Copy`, serde e il formato dei golden |
| A8 | **Chiusa**: `Arc<DataSet>` si tiene | serve a `World: Clone`, non al borrow checker |
| A9 | **Chiusa**: l'acqua è copertura, non una risorsa | ma `servizi_richiesti` è dato dichiarativo che M1 deve iniziare a leggere |
| A10 | **Chiusa in linea di principio**, da implementare in M1 | la soddisfazione è un accumulatore di tempo, non un livello di risorsa |
| A11 | **Chiusa: non si fa**, con la condizione per riaprirla | l'invalidazione mirata della copertura; il costo era altrove, e toglierlo ha dato 6,5× senza stato nuovo |

Sono nate smarcando `dubbi.md` (2026-08-09): tre erano domande di design, e le risposte
valevano più della domanda. A9 e A10 sono prerequisiti di M1 fase 1–2. A11 è l'unica che dice
di **non** fare qualcosa, ed è quella con la storia più utile: l'ipotesi di partenza era
sbagliata, e la misura l'ha detto prima che costasse.

## Decisioni prese pianificando M1

| # | Esito | Nota |
|---|---|---|
| A12 | **Presa**, fasi 13–15 | **i servizi rincorrono la popolazione**: la capacità si conta sugli abitanti presenti, e la copertura si ricalcola quando qualcuno si muove |
| A13 | **Presa**, fase 11 | la difficoltà di gioco è un asse nuovo: tabella di profili, id nel `World` e nell'header del replay |
| A14 | **Presa**, fasi 14–15 | demografia a quattro flussi, aggregata; tasso base con jitter da RNG seedato |
| A15 | **Presa**, fasi 15–16 | attrattività = soddisfazione media + posti liberi, e l'aliquota **solo** dalla fase 16 |
| A16 | **Presa**, fase 17 | gli obiettivi vivono nel `World`, `sim-scenario` li costruisce |
| A17 | **Aperta**, da chiudere **prima di M2** | il costo del ricalcolo della copertura a ogni tick, che è il prezzo di A12 |

A12 è quella da leggere: è una scelta di *gioco* che si paga in tempo di calcolo e in copertura
di test, ed è stata presa sapendolo. A17 è la prima decisione davvero aperta del progetto da
fine M0, e non è un caso: è il debito che A12 apre.

A12–A16 sono ancora **previsioni**. Alla chiusura di M1 (fase 18) vanno riscritte con come sono
andate davvero — che per A2, A5 e A11 è stata l'informazione più utile del documento.

---

## A1 — Il denaro non è `Milli`

**Raccomandazione:** `Milli(i32)` resta per le quantità frazionarie (cibo, lavoro, usura).
Il denaro è un newtype intero separato, `Coins(i32)`, perché non ha frazioni di gioco e usare
millesimi dimezzerebbe il range utile per niente.

`Milli(i32)` copre ±2.147.483 unità. Basta per cibo e popolazione (target 15.000 abitanti,
D5). Il tesoro su una partita lunga può superarlo, quindi tenerlo fuori da `Milli` è anche
prudenza sul range.

*Se si decide il contrario:* un solo tipo semplifica le firme, ma serve `Milli(i64)` e va
verificato l'impatto su `size_of::<House>()`.

---

## A2 — Dove vive il `DataSet`

`CLAUDE.md` descrive `step(&mut World, &[Command])`, ma i sistemi hanno bisogno delle tabelle.

**Raccomandazione:** `World` contiene un `Arc<DataSet>` e un campo `data_hash: [u8; 32]`.
La firma resta quella dichiarata, il caricamento (I/O) resta fuori dal core, e l'hash del
dataset entra nell'hash canonico dello stato: se qualcuno cambia un numero di bilanciamento,
il golden replay fallisce **subito e per il motivo giusto**, invece di divergere dieci tick
dopo per un effetto secondario.

*Alternativa:* passare `&DataSet` come parametro a `step`. Più puro, ma cambia la firma
dichiarata e propaga un parametro in ogni funzione interna.

**Come è stata risolta davvero (fase 04).** La raccomandazione regge, ma ha una conseguenza
che la fase 03 non aveva previsto: se il `World` tiene un `Arc<DataSet>`, allora `DataSet` deve
essere visibile da `sim-core`, e `sim-core` non può dipendere da `sim-data` senza invertire il
grafo delle dipendenze.

Le **definizioni** (`DataSet`, `Rules`, `BuildingDef`, `ServiceDef`, `TerrainDef`) e l'hash
canonico sono quindi state spostate in `sim-core::data`: sono struct pure, senza I/O, e
appartengono al vocabolario del core. In `sim-data` restano il parsing RON, la validazione e
l'I/O — cioè esattamente ciò che D4 vieta al core. `sim-data` ri-esporta i tipi per comodità di
chi carica le tabelle, così i chiamanti non devono sapere dove sono definiti.

Il confine resta quello che conta: *nessun numero di bilanciamento nel codice*, e *nessun I/O
nel core*.

---

## A3 — L'hash canonico è scritto a mano, non delegato a serde

**Raccomandazione:** `fn hash_world(&World) -> [u8; 32]` alimenta un hasher `blake3` campo per
campo in un ordine esplicito nel codice.

Motivo: con `serde` l'hash dipende dall'ordine di dichiarazione dei campi e dal formato, quindi
un refactoring innocuo (spostare un campo) invaliderebbe tutti i golden senza che nessuno abbia
cambiato semantica. È esattamente il falso positivo che rende inutile il test più prezioso del
progetto. Il costo è che aggiungere un campo allo stato richiede di aggiungerlo all'hash a mano
— mitigato da un test di copertura (vedi fase 08).

---

## A4 — Dimensione della mappa in M0

**Raccomandazione:** `Grid` accetta larghezza e altezza fino a 256, con `TileIdx(u16)`
(D: `Tile` piccolo, indici `u16`). Gli scenari di test di M0 usano griglie 32×32: un golden
replay leggibile a occhio vale più di uno realistico. Un caso 200×200 esiste solo come
tripwire di performance in fase 09.

---

## A5 — Il cibo in M0 non ha walker

**Raccomandazione:** in M0 la fattoria è un **provider di servizio** (come il pozzo): produce
in una giacenza locale e copre le case entro raggio su strada, consumando dalla giacenza.

Non è la catena produttiva definitiva: magazzini e walker logistici reali sono M3 (D3). È però
la versione minima che chiude un ciclo produzione→consumo osservabile, e non introduce
concetti che andranno rimossi: la copertura resta valida, in M3 cambia solo *da dove* arriva
la merce.

*Da tenere d'occhio:* il rischio è che il raggio della fattoria diventi un numero di
bilanciamento su cui si costruisce M1, e che M3 lo debba smontare. Accettato, e annotato nella
tabella RON della fattoria.

**Cosa è emerso implementandola (fase 07).** La semplificazione ha una conseguenza che nessuno
aveva previsto scrivendo il piano: **lo stato di fame è assorbente per una casa già assegnata**.
La contesa tra provider la vince il primo (fase 06) e la capacità si conta in *case*, non in
abitanti; quindi una casa coperta da una fattoria in deficit occupa un posto che nessun'altra
fattoria può rilevarle, e continua a non mangiare per sempre.

Una seconda fattoria aiuta solo le case che la prima aveva lasciato **fuori** dalla propria
capacità. Entrambi i comportamenti hanno un test che li fissa
(`una_seconda_fattoria_copre_le_case_lasciate_fuori`,
`una_casa_affamata_non_viene_salvata_da_una_seconda_fattoria`), il secondo dei quali è scritto
per **cambiare esito** quando in M1 la capacità diventerà "abitanti serviti": sarà il segnale
che la semplificazione è stata sciolta, non una regressione.

Aggiunta anche una voce al `FoodLedger`, `perso_per_demolizione`: demolire una fattoria piena
fa sparire la sua giacenza dal mondo, e senza registrarlo la conservazione smetteva di essere
un'uguaglianza. Stesso motivo per cui esiste `perso_per_giacenza_piena`.

**Come è stata sciolta (2026-08-08, prerequisito di M1 fase 1).** La capacità ora si conta in
**abitanti serviti**. È stato fatto adesso e non dentro M1 perché i livelli delle case rendono
la popolazione variabile per casa, e con popolazione variabile una capacità in case non dice
più quanta gente un provider riesca a servire.

Cambiare unità, da solo, **non è servito a niente**: convertendo i valori in tabella per il
numero di abitanti per casa (pozzo 8 → 32, fattoria 6 → 24) il comportamento è rimasto
identico al tile — verificato eseguendo i due scenari su un anno di gioco e confrontando
l'output. Lo stato assorbente era lì lo stesso, espresso in un'altra unità. È il motivo per cui
i due passaggi sono commit separati: il primo lo dimostra.

Ciò che l'ha sciolto è la seconda metà: **la capacità di un produttore dev'essere coerente con
ciò che la sua produzione sostiene**. La fattoria produce 400 milli/tick e un abitante ne
consuma 20, quindi ne sostiene 20 — cinque case, non sei. Da questo segue un invariante che
prima non c'era, ed è il vero guadagno: *una casa coperta dal servizio cibo mangia sempre*
(`invarianti.rs::invarianti_coperta_significa_sfamata`). La fame resta raggiungibile, ma è
mancanza di **copertura** e si cura costruendo.

La coerenza è un **controllo di validazione**, non una convenzione nei commenti:
`DataSet::capacita_cibo_insostenibile` in `sim-core`, richiamato dalla validazione di
`sim-data`. Sta nel core perché è lì che vivono le definizioni (A2) e perché serve anche alla
fixture dei test di `sim-core`, che non passa per `sim-data` e altrimenti potrebbe scivolare su
un bilanciamento che in produzione sarebbe rifiutato. Il conto è sul caso peggiore, giacenza a
zero: il minore tra la produzione di un tick e la capienza del granaio. **Va tolto in M3**
insieme ad A5: quando la merce arriverà da un magazzino via walker (D3), la capacità smetterà
di dipendere dalla produzione locale.

Fissata anche la regola di riempimento, che con popolazioni miste diventerà osservabile:
nessuna assegnazione parziale, e chi non ci sta viene **saltato**, non fa da barriera. Fermarsi
alla prima casa che non entra terrebbe la distanza come priorità assoluta, ma lascerebbe posti
inutilizzati — e la capacità in tabella smetterebbe di dire il vero, il che romperebbe proprio
la coerenza con la produzione. Il costo è che una casa più lontana può passare davanti a una
più vicina che non ci sta; accettato. La regola è una funzione pura,
`coverage::scelte_entro_capacita`, con un test tabellare che copre i casi a popolazione mista
che M0 non sa ancora produrre.

Il test scritto in fase 07 per cambiare esito ha cambiato esito, ed è ora
`la_fame_si_cura_costruendo_una_seconda_fattoria`.

---

## A6 — Ritmo del tempo

`CLAUDE.md` fissa 1 tick = 1 giorno e il mese come multiplo fisso, senza dare il numero.

**Raccomandazione:** 30 tick = 1 mese, 12 mesi = 1 anno (360 tick). Il valore vive in
`sim-data` (`rules.ron`), non nel codice: gli obiettivi di scenario si esprimono in mesi
e servono a M1.

---

## A7 — I comandi restano un enum, non un trait

**Decisione:** `Command` resta un enum chiuso, applicato da un `match` che chiama tre funzioni
libere (`tick.rs`). Niente `trait TryApply`, niente `Box<dyn …>`.

Il motivo non è stilistico, è che il trait romperebbe cose che esistono:

- `Command` è `Copy` e `Serialize`/`Deserialize`, e un salvataggio è `seed + Vec<Command>`
  serializzato in RON con `struct_names(true)`. Un trait object fa saltare `Copy`, il derive di
  serde, `PartialEq`/`Eq` — e **il formato dei golden già committati**.
- D4 vuole un vocabolario **chiuso**: un trait invita implementazioni esterne, cioè comandi che
  il log di determinismo non sa rappresentare.
- L'adattatore LLM di M3 ha bisogno di uno schema enumerabile: `Intent → Vec<Command>` con un
  insieme finito e ispezionabile.

E non c'è niente da risparmiare: il `match` è **cinque righe e tre varianti**, e la logica sta
già in tre funzioni separate.

*Se il fastidio è un altro* — `tick.rs` è il file più lungo del core (434 righe) e mescola
dispatch, applicazione dei comandi, i dieci passi e sette helper — allora la risposta è
separare in **moduli**, non in trait object. Sono due problemi diversi con due soluzioni
diverse.

---

## A8 — `World` tiene un `Arc<DataSet>`, e non è per il borrow checker

A2 aveva deciso l'`Arc` senza dire perché fosse un `Arc` e non un valore. La risposta:

**`World` è `Clone` e viene clonato moltissimo.** `invarianti_i_rifiuti_non_mutano` lo clona una
volta per tick per ogni caso proptest — migliaia di volte per esecuzione — e
`l_hash_copre_tutto_lo_stato` una volta per perturbazione. Con l'`Arc` il campo costa un
incremento di refcount; senza, ogni clone rifarebbe le allocazioni di `Vec<BuildingDef>`, delle
`String` degli id e dei `Vec<u16>` per livello. In più `sim-replay` e `xtask` costruiscono più
`World` dallo stesso dataset caricato una volta sola.

Le alternative sono peggiori: `World<'a>` propaga un lifetime in `Scenario`, nelle firme di
replay e in ogni test; passare `&DataSet` a `step` cambia la firma dichiarata in `CLAUDE.md`
(già valutata e scartata in A2).

**`Arc` e non `Rc`** benché il core sia monothread: `Arc` rende `World: Send`, e `xtask` ha in
programma i batch di partite per il bilanciamento automatico — è esattamente il caso in cui si
vorrà parallelizzare.

*Nota su un falso indizio.* `production.rs` fa `Arc::clone(&world.data)` e sembra usare l'`Arc`
per niente. Lì è davvero un aggiramento del borrow checker: `consuma(world, …)` prende
`&mut World` intero, quindi un `&world.data` vivo per tutta la funzione sarebbe incompatibile.
Si toglierebbe passando a `consuma` i soli campi che tocca, ma costa un incremento atomico per
tick e renderebbe la firma più rumorosa. Lasciato com'è, di proposito.

---

## A9 — L'acqua è copertura, non una risorsa

**Decisione:** il pozzo non dichiara `produzione_per_tick` né `giacenza_max`, quindi il passo 4
non lo tocca mai. L'acqua ha raggio e capacità, e basta. È il modello di Zeus, ed è quello
giusto: una catena produttiva dell'acqua sarebbe una risorsa in più da bilanciare senza nessun
payoff di gioco.

**Il buco che sta accanto, e che va chiuso in M1.** `servizi_richiesti` (`["acqua","cibo"]`
sulla casa) è letto **solo** da `BuildingDef::e_una_casa()`: serve a classificare il tipo di
edificio, non a decidere niente in simulazione. La copertura assegna qualunque casa
raggiungibile senza mai guardare se quel servizio le serva davvero. Oggi non è osservabile —
c'è un solo tipo di casa — ma è il campo che M1 deve iniziare a leggere sul serio, quando i
livelli avranno requisiti di servizio diversi.

**Un'asimmetria da sciogliere insieme.** `House::servita` significa due cose a seconda del bit:
per l'acqua "è coperta" (scritto dal passo 3), per il cibo "ha mangiato" (riscritto dal passo
4). Con l'invariante *coperta ⇒ mangia sempre* le due coincidono sempre, quindi la differenza
oggi esiste solo sulla carta — ma è una trappola per chi leggerà quel campo in M1.

---

## A10 — La soddisfazione delle case è un accumulatore di tempo, non un livello di risorsa

Serve a M1 fase 1–2 (evoluzione, degrado, migrazione) e va deciso prima di scriverle.

**Non un "livello di cibo arrivato".** Misurare *quanto* cibo entra in una casa romperebbe
"niente consumo parziale" (fase 07), ed è quella scelta a rendere la conservazione
un'**uguaglianza esatta** invece che una disuguaglianza — cioè a rendere il test più prezioso
del progetto capace di trovare un bug invece che di rassicurare. Sarebbe anche degenere: con la
coerenza capacità/produzione (A5) una casa coperta riceve sempre il 100%.

**Decisione:** un accumulatore per servizio, `House { soddisfazione: [i16; ServiceKind::COUNT] }`,
che sale quando il servizio c'è e scende quando manca. Non misura quanto arriva, misura **da
quanto tempo** arriva — che è ciò che serve davvero a "me ne vado / sto / cresco", e che non è
degenere nemmeno oggi, perché la copertura si perde (demolisci una fattoria, spezzi una strada).

> **Emendata pianificando M1.** Il tipo è `[u8; ServiceKind::COUNT]`, non `[i16; …]`:
> l'accumulatore è clampato in `0..=massimo` e non ha mai bisogno del segno né del range, e
> così `House` resta piccola. Il resto della decisione regge senza modifiche, e la
> [fase 12](12-soddisfazione.md) la implementa alla lettera.

Con **soglie diverse per evolvere e degradare**: la banda d'isteresi evita che la città oscilli
a ogni tick sul confine, ed è anche ciò che tiene stabili i golden. Tutte le soglie in RON (D6).

**Sulla migrazione.** D5 dice che l'unità è la casa, non l'individuo, e gli immigranti come
walker veri sono D3/M3. In M1 la migrazione va tenuta **aggregata**: un indice di attrattività
cittadino decide il flusso netto, che riempie le case con posto. Allora "i migranti vanno solo
dove si sta bene" non è un meccanismo separato — è la stessa regola dell'evoluzione (una casa
non servita non accetta nuovi abitanti e a lungo li perde). Un meccanismo in meno per lo stesso
comportamento.

**Sequenza:** va fatto insieme ai livelli delle case, non prima. È il momento in cui `abitanti`
smette di essere costante, ed è la ragione per cui la capacità dei servizi è già stata spostata
in abitanti (A5). Quando arriva, `RngDomain::Migration` viene usato per la prima volta e
`seed_diversi_danno_hash_diversi`, oggi `#[ignore]`, si riattiva e deve passare.

**Attenzione a un dettaglio meccanico:** ogni campo nuovo su `House` va aggiunto a mano a
`hash_world` (A3). Se ci si dimentica, `l_hash_copre_tutto_lo_stato` fallisce — la rete c'è.

---

## A11 — L'invalidazione mirata della copertura non si fa (per ora)

Il passo 3 costava 19,83 ms per ricalcolo alla scala di riferimento, e l'ipotesi ovvia era che
il problema fosse *quante volte* si rifà il BFS: qualunque comando accettato invalida tutti i
1.219 provider. La cura ovvia era cachare per provider i tile raggiunti — il BFS dipende solo
da strade, posizione e raggio, **non dalle case**, quindi piazzare una casa non dovrebbe
invalidare niente.

**Non è stata l'ipotesi giusta.** Il costo non era il numero di BFS, era ciò che ciascuno si
portava dietro: tre `BTreeMap` nel ciclo interno e uno scratch grande quanto la griglia
allocato per provider. Toglierli ha dato **6,5×** senza aggiungere un byte di stato
([09-invarianti-chiusura.md](09-invarianti-chiusura.md) per i numeri).

**Decisione:** con `G` a 3,05 ms e 2,5 µs per provider, la cache non si fa adesso.

- Eliminerebbe la sola traversata del BFS, cioè una parte dei 2,5 µs residui — non tutti:
  restano l'ordinamento delle candidate, il riempimento della capacità e gli ingressi.
- Non farebbe nulla per `F`, il caso in cui si posa una strada: lì la topologia cambia e ogni
  BFS va rifatto comunque. Oggi `F ≈ D`, quindi coprirebbe metà dei casi.
- In cambio chiede la prima **struttura derivata persistente** del `World` (~0,5 MB, più grande
  della griglia), clonata a ogni `World::clone`, e un contratto di invalidazione che qualcuno
  deve ricordarsi di onorare. È anche il primo di questi interventi che può introdurre un bug
  di correttezza invece che solo di lentezza.

Gli altri tre erano riscritture a risultato bit-identico di codice che già c'era. Questa è
un'altra categoria, e `CLAUDE.md` è esplicito su entrambe le cose: *non ottimizzare prima del
profiler*, e *ogni sistema nuovo dev'essere raggiungibile e osservabile*.

**Quando riaprirla.** Quando un batch di partite di M2 mostra il tempo dominato dai tick in cui
si costruisce; o quando M1 fa crescere molto il numero di provider; o quando un profiler mostra
`bfs_strade` come voce dominante di `calcola_da_zero`. Prima di allora, il numero da guardare
non è `D` ma `A` — 248 µs pagati a **ogni** tick, contro 3,35 ms pagati solo quando il giocatore
costruisce.

**Se si farà, tre cose sono già decise**, perché sono le trappole e non i dettagli:

1. La cache va indicizzata con la **chiave intera** (`slotmap::SecondaryMap<BuildingId, _>`),
   mai con il solo indice di slot. Uno slot riusato da un edificio nuovo erediterebbe in
   silenzio i tile del vecchio, e se raggio e strade coincidono la copertura risulterebbe
   sbagliata **senza nessun segnale**. `SecondaryMap` confronta anche la versione e tratta il
   mismatch come assenza: il miss è automatico.
2. `calcola_da_zero` deve restare **senza cache**. È l'oracolo di `equivalenza_copertura`: se
   leggesse la cache del mondo, il test più importante della fase 06 diventerebbe una
   tautologia. Serve un test che lo fissi, avvelenando la cache e verificando che
   `calcola_da_zero` la ignori.
3. Memorizzare accanto ai tile il **raggio** con cui sono stati calcolati. Così la voce si
   autovalida contro il livello dell'edificio (M1) invece di dipendere da chi si ricorda di
   invalidarla. Resta non presidiata l'origine: se un giorno arriverà un `Command::Move`, dovrà
   invalidare la cache a mano.

---

## A12 — I servizi rincorrono la popolazione

La capacità di un provider si consuma sugli **abitanti presenti**, non sulla capienza della
casa. Una casa da otto posti con due abitanti pesa due. È la scelta di gioco che decide se la
città si costruisce in anticipo o in rincorsa, e vale la pena scriverla per esteso perché ha un
prezzo che si paga altrove.

### Il problema che apre

In M0 `abitanti` è scritto una volta alla costruzione e non cambia mai più, quindi
`coverage.rs` può contare la capacità su di lui senza conseguenze. Con la demografia
(fase 14) cambia a ogni tick, e la copertura calcolata al passo 3 invecchia entro lo stesso
tick. Due cose ne risentono, e in due modi diversi.

**La crescita rompe *una casa coperta dal cibo mangia sempre*** (A5). Cinque case da quattro
abitanti su una fattoria sono 400 milli/tick contro 400 prodotti: pareggio esatto, per
costruzione della validazione. A otto abitanti la domanda raddoppia, la giacenza regge una
cinquantina di tick, poi `consuma()` torna `false`.

**Sia la crescita sia il calo rompono `equivalenza_copertura`.** Capacità 20, candidate
`A4 B4 C4 D4 E4` tutte assegnate. Se `A` sale a 8, `calcola_da_zero` assegna `A8 B4 C4 D4` e
scarta `E`; se `B` scende a 2, l'usato passa a 18 e fa entrare una `F2` che la regola del salto
aveva lasciato fuori. In entrambi i casi la copertura memorizzata e quella da zero divergono, su
un test che nessuno ha sbagliato.

### La decisione, e cosa costa

**Si invalida la copertura ogni volta che la popolazione si muove.** Il passo 6 chiude segnando
i provider come sporchi, e il passo 3 del tick successivo rifà l'assegnazione sulla popolazione
vera. Tutti gli invarianti restano in piedi, nessuno va indebolito per convenienza.

Il prezzo è diretto: il ricalcolo passa da "quando il giocatore costruisce" a "quasi ogni tick".
`G` vale 3,05 ms contro i 248 µs del tick a vuoto, quindi il tick tipico va verso ~3,3 ms —
**~13×**. Dieci anni di gioco a 200×200 passano da ~0,9 s a ~12 s, e un batch di cento partite
per il bilanciamento automatico da un minuto e mezzo a venti minuti. Il requisito 2 di
`CLAUDE.md` — il gioco è pilotabile da un'AI che gira molte partite — è quello che paga.

Non si ottimizza dentro M1: il costo va **misurato e attribuito** (fasi 14 e 18) e le
contromisure sono [A17](#a17--il-costo-del-ricalcolo-per-tick), da chiudere prima di M2. È la
lezione di A11 applicata prima invece che dopo: l'ipotesi ovvia su dove sia il costo è già stata
sbagliata una volta.

C'è anche un costo che non si misura in microsecondi. `equivalenza_copertura` confronta a **fine
tick**, cioè dopo che il passo 6 ha mosso la popolazione: con la demografia attiva diverge
sempre, e per costruzione. Va spezzato in due — la forma originale su un dataset a tassi zero,
più un test diretto sul contratto di invalidazione — e le due metà insieme verificano meno di
quanto verificasse l'originale. Il presidio del passo 3 si indebolisce, ed è il motivo per cui
A17 va chiusa prima che qualcuno ci ottimizzi sopra.

### Cosa compra

La dinamica di gioco per cui esiste. I servizi si costruiscono **dietro** la popolazione, non
davanti: la città cresce, supera i propri pozzi, la copertura comincia a mancare a qualcuno, la
soddisfazione cala, il giocatore vede il problema e costruisce. L'overshoot è il segnale, non un
guasto — ed è il ciclo che rende il gioco un gioco invece di un pianificatore.

Due conseguenze tecniche, entrambe favorevoli:

- **Il conto del cibo diventa stretto.** Capacità e consumo sono ora nella stessa unità, quindi
  `capacita × consumo ≤ produzione` è esatto invece che un maggiorante. Nessuna produzione
  sprecata su case mezze vuote, e `capacita_cibo_insostenibile` non cambia una riga.
- **L'evoluzione non tocca la copertura.** Salire di livello non porta gente, porta il permesso
  di ospitarne di più: non serve nessun gate sull'evoluzione, e la fase 13 è più semplice di
  quanto sembrasse.

### L'alternativa scartata, e perché la si è guardata

Contare la capacità in **posti** — la capienza del livello, occupata o no. La copertura
tornerebbe una funzione di grandezze che cambiano di rado (griglia, edifici, livelli), quindi
`equivalenza_copertura` resterebbe intatto senza modifiche, l'invalidazione tornerebbe rara, e
il tick a vuoto resterebbe a 248 µs. Tecnicamente è l'opzione migliore su ogni asse.

È stata scartata perché rovescia la sensazione di gioco: con i posti, un pozzo si riempie di
case vuote e il giocatore deve dimensionare i servizi **prima** che la gente arrivi. Il gioco
diventa un esercizio di pianificazione anticipata invece che di reazione. Fra un vincolo tecnico
e una dinamica di gioco ha vinto la dinamica, e il vincolo è diventato A17.

Da tenere presente se un giorno A17 non si chiudesse: il passaggio ai posti è
`rules.capienza(casa.level)` al posto di `casa.abitanti` in `coverage.rs`, cioè una riga. È la
via d'uscita, e costa esattamente la dinamica che si era voluta.

### Un'altra cosa che non si fa: la copertura appiccicosa

Dare priorità alle case già servite ridurrebbe il churn — con la capacità sugli abitanti, la
casa marginale entra ed esce ogni volta che la domanda attraversa la soglia. Ma renderebbe la
copertura dipendente dalla **storia**, e `calcola_da_zero` non ha storia: `equivalenza_copertura`
cadrebbe del tutto invece che a metà. Il churn si smorza con l'inerzia della soddisfazione
(A10), che è già lì.

---

## A13 — La difficoltà di gioco è un asse dello stato

Serve a rispondere a una domanda che la demografia pone: **una casa appena costruita ha
abitanti?** Se nasce piena, il giocatore non vede mai la parte difficile del gioco; se nasce
sempre vuota, i primi anni sono lentissimi.

**Decisione:** dipende dalla difficoltà. Tabella di profili in `sim-data/data/difficolta.ron`,
`DifficoltaId` nel `World`, id **testuale** nell'header del replay, e il profilo dentro
entrambi gli hash.

Non è un enum: la difficoltà è dato (D6), e uno scenario o una civiltà potranno dichiararne di
propri senza toccare il codice. Nell'header va l'id testuale e non l'indice, perché un `.ron`
golden con `difficolta: 1` non si legge e riordinare la tabella cambierebbe in silenzio il
significato di ogni salvataggio già scritto.

**Va fatta presto** — [fase 11](11-difficolta.md), subito dopo A12 — per lo stesso argomento
dei `DirtyFlags`: tocca `World::new`, l'`Header` e `hash_world`, cioè le tre cose che
rigenerano i golden, e farla tardi li rigenera due volte. Nasce con **un solo knob**
(`abitanti_iniziali_casa`) e le fasi 14, 15 e 17 ci appendono i loro senza toccare più né
l'header né la firma di `World::new`.

*Se si decide diversamente* — la difficoltà come parametro dello scenario e non dello stato —
il replay non la porta con sé, e due partite con lo stesso `.ron` possono divergere. È
esattamente ciò che D4 vieta.

---

## A14 — Demografia a quattro flussi, aggregata, con randomicità limitata

D5 dice che l'unità di simulazione è la casa e non l'individuo, e D3 dice che gli immigranti
sono walker veri. Le due cose insieme lasciano aperto *quanto* grossolana debba essere la
demografia di M1.

**Decisione:** quattro flussi — nascite legate al benessere complessivo della città, morti,
immigrazione, emigrazione — tutti **aggregati**. I walker immigranti restano M3: ciò che M3
aggiungerà è il **tempo di viaggio**, non la regola di attrattività. È la stessa forma di A5
per la fattoria, e come quella va annotata dove sarà chi legge in M3 a trovarla.

**Sulla randomicità.** Un tasso base calcolato più un jitter estratto da RNG seedato, non una
formula pura e non un dado per casa. Il determinismo del replay resta assoluto (D4): stesso
seed, stesso risultato bit a bit. La formulazione da tenere è ***la randomicità è nella scelta
del seed, non nell'esecuzione***.

Due estrazioni per flusso e non 3.750: il jitter sul tasso, una volta per tick, e la scelta
della casa su cui l'evento cade. Non è per il costo — è che "tasso casuale, distribuzione
deterministica" è più facile da spiegare, da bilanciare e da leggere in un golden.

**Due domini RNG, non uno**: `Demografia` per nascite e morti, `Migration` — che esiste dalla
fase 02 e non ha mai fatto niente — per i flussi migratori. Separarli fa sì che scrivere la
fase 16 non sfasi la sequenza della 15. È il caso d'uso per cui `RngDomain` è stato progettato,
e in quattro fasi di M0 non si era mai presentato.

**La trappola, che vale più della decisione.** `rand::Rng::random_range` usa campionamento a
rigetto: il numero di `next_u64()` consumati dipende dai valori estratti, quindi dal seed.
Allora `rng.draws(d)` smette di essere funzione dello stato di gioco — cioè smette di servire
all'hash canonico, che è il motivo per cui la fase 02 ce l'ha messo — e
`seed_diversi_danno_hash_diversi` passerebbe **anche se la demografia non facesse
assolutamente nulla**. Serve un'estrazione a costo fisso e il test che lo fissa
(`il_numero_di_estrazioni_non_dipende_dal_seed`), scritti *prima* delle nascite.

---

## A15 — Attrattività: due termini, più l'aliquota una fase dopo

**Decisione:** l'attrattività della città è la soddisfazione media pesata sugli abitanti più i
posti liberi in case **servite**, e l'aliquota fiscale entra come terzo termine **solo nella
fase 17**, non nella 16 che introduce la migrazione.

Il rinvio è deliberato: mettere l'aliquota nell'attrattività insieme alla migrazione
significherebbe bilanciare due meccaniche in un colpo solo, e quando la curva non torna non si
sa quale delle due sistemare. Le fasi non si accoppiano.

Il gate sui posti liberi è **duro**: zero posti in case servite ⇒ zero immigrazione, a
qualunque attrattività. Ne segue la parte più economica di A10: *"i migranti vanno solo dove si
sta bene"* non è un meccanismo separato, è il filtro dell'insieme eleggibile — le stesse case
che l'evoluzione considera degne, per la stessa ragione. Un meccanismo in meno e un numero di
bilanciamento in meno.

L'attrattività è una funzione **pura**, senza RNG: il jitter sta nel flusso, non nell'indice.
Così si può stampare nel dump, confrontare fra due partite, ed è già nella forma che
l'osservazione semantica per l'LLM (M3) vorrà — un indicatore aggregato, non stato
serializzato.

*Da tenere d'occhio:* la media pesata divide per la popolazione, che può essere zero. Una città
vuota è uno stato normalissimo e `nessun_panic_su_diecimila_comandi` ne produce di continuo.

---

## A16 — Gli obiettivi vivono nel `World`, `sim-scenario` li costruisce

`CLAUDE.md` mette la verifica degli obiettivi come passo 9 del tick, dentro `step`. Ma le
dipendenze puntano verso `sim-core`, quindi `sim-scenario` dipende dal core e il core non può
chiamarlo.

**Decisione, ed è la stessa forma di A2:** l'enum `Objective`, la sua valutazione e lo stato di
avanzamento stanno in `sim-core` — sono struct pure senza I/O e appartengono al vocabolario del
core, come `ServiceKind`. In `sim-scenario` restano la definizione degli scenari, il
caricamento da file e la composizione di obiettivi obbligatori e opzionali.

È la seconda volta che il grafo delle dipendenze detta il confine, e vale la pena scriverlo:
chi legge deve riconoscere il pattern invece di riscoprirlo.

Il corollario che conta: tenendo gli obiettivi nel `World`, il **tick di completamento entra
nell'hash canonico**. "Lo scenario si dichiara completato al tick giusto" diventa allora un
golden che diverge se il tick cambia, invece di un `assert` che qualcuno deve ricordarsi di
scrivere — ed è ciò che rende lo scenario della fase 18 il canarino sul bilanciamento che
`CLAUDE.md` chiede al punto 4 del Testing, nella versione che M1 può avere prima del bot di M2.

*Alternativa scartata:* passare gli obiettivi a `step` come parametro. Cambia la firma
dichiarata in D4 e lascia il loro stato fuori dall'hash, cioè fuori dai golden.

---

## A17 — Il costo del ricalcolo per tick — **APERTA**, da chiudere prima di M2

L'unica decisione aperta del progetto, e non è un caso: è il debito che
[A12](#a12--i-servizi-rincorrono-la-popolazione) apre di proposito.

Con i servizi che rincorrono la popolazione, la copertura si ricalcola quasi a ogni tick invece
che solo quando il giocatore costruisce. Il tick tipico passa da 248 µs a ~3,3 ms — **~13×** — e
quel costo si paga su ogni partita, quindi su ogni batch di bilanciamento automatico
(`CLAUDE.md`, requisito 2). È stato accettato per comprare una dinamica di gioco, non per
distrazione.

**Perché resta aperta invece di essere risolta in M1.** Perché A11 è appena successa: l'ipotesi
ovvia su dove fosse il costo del passo 3 era sbagliata, e a dirlo è stata la misura. Fare
l'ottimizzazione nella stessa fase in cui si prende la misura significa non avere il *prima*.
`CLAUDE.md` è esplicito — *non ottimizzare prima del profiler* — e questa volta il profiler
arriva alla fine di M1.

**Cosa serve per chiuderla.** I numeri delle fasi 14 e 18, che vanno riportati qui:

- `H` — tick a vuoto con la demografia spenta. Deve coincidere con i 248 µs di fine M0; se non
  coincide, il costo non è il ricalcolo ma il passo 6 in sé.
- `I` — il solo passo 6, che separa il lavoro demografico dal ricalcolo che innesca.
- `J` — frazione di tick in cui la popolazione si è mossa, su una partita intera. È il
  moltiplicatore vero: il costo di A12 è `J × G`, non `G`. In una città satura `J` è basso.

**Le contromisure candidate, in ordine di rapporto fra resa e rischio.**

1. **Invalidazione mirata invece che globale.** Oggi qualunque cambiamento chiama
   `segna_tutti_i_provider`. Se cambiano gli abitanti di una casa, i soli provider da rivedere
   sono quelli che la servono e quelli che potrebbero servirla — un insieme piccolo. La lista
   `DirtyFlags::coverage` esiste dalla fase 04 apposta e **non è mai stata letta**: è il momento
   per cui era stata scritta.
2. **Ricalcolo a soglia.** Non invalidare per ogni singolo abitante che si muove, ma quando lo
   scostamento accumulato presso un provider supera una soglia. Cambia la semantica di gioco —
   la copertura reagisce con ritardo — quindi va deciso come regola, non come ottimizzazione, e
   messo in tabella.
3. **La cache delle candidate di A11**, ora con una ragione in più: il BFS dipende da strade,
   posizione e raggio, non dalla popolazione, quindi con A12 diventa la parte *interamente*
   ricalcolabile a vuoto. Le tre trappole già scritte in A11 valgono identiche.
4. **Il passaggio ai posti**, cioè ribaltare A12. È una riga in `coverage.rs` e riporta il costo
   a quello di M0, al prezzo della dinamica di gioco. È la via d'uscita, non la soluzione, e va
   considerata solo se le prime tre non bastano.

**Attenzione a una cosa che A12 ha indebolito.** `equivalenza_copertura` non è più l'oracolo
completo che era: gira su un dataset a tassi zero, e il contratto sulla popolazione è un test
separato. Chi lavorerà su questa decisione deve tenere verdi **entrambi**, e vale la pena
chiedersi se prima non convenga ricostruire un oracolo unico — per esempio confrontando la
copertura con `calcola_da_zero` nel punto del tick in cui è appena stata calcolata, invece che
a fine tick. Sarebbe il primo lavoro da fare, prima di toccare qualunque prestazione.
