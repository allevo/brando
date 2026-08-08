//! Fase 07 — produzione, consumo e conservazione del cibo.

mod comune;

use comune::*;
use proptest::prelude::*;
use sim_core::{Command, Event, Milli, ServiceKind, TilePos, World};

fn strade(w: &mut World, celle: &[(u8, u8)]) {
    let cmds: Vec<_> = celle
        .iter()
        .map(|(x, y)| Command::PlaceRoad {
            at: TilePos::new(*x, *y),
        })
        .collect();
    let r = tick(w, &cmds);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
}

fn costruisci(w: &mut World, kind: sim_core::BuildingKindId, x: u8, y: u8) {
    let r = tick(
        w,
        &[Command::PlaceBuilding {
            kind,
            origin: pos(x, y),
        }],
    );
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
}

/// La conservazione, come uguaglianza esatta.
fn conservazione(w: &World) -> Result<(), String> {
    let l = w.food();
    let giacenza = w.giacenza_totale();
    if l.atteso_in_giacenza() != giacenza {
        return Err(format!(
            "prodotto {} != consumato {} + perso_giacenza {} + perso_demolizione {} \
             + giacenza {giacenza}",
            l.prodotto, l.consumato, l.perso_per_giacenza_piena, l.perso_per_demolizione
        ));
    }
    Ok(())
}

/// Giacenze sempre tra zero e il massimo dichiarato in tabella.
fn giacenze_nel_range(w: &World) -> Result<(), String> {
    for (id, b) in w.buildings() {
        let Some(def) = w.data().def(b.kind) else {
            continue;
        };
        let Some(max) = def.giacenza_max else {
            continue;
        };
        if b.stock.is_negative() {
            return Err(format!("{id:?} ha giacenza negativa: {}", b.stock));
        }
        if b.stock > max {
            return Err(format!("{id:?} sfora la giacenza massima: {}", b.stock));
        }
    }
    Ok(())
}

/// Consumo di cibo di una casa in un tick, letto dal `DataSet`.
fn consumo_per_casa(w: &World) -> Milli {
    let abitanti = i32::from(w.data().rules.abitanti_per_livello_casa[0]);
    w.data()
        .rules
        .consumo_cibo_per_abitante
        .checked_mul_int(abitanti)
        .expect("consumo di una casa")
}

/// Fattoria in (2,2) affacciata su una strada orizzontale, piu' `n` case.
fn scenario_fattoria(n: u8) -> World {
    let mut w = mondo();
    let celle: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
    strade(&mut w, &celle);
    costruisci(&mut w, FATTORIA, 2, 2); // occupa (2,2)..(3,3), tocca la riga 4
    for i in 0..n {
        costruisci(&mut w, CASA, 5 + i, 5);
    }
    w
}

// --- 5. saturazione ---------------------------------------------------------

#[test]
fn senza_case_la_giacenza_cresce_fino_al_massimo_e_si_ferma() {
    let mut w = scenario_fattoria(0);
    let (id, _) = w.buildings().next().expect("la fattoria");
    let def = w
        .data()
        .def(w.building(id).expect("viva").kind)
        .expect("def");
    let max = def.giacenza_max.expect("giacenza massima");
    let per_tick = def.produzione_per_tick.expect("produzione");

    // Tick ancora necessari a riempire il granaio, calcolati dai dati e dalla
    // giacenza attuale: la fattoria produce gia' nel tick in cui nasce, quindi
    // partire da zero sbaglierebbe di uno.
    let mancano = max.to_millis() - w.building(id).expect("viva").stock.to_millis();
    let necessari = mancano / per_tick.to_millis();
    for _ in 0..necessari {
        tick(&mut w, &[]);
    }
    assert_eq!(w.building(id).expect("viva").stock, max);
    assert_eq!(
        w.food().perso_per_giacenza_piena,
        0,
        "finche' non e' pieno non si perde niente"
    );

    for _ in 0..5 {
        tick(&mut w, &[]);
    }
    assert_eq!(
        w.building(id).expect("viva").stock,
        max,
        "la giacenza si ferma al massimo"
    );
    assert_eq!(
        w.food().perso_per_giacenza_piena,
        i64::from(per_tick.to_millis()) * 5,
        "il resto e' perso, e il ledger lo registra"
    );
    conservazione(&w).expect("conservazione");
}

// --- 3, 4. fame e ripresa ---------------------------------------------------

