# Fase 18 — Suite invarianti e chiusura M1

**Goal:** la tabella in testa a `invarianti.rs` copre i nuovi invarianti, il bench è rimisurato
con le misure di M1, e `CLAUDE.md` dichiara M1 completato.
**Dipende da:** 17.
**Dimensione:** M.
**Decisioni coinvolte:** tutta la sezione Testing di `CLAUDE.md`, A12–A17.

## Perché adesso

Per lo stesso motivo della [fase 09](09-invarianti-chiusura.md): gli invarianti di M1 sono
stati scritti fase per fase, ognuno accanto al sistema che verifica, e serve un posto unico
dove si legga *cosa il progetto garantisce*. È il documento che un contributore — o un LLM che
lavora sul repo — legge per capire cosa non deve rompere.

E per una ragione che la fase 09 ha dimostrato valere più del previsto: è **qui** che si scrive
come le decisioni sono andate davvero. A5 e A11 sono le due voci più utili di
[decisioni-aperte.md](decisioni-aperte.md) proprio perché entrambe raccontano che l'ipotesi di
partenza era sbagliata. Senza questa fase, le sorprese di M1 restano nella testa di chi le ha
avute.

## Cosa si costruisce

### La tabella degli invarianti, aggiornata

`sim-core/tests/invarianti.rs` ha in testa una tabella markdown che elenca ogni invariante e
dove vive. M1 ne aggiunge nove e ne cambia tre.

| Invariante | Origine |
|---|---|
| `abitanti <= rules.capienza(level)`, per ogni casa e sempre | fase 13 |
| `popolazione == Σ abitanti`, mai negativa | D5, sostituisce la forma di M0 |
| Σ abitanti serviti da un provider `<= capacita(level)` | fase 14 |
| *Una casa coperta dal cibo mangia sempre* — ora un'uguaglianza stretta | A5, sotto A12 |
| Se la popolazione è cambiata, la copertura è sporca | fase 14, è il contratto di A12 |
| Conservazione della popolazione, uguaglianza esatta | fase 14 |
| `tesoro×1000 + residuo == (iniziale − Σcosti)×1000 + incassato` | fase 16 |
| Con servizi costanti, il livello di una casa è monotono su 360 tick | fase 13 |
| Il ciclo copertura↔popolazione smorza: l'oscillazione non cresce | fase 15 |
| Il numero di estrazioni RNG non dipende dal seed | fase 14 |
| `soddisfazione <= massimo`, per ogni casa e servizio | fase 12 |
| Copertura incrementale ≡ da zero, **su dataset a tassi zero** | fase 06, riformulato in fase 14 |

L'ultima riga è quella su cui vale la pena fermarsi, ed è il punto in cui M1 ha pagato
[A12](decisioni-aperte.md). `equivalenza_copertura` non è sopravvissuta intatta: con la
popolazione che si muove ogni tick, il confronto a fine tick diverge sempre e per costruzione.
La forma è la stessa, ma gira su una fixture con la demografia spenta, e ciò che verifica è
diventato **metà** di quello che verificava prima — l'invalidazione dimenticata dopo un comando,
non dopo un cambio di popolazione. L'altra metà è la riga sopra, che è un test diverso.

Va scritto senza addolcirlo: il presidio del passo 3 è ora diviso in due, e chi vorrà
ottimizzare la copertura prima di M2 ([A17](decisioni-aperte.md)) deve tenerli verdi entrambi.

**I tre che hanno cambiato esito di proposito**, da annotare accanto al test come si è fatto
con `una_casa_affamata_non_viene_salvata_da_una_seconda_fattoria`:

- `invarianti_popolazione` — `popolazione == n_case × 4` è falso per costruzione dalla fase 14.
- `invarianti_i_rifiuti_non_mutano` — riformulato, e più forte: *lo stato dopo un tick di soli
  rifiuti coincide con quello dopo un tick vuoto*. Coglie anche un rifiuto che consuma
  un'estrazione RNG, che la forma di M0 non vedeva.
- `due_tick_a_vuoto_non_ricalcolano_niente` — con l'evoluzione mensile, due tick a vuoto
  possono ricalcolare. Riscritto in "nessun ricalcolo se nessun livello è cambiato".

E `seed_diversi_danno_hash_diversi`, che non cambia esito ma **si riattiva** dalla fase 14 e da
lì non può più essere `#[ignore]`. Chiude un test scritto in fase 08 per essere rosso, come era
stato progettato.

### Il generatore condiviso, esteso

Il generatore di `partita()` deve produrre anche `SetAliquota`, inclusi livelli inesistenti in
proporzione significativa — vale ancora la regola della fase 09: *un generatore che produce
solo comandi validi verifica un decimo di quello che sembra verificare*.

E le partite generate devono essere **abbastanza lunghe** da far girare la demografia: una
sequenza di dodici tick non vede mai una valutazione mensile, quindi non testerebbe niente di
ciò che M1 ha aggiunto. Vale la pena un secondo generatore da qualche centinaio di tick, con
meno casi.

### Il bench, rimisurato — ed è il consegnabile principale della fase

M1 ha comprato una dinamica di gioco con del tempo di calcolo ([A12](decisioni-aperte.md)), e
questa fase deve dire **quanto**, in modo che chi ottimizzerà prima di M2 sappia dove guardare.
Non basta un numero: serve attribuzione.

`cargo xtask bench` con tre misure nuove accanto alle A–G di M0, sulla stessa macchina e con lo
stesso metodo (`--release`, 40 ripetizioni, mediana):

- **H. tick a vuoto con la demografia spenta** (tassi a zero). È il termine di paragone: deve
  coincidere con la `A` di fine M0, 248 µs. Se non coincide, il costo non è il ricalcolo della
  copertura ma il passo 6 in sé, ed è un'altra voce da togliere.
