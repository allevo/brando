Quaderno personale. Le voci smarcate se ne vanno da qui; quello che resta utile
finisce in `plan/decisioni-aperte.md`, che è il documento che si legge dopo.

## Smarcati il 2026-08-09

Tutti e sette. Dove è finita la risposta:

- Trait sui comandi → **A7**: no, romperebbe `Copy`, serde e il formato dei golden
  per risparmiare un match di cinque righe.
- `segna_tutti_i_provider` e la binary search → sospetto infondato (l'insert è una
  push in coda), ma la lista non veniva letta da nessuno e il commento diceva il
  falso. Corretti entrambi.
- `case_per_tile` con `Vec` → fondato, e più grosso del previsto: era la voce
  singola più cara del passo 3. Ora è CSR indicizzato.
- `propagate_coverage` → fondato: via il `Vec` intermedio con uno split borrow.
- `Arc<DataSet>` → **A8**: serve a `World: Clone`, non al borrow checker.
- Solo cibo in `production` → **A9**: l'acqua è copertura per scelta. Il buco vero
  accanto è `servizi_richiesti`, che nessuno legge.
- Livello di cibo e migrazione → **A10**: la primitiva è un accumulatore di tempo,
  non un livello di risorsa, e la migrazione di M1 resta aggregata.

Ne è uscita anche **A11** (l'invalidazione mirata della copertura non si fa) e un
6,5× sul passo 3, numeri in `plan/09-invarianti-chiusura.md`.

## Aperti

- Niente, per ora.
