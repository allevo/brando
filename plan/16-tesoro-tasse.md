# Fase 16 — Tesoro e tasse

**Goal:** l'invariante del tesoro della fase 09 si estende alle entrate e resta
un'**uguaglianza esatta**; l'aliquota è una leva vera, non denaro gratis.
**Dipende da:** 15.
**Dimensione:** M.
**Decisioni coinvolte:** [A15](decisioni-aperte.md), A1, D4, ordine del tick passo 7.

## Perché adesso

Perché il passo 7 è vuoto dalla fase 04 e perché è l'ultima meccanica che serve prima che uno
scenario possa avere obiettivi non banali: senza entrate, il tesoro può solo calare e ogni
scenario finisce per esaurimento.

E perché ora c'è qualcosa da tassare. Le tasse su una popolazione costante sarebbero una
funzione lineare del tempo; su una popolazione che cresce con il benessere chiudono l'anello —
servizi → benessere → popolazione → gettito → altri servizi. È l'anello che rende il gioco un
gioco, e va provato prima di metterci sopra un obiettivo (fase 17).

L'aliquota entra qui **anche** come terzo termine dell'attrattività, che la fase 15 aveva
lasciato fuori apposta per non dover bilanciare due cose insieme
([A15](decisioni-aperte.md)).

## Cosa si costruisce

### Il comando — il primo dopo M0

```rust
pub enum Command {
    PlaceRoad { at: TilePos },
    PlaceBuilding { kind: BuildingKindId, origin: TilePos },
    Demolish { at: TilePos },
    /// Livello di aliquota, come indice nella tabella `aliquote`.
    ///
    /// Un livello discreto e non un per-mille libero, per due motivi: e' la
    /// forma che `Intent::AdjustTax { delta }` sa produrre (CLAUDE.md,
    /// interfaccia AI), e tiene i numeri in tabella invece che nel comando —
    /// cosi' ribilanciare le aliquote non invalida i log dei comandi gia'
    /// registrati.
    SetAliquota { livello: u8 },
}

#[error("aliquota sconosciuta: livello {livello}, il massimo e' {massimo}")]
AliquotaSconosciuta { livello: u8, massimo: u8 },
```

Aggiungere una variante a `Command` non rompe i golden esistenti: il RON è additivo e i
`.ron` già scritti non la contengono. `FORMAT_VERSION` **non** cambia — cambia la forma dello
stato, non quella del file. È la distinzione che il doc comment di `FORMAT_VERSION` fa già.

E l'evento corrispondente, perché l'aliquota è stato che il renderer deve poter mostrare:

```rust
/// Emesso solo quando l'aliquota **cambia davvero**: un `SetAliquota` che
/// rimette il livello che c'era gia' non produce niente. E' la stessa regola
/// di `ServiceCoverageChanged` — un delta, non un'eco del comando.
Event::TaxRateChanged { livello: u8 },
```

Il generatore di `invarianti.rs::comando()` deve produrre anche `SetAliquota`, **inclusi
livelli inesistenti** in proporzione significativa: un generatore che produce solo comandi
validi verifica un decimo di quello che sembra verificare (fase 09).

### Lo stato

```rust
pub struct Economy {
    pub tesoro: Coins,
    /// Livello corrente, indice in `rules.aliquote`.
    pub aliquota: u8,
    /// Millesimi di moneta non ancora arrotondati a moneta intera.
    ///
    /// Le tasse si incassano a **ogni tick** e valgono frazioni di moneta:
    /// troncare a ogni tick perderebbe la maggior parte del gettito di una
    /// citta' piccola — lo stesso baratro che il `residuo` della demografia
    /// evita per le nascite (fase 14). Con il resto, `tesoro*1000 + residuo`
    /// e' un'uguaglianza esatta col cumulato, ed e' cio' che rende
    /// l'invariante del tesoro un test capace di trovare un bug invece che di
    /// rassicurare.
    pub residuo: Milli,
}

/// Cumulati, per l'invariante e per l'evaluator di M2. Diagnostica: non
/// influenza nessuna decisione di gioco e resta fuori dall'hash, come il
/// `FoodLedger`.
pub struct EconomyLedger { pub speso: i64, pub incassato: i64 }
```

`aliquota` e `residuo` entrano nell'hash; il ledger no.

### Il passo 7

```rust
/// Passo 7 — finanza. Incasso continuo, non mensile.
///
/// Continuo perche' la curva del tesoro resti liscia: un picco ogni trenta
/// tick sarebbe un gradino da spiegare al bot di M2 e renderebbe il tesoro
/// medio dipendente da *quando* si guarda. Il numero mensile — quello che il
/// giocatore ragiona — lo ricostruisce il renderer dal sommario, che e' il suo
/// mestiere.
fn finance(world: &mut World) { .. }
```

Il conto: `Σ_case imponibile_per_abitante(livello) × abitanti`, accumulato in **`i64`**, poi
moltiplicato per il per-mille dell'aliquota e sommato a `residuo`; le monete intere passano in
`tesoro`.

