# Fase 05 — Strade e rete stradale

**Goal:** la rete stradale si ricostruisce solo quando è `dirty`, e la sua etichettatura è
indipendente dall'ordine in cui le strade sono state costruite.
**Dipende da:** 04.
**Dimensione:** M.
**Decisioni coinvolte:** D2 (distanza su strada), D4 (determinismo), ordine del tick passo 2.

## Perché adesso

La copertura dei servizi (fase 06) è definita sulla distanza percorsa in rete. Senza una
struttura di rete la fase 06 finirebbe per calcolare distanze in linea d'aria "per adesso", che
è precisamente ciò che D2 vieta, e il bilanciamento successivo sarebbe costruito su un raggio
sbagliato.

## Cosa si costruisce

```rust
/// Struttura derivata: ricostruibile in qualunque momento da Grid.
/// Non entra nell'hash canonico dello stato — se ci entrasse, un bug nella
/// ricostruzione incrementale si mostrerebbe come divergenza di hash invece
/// che come test di equivalenza fallito (fase 06, test 6).
pub struct RoadNetwork {
    /// Per ogni tile: id del componente connesso, o NONE se non è strada.
    component: Vec<ComponentId>,
    /// Numero di ricostruzioni eseguite. Solo per i test (vedi test 4).
    rebuilds: u32,
}
```

`rebuild_roads(world)` (passo 2 del tick): se `!dirty.roads` ritorna immediatamente; altrimenti
riesegue l'etichettatura completa con BFS scandendo i tile in ordine di `TileIdx` crescente.

**Etichettatura canonica.** L'id di un componente è il `TileIdx` minimo tra i suoi tile, non un
contatore incrementale. Costa uguale (la scansione è già in ordine crescente) e rende
l'etichettatura una funzione del solo insieme di strade, non della storia degli inserimenti.
Senza questo, due partite che costruiscono le stesse strade in ordine diverso hanno stati
diversi e il test 3 è impossibile da scrivere.

Ricostruzione completa e non incrementale, deliberatamente: 40.000 tile scanditi una volta sono
irrilevanti finché non lo dice un profiler, e il `dirty` flag già evita di farlo ogni tick.
Quello che serve subito è il *flag*, non l'algoritmo furbo.

**Aggancio degli edifici alla rete.** Un edificio è connesso se almeno un tile del suo footprint
è 4-adiacente a un tile strada. La regola va in un doc comment: è semantica di gioco (in Zeus
conta l'ingresso, non l'edificio) e in M1 potrebbe diventare "un tile d'ingresso designato".

```rust
/// Distanza in tile percorsi sulla rete, da un edificio a un altro.
/// None se non connessi o oltre `max`. BFS troncato: senza il taglio,
/// un raggio 12 su una città grande visiterebbe tutta la rete.
pub fn road_distance(&self, from: BuildingId, to: BuildingId, max: u16) -> Option<u16>;
```

## Fuori scope

Livelli di strada (sterrata/lastricata), blocchi, senso di marcia, pathfinding per walker
(M3: i walker logistici useranno questa stessa rete ma vogliono un cammino, non una distanza).
Nessun costo di attraversamento variabile: ogni tile strada costa 1.

## Test

1. **Componenti**: due gruppi di strade separati ⇒ due componenti distinti. Aggiungendo il tile
   che li congiunge ⇒ un solo componente. Rimuovendolo ⇒ di nuovo due.
2. **Adiacenza corretta**: due strade in diagonale **non** sono connesse (solo 4-adiacenza).
   Una strada a x=0 e una a x=width-1 sulla stessa riga non sono connesse (niente wraparound —
   è il bug della fase 01 che si ripresenta a un livello più alto).
3. **Indipendenza dall'ordine** (property test): dato un insieme di posizioni di strada, due
   permutazioni dei `PlaceRoad` corrispondenti producono la **stessa** etichettatura
   `component`. Questo test è il motivo dell'etichettatura canonica.
4. **Il dirty flag funziona** (property test): dopo un `rebuild`, N tick senza comandi di strada
   lasciano `rebuilds` invariato; un solo `PlaceRoad` lo incrementa di esattamente 1, non di più
   (regressione contro il caso in cui qualcuno segna dirty dentro il loop).
5. **`road_distance`**: su un corridoio rettilineo di 10 tile la distanza è quella attesa; con
   `max = 5` restituisce `None` oltre; su una L la distanza è la lunghezza del percorso, **non**
   quella euclidea (è il test che documenta D2); edifici non connessi ⇒ `None`.
6. **Simmetria**: `road_distance(a, b) == road_distance(b, a)`. Vale in M0 (grafo non orientato)
   e va scritto: il giorno in cui si introdurrà un senso di marcia, questo test fallirà e
   costringerà a decidere consapevolmente.

## Verifica

```sh
cargo test -p sim-core strade
PROPTEST_CASES=2000 cargo test -p sim-core --release rete
```

**Fatto quando:** i test 3 e 4 passano — indipendenza dall'ordine e ricostruzione solo se dirty.
Gli altri sono correttezza di base; questi due sono il goal della fase.
