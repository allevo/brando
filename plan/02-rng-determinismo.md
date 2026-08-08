# Fase 02 — RNG per dominio

**Goal:** stesso seed ⇒ stessa sequenza per ogni dominio, e **aggiungere un dominio nuovo non
sfasa i domini esistenti**.
**Dipende da:** 00 (01 non serve).
**Dimensione:** S.
**Decisioni coinvolte:** D4.

## Perché adesso

È la fase più piccola del piano e quella con il ritorno più alto. Il requisito difficile non è
"seedare l'RNG" — è che tra sei mesi si aggiunga il dominio `Invasioni` e i golden replay degli
scenari esistenti **restino verdi**. Se i domini vengono derivati per split sequenziale da un
master, quel giorno tutti i golden cambiano e nessuno sa dire se è colpa del nuovo dominio o di
un bug. La derivazione va fatta giusta ora, quando c'è un solo dominio da testare.

## Cosa si costruisce

```rust
/// Domini RNG indipendenti. Aggiungere una variante NON deve alterare
/// le sequenze delle varianti esistenti (D4): per questo il seed di ogni
/// stream deriva dal *nome* del dominio, non dalla sua posizione nell'enum.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RngDomain { Events, Migration, Production }

impl RngDomain {
    /// Sale stabile. Non rinominare una variante senza rigenerare i golden:
    /// il nome è parte del contratto di determinismo.
    const fn salt(self) -> &'static str {
        match self {
            Self::Events     => "brando/rng/v1/events",
            Self::Migration  => "brando/rng/v1/migration",
            Self::Production => "brando/rng/v1/production",
        }
    }
}

pub struct RngSet { streams: [Pcg64; RngDomain::COUNT] }

impl RngSet {
    pub fn from_seed(seed: u64) -> Self { /* blake3(seed_le || salt) -> seed dello stream */ }
    pub fn get(&mut self, d: RngDomain) -> &mut Pcg64;
}
```

Derivazione: `blake3::Hasher` alimentato con `seed.to_le_bytes()` e poi con `salt().as_bytes()`,
e l'XOF usato per riempire il seed di `Pcg64`. Verificare in implementazione la dimensione esatta
del seed di `rand_pcg::Pcg64` e riempirla per intero dall'XOF (non da `seed_from_u64`, che
scarta entropia e rende più facile una collisione tra domini).

Due dettagli che sembrano pedanteria e non lo sono:

- **`get` prende `&mut self`**: un solo dominio in prestito alla volta. Non esporre i campi:
  se due sistemi possono estrarre dallo stesso stream nello stesso tick, l'ordine di estrazione
  diventa un contratto implicito.
- **La posizione dello stream entra nell'hash canonico** (fase 08). Uno stato in cui l'RNG di
  `Events` ha consumato 5 valori non è lo stesso stato in cui ne ha consumati 6, anche se tutto
  il resto coincide: se non entra nell'hash, una divergenza si manifesta molti tick dopo, dove è
  quasi impossibile da attribuire.

`RngSet` è serializzabile per il debug, ma il salvataggio resta `seed + Vec<Command>` (D4).

## Fuori scope

Nessun uso dell'RNG. Nessun evento casuale (M1+), nessuna migrazione (M1). Questa fase produce
solo il generatore e la prova che è affidabile.

## Test

1. **Riproducibilità**: `RngSet::from_seed(42)` estratto 8 volte per dominio produce una
   sequenza confrontata con **valori attesi scritti a mano nel test**. Non `assert_eq!(a, b)`
   tra due istanze — quello passa anche se la derivazione è sbagliata. Vanno hardcodati i numeri
   (li si genera una volta e li si incolla).
2. **Indipendenza tra domini**: le tre sequenze sono diverse a parità di seed. Test banale ma
   coglie l'errore di copia-incolla in cui due domini condividono il sale.
3. **Stabilità all'aggiunta di un dominio**: questo è *il* test. Il test 1 con valori hardcodati
   lo copre già, purché la sua documentazione lo dica esplicitamente:
   ```rust
   /// Se questo test rompe dopo aver AGGIUNTO una variante a RngDomain,
   /// la derivazione è per posizione e non per nome: è un bug, non un golden da rigenerare.
   ```
   Da rafforzare con un test che costruisce un `RngSet` e verifica che l'ordine di *chiamata*
   ai domini non influenzi le rispettive sequenze (estrarre A,B,A,B vs A,A,B,B dà le stesse
   sequenze per dominio).
4. **Sensibilità al seed**: seed 42 e 43 producono sequenze diverse su tutti i domini.

## Verifica

```sh
cargo test -p sim-core rng
```

Poi la prova manuale che chiude la fase: aggiungere temporaneamente
`RngDomain::Test => "brando/rng/v1/test"` come **prima** variante dell'enum e rieseguire i test.
Devono restare **verdi**. Se diventano rossi, la derivazione dipende dalla posizione: si
correggerà per forza il giorno in cui servirà un dominio nuovo, e quel giorno costerà tutti i
golden del progetto. Rimuovere la variante e committare.

**Fatto quando:** i test sono verdi con valori hardcodati e la variante aggiunta in testa
all'enum non li rompe.
