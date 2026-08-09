//! Banco di prova delle prestazioni del tick.
//!
//! Risponde a una domanda sola: **dove finisce il tempo di `step()`**, e in
//! particolare quanto ne consuma l'applicazione dei comandi (passo 1) rispetto
//! ai ricalcoli che la seguono (passi 2 e 3).
//!
//! La distinzione conta perche' i due costi scalano in modo diverso. Il costo
//! di un comando e' O(footprint) e non cresce con la citta'; il ricalcolo
//! della copertura e' proporzionale al numero di provider e si paga **una
//! volta per tick**, non una volta per comando. Le misure `C` ed `E` qui sotto
//! servono a rendere visibile questa differenza, non a produrre un numero solo.
//!
//! ## Come si usa
//!
//! ```text
//! cargo run --release -p xtask -- bench
//! ```
//!
//! Sempre in `--release`: in debug i numeri non dicono niente di utile.
//!
//! Non esiste una soglia di fallimento: i tempi assoluti dipendono dalla
//! macchina. Il modo di usarlo e' eseguirlo **prima e dopo** una modifica
//! sulla stessa macchina e confrontare. Cio' che deve restare stabile in
//! assoluto e' l'`hash` dello stato: se cambia senza che tu abbia cambiato le
//! regole del gioco o le tabelle, l'ottimizzazione ha cambiato la semantica ed
//! e' un bug, non un guadagno.
//!
//! ## Che citta' misura
//!
//! Una citta' sintetica a reticolo, con le **tabelle vere** di `sim-data`:
//! quote di case e provider derivate da capacita' e produzione in tabella, mai
//! da costanti scritte qui (D6). Cambia il bilanciamento, cambia il carico:
//! per questo l'intestazione stampa l'hash del dataset, cosi' una misura che
//! si sposta e' attribuibile.
//!
//! L'unico parametro forzato e' il tesoro iniziale, portato a un valore enorme
//! per togliere di mezzo l'economia: qui si misura il costo del tick, non se
//! la citta' e' finanziabile.

use std::sync::Arc;

use sim_core::{BuildingKindId, Coins, Command, DataSet, Grid, Terrain, TilePos, World, step};

/// Le due taglie misurate per default. Non sono numeri di bilanciamento: sono
/// la scala di riferimento del progetto (200x200, ~15.000 abitanti) e una
/// taglia intermedia che serve a leggere come scalano i costi.
const PROFILI: [(&str, u16, u32); 2] =
    [("meta' partita", 100, 3_000), ("fine partita", 200, 15_000)];

pub fn bench(args: &[String]) -> Result<(), String> {
    let ripetizioni = super::numero(args, "--ripetizioni")?.unwrap_or(40).max(1);
    let lato = super::numero(args, "--lato")?;
    let abitanti = super::numero(args, "--abitanti")?;

    let reale = sim_data::load_default().map_err(|e| format!("tabelle: {e}"))?;
    println!("dataset {}", &reale.hash_hex()[..16]);
    println!(
        "{} ripetizioni per misura — confronta due esecuzioni sulla stessa macchina",
        ripetizioni
    );

    // Con un solo parametro fra --lato e --abitanti si misura quel profilo e
    // basta; senza, si misurano i due di riferimento.
    match (lato, abitanti) {
        (None, None) => {
            for (nome, l, ab) in PROFILI {
                profilo(&reale, nome, l, ab, ripetizioni)?;
            }
        }
        (l, ab) => profilo(
            &reale,
            "su misura",
            l.unwrap_or(200).try_into().unwrap_or(u16::MAX),
            ab.unwrap_or(15_000),
            ripetizioni,
        )?,
    }
    Ok(())
}