/// Fattoria che copre piu' case di quante ne possa sfamare: dopo un numero di
/// tick calcolabile dai dati la giacenza si esaurisce e qualcuno resta a
/// bocca asciutta.
#[test]
fn a_regime_la_fattoria_non_sfama_tutta_la_sua_capacita() {
    let mut w = scenario_fattoria(6);
    let (id, _) = w.buildings().next().expect("la fattoria");
    let def = w
        .data()
        .def(w.building(id).expect("viva").kind)
        .expect("def");
    let per_tick = def.produzione_per_tick.expect("produzione");
    let consumo = consumo_per_casa(&w);

    let servite = w.n_case() as i32;
    let domanda = consumo.checked_mul_int(servite).expect("domanda");
    assert!(
        domanda > per_tick,
        "lo scenario deve essere in deficit: {domanda} <= {per_tick}"
    );

    // Girare abbastanza da esaurire qualunque scorta accumulata.
    for _ in 0..200 {
        tick(&mut w, &[]);
        conservazione(&w).expect("conservazione");
        giacenze_nel_range(&w).expect("giacenze nel range");
    }

    let affamate = w
        .houses()
        .filter(|(_, h)| !h.servita.get(ServiceKind::Cibo))
        .count();
    assert!(affamate > 0, "in deficit qualcuno deve restare senza cibo");
    let sazie = w.n_case() - affamate;
    assert!(sazie > 0, "il deficit non deve azzerare tutti");
}

/// La ripresa che M0 sa fare: una seconda fattoria copre le case che la
/// prima aveva lasciato fuori dalla propria capacita', e quelle tornano a
/// mangiare.
#[test]
fn una_seconda_fattoria_copre_le_case_lasciate_fuori() {
    // Otto case, capacita' della fattoria sei: due restano scoperte.
    let mut w = scenario_fattoria(8);
    for _ in 0..200 {
        tick(&mut w, &[]);
    }
    let scoperte: Vec<_> = w
        .houses()
        .filter(|(id, _)| !w.coverage().e_servita(*id, ServiceKind::Cibo))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(scoperte.len(), 2, "otto case, capacita' sei");

    costruisci(&mut w, FATTORIA, 12, 2);
    for _ in 0..50 {
        tick(&mut w, &[]);
    }

    for id in scoperte {
        assert!(
            w.coverage().e_servita(id, ServiceKind::Cibo),
            "{id:?} doveva essere raccolta dalla seconda fattoria"
        );
        assert!(
            w.house(id).expect("viva").servita.get(ServiceKind::Cibo),
            "{id:?} doveva anche riuscire a mangiare"
        );
    }
    conservazione(&w).expect("conservazione");
}

/// **Limite noto di M0, non un bug.** Una casa gia' assegnata a una fattoria
/// in deficit non viene salvata da una seconda fattoria: la contesa la vince
/// il primo provider (fase 06) e la capacita' si conta in case, non in
/// abitanti, quindi il posto resta occupato da chi non riesce a mangiare.
///
/// Il giorno in cui la capacita' diventera' "abitanti serviti" (M1) questo
/// test cambiera' esito, e sara' il segnale che la semplificazione e' stata
/// sciolta — non una regressione.
#[test]
fn una_casa_affamata_non_viene_salvata_da_una_seconda_fattoria() {
    let mut w = scenario_fattoria(6);
    for _ in 0..200 {
        tick(&mut w, &[]);
    }
    let affamate_prima = w
        .houses()
        .filter(|(_, h)| !h.servita.get(ServiceKind::Cibo))
        .count();
    assert_eq!(affamate_prima, 1, "sei case, la fattoria ne sfama cinque");

    costruisci(&mut w, FATTORIA, 12, 2);
    for _ in 0..50 {
        tick(&mut w, &[]);
    }

    assert_eq!(
        w.houses()
            .filter(|(_, h)| !h.servita.get(ServiceKind::Cibo))
            .count(),
        1,
        "in M0 la casa resta assegnata alla prima fattoria, che non la sfama"
    );
}

// --- 6. ordine deterministico ------------------------------------------------

/// Con giacenza per due case su tre, mangiano sempre le stesse due: quelle con
/// `HouseId` minore. Ripetuto, perche' un ordine non deterministico
/// passerebbe una volta su tante.
#[test]
fn a_giacenza_scarsa_mangiano_sempre_le_stesse() {
    let mut atteso: Option<Vec<bool>> = None;
    for _ in 0..100 {
        let mut w = scenario_fattoria(6);
        for _ in 0..200 {
            tick(&mut w, &[]);
        }
        let chi: Vec<bool> = w
            .houses()
            .map(|(_, h)| h.servita.get(ServiceKind::Cibo))
            .collect();
        match &atteso {
            None => atteso = Some(chi),
            Some(a) => assert_eq!(*a, chi, "l'insieme di chi mangia deve essere stabile"),
        }
    }
}

