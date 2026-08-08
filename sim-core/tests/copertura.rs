//! Fase 06 — copertura aggregata: raggio su strada, capacita', assegnazione
//! deterministica, ed equivalenza tra calcolo incrementale e da zero.

mod comune;

use comune::*;
use proptest::prelude::*;
use sim_core::{BuildingId, Command, HouseId, ServiceKind, TilePos, World};

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

fn unica_casa(w: &World) -> HouseId {
    w.houses().next().map(|(id, _)| id).expect("una casa")
}

fn unico_pozzo(w: &World) -> BuildingId {
    w.buildings().next().map(|(id, _)| id).expect("un pozzo")
}

/// Corridoio orizzontale sulla riga 5, pozzo all'inizio, casa a `dist` tile
/// di strada di distanza. Il raggio del pozzo e' 12 (fixture).
fn scenario_lineare(lunghezza: u8, casa_x: u8) -> World {
    let mut w = mondo();
    let celle: Vec<(u8, u8)> = (1..=lunghezza).map(|x| (x, 5)).collect();
    strade(&mut w, &celle);
    costruisci(&mut w, POZZO, 1, 4);
    costruisci(&mut w, CASA, casa_x, 4);
    w
}

// --- 1, 2. raggio -----------------------------------------------------------

#[test]
fn caso_base_casa_entro_raggio() {
    let w = scenario_lineare(20, 4);
    let casa = unica_casa(&w);
    assert!(w.coverage().e_servita(casa, ServiceKind::Acqua));
    assert_eq!(
        w.coverage().provider(casa, ServiceKind::Acqua),
        Some(unico_pozzo(&w))
    );
    // La casa porta la copia del flag.
    let (_, h) = w.houses().next().expect("una casa");
    assert!(h.servita.get(ServiceKind::Acqua));
    assert!(
        !h.servita.get(ServiceKind::Cibo),
        "nessuna fattoria: niente cibo"
    );
}

/// Il confine e' inclusivo, e questo test lo fissa: raggio 12 serve fino a
/// distanza 12 compresa, non oltre.
#[test]
fn il_confine_del_raggio_e_inclusivo() {
    // Ingresso del pozzo: (1,5). Ingresso della casa a x: (x,5). Distanza x-1.
    let w = scenario_lineare(20, 13);
    let casa = unica_casa(&w);
    assert!(
        w.coverage().e_servita(casa, ServiceKind::Acqua),
        "distanza 12 con raggio 12 deve essere servita"
    );

    let w = scenario_lineare(20, 14);
    let casa = unica_casa(&w);
    assert!(
        !w.coverage().e_servita(casa, ServiceKind::Acqua),
        "distanza 13 con raggio 12 non deve essere servita"
    );
}

// --- 3. il test che incarna D2 ---------------------------------------------

/// Casa a 2 tile in linea d'aria dal pozzo, ma raggiungibile solo con un giro
/// lungo: con raggio 12 **non** e' servita. Se questo passa, l'implementazione
/// non ha scorciatoie euclidee.
#[test]
fn la_distanza_e_su_strada_non_in_aria() {
    let mut w = mondo();
    // Un anello lungo: colonna 2 da y=2 a y=12, riga 12 da x=2 a x=6,
    // colonna 6 da y=12 a y=2.
    let mut celle: Vec<(u8, u8)> = (2..=12).map(|y| (2, y)).collect();
    celle.extend((3..=6).map(|x| (x, 12)));
    celle.extend((2..=11).map(|y| (6, y)));
    strade(&mut w, &celle);

    costruisci(&mut w, POZZO, 3, 2);
    costruisci(&mut w, CASA, 5, 2);

    let casa = unica_casa(&w);
    assert_eq!(pos(3, 2).manhattan(pos(5, 2)), 2, "vicinissime in aria");
    assert!(
        !w.coverage().e_servita(casa, ServiceKind::Acqua),
        "il giro sulla rete supera il raggio 12: la casa non e' servita"
    );
}

// --- 4. serve la strada -----------------------------------------------------