fn profilo(
    reale: &DataSet,
    nome: &str,
    lato: u16,
    abitanti: u32,
    ripetizioni: u32,
) -> Result<(), String> {
    println!("\n=== {nome}: {lato}x{lato}, {abitanti} abitanti ===");

    let data = Arc::new(con_tesoro_illimitato(reale));
    let piano = Piano::nuovo(&data, lato, abitanti)?;
    let mut w = costruisci(&data, &piano, lato)?;

    println!(
        "  {} case ({} ab.), {} provider, {} tile strada su {} tile",
        w.n_case(),
        w.popolazione(),
        w.n_edifici(),
        piano.n_strade,
        u32::from(lato) * u32::from(lato),
    );
    println!(
        "  hash dello stato: {}",
        &sim_replay::hash_hex(&sim_replay::hash_world(&w))[..16]
    );
    println!();
    println!("  {:<38} {:>12} {:>12}", "", "mediana", "peggiore");

    // A. Tick a vuoto, niente di sporco: e' cio' che si paga nei tick in cui
    //    il giocatore non costruisce, cioe' la stragrande maggioranza.
    let a = misura(ripetizioni * 5, |_| {
        step(&mut w, &[]);
    });
    riga("A. tick a vuoto, niente di sporco", &a, None);

    // B. Un comando rifiutato: percorso di validazione completo, nessuna
    //    invalidazione e quindi nessun ricalcolo.
    let occupato = Command::PlaceRoad {
        at: piano.strada_qualsiasi,
    };
    let b = misura(ripetizioni * 5, |_| {
        let r = step(&mut w, &[occupato]);
        debug_assert_eq!(r.rejected.len(), 1);
    });
    riga("B. tick, 1 comando rifiutato", &b, None);

    // C. Tanti comandi rifiutati nello stesso tick. Isola il costo marginale
    //    puro di applicare un comando: (C - B) / (LOTTO_RIFIUTI - 1).
    //    Il lotto e' volutamente assurdo perche' il costo per comando e' cosi'
    //    piccolo che sotto il migliaio sparirebbe nel rumore del resto del
    //    tick; non e' uno scenario di gioco, e' uno strumento di misura.
    let molti: Vec<Command> = vec![occupato; LOTTO_RIFIUTI];
    let c = misura(ripetizioni, |_| {
        let r = step(&mut w, &molti);
        debug_assert_eq!(r.rejected.len(), LOTTO_RIFIUTI);
    });
    let per_comando = c.mediana.saturating_sub(b.mediana) / (LOTTO_RIFIUTI as u128 - 1);
    riga(
        &format!("C. tick, {LOTTO_RIFIUTI} comandi rifiutati"),
        &c,
        Some(format!("{per_comando} ns/comando")),
    );

    // D. Un comando accettato. Caso peggiore reale: oggi qualunque
    //    costruzione o demolizione invalida la copertura di *tutti* i
    //    provider, quindi qui si legge il costo del passo 3 per intero.
    //    Si alterna costruzione e demolizione sullo stesso tile: dopo ogni
    //    coppia lo stato e' tornato dov'era.
    let slot = piano.slot_liberi[0];
    let costruisci_uno = Command::PlaceBuilding {
        kind: piano.casa,
        origin: slot,
    };
    let demolisci_uno = Command::Demolish { at: slot };
    let d = misura(ripetizioni, |i| {
        let cmd = if i % 2 == 0 {
            costruisci_uno
        } else {
            demolisci_uno
        };
        let r = step(&mut w, &[cmd]);
        assert!(r.rejected.is_empty(), "comando rifiutato: {:?}", r.rejected);
    });
    riga("D. tick, 1 comando accettato", &d, None);

    // E. Gli stessi comandi, ma tutti in un tick solo. Se il costo fosse per
    //    comando questo varrebbe N volte D; vale poco piu' di 1x, ed e' la
    //    misura che dice che l'applicazione sincrona in blocco non e' il collo
    //    di bottiglia.
    let lotto: Vec<TilePos> = piano.slot_liberi[1..].to_vec();
    let n = lotto.len();
    let costruisci_lotto: Vec<Command> = lotto
        .iter()
        .map(|p| Command::PlaceBuilding {
            kind: piano.casa,
            origin: *p,
        })
        .collect();
    let demolisci_lotto: Vec<Command> =
        lotto.iter().map(|p| Command::Demolish { at: *p }).collect();
    let e = misura(ripetizioni, |i| {
        let cmds = if i % 2 == 0 {
            &costruisci_lotto
        } else {
            &demolisci_lotto
        };
        let r = step(&mut w, cmds);
        assert!(r.rejected.is_empty(), "comando rifiutato: {:?}", r.rejected);
    });
    let rapporto = e.mediana.saturating_mul(1000) / d.mediana.max(1);
    riga(
        &format!("E. tick, {n} comandi accettati"),
        &e,
        Some(format!(
            "{}.{:03}x il costo di D",
            rapporto / 1000,
            rapporto % 1000
        )),
    );

    // F. Una strada, non un edificio. E' il caso peggiore strutturale: cambiare
    //    la topologia invalida *tutto* — la rete va rietichettata e nessuna
    //    furbizia sull'incrementalita' della copertura puo' aiutare. D misura
    //    il caso frequente in partita (si costruisce), F quello che nessuna
    //    cache potra' mai coprire, e servono entrambi: un'ottimizzazione che
    //    fa scendere D e lascia F dov'e' ha coperto meta' del problema.
    let posa = Command::PlaceRoad {
        at: piano.slot_strada,
    };
    let togli = Command::Demolish {
        at: piano.slot_strada,
    };
    let f = misura(ripetizioni, |i| {
        let cmd = if i % 2 == 0 { posa } else { togli };
        let r = step(&mut w, &[cmd]);
        assert!(r.rejected.is_empty(), "comando rifiutato: {:?}", r.rejected);
    });
    riga("F. tick, 1 strada posata o tolta", &f, None);

    // G. Il solo passo 3, senza il resto del tick. E' la misura che attribuisce
    //    il costo invece di dedurlo per differenza da D e A, ed e' quella da
    //    guardare quando si ottimizza la copertura: D contiene anche
    //    produzione, eventi e validazione del comando.
    let g = misura(ripetizioni, |_| {
        let _ = sim_core::coverage::calcola_da_zero(&w);
    });
    let per_provider = g.mediana / w.n_edifici().max(1) as u128;
    riga(
        "G. solo calcola_da_zero (passo 3)",
        &g,
        Some(format!("{per_provider} ns/provider")),
    );

    Ok(())
}

