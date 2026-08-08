//! Invarianti del core (CLAUDE.md, Testing punto 1).
//!
//! Ognuno di questi ha trovato o trovera' un bug vero; nessuno va silenziato
//! per far passare la CI. E' il posto dove si legge *cosa il progetto
//! garantisce*, indipendentemente da come e' organizzato il codice — quello
//! che un contributore, o un LLM che lavora sul repo, legge per capire cosa
//! non deve rompere.
//!
//! | Invariante | Dove |
//! |---|---|
//! | Nessuna sovrapposizione, in entrambe le direzioni tile <-> edificio | qui, `invarianti_nessuna_sovrapposizione` |
//! | Ogni tile occupato risolve a un id vivo nello slotmap | qui, `invarianti_nessuna_sovrapposizione` |
//! | Popolazione mai negativa e coerente con le case esistenti | qui, `invarianti_popolazione` |
//! | Tesoro coerente: iniziale meno la somma dei costi accettati | qui, `invarianti_tesoro` |
//! | Cibo conservato: prodotto = consumato + perso + giacenza | qui, `invarianti_conservazione_del_cibo` |
//! | Una casa coperta dal cibo mangia sempre | qui, `invarianti_coperta_significa_sfamata` |
//! | Un comando rifiutato non muta niente | qui, `invarianti_i_rifiuti_non_mutano` |
//! | Nessun panic su comandi arbitrari, inclusi malformati | qui, `nessun_panic_su_diecimila_comandi` |
//! | Copertura incrementale identica a quella da zero | `copertura.rs`, `equivalenza_copertura` |
//! | Etichettatura della rete indipendente dall'ordine di costruzione | `strade.rs`, `rete_etichettatura_non_dipende_dall_ordine` |
//! | Determinismo: stesso seed e stessi comandi, stesso hash | `sim-replay/tests/golden.rs` |
//!
//! Gli ultimi tre stanno accanto al sistema che verificano, non qui: hanno
//! bisogno di scenari costruiti apposta, e spostarli renderebbe questo file
//! meno leggibile senza renderli piu' veri.
//!
//! Generatore condiviso: sequenze di `Command` arbitrari — inclusi invalidi,
//! in proporzione significativa — su una griglia 32x32, poi N tick. Un
//! generatore che produce solo comandi validi verifica un decimo di quello
//! che sembra verificare.
//!
//! **Debito noto.** Un target `cargo-fuzz` vero
//! (`fuzz/fuzz_targets/commands.rs`) non c'e': `proptest` con molti casi piu'
//! il fuzz deterministico qui sotto coprono gia' il punto 3 di `CLAUDE.md`, e
//! un target a meta' sarebbe peggio di nessun target. Da fare quando una
//! meccanica nuova allarghera' lo spazio dei comandi.

mod comune;

use comune::*;
use proptest::prelude::*;
use sim_core::{Coins, Command, CommandError, Occupante, TilePos, World};

/// Lato della griglia dei test.
const LATO: u8 = 32;
/// Le coordinate generate arrivano oltre il bordo di proposito.
const LATO_GEN: u8 = LATO + 2;
/// I `kind` generati superano i tre in tabella: l'LLM ne produrra' di
/// inesistenti (CLAUDE.md, interfaccia AI).
const KIND_GEN: u16 = 5;

// --- invarianti, come funzioni riusabili ------------------------------------

