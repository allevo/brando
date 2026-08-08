//! Fase 05 — componenti connesse, dirty flag e distanza percorsa.

mod comune;

use comune::*;
use proptest::prelude::*;
use sim_core::{Command, TilePos, World};

/// Costruisce le strade indicate, un tick solo.
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

fn comp(w: &World, x: u8, y: u8) -> Option<sim_core::ComponentId> {
    w.roads()
        .component(w.grid().idx(pos(x, y)).expect("in mappa"))
}

// --- 1. componenti ----------------------------------------------------------

#[test]
fn due_gruppi_separati_poi_uniti_poi_di_nuovo_separati() {
    let mut w = mondo();
    strade(&mut w, &[(1, 1), (2, 1), (4, 1), (5, 1)]);
    assert_eq!(w.roads().n_componenti(), 2);
    assert_ne!(comp(&w, 1, 1), comp(&w, 4, 1));
    assert_eq!(comp(&w, 1, 1), comp(&w, 2, 1));

    // Il tile che li congiunge.
    strade(&mut w, &[(3, 1)]);
    assert_eq!(w.roads().n_componenti(), 1);
    assert_eq!(comp(&w, 1, 1), comp(&w, 5, 1));

    // E toglierlo li rispezza.
    let r = tick(&mut w, &[Command::Demolish { at: pos(3, 1) }]);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.roads().n_componenti(), 2);
    assert_ne!(comp(&w, 1, 1), comp(&w, 4, 1));
}

/// L'id di una componente e' il `TileIdx` minimo dei suoi tile, non un
/// contatore: e' cio' che rende l'etichettatura indipendente dalla storia.
#[test]
fn l_id_della_componente_e_il_tile_minimo() {
    let mut w = mondo();
    strade(&mut w, &[(5, 5), (5, 4), (5, 6)]);
    let atteso = w.grid().idx(pos(5, 4)).expect("in mappa");
    for (x, y) in [(5, 4), (5, 5), (5, 6)] {
        assert_eq!(
            comp(&w, x, y).map(sim_core::ComponentId::tile),
            Some(atteso)
        );
    }
}

// --- 2. adiacenza -----------------------------------------------------------

#[test]
fn la_diagonale_non_connette() {
    let mut w = mondo();
    strade(&mut w, &[(1, 1), (2, 2)]);
    assert_eq!(w.roads().n_componenti(), 2);
}

/// Il bug della fase 01 che si ripresenta a un livello piu' alto: senza
/// controlli, l'ultimo tile di una riga e' "vicino" del primo della
/// successiva.
#[test]
fn niente_wraparound_di_riga() {
    let mut w = mondo_con(8, 8);
    strade(&mut w, &[(7, 0), (0, 1)]);
    assert_eq!(w.roads().n_componenti(), 2);
    let a = w.grid().idx(pos(7, 0)).expect("in mappa");
    let b = w.grid().idx(pos(0, 1)).expect("in mappa");
    assert_eq!(b.get(), a.get() + 1, "sono contigui come indici...");
    assert!(!w.roads().connessi(a, b), "...ma non come strade");
}

// --- 4. dirty flag ----------------------------------------------------------

#[test]
fn rete_senza_comandi_non_si_ricostruisce() {
    let mut w = mondo();
    strade(&mut w, &[(1, 1)]);
    let dopo_prima_strada = w.roads().rebuilds();
    assert_eq!(dopo_prima_strada, 1);

    for _ in 0..10 {
        tick(&mut w, &[]);
    }
    assert_eq!(
        w.roads().rebuilds(),
        dopo_prima_strada,
        "dieci tick a vuoto non devono ricostruire niente"
    );

    // Anche costruire un edificio non tocca la rete.
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: POZZO,
            origin: pos(5, 5),
        }],
    );
    assert_eq!(w.roads().rebuilds(), dopo_prima_strada);
}

/// Regressione contro il caso in cui qualcuno segna dirty dentro il loop:
/// N strade in un tick devono costare **una** ricostruzione, non N.
#[test]
fn rete_molte_strade_in_un_tick_ricostruiscono_una_volta_sola() {
    let mut w = mondo();
    let prima = w.roads().rebuilds();
    strade(&mut w, &[(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)]);
    assert_eq!(w.roads().rebuilds(), prima + 1);
}

// --- 5. distanza percorsa ---------------------------------------------------