// --- costruzione della citta' ----------------------------------------------

/// Quante case e quanti provider per ogni tipo, e dove metterli.
///
/// Il reticolo ha passo 4: isolati di 3x3 tile, ognuno con cinque celle 1x1
/// sull'anello (tutte affacciate su strada) e un quadrato 2x2 al centro. La
/// cella centrale non tocca nessuna strada, quindi da sola sarebbe inservibile:
/// il quadrato 2x2 esiste apposta per usarla.
struct Piano {
    casa: BuildingKindId,
    n_case: u32,
    /// Provider 1x1, sull'anello: (tipo, quanti).
    piccoli: Vec<(BuildingKindId, u32)>,
    /// Provider 2x2, al centro dell'isolato.
    grandi: Vec<(BuildingKindId, u32)>,
    blocchi_per_lato: u32,
    blocchi_citta: u32,
    n_strade: u32,
    /// Slot 1x1 liberi oltre la citta', per le misure D ed E.
    slot_liberi: Vec<TilePos>,
    /// Slot riservato alla misura F, che ci costruisce e demolisce una strada.
    /// Separato da `slot_liberi` perche' D ed E possono lasciarci sopra un
    /// edificio se il numero di ripetizioni e' dispari.
    slot_strada: TilePos,
    strada_qualsiasi: TilePos,
}

/// Quante celle 1x1 utilizzabili ha un isolato sull'anello.
const SLOT_ANELLO: u32 = 5;
/// Quanti slot liberi servono a D (1), E (i 50 restanti) ed F (1).
///
/// Passando da 51 a 52 il reticolo non cambia — `52.div_ceil(5)` e
/// `51.div_ceil(5)` fanno entrambi 11 isolati — quindi le misure restano
/// confrontabili con quelle registrate in `plan/09-invarianti-chiusura.md`.
const SLOT_DI_PROVA: u32 = 52;
/// Quanti comandi invalidi si mandano nella misura C.
const LOTTO_RIFIUTI: usize = 10_000;

