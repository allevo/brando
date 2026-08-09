# Fase 12 — Soddisfazione delle case

**Goal:** una casa servita raggiunge la soddisfazione massima in `massimo / passo_su` tick
**calcolati dal `DataSet`**; tolta l'acqua torna a zero in `massimo / passo_giu`.
**Dipende da:** 11.
**Dimensione:** M.
**Decisioni coinvolte:** [A10](decisioni-aperte.md), A9, D5, ordine del tick passo 6.

## Perché adesso

Perché è la prima metà del passo 6, che dalla fase 04 è una funzione vuota, ed è la grandezza
che tutte le fasi successive leggono: i livelli (14), le nascite (15), l'attrattività (16).
Introdurla da sola dà un goal che si calcola a mano dalle tabelle — `massimo / passo_su` tick,
niente di più — e che quindi si può sbagliare in un modo solo.

Tenerla insieme ai livelli farebbe una fase L il cui fallimento non direbbe **quale** delle due
metà è rotta: se una casa non evolve, è perché la soddisfazione non sale o perché la soglia non
scatta? Separandole, la seconda domanda ha già una risposta.

## Cosa si costruisce

### Il campo, in `sim-core/src/world.rs`

```rust
pub struct House {
    pub origin: TilePos,
    pub level: u8,
    pub abitanti: u16,
    pub servita: ServiceFlags,
    /// Da quanto tempo il servizio c'e', **non quanto ne arriva** (A10).
    ///
    /// Sale di `passo_su` quando il servizio e' soddisfatto in questo tick,
    /// scende di `passo_giu` quando manca, saturando in `0..=massimo`. Una
    /// casella per servizio, anche per quelli che il livello corrente non
    /// richiede: se la casa sale di livello, il tempo gia' accumulato su un
    /// servizio che riceveva comunque non era una bugia.
    pub soddisfazione: [u8; ServiceKind::COUNT],
}
```

`u8` e non `i16` come ipotizzava [A10](decisioni-aperte.md): l'accumulatore è clampato in
`0..=massimo` e non ha mai bisogno del segno né del range. `House` resta piccola, e non c'è
niente da difendere in più — il budget stretto è quello di `Tile`, non questo.

**Perché non un livello di risorsa.** Misurare *quanto* cibo entra in una casa romperebbe
"niente consumo parziale" della fase 07, ed è quella scelta a rendere la conservazione
un'uguaglianza esatta invece che una disuguaglianza. Sarebbe anche degenere: con la coerenza
capacità/produzione ([A5](decisioni-aperte.md)) una casa coperta riceve sempre il 100%.
Il ragionamento per esteso è in [A10](decisioni-aperte.md).

### Il passo 6.1, in `sim-core/src/tick.rs`

```rust
/// Passo 6 — evoluzione, degrado e migrazione.
///
/// L'ordine interno e' **semantica di gioco** quanto l'ordine dei dieci passi,
/// e vale la stessa regola: non si riordina senza rigenerare i golden e
/// scrivere perche'. In questa fase esiste solo 6.1; le fasi 13, 14 e 15
/// aggiungono i sottopassi successivi **sotto**, mai sopra.
fn houses_and_migration(world: &mut World) {
    aggiorna_soddisfazione(world); // 6.1
}
```

`aggiorna_soddisfazione` scorre le case in ordine di `HouseId` e, per ogni `ServiceKind`
richiesto dal livello corrente, applica `passo_su` o `passo_giu` a seconda di `servita.get(k)`,
saturando.

**Perché 6.1 è il primo sottopasso e non l'ultimo.** Legge soltanto ciò che i passi 3 e 4 hanno
scritto *in questo tick*, e tutto il resto del passo 6 legge lui. Se stesse dopo l'evoluzione,
una casa evolverebbe sui dati del tick precedente: corretto in media, illeggibile in un golden
che si prova a seguire a mano.

**La saturazione è semantica di gioco**, non una scorciatoia contro l'overflow — stessa
distinzione del granaio pieno in fase 07, e va commentata come tale. Il tempo di servizio non
si accumula all'infinito: oltre il massimo, un mese in più di acqua non compra niente.

### L'asimmetria di `servita`, che questa fase scioglie

[A9](decisioni-aperte.md) segnala che `House::servita` significa due cose a seconda del bit:
per l'acqua "è coperta" (scritto dal passo 3), per il cibo "ha mangiato" (riscritto dal passo
4). Con l'invariante *coperta ⇒ mangia sempre* le due coincidono, quindi oggi la differenza
esiste solo sulla carta — ma questa fase è la prima a **leggere** quel campo, e leggerlo con
due significati è la trappola che A9 annunciava.

**Decisione: `servita` significa "coperta", per entrambi i bit.** Il passo 4 smette di
riscrivere il bit del cibo e si limita a consumare. Una casa coperta che non mangia diventa una
violazione d'invariante — che è esattamente ciò che dev'essere, ed è già presidiata da
`invarianti_coperta_significa_sfamata`.

