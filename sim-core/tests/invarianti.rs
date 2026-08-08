//! Invarianti del core (CLAUDE.md, Testing punto 1).
//!
//! Ognuno di questi ha trovato o trovera' un bug vero; nessuno va silenziato
//! per far passare la CI. Il file cresce fase per fase ed e' il posto dove si
//! legge *cosa il progetto garantisce*, indipendentemente da come e'
//! organizzato il codice.
//!
//! Generatore condiviso: sequenze di `Command` arbitrari — inclusi invalidi,
//! in proporzione significativa — su una griglia 32x32, poi N tick. Un
//! generatore che produce solo comandi validi verifica un decimo di quello
//! che sembra verificare.

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