impl Piano {
    fn nuovo(data: &DataSet, lato: u16, abitanti: u32) -> Result<Self, String> {
        let casa = data
            .buildings
            .iter()
            .position(|d| d.e_una_casa())
            .and_then(|i| u16::try_from(i).ok())
            .map(BuildingKindId::new)
            .ok_or("il dataset non contiene nessuna casa")?;

        let per_casa = u32::from(
            *data
                .rules
                .abitanti_per_livello_casa
                .first()
                .ok_or("rules.abitanti_per_livello_casa e' vuoto")?,
        );
        if per_casa == 0 {
            return Err("una casa con zero abitanti non fa una citta'".into());
        }
        let n_case = abitanti.div_ceil(per_casa);
        // Gli abitanti effettivamente insediati: `abitanti` arrotondato in su
        // all'ultima casa piena. E' su questo, non su `n_case`, che si contano
        // sia la capacita' dei provider sia il consumo.
        let popolazione = n_case * per_casa;
        let consumo_abitante = i64::from(data.rules.consumo_cibo_per_abitante.to_millis());

        // Le quote dei provider escono dalle tabelle, non da qui: quanti ne
        // servono per coprire la popolazione alla capacita' dichiarata — che
        // e' in abitanti — e, per chi produce, quanti per sostenerne il
        // consumo.
        let mut piccoli = Vec::new();
        let mut grandi = Vec::new();
        for (i, def) in data.buildings.iter().enumerate() {
            let Some(servizio) = def.servizio.as_ref() else {
                continue;
            };
            let kind = u16::try_from(i)
                .map(BuildingKindId::new)
                .map_err(|_| "troppi tipi di edificio")?;
            let capacita =
                u32::from(servizio.capacita(1).ok_or_else(|| {
                    format!("'{}' non dichiara la capacita' al livello 1", def.id)
                })?);
            if capacita == 0 {
                return Err(format!("'{}' ha capacita' 0", def.id));
            }
            let mut quanti = popolazione.div_ceil(capacita);
            if let Some(prod) = def.produzione_per_tick {
                let per_provider = i64::from(prod.to_millis()) / consumo_abitante.max(1);
                if per_provider > 0 {
                    let sostenibili = u32::try_from(per_provider).unwrap_or(u32::MAX);
                    quanti = quanti.max(popolazione.div_ceil(sostenibili));
                }
            }
            match def.footprint {
                (1, 1) => piccoli.push((kind, quanti)),
                (2, 2) => grandi.push((kind, quanti)),
                f => {
                    return Err(format!(
                        "'{}' ha footprint {f:?}: il banco di prova sa disporre solo 1x1 e 2x2, aggiornalo",
                        def.id
                    ));
                }
            }
        }

        // Isolati necessari: quelli che servono all'anello, e comunque non
        // meno di quanti ne chiedono i provider 2x2, uno per isolato.
        let su_anello = n_case + piccoli.iter().map(|(_, q)| *q).sum::<u32>();
        let per_grandi = grandi.iter().map(|(_, q)| *q).sum::<u32>();
        let blocchi_citta = su_anello.div_ceil(SLOT_ANELLO).max(per_grandi).max(1);
        // Gli isolati per D ed E stanno dopo la citta'.
        let blocchi_totali = blocchi_citta + SLOT_DI_PROVA.div_ceil(SLOT_ANELLO);
        let blocchi_per_lato = (1..).find(|n| n * n >= blocchi_totali).unwrap_or(1);

        // Il reticolo copre solo la parte di mappa occupata, con un tile di
        // strada di chiusura: una citta' che non riempie la mappa e' il caso
        // normale, e stendere strade ovunque gonfierebbe la misura.
        let lato_usato = blocchi_per_lato * 4 + 1;
        if lato_usato > u32::from(lato) {
            return Err(format!(
                "servono {lato_usato} tile di lato per {abitanti} abitanti, la mappa ne ha {lato}"
            ));
        }

        let mut piano = Self {
            casa,
            n_case,
            piccoli,
            grandi,
            blocchi_per_lato,
            blocchi_citta,
            // Il quadrato coperto dal reticolo meno i 3x3 degli isolati.
            n_strade: lato_usato * lato_usato - blocchi_per_lato * blocchi_per_lato * 9,
            slot_liberi: Vec::new(),
            slot_strada: TilePos::new(0, 0),
            strada_qualsiasi: TilePos::new(0, 0),
        };
        piano.slot_liberi = (blocchi_citta..blocchi_totali)
            .flat_map(|b| {
                (0..SLOT_ANELLO).filter_map(move |s| {
                    let (bx, by) = (b % blocchi_per_lato, b / blocchi_per_lato);
                    cella_anello(bx, by, s)
                })
            })
            .take(SLOT_DI_PROVA as usize)
            .collect();
        if piano.slot_liberi.len() < SLOT_DI_PROVA as usize {
            return Err("non restano abbastanza slot liberi per le misure D, E ed F".into());
        }
        piano.slot_strada = piano
            .slot_liberi
            .pop()
            .ok_or("nessuno slot per la misura F")?;
        Ok(piano)
    }
}

