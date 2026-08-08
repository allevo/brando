# Decisioni aperte

Scelte necessarie per implementare M0 che `CLAUDE.md` non fissa. Per ognuna: la
raccomandazione che le fasi assumono, e cosa cambia se si decide diversamente.

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
