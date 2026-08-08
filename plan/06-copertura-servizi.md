# Fase 06 — Copertura servizi aggregata

**Goal:** il pozzo serve le case entro il raggio **percorso su strada**, con capacità limitata e
assegnazione deterministica; il risultato del calcolo incrementale coincide con quello calcolato
da zero.
**Dipende da:** 05.
**Dimensione:** L — è il passo 3 del tick, l'hot path del progetto.
**Decisioni coinvolte:** D2 (copertura aggregata, walker decorativi), D4, D5.

## Perché adesso

È il cuore di D2 e la meccanica su cui poggia tutto M1 (le case evolvono con acqua e cibo).
Ed è il punto in cui è più facile scivolare verso i walker di servizio: se la copertura non
esiste, la tentazione di simulare un portatore d'acqua è forte, e a quel punto D2 è violata in
modo irreversibile.

## Cosa si costruisce

`propagate_coverage(world)`, passo 3 del tick.

```rust
/// Per ogni servizio, chi lo serve. Struttura derivata, come RoadNetwork.
pub struct Coverage {
    /// servita[house][service] = Some(provider) | None
    served_by: Vec<[Option<BuildingId>; ServiceKind::COUNT]>,
}
```

Algoritmo, per ogni provider in `dirty.coverage`:

1. BFS sulla rete stradale dai tile strada adiacenti al provider, con taglio a
   `raggio_per_livello[level]` (dal `DataSet`, mai una costante nel codice).
2. Raccogli le case i cui tile sono 4-adiacenti a un tile strada visitato, con la distanza a cui
   sono state raggiunte.
3. Assegna fino a `capacita_per_livello[level]` case.

**Regola di assegnazione — è semantica di gioco, va documentata e non cambiata alla leggera.**
Quando le case candidate superano la capacità, si servono le più vicine; a parità di distanza,
quella con `TileIdx` minore. Ordinamento su `(distanza, TileIdx)`, che è un ordine totale: senza
il secondo criterio due case equidistanti sarebbero ordinate dall'ordine di visita del BFS,
cioè da un dettaglio implementativo, e il golden replay diventerebbe fragile.

**Contesa tra provider.** Due pozzi che coprono la stessa casa: in M0 la casa è servita e basta
(il campo è `Option<BuildingId>`, vince il primo che la prende nell'ordine di iterazione dei
provider — che è l'ordine di `BuildingId`, deterministico). Da annotare come semplificazione: se
in M1 la capacità dovesse diventare "abitanti serviti" invece di "case servite", questo punto va
riaperto.

> **Aggiornato dopo M0.** È successo: la capacità si conta in abitanti, e con essa sono arrivate
> due regole che questa fase non aveva — nessuna assegnazione parziale, e chi non entra nella
> capacità residua viene *saltato* invece di fermare la scansione. La contesa continua a vincerla
> il primo provider, ma una casa contesa non consuma più la capacità di chi arriva secondo.
> Vedi [A5](decisioni-aperte.md) e `coverage::scelte_entro_capacita`.

**`dirty.coverage` in M0 può essere ricalcolato in modo ingenuo**: quando cambiano le strade,
tutti i provider sono dirty. `CLAUDE.md` autorizza l'ingenuità qui, purché i flag esistano — ed
esistono dalla fase 04. Ciò che non va rimandato è il test 6.

### Cosa non fa

Non produce walker. I portatori d'acqua che il giocatore vedrà sono decorativi, vivono nel
renderer e sono derivati da `Coverage` (D2). Se un `Walker` compare in questa fase, è un bug —
vale la pena un commento in testa al modulo che lo dica.

## Fuori scope

Effetti della copertura (evoluzione, degrado, insoddisfazione): sono M1. Questa fase **registra**
chi è servito, non ne trae conseguenze. Il cibo che si consuma è la fase 07.

## Test

1. **Caso base**: pozzo, strada, casa a distanza 3 su strada, raggio 12 ⇒ servita.
2. **Fuori raggio**: casa a distanza 13, raggio 12 ⇒ non servita. Con raggio 13 ⇒ servita
   (verifica che il confine sia inclusivo, e lo fissa).
3. **Distanza su strada, non in aria**: casa a 2 tile in linea d'aria dal pozzo ma raggiungibile
   solo con un giro di 20 tile, raggio 12 ⇒ **non servita**. È il test che incarna D2; se questo
   passa, l'implementazione non ha scorciatoie euclidee.
4. **Serve la strada**: casa non adiacente a nessun tile strada ⇒ mai servita, a qualunque
   distanza.
5. **Capacità**: capacità 2 e tre case candidate a distanze 3, 5, 7 ⇒ servite quelle a 3 e 5. A
   parità di distanza, vince il `TileIdx` minore. Test tabellare esplicito: è la regola di gioco.
6. **Equivalenza incrementale ↔ da zero** (property test, *il* test della fase): data una
   sequenza casuale di comandi (strade, case, pozzi, demolizioni), la `Coverage` ottenuta con i
   dirty flag è **identica** a quella ricalcolata da zero sullo stato finale. Questo test è
   ciò che rende sicura l'ottimizzazione del passo 3 quando arriverà: qualunque furbizia futura
   sull'incrementalità è coperta.
7. **Demolizione**: demolito il pozzo, le case tornano non servite nello stesso tick.
   Demolita una strada che spezza la rete, le case oltre la rottura tornano non servite.
8. **Idempotenza**: due tick consecutivi senza comandi lasciano `Coverage` invariata e non
   ricalcolano nulla (`rebuilds`/contatore di BFS invariato).

## Verifica

```sh
cargo test -p sim-core copertura
PROPTEST_CASES=1000 cargo test -p sim-core --release equivalenza_copertura
```

**Fatto quando:** passano il test 3 (distanza su strada) e il test 6 (equivalenza
incrementale/da zero). Sono i due che definiscono la fase; i restanti sono correttezza di
contorno.