/// Ogni tile ha al piu' un occupante, e per ogni edificio i tile del suo
/// footprint puntano a lui. Doppia direzione: e' cio' che coglie i bug di
/// demolizione.
fn nessuna_sovrapposizione(w: &World) -> Result<(), String> {
    // tile -> id: ogni occupante dichiarato risolve a un id vivo.
    let mut occupati = 0usize;
    for idx in w.grid().indices() {
        let tile = w.grid().get(idx).ok_or("indice fuori dalla griglia")?;
        if tile.occupante().is_none() {
            continue;
        }
        occupati += 1;
        match w.occupante(idx) {
            Some(Occupante::Edificio(id)) => {
                if w.building(id).is_none() {
                    return Err(format!("{idx:?} punta a un edificio morto"));
                }
            }
            Some(Occupante::Casa(id)) => {
                if w.house(id).is_none() {
                    return Err(format!("{idx:?} punta a una casa morta"));
                }
            }
            None => return Err(format!("{idx:?} e' occupato ma non risolve a nessun id")),
        }
        if tile.flags.has_road() {
            return Err(format!("{idx:?} ha insieme strada e occupante"));
        }
    }

    // id -> tile: ogni edificio possiede esattamente i tile del suo footprint.
    let mut attesi = 0usize;
    for (id, b) in w.buildings() {
        let def = w
            .data()
            .def(b.kind)
            .ok_or_else(|| format!("edificio {id:?} con kind sconosciuto"))?;
        attesi += usize::from(def.tile_occupati());
        for (dx, dy) in tile_del_footprint(def.footprint) {
            let p = TilePos::new(b.origin.x + dx, b.origin.y + dy);
            let idx = w
                .grid()
                .idx(p)
                .ok_or_else(|| format!("edificio {id:?} sfora la mappa in {p:?}"))?;
            if w.occupante(idx) != Some(Occupante::Edificio(id)) {
                return Err(format!("{p:?} non appartiene a {id:?}"));
            }
        }
    }
    for (id, h) in w.houses() {
        attesi += 1; // le case sono 1x1 in M0
        let idx = w
            .grid()
            .idx(h.origin)
            .ok_or_else(|| format!("casa {id:?} fuori mappa"))?;
        if w.occupante(idx) != Some(Occupante::Casa(id)) {
            return Err(format!("{:?} non appartiene a {id:?}", h.origin));
        }
    }

    if occupati != attesi {
        return Err(format!(
            "tile occupati {occupati}, attesi {attesi}: c'e' un tile orfano o una sovrapposizione"
        ));
    }
    Ok(())
}

/// Popolazione mai negativa e coerente con le case esistenti (D5).
fn popolazione_coerente(w: &World) -> Result<(), String> {
    let attesa = w.n_case() as u32 * u32::from(ABITANTI_PER_CASA);
    if w.popolazione() != attesa {
        return Err(format!(
            "popolazione {} ma {} case da {ABITANTI_PER_CASA}",
            w.popolazione(),
            w.n_case()
        ));
    }
    Ok(())
}

/// Una casa coperta dal servizio cibo ha mangiato in questo tick.
///
/// Non e' una proprieta' del codice della copertura da sola: discende dal
/// **bilanciamento**. Se un provider di cibo potesse assegnarsi piu' abitanti
/// di quanti ne sfami, le eccedenti resterebbero coperte e affamate per
/// sempre, perche' la contesa la vince il primo provider. Vale perche' la
/// capacita' dichiarata non supera cio' che la produzione sostiene — che e'
/// esattamente cio' che `DataSet::capacita_cibo_insostenibile` presidia, e che
/// per la fixture di questi test fissa
/// `la_fixture_rispetta_la_coerenza_fra_capacita_e_produzione`.
///
/// `House::servita` per il cibo e' scritto dal passo 4 e vale "ha mangiato";
/// `Coverage` vale "e' raggiunta da una fattoria". Qui i due devono
/// coincidere: se divergono, la fame e' tornata a essere uno stato assorbente.
fn coperta_significa_sfamata(w: &World) -> Result<(), String> {
    for (id, h) in w.houses() {
        let coperta = w.coverage().e_servita(id, sim_core::ServiceKind::Cibo);
        let ha_mangiato = h.servita.get(sim_core::ServiceKind::Cibo);
        if coperta != ha_mangiato {
            return Err(format!(
                "{id:?} in {:?}: coperta dal cibo = {coperta}, ha mangiato = {ha_mangiato}",
                h.origin
            ));
        }
    }
    Ok(())
}

fn tile_del_footprint((w, h): (u8, u8)) -> impl Iterator<Item = (u8, u8)> {
    (0..h).flat_map(move |dy| (0..w).map(move |dx| (dx, dy)))
}

/// Costo di un comando accettato, per il bilancio del tesoro.
fn costo_accettato(w: &World, cmd: &Command) -> Coins {
    match cmd {
        Command::Demolish { .. } => Coins::ZERO,
        Command::PlaceRoad { at } => w
            .grid()
            .at(*at)
            .and_then(|t| w.data().terrain(t.terrain))
            .map_or(Coins::ZERO, |d| d.costo_strada),
        Command::PlaceBuilding { kind, .. } => w.data().def(*kind).map_or(Coins::ZERO, |d| d.costo),
    }
}

// --- generatori -------------------------------------------------------------