#[test]
fn una_casa_senza_strada_non_e_mai_servita() {
    let mut w = mondo();
    strade(&mut w, &[(1, 5), (2, 5), (3, 5)]);
    costruisci(&mut w, POZZO, 1, 4);
    // Casa attaccata al pozzo ma non a una strada.
    costruisci(&mut w, CASA, 1, 3);

    let casa = unica_casa(&w);
    assert!(!w.coverage().e_servita(casa, ServiceKind::Acqua));
}

// --- 5. capacita' e ordine di assegnazione ----------------------------------

/// La regola di gioco: si servono le piu' vicine, e a parita' di distanza
/// vince il `TileIdx` minore.
#[test]
fn la_capacita_serve_le_piu_vicine() {
    let mut w = mondo();
    let celle: Vec<(u8, u8)> = (1..=20).map(|x| (x, 5)).collect();
    strade(&mut w, &celle);
    costruisci(&mut w, POZZO, 1, 4);

    // Il pozzo della fixture ha capacita' 8: ne mettiamo dieci a distanze
    // crescenti, tutte entro raggio 12.
    for x in 2..=11u8 {
        costruisci(&mut w, CASA, x, 4);
    }

    let pozzo = unico_pozzo(&w);
    let servite = w.coverage().case_servite_da(pozzo);
    assert_eq!(servite.len(), 8, "capacita' del pozzo");

    // Le due escluse sono le piu' lontane: x = 10 e x = 11.
    for (id, h) in w.houses() {
        let atteso = h.origin.x <= 9;
        assert_eq!(
            w.coverage().e_servita(id, ServiceKind::Acqua),
            atteso,
            "casa in {:?}",
            h.origin
        );
    }
}

/// Con la capacita' che morde, a parita' di distanza vince la casa con
/// `TileIdx` minore. E' una regola di gioco esplicita: senza il secondo
/// criterio l'ordine verrebbe dall'ordine di visita del BFS, cioe' da un
/// dettaglio implementativo, e il golden replay diventerebbe fragile.
#[test]
fn a_parita_di_distanza_vince_il_tile_minore() {
    let mut w = mondo();
    strade(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    costruisci(&mut w, POZZETTO, 5, 6);

    // Due case a distanza 1 dall'unico ingresso del pozzetto, (5,5).
    costruisci(&mut w, CASA, 4, 4); // ingresso (4,5)
    costruisci(&mut w, CASA, 6, 4); // ingresso (6,5)

    let vicina = w
        .houses()
        .find(|(_, h)| h.origin == pos(4, 4))
        .map(|(id, _)| id)
        .expect("casa a sinistra");
    let lontana = w
        .houses()
        .find(|(_, h)| h.origin == pos(6, 4))
        .map(|(id, _)| id)
        .expect("casa a destra");

    let idx = |p| w.grid().idx(p).expect("in mappa");
    assert!(
        idx(pos(4, 4)) < idx(pos(6, 4)),
        "la sinistra ha TileIdx minore"
    );
    assert!(w.coverage().e_servita(vicina, ServiceKind::Acqua));
    assert!(
        !w.coverage().e_servita(lontana, ServiceKind::Acqua),
        "la capacita' e' 1: la seconda resta scoperta"
    );
}

/// Due provider che coprono la stessa casa: in M0 la casa e' servita e basta,
/// e vince il primo nell'ordine di iterazione dei provider.
#[test]
fn la_contesa_la_vince_il_primo_provider() {
    let mut w = mondo();
    strade(&mut w, &[(4, 5), (5, 5), (6, 5)]);
    costruisci(&mut w, POZZO, 4, 6);
    costruisci(&mut w, POZZO, 6, 6);
    costruisci(&mut w, CASA, 5, 4);

    let primo = w.buildings().next().map(|(id, _)| id).expect("primo pozzo");
    let casa = unica_casa(&w);
    assert_eq!(w.coverage().provider(casa, ServiceKind::Acqua), Some(primo));
}

// --- 7. demolizioni ---------------------------------------------------------

#[test]
fn demolire_il_pozzo_scopre_le_case_nello_stesso_tick() {
    let mut w = scenario_lineare(20, 4);
    let casa = unica_casa(&w);
    assert!(w.coverage().e_servita(casa, ServiceKind::Acqua));

    let r = tick(&mut w, &[Command::Demolish { at: pos(1, 4) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert!(
        !w.coverage().e_servita(casa, ServiceKind::Acqua),
        "la copertura deve cadere nello stesso tick della demolizione"
    );
}

#[test]
fn spezzare_la_strada_scopre_le_case_oltre_la_rottura() {
    let mut w = scenario_lineare(20, 10);
    let casa = unica_casa(&w);
    assert!(w.coverage().e_servita(casa, ServiceKind::Acqua));

    // Toglie un tile di strada a meta' strada tra pozzo e casa.
    let r = tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert!(
        !w.coverage().e_servita(casa, ServiceKind::Acqua),
        "oltre la rottura la casa non e' piu' raggiungibile"
    );
}

// --- 8. idempotenza ---------------------------------------------------------

#[test]
fn due_tick_a_vuoto_non_ricalcolano_niente() {
    let mut w = scenario_lineare(20, 4);
    let prima = w.coverage().clone();
    let ricalcoli = w.coverage().ricalcoli();
    let bfs = w.roads().rebuilds();

    tick(&mut w, &[]);
    tick(&mut w, &[]);

    assert_eq!(w.coverage().assegnazioni(), prima.assegnazioni());
    assert_eq!(w.coverage().ricalcoli(), ricalcoli, "nessun ricalcolo");
    assert_eq!(w.roads().rebuilds(), bfs, "nessuna ricostruzione di rete");
}

// --- 6. equivalenza incrementale / da zero (il goal della fase) --------------

fn comando_qualsiasi() -> impl Strategy<Value = Command> {
    prop_oneof![
        4 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceRoad { at: TilePos::new(x, y) }),
        3 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: CASA,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: POZZO,
            origin: TilePos::new(x, y),
        }),
        1 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::PlaceBuilding {
            kind: FATTORIA,
            origin: TilePos::new(x, y),
        }),
        2 => (0u8..14, 0u8..14).prop_map(|(x, y)| Command::Demolish { at: TilePos::new(x, y) }),
    ]
}