/// Costruisce un corridoio orizzontale e due edifici affacciati agli estremi.
fn corridoio(lunghezza: u8) -> (World, sim_core::BuildingId, sim_core::BuildingId) {
    let mut w = mondo();
    let celle: Vec<(u8, u8)> = (0..lunghezza).map(|i| (i + 1, 5)).collect();
    strade(&mut w, &celle);
    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(1, 4),
            },
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(lunghezza, 4),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    (w, ids[0], ids[1])
}

#[test]
fn distanza_su_corridoio_rettilineo() {
    let (w, a, b) = corridoio(10);
    // Ingressi agli estremi di un corridoio di 10 tile: 9 passi.
    assert_eq!(w.road_distance(a, b, 100), Some(9));
    assert_eq!(w.road_distance(a, a, 100), Some(0));
}

#[test]
fn oltre_max_e_none() {
    let (w, a, b) = corridoio(10);
    assert_eq!(w.road_distance(a, b, 9), Some(9), "il confine e' inclusivo");
    assert_eq!(w.road_distance(a, b, 8), None);
}

/// Il test che incarna D2: due edifici vicinissimi in linea d'aria ma
/// raggiungibili solo con un giro lungo. Se questo passa, l'implementazione
/// non ha scorciatoie euclidee.
#[test]
fn la_distanza_e_percorsa_non_euclidea() {
    let mut w = mondo();
    // Una U: giu' per la colonna 2, a destra sulla riga 10, su per la colonna 6.
    let mut celle: Vec<(u8, u8)> = (2..=10).map(|y| (2, y)).collect();
    celle.extend((3..=6).map(|x| (x, 10)));
    celle.extend((2..=9).map(|y| (6, y)));
    strade(&mut w, &celle);

    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(3, 2),
            },
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(5, 2),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    let (a, b) = (ids[0], ids[1]);

    // In linea d'aria distano 2 tile.
    assert_eq!(pos(3, 2).manhattan(pos(5, 2)), 2);
    // Sulla rete, il giro completo della U.
    let d = w.road_distance(a, b, 100).expect("connessi");
    assert!(d >= 16, "distanza percorsa attesa lunga, trovata {d}");
    assert_eq!(w.road_distance(a, b, 8), None, "il raggio corto non basta");
}

#[test]
fn edifici_non_connessi_o_senza_strada() {
    let mut w = mondo();
    strade(&mut w, &[(1, 1), (10, 10)]);
    tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(1, 2),
            },
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(10, 11),
            },
            // Questo non tocca nessuna strada.
            Command::PlaceBuilding {
                kind: POZZO,
                origin: pos(20, 20),
            },
        ],
    );
    let ids: Vec<_> = w.buildings().map(|(id, _)| id).collect();
    assert_eq!(w.road_distance(ids[0], ids[1], 200), None, "reti separate");
    assert_eq!(w.road_distance(ids[0], ids[2], 200), None, "senza aggancio");
    assert!(w.ingressi_edificio(ids[2]).is_empty());
}

/// Vale in M0 perche' il grafo non e' orientato. Il giorno in cui si
/// introdurra' un senso di marcia questo test fallira', e costringera' a
/// decidere consapevolmente invece che per distrazione.
#[test]
fn la_distanza_e_simmetrica() {
    let (w, a, b) = corridoio(10);
    assert_eq!(w.road_distance(a, b, 100), w.road_distance(b, a, 100));
}

// --- 3. indipendenza dall'ordine (il goal della fase) ------------------------

proptest! {
    /// Dato un insieme di posizioni, due permutazioni dei `PlaceRoad`
    /// corrispondenti producono la **stessa** etichettatura. E' il motivo per
    /// cui l'id di componente e' il tile minimo e non un contatore.
    #[test]
    fn rete_etichettatura_non_dipende_dall_ordine(
        mut celle in prop::collection::vec((0u8..12, 0u8..12), 1..40),
        permuta in prop::collection::vec(0usize..1000, 40),
    ) {
        celle.sort_unstable();
        celle.dedup();

        let mut a = mondo();
        strade(&mut a, &celle);

        // Stesse celle, ordine mescolato in modo deterministico.
        let mut altre = celle.clone();
        let n = altre.len();
        if n > 1 {
            for (i, k) in permuta.iter().enumerate() {
                altre.swap(i % n, k % n);
            }
        }
        let mut b = mondo();
        strade(&mut b, &altre);

        for idx in a.grid().indices() {
            prop_assert_eq!(
                a.roads().component(idx),
                b.roads().component(idx),
                "etichettatura diversa in {:?}", idx
            );
        }
    }
}