fn comando() -> impl Strategy<Value = Command> {
    prop_oneof![
        3 => (0..LATO_GEN, 0..LATO_GEN)
            .prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        4 => (0..KIND_GEN, 0..LATO_GEN, 0..LATO_GEN)
            .prop_map(|(k, x, y)| Command::PlaceBuilding {
                kind: sim_core::BuildingKindId::new(k),
                origin: TilePos::new(x, y),
            }),
        2 => (0..LATO_GEN, 0..LATO_GEN)
            .prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

/// Sequenza di tick, ognuno con i suoi comandi.
fn partita() -> impl Strategy<Value = Vec<Vec<Command>>> {
    prop::collection::vec(prop::collection::vec(comando(), 0..6), 1..12)
}

// --- i test ------------------------------------------------------------------

proptest! {
    /// Nessuna sovrapposizione, in entrambe le direzioni tile <-> edificio.
    #[test]
    fn invarianti_nessuna_sovrapposizione(p in partita()) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = nessuna_sovrapposizione(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Ogni tile occupato risolve a un id vivo nello slotmap, e la
    /// popolazione resta coerente con le case.
    #[test]
    fn invarianti_popolazione(p in partita()) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = popolazione_coerente(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Tesoro coerente: iniziale meno la somma dei costi accettati. In M0 non
    /// ci sono entrate, quindi vale come uguaglianza esatta.
    #[test]
    fn invarianti_tesoro(p in partita()) {
        let mut w = mondo();
        let mut speso = Coins::ZERO;

        for cmds in &p {
            // Il costo va letto **prima** del tick: dopo, il tile su cui si
            // e' costruita la strada e' cambiato.
            let costi: Vec<Coins> = cmds.iter().map(|c| costo_accettato(&w, c)).collect();
            let r = tick(&mut w, cmds);

            for (i, costo) in costi.iter().enumerate() {
                let rifiutato = r.rejected.iter().any(|(j, _)| *j == i);
                if !rifiutato {
                    speso = speso.checked_add(*costo).expect("i costi non traboccano");
                }
            }

            let atteso = Coins::new(TESORO_INIZIALE)
                .checked_sub(speso)
                .expect("il tesoro non trabocca");
            prop_assert_eq!(w.economy().tesoro, atteso);
            prop_assert!(!w.economy().tesoro.is_negative(), "tesoro negativo");
        }
    }

    /// Cibo conservato: `prodotto == consumato + perso + giacenza`, come
    /// uguaglianza esatta. Il generatore costruisce anche fattorie e le
    /// demolisce, quindi copre entrambi i termini di perdita.
    #[test]
    fn invarianti_conservazione_del_cibo(p in partita()) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            tick(&mut w, &[]);
            let l = w.food();
            prop_assert_eq!(
                l.atteso_in_giacenza(),
                w.giacenza_totale(),
                "prodotto {} != consumato {} + perso_giacenza {} + perso_demolizione {} + giacenza",
                l.prodotto, l.consumato, l.perso_per_giacenza_piena, l.perso_per_demolizione
            );
        }
    }

    /// Una casa coperta dal cibo mangia: la fame e' mancanza di copertura, mai
    /// uno stato in cui si resti pur essendo serviti.
    #[test]
    fn invarianti_coperta_significa_sfamata(p in partita()) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            if let Err(e) = coperta_significa_sfamata(&w) {
                return Err(TestCaseError::fail(e));
            }
            // Anche a regime, non solo nel tick della costruzione: una
            // fattoria appena nata ha la giacenza del primo tick, che
            // maschererebbe un deficit strutturale.
            for _ in 0..3 {
                tick(&mut w, &[]);
                if let Err(e) = coperta_significa_sfamata(&w) {
                    return Err(TestCaseError::fail(e));
                }
            }
        }
    }

    /// Un comando rifiutato non muta niente: lo stato dopo un tick di soli
    /// comandi rifiutati e' quello di prima, tick a parte.
    #[test]
    fn invarianti_i_rifiuti_non_mutano(p in partita()) {
        let mut w = mondo();
        for cmds in &p {
            let prima = w.clone();
            let r = tick(&mut w, cmds);
            if r.rejected.len() == cmds.len() {
                prop_assert_eq!(w.grid(), prima.grid());
                prop_assert_eq!(w.economy(), prima.economy());
                prop_assert_eq!(w.n_edifici(), prima.n_edifici());
                prop_assert_eq!(w.n_case(), prima.n_case());
            }
        }
    }
}

// --- fuzzing dei comandi (CLAUDE.md, Testing punto 3) ------------------------