- **I. solo il passo 6.** L'analogo della `G` di M0, e separa il lavoro demografico dal
  ricalcolo che innesca. Senza, il costo si può solo dedurre per differenza, e la differenza è
  dominata dal rumore.
- **J. frazione di tick in cui la popolazione si è mossa**, su una partita intera. È il
  moltiplicatore vero: il costo totale di A12 è `J × G`, non `G`. In una città satura o
  disabitata `J` è basso e non si paga nulla; in una che cresce è vicino a 1.

Le misure vanno **registrate in questo file**, con data e macchina, come in
[09](09-invarianti-chiusura.md). E vanno lette insieme: `A` era 248 µs a fine M0 ed è il numero
che [A11](decisioni-aperte.md) indica come quello da guardare, perché si paga a ogni tick. Con
A12 diventa `248 µs + J × 3,05 ms`, e l'attesa è ~3,3 ms in una città viva — **~13×**.

Quel numero non è un fallimento: è il prezzo dichiarato di A12, deciso sapendolo. Ma è anche
l'input di [A17](decisioni-aperte.md), il compito aperto da chiudere **prima di M2**, e in
questa fase va scritto senza arrotondarlo verso il basso.

Tre correzioni al banco, che le fasi hanno accumulato:

1. `Piano::nuovo` dimensiona la città sugli abitanti che ci saranno davvero: con
   `abitanti_iniziali_casa` che dipende dalla difficoltà, "15.000 abitanti" diventerebbe "3.750
   case da zero abitanti". Stampare `case`, `capienza totale` e `popolazione` separatamente,
   perché con A12 il carico dei provider dipende dalla terza e non dalla seconda.
2. Il conteggio dei ricalcoli va letto prima e dopo **ogni** misura e stampato come delta. È ciò
   che rende `H` e `J` interpretabili, e ciò che distingue "il tick costa perché ricalcola" da
   "il tick costa e non si sa perché".
3. Il profilo di difficoltà va scelto **esplicitamente** e stampato nell'intestazione, accanto
   all'hash del dataset e per lo stesso motivo: un numero che si sposta dev'essere attribuibile.

### La documentazione

- `CLAUDE.md`, **Stato attuale** → `Milestone: M1 — completato`, con una riga per crate su cosa
  copre e l'elenco di cosa M1 non ha di proposito.
- Le "cose imparate implementando" di M1, nella stessa forma delle due di M0. I candidati, da
  confermare o smentire con quello che sarà successo davvero: che un test verde possa non
  verificare niente, se il numero di estrazioni RNG dipende dal seed (fase 14); e che una scelta
  di *gioco* — i servizi rincorrono la popolazione — abbia comprato un costo di calcolo
  quantificabile e una perdita di copertura di test, entrambi accettati con gli occhi aperti
  (A12).
- [decisioni-aperte.md](decisioni-aperte.md): A12–A16 segnate come chiuse, **con come sono
  andate davvero**. Per A12 in particolare: il costo previsto era ~13× sul tick a vuoto — dire
  quanto è stato, e se la dinamica di gioco che comprava si è vista davvero nelle partite.
- **[A17](decisioni-aperte.md) resta aperta**, ed è l'unica. Va aggiornata con i numeri
  misurati qui, che sono il suo input: è il compito da chiudere prima di M2.
- [10-oltre-m0.md](10-oltre-m0.md): le domande a cui M1 ha risposto vanno barrate con la
  risposta, come si è fatto con quelle di M0. Due sono già lì e aspettano: *30 tick/mese dà una
  curva giocabile?* — lo dice lo scenario della fase 17 — e *un `Tile` di 4 byte basta?*

## Fuori scope

**L'ottimizzazione del ricalcolo per tick**, che è il tema di [A17](decisioni-aperte.md) e va
fatta prima di M2, non qui. Questa fase la **misura** e le dà i numeri; separare le due cose è
deliberato, ed è la lezione di A11: l'ipotesi ovvia su dove fosse il costo era sbagliata, e a
dirlo è stata la misura. Ottimizzare nella stessa fase in cui si misura significa non avere il
prima.

Il bot euristico e l'evaluator: sono M2, e sono il canarino vero sul bilanciamento
(`CLAUDE.md`, Testing punto 4). Il target `cargo-fuzz`, ancora debito dalla fase 09: M1 ha
allargato lo spazio dei comandi di una variante sola, quindi la condizione che la fase 09
poneva per riprenderlo — "quando una meccanica nuova allargherà lo spazio dei comandi" — non è
ancora scattata davvero. Con `Intent` e bot (M2/M3) lo sarà.

## Test

Questa fase *è* test. Il criterio è lo stesso della 09: ogni riga della tabella corrisponde a
un `#[test]` esistente, e nessuno è `#[ignore]` senza una riga di motivazione accanto — e in M1
la lista degli `#[ignore]` legittimi deve essere **vuota**, perché l'unico che c'era si è
riattivato.

## Verifica

```sh
cargo test --workspace
PROPTEST_CASES=10000 cargo test --workspace --release invarianti
cargo xtask regen-golden --check
cargo xtask bench
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

**Fatto quando:** tutti i comandi sono verdi, le misure H, I e J sono registrate in questo file
e riportate in [A17](decisioni-aperte.md), e `CLAUDE.md` dice M1 completato. A quel punto esiste
un gioco: una città che cresce se la servi,
si spopola se la trascuri, paga le tasse e sa dire quando hai vinto — deterministica, misurata
e protetta da golden. È ciò su cui M2 può appoggiare un renderer e un bot senza toccare il
core.
