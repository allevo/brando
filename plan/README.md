# Piano dei primi sviluppi — Brando

Questo piano copre **M0 — Fondamenta** della roadmap in `CLAUDE.md`, spezzata in fasi piccole.
M1 è solo tracciata (vedi [10-oltre-m0.md](10-oltre-m0.md)), non pianificata in dettaglio:
pianificarla ora significherebbe decidere sul bilanciamento prima di aver visto un tick girare.

## Principio guida

Ogni fase ha **un solo goal**, chiuso da un comando che si esegue e che dà una risposta
binaria. Se una fase finisce e non si sa dire se ha funzionato, la fase è mal definita.

Regola operativa: una fase = un commit (o una PR). Non si inizia la fase N+1 con la N rossa.

## Le fasi

| #  | Fase | Goal verificabile in una riga | Dim. |
|----|------|-------------------------------|------|
| 00 | [Workspace e guardrail](00-workspace.md) | Il workspace compila e i divieti di `CLAUDE.md` (float, `HashMap`, `thread_rng`, `unsafe`) sono errori del compilatore/linter, non convenzioni | S |
| 01 | [Tipi fondamentali e griglia](01-tipi-base.md) | `Milli`, gli id newtype e la `Grid` esistono; `size_of::<Tile>()` è sotto il budget e la conversione pos↔indice è biiettiva su tutta la mappa | M |
| 02 | [RNG per dominio](02-rng-determinismo.md) | Stesso seed ⇒ stessa sequenza per ogni dominio, e aggiungere un dominio nuovo non sfasa quelli esistenti | S |
| 03 | [sim-data: tabelle RON validate](03-sim-data.md) | Una tabella valida carica, una rotta produce un report con **tutti** gli errori; il dataset ha un hash stabile | M |
| 04 | [World, tick, comandi](04-world-tick-comandi.md) | `step()` avanza il tick, applica comandi validi, rifiuta gli invalidi con errore strutturato, e non panica su 10.000 comandi casuali | M |
| 05 | [Strade e rete stradale](05-strade-rete.md) | La rete si ricostruisce solo se `dirty` e la sua etichettatura è indipendente dall'ordine di costruzione | M |
| 06 | [Copertura servizi aggregata](06-copertura-servizi.md) | Il pozzo serve le case entro raggio **su strada**; il calcolo incrementale coincide con quello da zero | L |
| 07 | [Fattoria, cibo, primo loop](07-fattoria-cibo.md) | Il cibo prodotto è conservato (prodotto = consumato + giacenza) e le case restano scoperte a magazzino vuoto | M |
| 08 | [Replay, hash canonico, xtask](08-replay-golden.md) | `seed + Vec<Command>` rigioca allo stesso hash; `regen-golden` è idempotente | L |
| 09 | [Suite invarianti e chiusura M0](09-invarianti-chiusura.md) | Gli invarianti di `CLAUDE.md` sono property test verdi; `CLAUDE.md` dice "M0 completato" | M |

Dipendenze: la catena è sequenziale, con due eccezioni.
La **03** è indipendente dalla **02** (si possono fare in parallelo dopo la 01).
La **08** ha bisogno solo che esista *qualche* stato che evolve: se serve, la si anticipa dopo
la **05** con una griglia di sole strade, e si arricchisce il golden man mano.

## Cosa si costruisce e cosa no

Dei nove crate previsti in `CLAUDE.md` nascono adesso solo quattro: `sim-core`, `sim-data`,
`sim-replay`, `xtask`. `sim-scenario` arriva con M1, `game-bevy` e `agent-*` con M2, `sim-civ`
con M3 e la seconda civiltà (D6). Crate vuoti scaffoldati in anticipo sono superficie che
invita a riempirla.

## Come si verifica una fase

Ogni file di fase chiude con una sezione **Verifica**: i comandi da eseguire e cosa deve
succedere. Il baseline che deve restare verde in ogni fase è:

```sh
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Decisioni ancora aperte

Alcune scelte sono necessarie per implementare ma non sono fissate da `CLAUDE.md`: sono
raccolte in [decisioni-aperte.md](decisioni-aperte.md) con una raccomandazione per ciascuna.
Le fasi 01, 03 e 08 assumono la raccomandazione; se una viene ribaltata, cambia il contenuto
di quella fase, non l'ordine del piano.
