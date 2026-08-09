# Piano dei primi sviluppi — Brando

Questo piano copre **M0 — Fondamenta** (fasi 00–09, completate) e **M1 — Loop di gioco minimo**
(fasi 11–19) della roadmap in `CLAUDE.md`, spezzate in fasi piccole.

M1 è stata pianificata dopo la chiusura di M0, non prima: farlo prima avrebbe significato
decidere sul bilanciamento senza aver visto un tick girare. [10-oltre-m0.md](10-oltre-m0.md)
resta il documento che traccia M2 e M3 con la stessa regola.

## Principio guida

Ogni fase ha **un solo goal**, chiuso da un comando che si esegue e che dà una risposta
binaria. Se una fase finisce e non si sa dire se ha funzionato, la fase è mal definita.

Regola operativa: una fase = un commit (o una PR). Non si inizia la fase N+1 con la N rossa.

## Le fasi di M0 — Fondamenta

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

Fra M0 e M1 c'è un lotto di lavoro senza file di fase, documentato in
[decisioni-aperte.md](decisioni-aperte.md): la coerenza fra capacità e produzione
(**A5**), lo smarcamento di `dubbi.md` (**A7–A10**) e le ottimizzazioni del passo 3 (**A11**).

## Le fasi di M1 — Loop di gioco minimo

| #  | Fase | Goal verificabile in una riga | Dim. |
|----|------|-------------------------------|------|
| 11 | [Difficoltà](11-difficolta.md) | Stesso seed e stessi comandi, difficoltà diverse ⇒ hash diversi; la difficoltà sta nell'header del replay e nell'hash | S |
| 12 | [Soddisfazione](12-soddisfazione.md) | Una casa servita arriva al massimo in `massimo/passo_su` tick calcolati dal `DataSet`; tolta l'acqua torna a zero in `massimo/passo_giu` | M |
| 13 | [Livelli delle case](13-livelli-case.md) | Una casa servita raggiunge il livello 2 in un numero di tick calcolabile dal `DataSet`; togliendo l'acqua torna al livello 1, e non oscilla | L |
| 14 | [Nascite e morti](14-nascite-morti.md) | Una città servita cresce fino a saturare le capienze, una che perde i servizi si spopola; il test di sensibilità al seed si riattiva | L |
| 15 | [Immigrazione ed emigrazione](15-migrazione.md) | Due città identiche tranne per la copertura ricevono flussi diversi; e il ciclo copertura↔popolazione **smorza** | M |
| 16 | [Tesoro e tasse](16-tesoro-tasse.md) | L'invariante del tesoro resta un'**uguaglianza esatta** con le entrate dentro | M |
| 17 | [`sim-scenario`](17-sim-scenario.md) | "500 abitanti entro 5 anni" si dichiara completato al tick giusto, e non prima | L |
| 18 | [Invarianti e chiusura M1](18-invarianti-chiusura-m1.md) | Gli invarianti nuovi sono property test verdi, il costo di A12 è misurato e attribuito, `CLAUDE.md` dice M1 completato | M |

Catena sequenziale, nessuna eccezione: ogni fase legge lo stato che la precedente introduce.
Accorpamenti accettabili se otto fasi sembrano troppe: **14+15** se la demografia risulta più
piccola del previsto. Mai 12+13, mai 16 con altro, e mai la 14 con niente — è quella che tocca
l'hot path.

La decisione su cui poggia tutta M1 è [A12](decisioni-aperte.md): **i servizi rincorrono la
popolazione**. La capacità di un provider si conta sugli abitanti presenti, quindi la copertura
si ricalcola quando qualcuno si muove — cioè quasi a ogni tick. È una scelta di gioco che si
paga in tempo di calcolo, presa sapendolo, e il debito che apre è
[A17](decisioni-aperte.md): **l'unica decisione aperta del progetto**, da chiudere prima di M2.
Vale la pena leggere entrambe prima della fase 13.

## Cosa si costruisce e cosa no

Dei nove crate previsti in `CLAUDE.md`, M0 ne fa nascere quattro — `sim-core`, `sim-data`,
`sim-replay`, `xtask` — e M1 un quinto, `sim-scenario`, nella fase 18. `game-bevy` e `agent-*`
arrivano con M2, `sim-civ` con M3 e la seconda civiltà (D6). Crate vuoti scaffoldati in
anticipo sono superficie che invita a riempirla.

## Come si verifica una fase

Ogni file di fase chiude con una sezione **Verifica**: i comandi da eseguire e cosa deve
succedere. Il baseline che deve restare verde in ogni fase è:

```sh
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Rigenerare i golden senza perdere il segnale

Ogni fase di M1 tranne la 11 rigenera i golden, e quello è il momento in cui il test più
prezioso del progetto rischia di diventare un rito. Il messaggio in testa a ogni `.hashes` dice
già la regola — *se cambia senza che sia cambiato il bilanciamento, è stata introdotta una
fonte di non-determinismo: fermarsi e trovarla, non rigenerare* — ma applicarla richiede un
protocollo, e ogni file di fase lo cita invece di ripeterlo.

1. **Verde prima di cominciare.** `cargo xtask regen-golden --check` dev'essere verde *prima*
   di toccare il codice. Se non lo è, l'albero è già sporco per altro e il segnale è perso.
2. **Esattamente i file previsti.** Dopo la modifica, `--check` deve elencare i file che la
   fase dichiara di rigenerare, né uno di più. Un `.ron` che cambia in una fase che non tocca
   l'header è già l'indizio.
3. **Idempotenza.** `regen-golden` due volte di fila: la seconda deve dire "niente da fare". È
   dove il non-determinismo intra-processo si vede per primo.
4. **Processo separato.** `cargo test -p sim-replay` coglie ciò che il punto 3 non può:
   indirizzi di memoria, `RandomState`, ordini di iterazione di collezioni hash.
5. **Guardare il primo tick divergente** nel diff degli `.hashes`. È il controllo che nessuno
   fa e che vale più degli altri quattro: se la meccanica nuova non può agire prima del tick 60
   e il diff comincia al 30, la causa è un'altra e va trovata prima di committare. Il formato
   testuale del golden esiste per questo.
6. **Un solo motivo di rigenerazione per commit.** Un commit che rigenera i golden e cambia due
   meccaniche non è più diffabile.

## Decisioni

Alcune scelte sono necessarie per implementare ma non sono fissate da `CLAUDE.md`: sono
raccolte in [decisioni-aperte.md](decisioni-aperte.md) con una raccomandazione per ciascuna.
Le fasi 01, 03 e 08 assumono la raccomandazione; se una viene ribaltata, cambia il contenuto
di quella fase, non l'ordine del piano.

Il documento continua oltre M0: A7–A11 sono nate smarcando `dubbi.md`, A12–A16 pianificando M1.
Per ognuna, oltre alla raccomandazione, c'è **come è andata davvero** — che è quasi sempre
l'informazione più utile, perché diverge.