/// Angolo in alto a sinistra di un isolato, in tile.
const fn angolo(bx: u32, by: u32) -> (u32, u32) {
    (bx * 4 + 1, by * 4 + 1)
}

/// La `s`-esima cella 1x1 dell'anello di un isolato.
///
/// L'anello e' l'isolato meno il quadrato 2x2 in basso a destra: cinque celle,
/// tutte affacciate su una strada.
fn cella_anello(bx: u32, by: u32, s: u32) -> Option<TilePos> {
    let (ox, oy) = angolo(bx, by);
    let (dx, dy) = match s {
        0 => (0, 0),
        1 => (1, 0),
        2 => (2, 0),
        3 => (0, 1),
        4 => (0, 2),
        _ => return None,
    };
    let x = u8::try_from(ox + dx).ok()?;
    let y = u8::try_from(oy + dy).ok()?;
    Some(TilePos::new(x, y))
}

/// Angolo del quadrato 2x2 al centro dell'isolato.
fn cella_centro(bx: u32, by: u32) -> Option<TilePos> {
    let (ox, oy) = angolo(bx, by);
    Some(TilePos::new(
        u8::try_from(ox + 1).ok()?,
        u8::try_from(oy + 1).ok()?,
    ))
}

/// Distribuisce `quota` elementi su `su` posizioni in modo uniforme.
///
/// Bresenham su interi: niente float e nessun accumulo di errore, e i provider
/// finiscono sparsi invece che ammassati all'inizio — che cambierebbe la forma
/// del carico sul BFS.
struct Sparpaglia {
    quota: u32,
    su: u32,
    fatti: u32,
}

impl Sparpaglia {
    const fn nuovo(quota: u32, su: u32) -> Self {
        Self {
            quota,
            su: if su == 0 { 1 } else { su },
            fatti: 0,
        }
    }

    fn tocca(&mut self, i: u32) -> bool {
        if (i + 1) * self.quota / self.su > self.fatti {
            self.fatti += 1;
            true
        } else {
            false
        }
    }
}

fn costruisci(data: &Arc<DataSet>, piano: &Piano, lato: u16) -> Result<World, String> {
    let grid = Grid::new(lato, lato, Terrain::Pianura).map_err(|e| format!("griglia: {e}"))?;
    let mut w = World::new(grid, Arc::clone(data), 42);

    // Le strade prima, tutte in un tick: la topologia deve esserci prima che
    // gli edifici cerchino un ingresso.
    let mut strade = Vec::new();
    let lato_usato = piano.blocchi_per_lato * 4 + 1;
    for y in 0..lato_usato {
        for x in 0..lato_usato {
            if x % 4 == 0 || y % 4 == 0 {
                let (Ok(x), Ok(y)) = (u8::try_from(x), u8::try_from(y)) else {
                    continue;
                };
                strade.push(Command::PlaceRoad {
                    at: TilePos::new(x, y),
                });
            }
        }
    }
    applica(&mut w, &strade)?;

    let mut edifici = Vec::new();
    let mut piccoli: Vec<(BuildingKindId, Sparpaglia)> = piano
        .piccoli
        .iter()
        .map(|(k, q)| (*k, Sparpaglia::nuovo(*q, piano.blocchi_citta * SLOT_ANELLO)))
        .collect();
    let mut grandi: Vec<(BuildingKindId, Sparpaglia)> = piano
        .grandi
        .iter()
        .map(|(k, q)| (*k, Sparpaglia::nuovo(*q, piano.blocchi_citta)))
        .collect();
    let mut case_rimaste = piano.n_case;

    for b in 0..piano.blocchi_citta {
        let (bx, by) = (b % piano.blocchi_per_lato, b / piano.blocchi_per_lato);

        for (kind, sp) in &mut grandi {
            if sp.tocca(b) {
                if let Some(origin) = cella_centro(bx, by) {
                    edifici.push(Command::PlaceBuilding {
                        kind: *kind,
                        origin,
                    });
                }
                break;
            }
        }

        for s in 0..SLOT_ANELLO {
            let Some(origin) = cella_anello(bx, by, s) else {
                continue;
            };
            let i = b * SLOT_ANELLO + s;
            // Il primo provider che rivendica questo slot lo prende; gli altri
            // scivolano al prossimo, perche' `tocca` avanza solo quando piazza.
            let mut provider = None;
            for (k, sp) in &mut piccoli {
                if sp.tocca(i) {
                    provider = Some(*k);
                    break;
                }
            }
            let kind = match provider {
                Some(k) => k,
                None if case_rimaste > 0 => {
                    case_rimaste -= 1;
                    piano.casa
                }
                None => continue,
            };
            edifici.push(Command::PlaceBuilding { kind, origin });
        }
    }
    applica(&mut w, &edifici)?;
    Ok(w)
}