Il costo: se un giorno l'invariante cadesse (M3, quando la merce arriverà da un magazzino), la
soddisfazione salirebbe per una casa che non ha mangiato. Va scritto **adesso** nel doc comment
del campo, perché è il momento in cui si capisce; in M3 si presenterebbe come un bug di
bilanciamento inspiegabile.

### La tabella, in `rules.ron`

```ron
soddisfazione: (
    massimo: 100,
    // 25 tick, poco meno di un mese, da zero al massimo.
    passo_su: 4,
    // Si perde piu' in fretta di quanto si guadagni: perdere l'acqua e' un
    // evento, riaverla e' un investimento.
    passo_giu: 8,
),
```

Validazione di forma: i tre valori maggiori di zero, `passo_su` e `passo_giu` non maggiori di
`massimo`. Il controllo **incrociato** con le soglie di livello arriva con la fase 13, che le
introduce.

### L'evento

Non uno per casa per tick: sarebbero 40.000 eventi per tick, cioè esattamente ciò che il
confine col renderer vieta. Il renderer ha bisogno di un umore, non di un numero.

```rust
/// Fascia di soddisfazione di una casa. E' cio' che il renderer disegna, ed e'
/// grossa apposta: l'evento si emette al cambio di **fascia**, non di valore.
pub enum Umore { Disperato, Scontento, Contento, Prospero }

Event::HouseMoodChanged { house: HouseId, umore: Umore }
```

I confini delle fasce stanno in `rules.ron`, non nel codice. L'umore è il minimo fra i servizi
richiesti dal livello: una casa con l'acqua e senza cibo è disperata, non contenta a metà.

### L'hash

`h.update(&c.soddisfazione);` nel ciclo delle case di `hash_world`. Il canarino della fase 11
lo ricorda da solo.

## Fuori scope

Qualunque **conseguenza** della soddisfazione: evoluzione, degrado, migrazione, nascite. Questa
fase la calcola e basta, come la fase 06 registrava chi è servito senza trarne conseguenze. Le
soglie, l'isteresi e la valutazione mensile sono la fase 13.

## Test

1. **Salita** (*il goal*): casa servita da acqua e cibo, dopo `massimo / passo_su` tick — **letti
   dal `DataSet`, mai scritti nel test** — la soddisfazione è al massimo su entrambi i servizi.
   È la lezione del test 3 della fase 07: un numero hardcodato si rompe a ogni ribilanciamento
   senza segnalare nulla di reale.
2. **Discesa**: demolito il pozzo, dopo `massimo / passo_giu` tick la soddisfazione dell'acqua è
   zero e **quella del cibo è ancora al massimo**. La seconda metà è quella che conta: fissa che
   gli accumulatori sono indipendenti.
3. **Saturazione ai due estremi** (property test): dopo qualunque sequenza di comandi e
   qualunque numero di tick, `soddisfazione[k] <= massimo` per ogni casa e servizio. Mai
   negativa è garantito dal tipo, ed è metà del motivo per cui è `u8`.
4. **Una casa nuova parte da zero**, non dal massimo: nasce scoperta (il passo 3 la copre nello
   stesso tick, ma l'accumulatore parte comunque da zero) e la sua prima evoluzione costa il
   tempo pieno.
5. **Servizi non richiesti**: un servizio fuori da `servizi_richiesti` del livello non viene
   toccato. Oggi la casa li richiede entrambi, quindi il test si scrive su una fixture con una
   casa che ne richiede uno solo — ed è il caso che la fase 13 renderà reale.
6. **Evento su fascia, non su valore**: una casa che passa da soddisfazione 40 a 44 dentro la
   stessa fascia non emette niente; quella che attraversa un confine emette una volta sola. È
   il test 7 della fase 07 riscritto per l'umore, e per lo stesso motivo.
7. **L'hash copre la soddisfazione**: perturbazione in `l_hash_copre_tutto_lo_stato`.

## Verifica

```sh
cargo test -p sim-core soddisfazione
PROPTEST_CASES=2000 cargo test -p sim-core --release soddisfazione
cargo xtask run --ticks 120 --dump-every 30     # la colonna nuova deve muoversi in modo leggibile
cargo xtask regen-golden                        # rigenerazione voluta: campo nuovo nello stato
cargo xtask bench                               # la misura A: il passo 6 non era mai costato niente
```

Il dump di `xtask run` guadagna una colonna con la soddisfazione media. Serve a chiudere la
fase a occhio: se sale troppo in fretta o troppo piano, è bilanciamento (`sim-data`), non
codice — ma va guardato adesso, perché `regen-golden` lo congela in un golden.

`bench` va guardato in questa fase più che nelle altre: il passo 6 è sempre costato zero, e da
qui costa una passata su tutte le case a ogni tick. La misura da confrontare è `A`, il tick a
vuoto, che [A11](decisioni-aperte.md) indica come il numero da tenere d'occhio.

**Fatto quando:** i test 1 e 2 passano con i tick calcolati dal `DataSet`, e il dump a 120 tick
è interpretabile.
