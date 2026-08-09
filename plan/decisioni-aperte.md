# Decisioni aperte

Scelte necessarie per implementare M0 che `CLAUDE.md` non fissa. Per ognuna: la
raccomandazione che le fasi assumono, e cosa cambia se si decide diversamente.

Da A7 in poi il file continua oltre M0: sono decisioni prese dopo la chiusura, con la stessa
forma. Chi cerca "perché il codice fa così e non cosà" trova qui la risposta.

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

Sono nate smarcando `dubbi.md` (2026-08-09): tre erano domande di design, e le risposte
valevano più della domanda. A9 e A10 sono prerequisiti di M1 fase 1–2.

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