L'`i64` non è prudenza generica: 15.000 abitanti per un imponibile per un per-mille esce da
`i32`, e `Milli` è `i32` ([A1](decisioni-aperte.md)). Stessa regola del `FoodLedger`, che è
`i64` per lo stesso motivo — gli aggregati cittadini si accumulano larghi e si convertono una
volta sola.

`imponibile_per_abitante` sta in `LivelloCasaDef` (fase 13), dove era già previsto a zero: le
case migliori pagano di più, ed è il motivo per cui il giocatore vuole farle evolvere.

### L'aliquota nell'attrattività

Terzo termine di `attrattivita()` (fase 15), con il suo peso in tabella. Rende la leva una
scelta vera invece che denaro gratis: alzare l'aliquota aumenta il gettito per abitante e
riduce gli abitanti.

**Controllo incrociato** (`Incoerenza::AliquoteSenzaScelta`): fra l'aliquota minima e la massima
il gettito totale a regime deve avere un massimo interno, non essere monotono. Se il gettito
crescesse sempre con l'aliquota, il gioco avrebbe una risposta ovvia e la leva non sarebbe una
leva. È un controllo di bilanciamento come `DemografiaNonSostenibile`, e per la stessa ragione:
un dataset che rende una scelta finta non produce nessun errore visibile.

### La tabella

```ron
aliquote: [
    (id: "nessuna", per_mille: 0),
    (id: "bassa",   per_mille: 60),
    (id: "media",   per_mille: 100),
    (id: "alta",    per_mille: 160),
],
// Millesimi di attrattivita' persi per ogni per-mille di aliquota.
peso_aliquota: 4,
aliquota_iniziale: 2,
```

L'aliquota iniziale può passare nel profilo di difficoltà, se si vuole che a `difficile` si
parta con meno margine.

## Fuori scope

**Manutenzione degli edifici e tesoro negativo.** Senza uscite ricorrenti il tesoro può solo
salire e l'invariante resta semplice; con esse servono la bancarotta, il decadimento degli
edifici non pagati e una condizione di sconfitta. È la cosa che vorrà entrare per prima in M2,
ed è giusto che entri lì: è raggiungibile e osservabile in uno scenario solo quando c'è un bot
che ci gioca contro.

Costi di costruzione variabili, sussidi, prestiti, commercio. Il commercio è M3 con le carovane
(D3).

## Test

1. **Invariante del tesoro** (property test, *il goal*):
   `tesoro×1000 + residuo == (iniziale − Σ costi accettati)×1000 + incassato`, uguaglianza
   esatta dopo qualunque sequenza di comandi. È l'estensione di `invarianti_tesoro`, e resta
   un'uguaglianza — che è ciò che [plan/10](10-oltre-m0.md) chiedeva.
2. **Nessun millesimo perso**: su diecimila tick con una città piccola, il cumulato coincide
   con la somma dei gettiti per tick. È il test che giustifica `residuo`; senza il campo,
   fallisce vistosamente.
3. **Aliquota fuori range rifiutata** con `AliquotaSconosciuta`, e lo stato non cambia — il
   caso normale, non un guasto (fase 04).
4. **La leva morde**: due partite identiche a parte l'aliquota ⇒ quella con l'aliquota alta ha
   più gettito per abitante e **meno abitanti**. Se il secondo effetto non si vede, il peso
   nell'attrattività è troppo basso ed è bilanciamento, non codice.
5. **`invarianti_i_rifiuti_non_mutano` cambia esito di proposito**: con le tasse continue
   l'economia cambia a ogni tick, anche a comandi tutti rifiutati. La riformulazione è **più
   forte** dell'originale — *lo stato dopo `step(w, cmds)` con tutti i comandi rifiutati
   coincide con quello dopo `step(w.clone(), &[])`* — e coglie di più: oggi non si accorgerebbe
   di un rifiuto che consuma un'estrazione RNG, con la nuova forma sì.
6. **Il tesoro non va negativo**: `paga` continua a rifiutare per fondi insufficienti, e non
   esistono uscite ricorrenti. Il test esiste già in `nessun_panic_su_diecimila_comandi`, e va
   esteso al fatto che `residuo` non è mai negativo.
7. **Aliquota zero**: gettito zero, `residuo` fermo, nessuna divisione per zero da nessuna
   parte.
8. **L'hash copre aliquota e residuo**: due perturbazioni in `l_hash_copre_tutto_lo_stato`.

## Verifica

```sh
cargo test -p sim-core tasse
PROPTEST_CASES=2000 cargo test -p sim-core --release invarianti_tesoro
cargo xtask run --ticks 1800 --dump-every 90
cargo xtask regen-golden
```

Il dump chiude la fase: la colonna del tesoro deve salire in modo liscio, senza gradini
mensili, e la città deve poter finanziare la propria crescita. Se il tesoro esplode, l'aliquota
o l'imponibile sono alti; se non basta mai a costruire un pozzo, la fase 17 non avrà uno
scenario vincibile — e va sistemato adesso.

**Fatto quando:** il test 1 passa come uguaglianza esatta con 2000 casi, e il test 4 mostra
entrambi gli effetti dell'aliquota.
