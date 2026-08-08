# Fase 03 — sim-data: tabelle RON validate

**Goal:** una tabella valida carica; una rotta produce un report con **tutti** gli errori, non
solo il primo; il dataset ha un hash stabile.
**Dipende da:** 01 (usa `Milli`, `Coins`). Parallelizzabile con 02.
**Dimensione:** M.
**Decisioni coinvolte:** D6 (civiltà = dati + regole), convenzioni (nessun numero di
bilanciamento nel codice), A2, A6.

## Perché adesso

La regola "se compare una costante numerica di gioco in un `.rs`, è un bug" non si può
rispettare se non c'è un posto dove mettere i numeri. Se `sim-data` arriva dopo `World`, i
numeri finiscono nel codice "temporaneamente" e restano lì.

## Cosa si costruisce

### File di dati (`sim-data/data/`)

```
rules.ron       costanti globali: tick_per_mese, tesoro_iniziale, abitanti_per_livello_casa
terrain.ron     per ogni Terrain: costruibile, costo_strada, attraversabile
buildings.ron   i tre edifici di M0
```

`buildings.ron`, la tabella che porta il peso:

```ron
(
  buildings: [
    (
      id: "casa",
      footprint: (1, 1),
      costo: 10,
      livelli: 1,                       // l'evoluzione è M1
      servizio: None,
      servizi_richiesti: ["acqua", "cibo"],
    ),
    (
      id: "pozzo",
      footprint: (1, 1),
      costo: 12,
      livelli: 1,
      servizio: Some((
        kind: "acqua",
        // un valore per livello: raggio in tile percorsi su strada (D2)
        raggio_per_livello: [12],
        capacita_per_livello: [8],      // case servite contemporaneamente
      )),
      servizi_richiesti: [],
    ),
    (
      id: "fattoria",
      footprint: (2, 2),
      costo: 40,
      livelli: 1,
      servizio: Some((
        kind: "cibo",
        raggio_per_livello: [10],
        capacita_per_livello: [6],
      )),
      // vedi A5: in M0 la fattoria copre come un pozzo. In M3 la merce
      // arriverà da un magazzino via walker e questo campo cambierà.
      produzione_per_tick: 400,         // Milli di cibo
      giacenza_max: 20000,
      servizi_richiesti: [],
    ),
  ],
)
```

### Caricamento e validazione

```rust
pub struct DataSet {
    pub rules: Rules,
    pub terrain: BTreeMap<Terrain, TerrainDef>,     // BTreeMap, non HashMap (D4)
    pub buildings: Vec<BuildingDef>,                // indicizzato per BuildingKindId
    pub hash: [u8; 32],
}

pub fn load_from_dir(path: &Path) -> Result<DataSet, LoadError>;
pub fn validate(raw: &RawDataSet) -> Result<(), ValidationReport>;
```

`ValidationReport` è un `Vec<ValidationError>`, **non** un errore singolo: chi corregge una
tabella vuole vedere tutti i problemi in un giro, non ricompilare sei volte. Ogni errore
riporta il percorso logico del campo (`buildings[1].servizio.raggio_per_livello`).

Controlli minimi:

- id degli edifici unici e non vuoti
- `livelli >= 1`; `raggio_per_livello.len() == livelli` e idem per `capacita_per_livello`
  (è l'errore più probabile quando si aggiungeranno i livelli in M1)
- costi e raggi non negativi; `footprint` con entrambe le dimensioni `>= 1`
- ogni `servizi_richiesti` e ogni `servizio.kind` risolve a un `ServiceKind` noto
- `produzione_per_tick` presente ⟺ l'edificio è un produttore; `giacenza_max > 0` se produce
- `rules.tick_per_mese >= 1`

`hash` è `blake3` sul contenuto **canonicalizzato** del dataset (i valori validati, non i byte
dei file: così una riformattazione o un commento nel RON non invalida i golden, mentre un numero
cambiato sì). Entra nell'hash dello stato (vedi [A2](decisioni-aperte.md)).

Il caricamento è I/O e vive in `sim-data`, mai dentro `sim-core` (D4: nessun I/O nel core).

## Fuori scope

Catene produttive multi-step, requisiti di evoluzione delle case, tabelle per civiltà (D6/M3),
tasse. Tre edifici, un livello ciascuno.

## Test

1. **Fixture valida**: `data/` di produzione carica senza errori. Test permanente: è la rete di
   sicurezza per ogni futura modifica di bilanciamento.
2. **Fixture rotte**, una per controllo, in `tests/fixtures/rotte/`: id duplicato, `livelli: 2`
   con un solo raggio, costo negativo, `servizio.kind: "trasporto_astrale"`, campo mancante,
   RON sintatticamente invalido.
3. **Il report è completo**: una fixture con **tre** errori diversi produce tre voci. È il test
   che distingue una validazione vera da un `?` sul primo controllo.
4. **Messaggi stabili**: snapshot `insta` del report renderizzato. Se un messaggio peggiora, il
   diff lo mostra. (Vale anche come documentazione leggibile degli errori.)
5. **Hash**: stabile su due caricamenti; **invariato** se si riformatta il RON o si aggiunge un
   commento; **diverso** se si cambia `costo: 10` in `costo: 11`.

## Verifica

```sh
cargo test -p sim-data
cargo insta review        # solo se gli snapshot cambiano
```

**Fatto quando:** il test 3 passa (tre errori riportati insieme) e il test 5 mostra che l'hash
distingue una modifica di bilanciamento da una riformattazione.