/// PRNG locale al test: deterministico e senza dipendenze aggiuntive.
/// Non e' `RngSet` di proposito — questo test non deve dipendere dal
/// generatore del gioco, che e' esso stesso sotto test.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// 10.000 comandi casuali su 1.000 tick: il core non panica mai e `rejected`
/// cresce coerentemente.
///
/// Non e' un property test perche' una sola esecuzione lunga costa quanto
/// migliaia di casi corti, e ripeterla 256 volte non aggiungerebbe copertura:
/// il seed e' fisso, quindi e' riproducibile come un golden.
#[test]
fn nessun_panic_su_diecimila_comandi() {
    const TICK: usize = 1_000;
    const COMANDI: usize = 10_000;

    let mut rng = SplitMix64(0x5EED);
    let mut w = mondo();
    let mut emessi = 0usize;
    let mut rifiutati = 0usize;
    let mut accettati = 0usize;

    for t in 0..TICK {
        let quanti = if emessi < COMANDI {
            (rng.range(21) as usize).min(COMANDI - emessi)
        } else {
            0
        };
        let cmds: Vec<Command> = (0..quanti).map(|_| comando_casuale(&mut rng)).collect();
        emessi += cmds.len();

        let r = tick(&mut w, &cmds);

        assert!(
            r.rejected.len() <= cmds.len(),
            "piu' rifiuti che comandi al tick {t}"
        );
        for (i, _) in &r.rejected {
            assert!(*i < cmds.len(), "indice di rifiuto fuori range al tick {t}");
        }
        rifiutati += r.rejected.len();
        accettati += cmds.len() - r.rejected.len();

        assert!(
            !w.economy().tesoro.is_negative(),
            "tesoro negativo al tick {t}"
        );
        nessuna_sovrapposizione(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
        popolazione_coerente(&w).unwrap_or_else(|e| panic!("tick {t}: {e}"));
    }

    assert_eq!(w.tick(), TICK as u32);
    assert_eq!(emessi, COMANDI, "il test deve emettere tutti i comandi");
    assert_eq!(accettati + rifiutati, emessi);
    assert!(rifiutati > 0, "il generatore non produce comandi cattivi");
    assert!(accettati > 0, "il generatore non produce comandi validi");
}

fn comando_casuale(rng: &mut SplitMix64) -> Command {
    let x = rng.range(u64::from(LATO_GEN)) as u8;
    let y = rng.range(u64::from(LATO_GEN)) as u8;
    match rng.range(9) {
        0..=2 => Command::PlaceRoad {
            at: TilePos::new(x, y),
        },
        3..=6 => Command::PlaceBuilding {
            kind: sim_core::BuildingKindId::new(rng.range(u64::from(KIND_GEN)) as u16),
            origin: TilePos::new(x, y),
        },
        _ => Command::Demolish {
            at: TilePos::new(x, y),
        },
    }
}

/// La fixture dev'essere bilanciata come le tabelle vere.
///
/// I test di `sim-core` girano su un dataset costruito a mano, che non passa
/// per la validazione di `sim-data`. Senza questo controllo la fixture
/// potrebbe scivolare su numeri che in produzione sarebbero rifiutati, e
/// `invarianti_coperta_significa_sfamata` verificherebbe una proprieta' che il
/// gioco vero non ha.
#[test]
fn la_fixture_rispetta_la_coerenza_fra_capacita_e_produzione() {
    let d = dataset();
    assert_eq!(
        d.capacita_cibo_insostenibile(),
        vec![],
        "la fixture dichiara piu' capacita' di quanta la produzione ne sostenga"
    );
}

/// Un comando fuori mappa e' sempre rifiutato, mai un panic e mai un
/// silenzioso successo.
#[test]
fn fuori_mappa_sempre_rifiutato() {
    let mut w = mondo_con(4, 4);
    let fuori = [
        Command::PlaceRoad { at: pos(4, 0) },
        Command::PlaceRoad { at: pos(0, 4) },
        Command::PlaceRoad { at: pos(255, 255) },
        Command::PlaceBuilding {
            kind: CASA,
            origin: pos(9, 9),
        },
        Command::Demolish { at: pos(200, 1) },
    ];
    let r = tick(&mut w, &fuori);
    assert_eq!(r.rejected.len(), fuori.len());
    for (_, e) in &r.rejected {
        assert!(matches!(e, CommandError::OutOfBounds(_)), "{e:?}");
    }
}
