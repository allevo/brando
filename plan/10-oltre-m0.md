# Oltre M0 — tracciato, non pianificato

Questo file **non** è un piano. È l'elenco di ciò che M0 deve rendere possibile, con le domande
a cui M0 stesso darà risposta. Pianificare M1 in dettaglio adesso significherebbe decidere sul
bilanciamento prima di aver visto un tick girare, e ogni numero deciso in astratto va poi
rifatto.

## M1 — Loop di gioco minimo

> **Pianificata il 2026-08-09**, dopo la chiusura di M0 e le ottimizzazioni del passo 3. Le fasi
> vere sono [11](11-difficolta.md)–[18](18-invarianti-chiusura-m1.md), elencate in
> [README.md](README.md). Questa sezione resta com'era scritta: confrontarla con il piano vero è
> l'unico modo di sapere quanto valga tracciare in anticipo.
>
> **Cosa ha retto.** L'ordine — livelli → migrazione → tasse → scenario — e il fatto che il
> prerequisito dei livelli fosse la capacità dei servizi, che infatti era già stata sistemata
> ([A5](decisioni-aperte.md)).
>
> **Cosa no.** Tre cose, tutte emerse pianificando e nessuna tracciabile prima.
>
> 1. Il prerequisito **non era finito**, e il seguito non era una questione tecnica ma di gioco.
>    Con la popolazione che cambia a ogni tick, la capacità contata sugli abitanti presenti fa
>    invecchiare la copertura entro il tick stesso: `equivalenza_copertura` cade, e cade sia in
>    crescita sia in calo. Contarla in **posti** — la capienza del livello, occupata o no —
>    avrebbe risolto tutto senza costi, ma avrebbe imposto di dimensionare i servizi *prima*
>    della popolazione. Si è scelto il contrario: [A12](decisioni-aperte.md), **i servizi
>    rincorrono la popolazione**, con il ricalcolo a ogni tick e il suo prezzo (~13× sul tick a
>    vuoto). Da lì nasce [A17](decisioni-aperte.md), l'unica decisione aperta del progetto.
> 2. Le cinque fasi sono diventate **otto**. "Livelli delle case" e "migrazione" erano due righe
>    che ne contenevano quattro fasi; e mancava del tutto la fase di chiusura, che in M0 era la
>    09 ed è dove le lezioni si scrivono invece di ricordarsele.
> 3. Gli **eventi casuali sono usciti** da M1 — vedi sotto.

Come era stato tracciato, prima di pianificarla:

1. **Livelli delle case.** `livelli` nelle tabelle diventa > 1; requisiti di servizio per
   livello. Le case evolvono se servite per N tick consecutivi, degradano altrimenti.
   Goal verificabile: una casa servita da acqua e cibo raggiunge il livello 2 in un numero di
   tick calcolabile dal `DataSet`; togliendo l'acqua torna al livello 1.
   Il prerequisito — capacità dei servizi in abitanti anziché in case — è stato fatto prima,
   subito dopo M0: vedi [A5](decisioni-aperte.md). La forma della soddisfazione (un accumulatore
   di tempo per servizio, con isteresi, **non** un livello di risorsa) è decisa in
   [A10](decisioni-aperte.md). Qui `servizi_richiesti` smette di essere dato dichiarativo e
   inizia a essere letto ([A9](decisioni-aperte.md)).
2. **Migrazione.** Primo uso reale di `RngDomain::Migration` — ed è il tick in cui il test 7
   della fase 08 (sensibilità al seed) si riattiva e deve passare.
   Goal: una città attraente cresce, una città affamata si spopola, e due seed diversi danno
   traiettorie diverse ma entrambe plausibili.
   Aggregata, non per individui: il flusso netto lo decide un indice di attrattività cittadino
   ([A10](decisioni-aperte.md)), perché gli immigranti come walker veri sono M3 (D3).
3. **Tesoro e tasse.** Passo 7 del tick. Goal: l'invariante del tesoro della fase 09 si estende
   alle entrate e resta un'uguaglianza esatta.
4. **`sim-scenario`.** Obiettivi obbligatori e opzionali, condizioni di vittoria, passo 9 del
   tick. Scenario 1: "500 abitanti entro 5 anni" (D7).
   Goal: lo scenario si dichiara completato al tick giusto, e non prima.
5. **Eventi casuali** (incendi, malattie): passo 8. Il più rischioso per il determinismo, quindi
   l'ultimo — quando i golden replay sono maturi e coprono già molto.

