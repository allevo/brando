# Fase 08 — Replay, hash canonico e xtask

**Goal:** un `seed + Vec<Command>` rigiocato produce esattamente gli stessi hash di stato;
`cargo xtask regen-golden` non produce diff quando nulla è cambiato.
**Dipende da:** 07 (o 05, se si vuole anticipare — vedi README).
**Dimensione:** L.
**Decisioni coinvolte:** D4 (il salvataggio è seed+log), Testing punto 2 (il test più prezioso
del progetto), A2, A3.

## Perché adesso

È il test che protegge tutti gli altri. Da qui in avanti ogni fonte di non-determinismo
introdotta per distrazione si manifesta come un hash che cambia, entro un commit da quando è
stata introdotta, invece che come un bug di bilanciamento inspiegabile sei mesi dopo.

## Cosa si costruisce

### `sim-replay`

```rust
pub struct Recording {
    pub header: Header,
    /// Ordinato per tick crescente. Più comandi nello stesso tick mantengono
    /// l'ordine di inserimento: l'ordine è parte del contratto di determinismo.
    pub commands: Vec<(u32, Command)>,
}

pub struct Header {
    pub format_version: u16,
    pub seed: u64,
    pub grid: GridSpec,
    /// blake3 del DataSet (fase 03). Se il bilanciamento cambia, il replay
    /// fallisce subito e con il motivo giusto (A2).
    pub dataset_hash: [u8; 32],
}

pub fn replay(rec: &Recording, data: Arc<DataSet>, until: u32) -> Result<World, ReplayError>;
```

`ReplayError::DatasetMismatch { atteso, trovato }` è un errore distinto e ha un messaggio che
dice cosa fare: rigenerare i golden se il cambio era voluto.

### Hash canonico

```rust
/// Serializzazione canonica per l'hash. Scritta a mano, non delegata a serde (A3):
/// l'ordine dei campi qui è un contratto, spostare un campo nella struct non
/// deve invalidare i golden.
pub fn hash_world(w: &World) -> [u8; 32];
```

Cosa entra, in ordine fissato: `tick`, `dataset_hash`, dimensioni della griglia, i tile in
ordine di `TileIdx`, gli edifici in ordine di `BuildingId` (con kind, origin, level, stock), le
case in ordine di `HouseId`, l'economia, **la posizione di ogni stream RNG** (fase 02).

Cosa **non** entra: `RoadNetwork`, `Coverage`, `DirtyFlags`, `FoodLedger`. Sono strutture
derivate o diagnostiche: se entrassero nell'hash, un bug di ricostruzione incrementale si
presenterebbe come divergenza di hash — mentre il test che deve coglierlo è l'equivalenza
incrementale/da-zero della fase 06, che dice *dove* è il problema.

**Il rischio noto di scrivere l'hash a mano** è che un campo nuovo dello stato venga dimenticato,
rendendo l'hash cieco a una parte dello stato. Mitigazione, test 6.

### Golden

```
sim-replay/tests/golden/
  minimo.ron          seed + comandi (leggibile: griglia 32×32, A4)
  minimo.hashes       "tick,hash_esadecimale" ogni 30 tick
  fame.ron            scenario che esaurisce il cibo (fase 07, test 3)
  fame.hashes
```

I `.ron` sono scritti a mano o registrati da `xtask`, e devono restare **leggibili**: un golden
che nessuno riesce a leggere non aiuta a capire perché è cambiato.

### `xtask`

```sh
cargo xtask run --scenario minimo --ticks 360 [--dump-every N]
cargo xtask record --out sim-replay/tests/golden/nuovo.ron   # registra una partita
cargo xtask regen-golden                                     # rigenera tutti gli .hashes
cargo xtask regen-golden --check                             # fallisce se ci sarebbe un diff (CI)
```

## Fuori scope

Salvataggio dello stato (D4: non esiste, il salvataggio *è* seed+log). Compressione, formato
binario, versionamento del formato oltre il campo `format_version`. Scenari con obiettivi (M1).

## Test

1. **Determinismo intra-processo**: lo stesso `Recording` rigiocato due volte nello stesso
   processo dà hash identici a ogni checkpoint.
2. **Determinismo inter-processo**: gli hash coincidono con quelli committati in `.hashes`. È il
   test golden vero e proprio — coglie ciò che il test 1 non può, cioè la dipendenza da indirizzi
   di memoria, da `RandomState`, dall'ordine di iterazione di una collezione hash.
3. **Determinismo per esecuzione parziale**: rigiocare fino al tick 150 e poi continuare fino a
   360 dà lo stesso hash finale che rigiocare 360 tick di fila. Coglie gli stati "nascosti"
   ricostruiti male al riavvio.
4. **Mismatch del dataset**: alterato un numero nel `DataSet`, il replay fallisce con
   `DatasetMismatch`, **non** con un hash diverso al tick 200.
5. **`regen-golden` è idempotente**: eseguirlo su un albero pulito lascia `git diff` vuoto.
   Questo test va in CI come `regen-golden --check`: è la differenza tra golden che significano
   qualcosa e golden che si rigenerano per abitudine ogni volta che sono rossi.
6. **L'hash copre tutto lo stato** — mitigazione del rischio di A3. Un test che, per ogni campo
   mutabile del `World`, lo perturba e verifica che l'hash cambi:
   ```rust
   /// Se questo test fallisce, hash_world ha smesso di coprire un campo dello stato:
   /// da quel momento i golden sono ciechi su quel campo. Non silenziare: aggiungere
   /// il campo a hash_world.
   ```
   Copertura pragmatica (tick, un tile, un edificio, una casa, il tesoro, la posizione di ogni
   RNG), non riflessione automatica.
7. **Sensibilità al seed**: seed diverso, stessi comandi ⇒ hash diverso *appena* un dominio RNG
   viene usato. In M0 nessun sistema usa l'RNG, quindi questo test **è atteso fallire** e va
   scritto come `#[ignore]` con il motivo, da riattivare in M1 con gli eventi casuali. Scriverlo
   ora perché è il momento in cui si capisce perché serve.

## Verifica

```sh
cargo test -p sim-replay
cargo xtask regen-golden --check     # deve passare senza modificare nulla
```

Poi la prova manuale che chiude la fase: cambiare `produzione_per_tick` da `400` a `401` in
`buildings.ron`. Il replay deve fallire con `DatasetMismatch` (test 4). Ripristinare il valore e
verificare che tutto torni verde.

**Fatto quando:** i test 2, 5 e 6 passano e la perturbazione deliberata del dataset è stata vista
fallire con l'errore giusto. Da qui in poi ogni fase aggiunge righe ai golden.
