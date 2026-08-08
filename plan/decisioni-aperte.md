# Decisioni aperte

Scelte necessarie per implementare M0 che `CLAUDE.md` non fissa. Per ognuna: la
raccomandazione che le fasi assumono, e cosa cambia se si decide diversamente.

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

---

## A6 — Ritmo del tempo

`CLAUDE.md` fissa 1 tick = 1 giorno e il mese come multiplo fisso, senza dare il numero.

**Raccomandazione:** 30 tick = 1 mese, 12 mesi = 1 anno (360 tick). Il valore vive in
`sim-data` (`rules.ron`), non nel codice: gli obiettivi di scenario si esprimono in mesi
e servono a M1.