Ogni fase aggiunge righe ai golden esistenti e, se introduce una meccanica osservabile, un
nuovo scenario golden.

## Eventi casuali — fuori da M1, non ancora pianificati

Incendi, malattie, invasioni: il passo 8, che a fine M1 resterà l'unico vuoto insieme al 5
(walker logistici, M3). Primo uso di `RngDomain::Events`.

Fuori da M1 per tre motivi: `CLAUDE.md` non li elenca fra le meccaniche di quella milestone,
sono i più rischiosi per il determinismo, e la ragione per cui sembravano necessari —
riattivare `seed_diversi_danno_hash_diversi` — è già coperta dalla demografia.

Quando arriveranno, una lezione della [fase 14](14-nascite-morti.md) vale già scritta: il
**numero** di estrazioni non deve dipendere dal seed, o `rng.draws()` smette di essere funzione
dello stato e l'hash canonico smette di dire qualcosa. `Stream::sotto` esisterà già.

## Fra M1 e M2 — il debito di A12

Una cosa sola, ed è [A17](decisioni-aperte.md): il ricalcolo della copertura a ogni tick, che è
il prezzo di [A12](decisioni-aperte.md). Il tick tipico passa da 248 µs a ~3,3 ms, e il costo si
paga su ogni partita — quindi su ogni batch di bilanciamento automatico, che è il requisito 2 di
`CLAUDE.md`.

Va chiusa **prima** di M2 e non dentro M1, per la ragione che A11 ha appena dimostrato: si
misura in una fase e si ottimizza in un'altra, o non si ha il *prima*. La fase 18 produce i
numeri (`H`, `I`, `J`); A17 elenca le contromisure candidate in ordine di rapporto fra resa e
rischio, e la prima è finalmente leggere la lista `DirtyFlags::coverage`, che esiste dalla fase
04 e non è mai servita a nessuno.

Da fare nello stesso lotto, e prima di toccare le prestazioni: **ricostruire un oracolo unico
per il passo 3**. A12 ha spezzato `equivalenza_copertura` in due metà che insieme verificano
meno dell'originale, e ottimizzare con una rete a maglie larghe è il modo di introdurre un bug
di correttezza invece che di lentezza.

## M2 — I due client

Il punto di M2 non è la grafica: è **validare il confine snapshot/evento** della fase 07.
Bevy riceve uno snapshot al primo frame e poi solo delta. Se per renderizzare serve leggere il
`World` ogni frame, il confine è progettato male e si scopre qui — che è il momento giusto.

In parallelo e indipendente: bot euristico + evaluator, che chiudono il punto 4 del Testing
(un bot completa lo scenario 1 entro N mesi: il canarino sul bilanciamento).

## M3 — Profondità

Catene produttive con walker logistici reali (D3), `CivilizationRules` **insieme** alla seconda
civiltà (D6), adattatore LLM sopra un bot già funzionante.

Qui va rivista [A5](decisioni-aperte.md): la fattoria come provider di copertura è una
semplificazione di M0, e in M3 la merce arriverà da un magazzino trasportata da walker veri.
La copertura resta, cambia da dove viene la merce.

## Domande a cui risponderà M0 (e che ora non hanno risposta utile)

- ~~Il passo 3 (copertura) è davvero l'hot path, o lo è il rebuild della rete?~~ **Risposto:**
  il passo 3, e di molto — `G` vale 19,83 ms su 20,05 di `D`. Ma la risposta utile è arrivata
  dopo: il costo non era il numero di BFS, era ciò che ognuno si portava dietro. Toglierlo ha
  dato 6,5× senza stato nuovo, e la cache che sembrava ovvia non si fa
  ([A11](decisioni-aperte.md)). Il numero da guardare adesso è `A`, il tick a vuoto.
- ~~La capacità dei servizi ha senso in "case servite" o in "abitanti serviti"?~~ **Risposto:**
  in abitanti, e la risposta è arrivata prima di M1 perché era il prerequisito della fase 1.
  La parte che contava non era l'unità ma il ribilanciamento che l'accompagna — la capacità di
  un produttore dev'essere ciò che la sua produzione sostiene. Dettagli in
  [A5](decisioni-aperte.md).
- 30 tick/mese ([A6](decisioni-aperte.md)) dà una curva giocabile? Lo dirà il primo scenario con
  un obiettivo temporale.
- Un `Tile` di 4 byte basta quando arriveranno i walker e i magazzini? Il test di budget della
  fase 01 lo segnalerà nel momento in cui non basta più.
