# Glossary

This repository was written in Italian and translated to English in one pass. This file is the
lookup table for that translation, and it exists for one reason: so that no word in the code sends
you to a dictionary.

Three sections:

1. **[Words worth a definition](#words-worth-a-definition)** — the English words used in identifiers
   that are not everyday vocabulary, each with a plain explanation and the Italian it replaced.
2. **[Italian → English, by crate](#italian--english-by-crate)** — what every renamed thing is called
   now. Use it to read a diff against the old code.
3. **[Deliberately not translated](#deliberately-not-translated)** — the strings that are frozen, and
   why touching them would break something.

---

## Words worth a definition

The rule for the translation was **plain words over jargon**. Four terms the trade would normally use
were deliberately avoided; they are at the bottom of this section, in case you meet them in writing
elsewhere about Rust or game development.

### Used in the code

| Word | What it means here | Replaced |
|---|---|---|
| **treasury** | The city's money — the single pot the player spends from. `Economy.treasury`. | `tesoro` |
| **residents** | The people living in *one house*. The city-wide total is `population`. | `abitanti` |
| **range** | How far a well or a farm reaches, counted in tiles **walked along roads**, never in a straight line. | `raggio` |
| **capacity** | How many residents a provider can serve at once. Not how many houses. | `capacità` |
| **provider** | A building that supplies a service to nearby houses — the well provides water, the farm provides food. | (same) |
| **stock** | How much food a farm currently holds. A granary that fills up and stops. | `giacenza` |
| **entrance** | A road tile touching a building. A building with no entrance is cut off from the network and serves nobody. | `ingresso` |
| **walkable** | Whether a road can be laid on this terrain. Rock is walkable, water is not. | `attraversabile` |
| **buildable** | Whether a building can be placed on this terrain. | `costruibile` |
| **sustainable** / **unsustainable** | Whether a farm's declared capacity matches what it actually grows. An unsustainable capacity means houses that stay hungry forever, so it is a validation error. | `sostenibile` |
| **checkpoint** | A save point in a recorded game: every 30 ticks, the state's hash is written down so a later run can be compared against it. | (same) |
| **invariant** | Something that has to be true after *every* possible sequence of moves — "population is never negative". If one breaks, there is a real bug. | `invariante` |
| **absorbing state** | A situation you can get into and never get out of, no matter what you build. Hunger used to be one; it was a bug, and fixing it is written up in A5. | `stato assorbente` |
| **dirty (flag)** | A note saying "this has to be recomputed next tick". Not "unclean" — the opposite of "up to date". | `flag dirty` |
| **scratch (buffer)** | A piece of scrap memory reused between calls so it does not have to be allocated again each time. | `scratch` |
| **seed** | The one number a whole game is generated from. Same seed + same commands ⇒ exactly the same game, down to the bit. | `seed` |
| **salt** | A fixed piece of text mixed into a seed so two different systems drawing random numbers never produce the same sequence. | `sale` |
| **hot path** | The code that runs most often and therefore costs the most. Here it is step 3 of the tick. | (same) |
| **jitter** | A small random wobble added to a rate, so two games with different seeds do not play out identically. | (same) |
| **hysteresis** | Deliberately using two different thresholds — one to go up, a lower one to come down — so a value sitting on the boundary does not flicker back and forth. | `isteresi` |
| **attractiveness** | One number saying how much the city draws new people in. | `attrattività` |

### Words deliberately avoided

If you read about Rust or game development elsewhere you will meet these. They mean the same as the
plain word this repository uses instead.

| Common jargon | Used here instead | Why |
|---|---|---|
| **footprint** | `size` | The rectangle of tiles a building sits on. `size: (2, 2)` is a 2×2 farm. |
| **golden** (golden test, golden file) | `expected` | A recorded game plus its committed hashes, re-run every test to prove nothing drifted. `tests/expected/`, `cargo xtask regen-expected`. |
| **ledger** | `FoodTotals`, `PopulationTotals` | Running totals kept only so conservation can be checked as an exact equality. |
| **canonical** (canonical hash) | *the state hash*, *the dataset hash* | It only ever meant "the one true hash, computed in a fixed order", which the surrounding comments already explain. |

---

## Italian → English, by crate

Only the renames that are not obvious cognates. Things like `tick`, `grid`, `seed`, `blake3`,
`Command`, `World`, `Milli` were already English and did not change.

### Cross-cutting

| Italian | English |
|---|---|
| `casa` / `case` | `house` / `houses` |
| `edificio` / `edifici` | `building` / `buildings` |
| `strada` / `strade` | `road` / `roads` |
| `pozzo` | `well` |
| `fattoria` | `farm` |
| `pozzetto` (a deliberately tiny well, used in tests) | `small_well` |
| `acqua` / `cibo` | `water` / `food` |
| `abitanti` | `residents` |
| `popolazione` | `population` |
| `copertura` | `coverage` |
| `servizio` / `servita` | `service` / `served` |
| `livello` / `livelli` | `level` / `levels` |
| `raggio` | `range` |
| `capacità` | `capacity` |
| `capienza` (how many a house holds) | `max_residents` |
| `giacenza` | `stock` |
| `tesoro` | `treasury` |
| `costo` | `cost` |
| `produzione` | `output` |
| `soddisfazione` | `satisfaction` |
| `difficoltà` | `difficulty` |
| `aliquota` | `tax rate` |
| `attrattività` | `attractiveness` |
| `incoerenza` | `inconsistency` |
| `golden replay` | `expected replay`, or just *recording* |

### `sim-core`

| Italian | English |
|---|---|
| `LATO_MAX` | `MAX_SIDE` |
| `Terrain::Pianura` / `Acqua` / `Roccia` | `Terrain::Plain` / `Water` / `Rock` |
| `Terrain::TUTTI`, `ServiceKind::TUTTI`, `RngDomain::TUTTI` | `::ALL` |
| `GridError::DimensioneNonValida` | `GridError::InvalidSize` |
| `Tile::occupante` / `e_libero` / `set_occupante` / `libera_occupante` | `occupant` / `is_free` / `set_occupant` / `clear_occupant` |
| `TileOccupant.e_casa` | `TileOccupant.is_house` |
| `Occupante::Edificio` / `Casa` | `Occupant::Building` / `House` |
| `Occupato` (in error messages) | `OccupantKind` |
| `House.abitanti` / `servita` | `House.residents` / `served` |
| `Economy.tesoro` | `Economy.treasury` |
| `World::n_edifici` / `n_case` / `popolazione` | `building_count` / `house_count` / `population` |
| `World::giacenza_totale` | `World::total_stock` |
| `World::ingressi_edificio` / `ingressi_casa` / `ingressi` | `building_entrances` / `house_entrances` / `entrances` |
| `World::occupante` | `World::occupant` |
| `World::consuma_rng` | `World::consume_rng` |
| `edifici_per_origine` / `case_per_origine` | `buildings_by_origin` / `houses_by_origin` |
| `DirtyFlags::segna_coverage` | `mark_coverage` |
| `DirtyFlags::invalida_coverage` | `invalidate_coverage` |
| `DirtyFlags::coverage_da_rivedere` | `coverage_needs_recompute` |
| `DirtyFlags::dimentica_coverage` | `forget_coverage` |
| `coverage_invalidata` | `coverage_invalidated` |
| `Visitati` (`nuovo`, `apri`, `visto`, `segna`, `epoche`, `corrente`) | `Visited` (`new`, `begin`, `is_seen`, `mark`, `epochs`, `current`) |
| `bfs_strade` | `bfs_roads` |
| `e_strada` | `is_road` |
| `RoadNetwork::connessi` / `n_componenti` | `connected` / `component_count` |
| `Coverage::e_servita` / `ricalcoli` / `assegnazioni` / `case_servite_da` | `is_served` / `recomputes` / `assignments` / `houses_served_by` |
| `CasePerTile` | `HousesByTile` |
| `calcola_da_zero` | `compute_from_scratch` |
| `scelte_entro_capacita` | `pick_within_capacity` |
| `FoodLedger` | `FoodTotals` |
| `prodotto` / `consumato` | `produced` / `consumed` |
| `perso_per_giacenza_piena` / `perso_per_demolizione` | `lost_to_full_stock` / `lost_to_demolition` |
| `atteso_in_giacenza` | `expected_stock` |
| `consuma` | `take_from_stock` |
| `rimuovi_edificio` / `rimuovi_casa` | `remove_building` / `remove_house` |
| `paga` | `charge` |
| `tiles_del_footprint` | `tiles_covered` |
| `occupa` / `libera_footprint` | `occupy` / `clear_tiles` |
| `segna_tutti_i_provider` | `mark_all_providers_dirty` |
| `snapshot_servizi` | `service_snapshot` |
| `StepReport::accettati` | `StepReport::accepted` |
| `Rules.tick_per_mese` / `mesi_per_anno` / `tick_per_anno` | `ticks_per_month` / `months_per_year` / `ticks_per_year` |
| `Rules.tesoro_iniziale` | `starting_treasury` |
| `Rules.abitanti_per_livello_casa` | `residents_per_house_level` |
| `Rules.consumo_cibo_per_abitante` | `food_per_resident` |
| `TerrainDef.costruibile` / `attraversabile` / `costo_strada` | `buildable` / `walkable` / `road_cost` |
| `ServiceDef.raggio_per_livello` / `capacita_per_livello` | `range_per_level` / `capacity_per_level` |
| `BuildingDef.footprint` / `costo` / `livelli` | `size` / `cost` / `levels` |
| `BuildingDef.servizio` / `servizi_richiesti` | `service` / `required_services` |
| `BuildingDef.produzione_per_tick` / `giacenza_max` | `output_per_tick` / `max_stock` |
| `BuildingDef.e_una_casa` / `e_un_produttore` / `tile_occupati` | `is_house` / `is_producer` / `tile_count` |
| `capacita_cibo_insostenibile` | `unsustainable_food_capacity` |
| `CapacitaInsostenibile` | `UnsustainableCapacity` |
| `canonical_hash` | `dataset_hash` |
| `DOMINIO` | `DOMAIN` |
| `Stream::conta` | `Stream::count_draw` |

### `sim-data`

| Italian | English |
|---|---|
| `dir_dati_di_produzione` | `production_data_dir` |
| `LoadError::Validazione` | `LoadError::Validation` |
| `leggi` | `read` |
| `valida_*` | `validate_*` |
| `servizi_noti` | `known_services` |
| `ValidationErrorKind::Vuoto` | `Empty` |
| `IdDuplicato { precedente }` | `DuplicateId { previous }` |
| `TroppoPiccolo { min, trovato }` | `TooSmall { min, found }` |
| `Negativo { trovato }` | `Negative { found }` |
| `LunghezzaPerLivello { livelli, trovati }` | `WrongLengthPerLevel { levels, found }` |
| `ServizioSconosciuto { nome, noti }` | `UnknownService { name, known }` |
| `ProduttoreSenzaGiacenza` | `ProducerWithoutStock` |
| `GiacenzaSenzaProduzione` | `StockWithoutOutput` |
| `CapacitaOltreLaProduzione { capacita, sostenibili }` | `CapacityBeyondOutput { capacity, sustainable }` |
| `TerrenoMancante` / `TerrenoDuplicato` | `MissingTerrain` / `DuplicateTerrain` |
| `TroppiEdifici` | `TooManyBuildings` |
| `tests/fixtures/rotte/` | `tests/fixtures/broken/` |

### `sim-replay`

| Italian | English |
|---|---|
| `golden.rs`, `GoldenError`, `dir_golden`, `tests/golden/` | `expected.rs`, `ExpectedError`, `expected_dir`, `tests/expected/` |
| `golden::rendi` / `leggi` | `expected::render` / `parse` |
| `RigaMalformata { riga, contenuto }` | `MalformedLine { line, content }` |
| `CHECKPOINT_OGNI` | `CHECKPOINT_EVERY` |
| `mondo_iniziale` | `initial_world` |
| `avanza` | `advance` |
| `checkpoints(…, ogni)` | `checkpoints(…, every)` |
| `Recording::ultimo_tick` / `comandi_al_tick` | `last_tick` / `commands_at_tick` |
| `DatasetMismatch { atteso, trovato }` | `DatasetMismatch { expected, found }` |
| `FormatoNonSupportato { trovata, attesa }` | `UnsupportedFormat { found, expected }` |
| `GrigliaNonValida` | `InvalidGrid` |
| scenarios `minimo` / `fame` | `minimal` / `hunger` |

### `xtask`

| Italian | English |
|---|---|
| `regen-golden` (subcommand) | `regen-expected` |
| `--lato` / `--abitanti` / `--ripetizioni` | `--side` / `--residents` / `--reps` |
| `esito` / `uso` / `opzione` / `numero` | `exit_code` / `usage` / `flag` / `number` |
| `intestazione` / `riga` | `table_header` / `row` |
| `registra` / `tick_del_golden` / `confronta_o_scrivi` | `record` / `expected_ticks` / `compare_or_write` |
| `Scenario { nome, descrizione, lato, comandi }` | `Scenario { name, description, side, commands }` |
| `per_nome` / `NOMI` / `mondo` | `by_name` / `NAMES` / `world` |
| `PROFILI` / `profilo` | `PROFILES` / `profile` |
| `Piano` | `Layout` |
| `Sparpaglia` (`tocca`) | `Spread` (`takes`) |
| `Misura { mediana, peggiore }` | `Measurement { median, worst }` |
| `cronometra` / `durata` | `time_it` / `format_duration` |
| `con_tesoro_illimitato` | `with_unlimited_treasury` |
| `cella_anello` / `cella_centro` / `angolo` | `ring_cell` / `center_cell` / `corner` |
| `SLOT_ANELLO` / `SLOT_DI_PROVA` / `LOTTO_RIFIUTI` | `RING_SLOTS` / `TEST_SLOTS` / `REJECT_BATCH` |

### Test files

| Italian | English |
|---|---|
| `sim-core/tests/cibo.rs` | `food.rs` |
| `sim-core/tests/comandi.rs` | `commands.rs` |
| `sim-core/tests/copertura.rs` | `coverage.rs` |
| `sim-core/tests/invarianti.rs` | `invariants.rs` |
| `sim-core/tests/strade.rs` | `roads.rs` |
| `sim-core/tests/comune/` | `common/` |
| `sim-data/tests/tabelle.rs` | `tables.rs` |
| `sim-replay/tests/golden.rs` | `expected.rs` |

Test constants in `sim-core/tests/common/mod.rs`: `CASA`/`POZZO`/`FATTORIA`/`POZZETTO` →
`HOUSE`/`WELL`/`FARM`/`SMALL_WELL`, `INESISTENTE` → `UNKNOWN_KIND`, `COSTO_*` → `*_COST`,
`TESORO_INIZIALE` → `STARTING_TREASURY`, `ABITANTI_PER_CASA` → `RESIDENTS_PER_HOUSE`,
`CAPACITA_*` → `*_CAPACITY`.

### Plan documents

| Italian | English |
|---|---|
| `plan/01-tipi-base.md` | `01-core-types.md` |
| `plan/02-rng-determinismo.md` | `02-rng-determinism.md` |
| `plan/04-world-tick-comandi.md` | `04-world-tick-commands.md` |
| `plan/05-strade-rete.md` | `05-roads-network.md` |
| `plan/06-copertura-servizi.md` | `06-service-coverage.md` |
| `plan/07-fattoria-cibo.md` | `07-farm-food.md` |
| `plan/08-replay-golden.md` | `08-replay-expected.md` |
| `plan/09-invarianti-chiusura.md` | `09-invariants-closeout.md` |
| `plan/10-oltre-m0.md` | `10-beyond-m0.md` |
| `plan/11-difficolta.md` | `11-difficulty.md` |
| `plan/12-soddisfazione.md` | `12-satisfaction.md` |
| `plan/13-livelli-case.md` | `13-house-levels.md` |
| `plan/14-nascite-morti.md` | `14-births-deaths.md` |
| `plan/15-migrazione.md` | `15-migration.md` |
| `plan/16-tesoro-tasse.md` | `16-treasury-taxes.md` |
| `plan/18-invarianti-chiusura-m1.md` | `18-invariants-closeout-m1.md` |
| `plan/decisioni-aperte.md` | `open-decisions.md` |

`00-workspace.md`, `03-sim-data.md`, `17-sim-scenario.md` and `README.md` kept their names.

Names the M1 plan documents propose for code that does not exist yet are translated too — for
instance `difficoltà.ron` → `difficulty.ron`, `abitanti_iniziali_casa` →
`starting_residents_per_house`, `soglia_evoluzione` / `soglia_degrado` → `level_up_threshold` /
`decay_threshold`, `Umore` → `Mood`, `Sommario` → `Summary`, `DemografiaLedger` →
`PopulationTotals`, `Stream::sotto` → `Stream::below`.

---

## Deliberately not translated

These are load-bearing. Renaming the constant that holds one is fine; changing the **value** rewrites
history and breaks the recorded replays.

| What | Value | Why it is frozen |
|---|---|---|
| `RngDomain::salt()` | `"brando/rng/v1/events"`, `.../migration`, `.../production` | Each random-number stream is seeded from its domain's *name*. Change the text and every recorded game shifts. Already English. |
| The dataset hash prefix | `b"brando/dataset/v1"` | Keeps this hash apart from every other hash in the project. |
| The state hash prefix | `b"brando/world/v1"` | Same, for the state. |
| Declaration order of `Terrain` | `Plain, Water, Rock` | The hash stores the position, not the name. Reordering silently changes every hash. |
| Declaration order of `ServiceKind` and `ServiceKind::index()` | `Water = 0, Food = 1` | Same. |
| `FORMAT_VERSION` | `1` | The shape of a save file. It goes up when the shape changes, not when a name does. |
| `CHECKPOINT_EVERY` | `30` | How far apart the committed checkpoints are. |
| Benchmark labels `A.`–`G.` | — | `plan/09-invariants-closeout.md` and `plan/18` record timings against those letters. |
| Decision codes `A1`–`A17`, phase numbers `00`–`18` | — | Referenced from about forty places. |

One thing **was** translated even though it is hashed, deliberately and in its own commit: the
building ids in `buildings.ron` (`"casa"`/`"pozzo"`/`"fattoria"` → `"house"`/`"well"`/`"farm"`).
`sim-core/src/data_hash.rs` feeds each id string into blake3, so renaming them moved every recorded
hash. That is why it is a separate commit whose diff contains nothing else.
