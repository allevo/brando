# Oltre M0 — tracciato, non pianificato

Questo file **non** è un piano. È l'elenco di ciò che M0 deve rendere possibile, con le domande
a cui M0 stesso darà risposta. Pianificare M1 in dettaglio adesso significherebbe decidere sul
bilanciamento prima di aver visto un tick girare, e ogni numero deciso in astratto va poi
rifatto.

## M1 — Loop di gioco minimo

Fasi plausibili, nell'ordine in cui probabilmente conviene farle:

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

- Il passo 3 (copertura) è davvero l'hot path, o lo è il rebuild della rete? Lo dirà
  `bench-smoke` della fase 09, non il ragionamento.
- ~~La capacità dei servizi ha senso in "case servite" o in "abitanti serviti"?~~ **Risposto:**
  in abitanti, e la risposta è arrivata prima di M1 perché era il prerequisito della fase 1.
  La parte che contava non era l'unità ma il ribilanciamento che l'accompagna — la capacità di
  un produttore dev'essere ciò che la sua produzione sostiene. Dettagli in
  [A5](decisioni-aperte.md).
- 30 tick/mese ([A6](decisioni-aperte.md)) dà una curva giocabile? Lo dirà il primo scenario con
  un obiettivo temporale.
- Un `Tile` di 4 byte basta quando arriveranno i walker e i magazzini? Il test di budget della
  fase 01 lo segnalerà nel momento in cui non basta più.