// --- 7. eventi ---------------------------------------------------------------

/// `ServiceCoverageChanged` e' un delta: emesso al passaggio, non a ogni tick
/// di fame.
#[test]
fn l_evento_di_copertura_e_un_delta_non_un_polling() {
    let mut w = mondo();
    let celle: Vec<(u8, u8)> = (1..=20).map(|x| (x, 4)).collect();
    strade(&mut w, &celle);
    costruisci(&mut w, POZZO, 2, 3);

    // La casa nasce e viene servita: un evento.
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: CASA,
            origin: pos(5, 5),
        }],
    );
    let casa = w.houses().next().map(|(id, _)| id).expect("la casa");
    let acqua_on = r
        .events
        .iter()
        .filter(|e| {
            matches!(e, Event::ServiceCoverageChanged { house, service, served }
                if *house == casa && *service == ServiceKind::Acqua && *served)
        })
        .count();
    assert_eq!(acqua_on, 1);

    // Dieci tick a vuoto: nessun evento, lo stato non cambia.
    for _ in 0..10 {
        let r = tick(&mut w, &[]);
        assert!(
            !r.events
                .iter()
                .any(|e| matches!(e, Event::ServiceCoverageChanged { .. })),
            "nessun cambio di stato, nessun evento"
        );
    }

    // Demolito il pozzo: un evento di segno opposto, una volta sola.
    let r = tick(&mut w, &[Command::Demolish { at: pos(2, 3) }]);
    let acqua_off = r
        .events
        .iter()
        .filter(|e| {
            matches!(e, Event::ServiceCoverageChanged { house, service, served }
                if *house == casa && *service == ServiceKind::Acqua && !*served)
        })
        .count();
    assert_eq!(acqua_off, 1);

    for _ in 0..10 {
        let r = tick(&mut w, &[]);
        assert!(
            !r.events
                .iter()
                .any(|e| matches!(e, Event::ServiceCoverageChanged { .. })),
            "scoperta e basta: non si riemette a ogni tick"
        );
    }
}

// --- 1, 2. i property test che chiudono la fase ------------------------------

fn comando_qualsiasi() -> impl Strategy<Value = Command> {
    prop_oneof![
        4 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: CASA,
            origin: TilePos::new(x, y),
        }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: FATTORIA,
            origin: TilePos::new(x, y),
        }),
        1 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: POZZO,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

proptest! {
    /// Conservazione: `prodotto == consumato + perso + giacenza`, dopo
    /// qualunque sequenza di comandi e qualunque numero di tick.
    ///
    /// Uguaglianza esatta, non disuguaglianza: e' cio' che rende il test
    /// capace di trovare un bug invece che di rassicurare.
    #[test]
    fn conservazione_del_cibo(p in prop::collection::vec(
        prop::collection::vec(comando_qualsiasi(), 0..4), 1..12)
    ) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            // Qualche tick a vuoto, per far girare la produzione.
            for _ in 0..3 {
                tick(&mut w, &[]);
            }
            if let Err(e) = conservazione(&w) {
                return Err(TestCaseError::fail(e));
            }
            if let Err(e) = giacenze_nel_range(&w) {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}

/// Demolire una fattoria butta via la sua giacenza, e il ledger lo registra:
/// senza il termine `perso_per_demolizione` l'uguaglianza di conservazione si
/// romperebbe, e il test piu' importante della fase segnalerebbe un bug che
/// non c'e'.
#[test]
fn demolire_una_fattoria_registra_la_giacenza_persa() {
    let mut w = scenario_fattoria(0);
    for _ in 0..10 {
        tick(&mut w, &[]);
    }
    let giacenza = w.giacenza_totale();
    assert!(giacenza > 0);
    conservazione(&w).expect("conservazione con la fattoria viva");

    tick(&mut w, &[Command::Demolish { at: pos(2, 2) }]);

    assert_eq!(w.giacenza_totale(), 0);
    assert_eq!(w.food().perso_per_demolizione, giacenza);
    conservazione(&w).expect("conservazione anche dopo la demolizione");
}
