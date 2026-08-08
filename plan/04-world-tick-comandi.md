# Fase 04 — World, tick, comandi

**Goal:** `step()` avanza il tick, applica i comandi validi, rifiuta gli invalidi con un errore
strutturato, e non panica su 10.000 comandi casuali.
**Dipende da:** 01, 02, 03.
**Dimensione:** M.
**Decisioni coinvolte:** D1, D4, D5, A2. Testing punto 3 (fuzzing).

## Perché adesso

È la fase in cui il progetto diventa eseguibile. Dopo questa fase esiste un `World` che si
avanza e si ispeziona, e ogni fase successiva aggiunge **un passo** all'ordine del tick già
scritto.

## Cosa si costruisce

### Stato

```rust
pub struct World {
    tick: u32,
    grid: Grid,
    buildings: SlotMap<BuildingId, Building>,
    houses: SlotMap<HouseId, House>,
    walkers: Vec<Walker>,          // vuoto in M0: i walker reali sono M3 (D3)
    economy: Economy,              // { tesoro: Coins }
    rng: RngSet,
    dirty: DirtyFlags,
    data: Arc<DataSet>,            // A2
}

pub struct Building { kind: BuildingKindId, origin: TilePos, level: u8, stock: Milli }
pub struct House    { origin: TilePos, level: u8, abitanti: u16, servita: ServiceFlags }
```

`House` esiste già qui, anche se in M0 non evolve: è l'unità di simulazione della popolazione
(D5) e le fasi 06–07 hanno bisogno di qualcosa da servire. In M0 gli abitanti sono un valore
fisso dalle `rules`, la migrazione è M1.

### DirtyFlags — fin da subito

```rust
/// I flag esistono da subito per scelta esplicita: retrofittarli dopo è doloroso
/// (CLAUDE.md, ordine del tick). In M0 l'uso è ingenuo, la struttura è quella definitiva.
pub struct DirtyFlags {
    roads: bool,
    /// Provider la cui copertura va ricalcolata. Vec ordinato, non HashSet (D4).
    coverage: Vec<BuildingId>,
}
```

### Comandi

```rust
pub enum Command {
    PlaceRoad     { at: TilePos },
    PlaceBuilding { kind: BuildingKindId, origin: TilePos },
    Demolish      { at: TilePos },
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("posizione fuori dalla mappa: {0:?}")]            OutOfBounds(TilePos),
    #[error("tile già occupato da {occupant:?}")]             TileOccupied { .. },
    #[error("terreno non edificabile: {0:?}")]                UnsuitableTerrain(Terrain),
    #[error("fondi insufficienti: servono {needed}, disponibili {available}")]
                                                             InsufficientFunds { needed: Coins, available: Coins },
    #[error("tipo di edificio sconosciuto: {0:?}")]           UnknownBuildingKind(BuildingKindId),
    #[error("niente da demolire in {0:?}")]                   NothingToDemolish(TilePos),
}
```

I messaggi non sono cosmetici: sono il feedback che tornerà all'LLM (M3). Vale la pena scriverli
bene ora, sono gratis.

### Il tick

```rust
pub fn step(world: &mut World, cmds: &[Command]) -> StepReport {
    let mut r = StepReport::default();
    apply_commands(world, cmds, &mut r);   // 1
    rebuild_roads(world);                  // 2  -> fase 05
    propagate_coverage(world);             // 3  -> fase 06 (hot path)
    production(world);                     // 4  -> fase 07
    step_walkers(world);                   // 5  -> M3
    houses_and_migration(world);           // 6  -> M1
    finance(world);                        // 7  -> M1
    random_events(world);                  // 8  -> M1
    check_objectives(world, &mut r);       // 9  -> M1
    emit_events(world, &mut r);            // 10 -> fase 07 (minimale)
    world.tick += 1;
    r
}
```

**Tutte e dieci le funzioni esistono da subito**, anche vuote con un commento che dice in quale
fase si riempiono. L'ordine è semantica di gioco: se le funzioni nascono man mano, l'ordine
diventa un accidente della cronologia di sviluppo.

`StepReport { rejected: Vec<(usize, CommandError)>, events: Vec<Event> }`. Un comando invalido
**non** interrompe il tick e non è un `Err` del tick: viene scartato e registrato con l'indice
del comando. L'LLM produrrà comandi invalidi (Testing punto 3): rifiutarli è il comportamento
normale, non un errore del sistema.

## Fuori scope

Rete stradale (05), copertura (06), produzione (07), evoluzione/tasse/eventi (M1).
`PlaceRoad` in questa fase segna solo il flag nel tile e mette `dirty.roads = true`.

## Test

Unitari, tabellari:

1. `step(&mut w, &[])` incrementa il tick di 1 e non cambia nient'altro (confronto sull'hash una
   volta che esiste la fase 08; qui basta un confronto campo per campo).
2. `PlaceBuilding` di una fattoria 2×2 occupa **quattro** tile e scala il costo dal tesoro.
3. Sovrapposizione rifiutata: seconda fattoria che tocca un solo tile della prima ⇒
   `TileOccupied`, e **lo stato non è cambiato affatto** (nessuna occupazione parziale: la
   validazione dell'intero footprint precede qualunque mutazione).
4. Footprint che sfora il bordo ⇒ `OutOfBounds`, senza mutazioni.
5. Tesoro insufficiente ⇒ `InsufficientFunds` con i numeri giusti nel messaggio.
6. `Demolish` libera tutti i tile del footprint e rimuove l'id dallo slotmap.
7. `kind` inesistente ⇒ `UnknownBuildingKind` (arriverà dall'LLM).

Property test — sono questi che chiudono la fase:

8. **Nessun panic**: sequenze di 10.000 comandi generati casualmente da `proptest`, incluse
   coordinate fuori range e `kind` inesistenti, su 1.000 tick. Il core non panica mai e
   `rejected` cresce coerentemente.
9. **Nessuna sovrapposizione**, invariante globale: dopo qualunque sequenza di comandi, ogni
   tile ha al più un occupante, e per ogni edificio i tile del suo footprint puntano a lui.
   (Doppia direzione: è l'invariante che coglie i bug di demolizione.)
10. **Tesoro coerente**: `tesoro == tesoro_iniziale − Σ costi accettati` (in M0 non ci sono
    entrate). Test di conservazione, non di comportamento.

## Verifica

```sh
cargo test -p sim-core
PROPTEST_CASES=2000 cargo test -p sim-core --release invarianti
```

**Fatto quando:** i property test 8–10 passano con 2000 casi, e le dieci funzioni del tick
esistono nell'ordine di `CLAUDE.md`.