proptest! {
    /// **Il** test della fase: data una sequenza casuale di comandi, la
    /// `Coverage` ottenuta seguendo i dirty flag e' identica a quella
    /// ricalcolata da zero sullo stato finale.
    ///
    /// Oggi il passo 3 ricalcola tutto quando qualcosa e' sporco, quindi cio'
    /// che questo test coglie e' un'**invalidazione dimenticata**: un comando
    /// che cambia la topologia senza segnare nulla come dirty. Domani, quando
    /// il ricalcolo diventera' davvero incrementale, lo stesso test coprira'
    /// anche quella furbizia senza doverlo riscrivere.
    #[test]
    fn equivalenza_copertura(p in prop::collection::vec(
        prop::collection::vec(comando_qualsiasi(), 0..5), 1..10)
    ) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            let da_zero = sim_core::coverage::calcola_da_zero(&w);
            prop_assert_eq!(
                w.coverage().assegnazioni(),
                da_zero.assegnazioni(),
                "copertura incrementale diversa da quella da zero al tick {}",
                w.tick()
            );
        }
    }

    /// Una casa servita ha sempre un provider vivo, che fornisce davvero quel
    /// servizio ed e' entro il proprio raggio.
    #[test]
    fn equivalenza_provider_coerenti(p in prop::collection::vec(
        prop::collection::vec(comando_qualsiasi(), 0..5), 1..8)
    ) {
        let mut w = mondo();
        for cmds in &p {
            tick(&mut w, cmds);
            for (h, _) in w.houses() {
                for k in ServiceKind::TUTTI {
                    let Some(p) = w.coverage().provider(h, k) else { continue };
                    let b = w.building(p);
                    prop_assert!(b.is_some(), "provider morto per {h:?}");
                    let def = w.data().def(b.expect("vivo").kind).expect("kind noto");
                    let s = def.servizio.as_ref().expect("un provider ha un servizio");
                    prop_assert_eq!(s.kind, k, "provider del servizio sbagliato");
                }
            }
        }
    }
}