fn applica(w: &mut World, cmds: &[Command]) -> Result<(), String> {
    let r = step(w, cmds);
    match r.rejected.first() {
        None => Ok(()),
        Some((i, e)) => Err(format!(
            "il banco di prova ha prodotto un comando invalido ({} su {}): {e}",
            i,
            cmds.len()
        )),
    }
}

/// Copia del dataset col solo tesoro iniziale portato a un valore fuori scala.
///
/// Non e' un numero di bilanciamento: e' il modo di togliere l'economia dalla
/// misura, cosi' la citta' si costruisce sempre per intero. Tutto il resto
/// resta quello di `sim-data`.
fn con_tesoro_illimitato(reale: &DataSet) -> DataSet {
    let mut rules = reale.rules.clone();
    rules.tesoro_iniziale = Coins::new(i32::MAX / 2);
    DataSet::new(rules, reale.terrain.clone(), reale.buildings.clone())
}

// --- misura e stampa -------------------------------------------------------

struct Misura {
    mediana: u128,
    peggiore: u128,
}

fn misura(ripetizioni: u32, mut f: impl FnMut(u32)) -> Misura {
    // Due giri a vuoto prima di cronometrare: il primo tick di una misura paga
    // allocazioni e cache fredde, e da solo triplicava il "peggiore". Sono due
    // e non uno perche' D ed E alternano costruzione e demolizione, e dopo una
    // coppia lo stato e' tornato dov'era.
    f(0);
    f(1);

    let mut campioni: Vec<u128> = (0..ripetizioni).map(|i| cronometra(|| f(i))).collect();
    campioni.sort_unstable();
    Misura {
        // Mediana e non media: una preemption del sistema operativo sposta la
        // media e non la mediana, e qui interessa il costo tipico.
        mediana: campioni.get(campioni.len() / 2).copied().unwrap_or(0),
        peggiore: campioni.last().copied().unwrap_or(0),
    }
}

/// L'unico punto del progetto che legge l'orologio.
///
/// `clippy.toml` vieta `Instant::now` perche' **il core** non deve accedere
/// all'orologio (D4). Qui siamo nel runner headless, fuori dal core, e
/// cronometrare e' precisamente il suo mestiere: l'eccezione sta in un posto
/// solo apposta, invece che allentare la regola in `clippy.toml`.
#[allow(clippy::disallowed_methods)]
fn cronometra(f: impl FnOnce()) -> u128 {
    let t0 = std::time::Instant::now();
    f();
    t0.elapsed().as_nanos()
}

fn riga(nome: &str, m: &Misura, nota: Option<String>) {
    println!(
        "  {:<38} {:>12} {:>12}  {}",
        nome,
        durata(m.mediana),
        durata(m.peggiore),
        nota.unwrap_or_default()
    );
}

/// Formatta nanosecondi senza float: `float_arithmetic` e' `deny` nel
/// workspace, e comunque i decimali si ottengono meglio con una divisione
/// intera che con un arrotondamento binario.
fn durata(ns: u128) -> String {
    if ns >= 1_000_000 {
        format!("{}.{:03} ms", ns / 1_000_000, (ns / 1_000) % 1_000)
    } else if ns >= 1_000 {
        format!("{}.{:03} us", ns / 1_000, ns % 1_000)
    } else {
        format!("{ns} ns")
    }
}
