//! Fase 04 — comportamento tabellare di `step` e dei comandi.
//!
//! Gli invarianti globali (nessun panic, nessuna sovrapposizione, tesoro
//! coerente) stanno in `invarianti.rs`.

mod comune;

use comune::*;
use sim_core::{Coins, Command, CommandError, Event, Occupante, Terrain, TileOccupant};

#[test]
fn un_tick_vuoto_avanza_solo_il_tick() {
    let mut w = mondo();
    let prima = w.clone();

    let r = tick(&mut w, &[]);

    assert_eq!(w.tick(), prima.tick() + 1);
    assert!(r.rejected.is_empty());
    assert!(r.events.is_empty());
    assert_eq!(w.grid(), prima.grid());
    assert_eq!(w.economy(), prima.economy());
    assert_eq!(w.n_edifici(), 0);
    assert_eq!(w.n_case(), 0);
    assert_eq!(w.rng(), prima.rng(), "nessun sistema estrae dall'RNG in M0");
}

#[test]
fn una_fattoria_occupa_quattro_tile_e_costa() {
    let mut w = mondo();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FATTORIA,
            origin: pos(3, 3),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.n_edifici(), 1);
    assert_eq!(
        w.economy().tesoro,
        Coins::new(TESORO_INIZIALE - COSTO_FATTORIA)
    );

    let origine = w.grid().idx(pos(3, 3)).expect("in mappa");
    let mut occupati = 0;
    for (x, y) in [(3, 3), (4, 3), (3, 4), (4, 4)] {
        let idx = w.grid().idx(pos(x, y)).expect("in mappa");
        let tile = w.grid().get(idx).expect("tile");
        assert_eq!(
            tile.occupante(),
            Some(TileOccupant {
                origin: origine,
                e_casa: false
            }),
            "il tile ({x},{y}) deve puntare all'origine della fattoria"
        );
        assert!(matches!(w.occupante(idx), Some(Occupante::Edificio(_))));
        occupati += 1;
    }
    assert_eq!(occupati, 4);

    // I tile appena fuori dal footprint restano liberi.
    let fuori = w.grid().idx(pos(5, 3)).expect("in mappa");
    assert_eq!(w.occupante(fuori), None);
}

#[test]
fn la_sovrapposizione_e_rifiutata_senza_mutare_nulla() {
    let mut w = mondo();
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FATTORIA,
            origin: pos(3, 3),
        }],
    );
    let prima = w.clone();

    // La seconda fattoria tocca un solo tile della prima.
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FATTORIA,
            origin: pos(4, 4),
        }],
    );

    assert_eq!(r.rejected.len(), 1);
    assert!(matches!(r.rejected[0].1, CommandError::TileOccupied { .. }));
    assert_eq!(w.n_edifici(), prima.n_edifici());
    assert_eq!(w.economy(), prima.economy(), "niente costo per un rifiuto");
    assert_eq!(
        w.grid(),
        prima.grid(),
        "nessuna occupazione parziale: la validazione precede le mutazioni"
    );
}

#[test]
fn il_footprint_non_puo_sforare_il_bordo() {
    let mut w = mondo_con(8, 8);
    let prima = w.clone();

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FATTORIA,
            origin: pos(7, 7),
        }],
    );

    assert_eq!(r.rejected.len(), 1);
    assert!(matches!(r.rejected[0].1, CommandError::OutOfBounds(_)));
    assert_eq!(w.grid(), prima.grid());
    assert_eq!(w.economy(), prima.economy());
}

#[test]
fn tesoro_insufficiente_riporta_i_numeri_giusti() {
    let mut w = mondo();
    // Svuota il tesoro costruendo case finche' ce n'e'.
    let case = TESORO_INIZIALE / COSTO_CASA;
    let cmds: Vec<_> = (0..case)
        .map(|i| Command::PlaceBuilding {
            kind: CASA,
            origin: pos((i % 32) as u8, (i / 32) as u8),
        })
        .collect();
    let r = tick(&mut w, &cmds);
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.economy().tesoro, Coins::ZERO);

    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: POZZO,
            origin: pos(20, 20),
        }],
    );
    assert_eq!(
        r.rejected[0].1,
        CommandError::InsufficientFunds {
            needed: Coins::new(COSTO_POZZO),
            available: Coins::ZERO,
        }
    );
}

#[test]
fn demolire_libera_tutto_il_footprint_e_rimuove_l_id() {
    let mut w = mondo();
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: FATTORIA,
            origin: pos(3, 3),
        }],
    );
    let id = w.buildings().next().map(|(id, _)| id).expect("un edificio");

    // Demolire da un tile che non e' l'origine deve funzionare lo stesso.
    let r = tick(&mut w, &[Command::Demolish { at: pos(4, 4) }]);

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.n_edifici(), 0);
    assert_eq!(w.building(id), None, "l'id non e' piu' vivo");
    assert!(r.events.contains(&Event::BuildingRemoved { id }));
    for (x, y) in [(3, 3), (4, 3), (3, 4), (4, 4)] {
        let idx = w.grid().idx(pos(x, y)).expect("in mappa");
        assert_eq!(w.occupante(idx), None, "({x},{y}) deve tornare libero");
    }
}

#[test]
fn demolire_il_vuoto_e_un_errore() {
    let mut w = mondo();
    let r = tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert_eq!(r.rejected[0].1, CommandError::NothingToDemolish(pos(5, 5)));
}

#[test]
fn tipo_di_edificio_inesistente() {
    let mut w = mondo();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: INESISTENTE,
            origin: pos(1, 1),
        }],
    );
    assert_eq!(
        r.rejected[0].1,
        CommandError::UnknownBuildingKind(INESISTENTE)
    );
    assert_eq!(w.economy().tesoro, Coins::new(TESORO_INIZIALE));
}

#[test]
fn la_strada_costa_segna_il_flag_e_sporca_la_rete() {
    let mut w = mondo();
    let r = tick(&mut w, &[Command::PlaceRoad { at: pos(2, 2) }]);

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(
        w.economy().tesoro,
        Coins::new(TESORO_INIZIALE - COSTO_STRADA_PIANURA)
    );
    let idx = w.grid().idx(pos(2, 2)).expect("in mappa");
    assert!(w.grid().get(idx).expect("tile").flags.has_road());
    // Il flag e' gia' stato consumato dal passo 2 nello stesso tick: cio' che
    // resta osservabile e' che la ricostruzione sia avvenuta.
    assert!(!w.dirty().roads);
    assert_eq!(w.roads().rebuilds(), 1);

    // Due strade sullo stesso tile: la seconda e' rifiutata.
    let r = tick(&mut w, &[Command::PlaceRoad { at: pos(2, 2) }]);
    assert!(matches!(r.rejected[0].1, CommandError::TileOccupied { .. }));
}

#[test]
fn su_terreno_non_adatto_non_si_costruisce() {
    let mut w = mondo();
    // Un lago in mezzo alla mappa.
    assert!(w.set_terrain(pos(6, 6), Terrain::Acqua));

    let r = tick(
        &mut w,
        &[
            Command::PlaceBuilding {
                kind: CASA,
                origin: pos(6, 6),
            },
            Command::PlaceRoad { at: pos(6, 6) },
        ],
    );

    assert_eq!(r.rejected.len(), 2);
    for (_, e) in &r.rejected {
        assert!(
            matches!(
                e,
                CommandError::UnsuitableTerrain {
                    terrain: Terrain::Acqua,
                    ..
                }
            ),
            "{e:?}"
        );
    }
}

#[test]
fn una_casa_e_una_casa_non_un_edificio() {
    let mut w = mondo();
    let r = tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: CASA,
            origin: pos(1, 1),
        }],
    );

    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(w.n_case(), 1);
    assert_eq!(w.n_edifici(), 0, "le case non stanno tra gli edifici (D5)");
    assert_eq!(w.popolazione(), u32::from(ABITANTI_PER_CASA));
    let (_, h) = w.houses().next().expect("una casa");
    assert_eq!(h.level, 1);
    assert!(matches!(
        r.events[0],
        Event::HousePlaced { origin, .. } if origin == pos(1, 1)
    ));
}

#[test]
fn un_comando_invalido_non_ferma_gli_altri() {
    let mut w = mondo();
    let r = tick(
        &mut w,
        &[
            Command::PlaceRoad { at: pos(0, 0) },
            Command::Demolish { at: pos(31, 31) }, // niente da demolire
            Command::PlaceRoad { at: pos(1, 0) },
        ],
    );

    assert_eq!(r.rejected.len(), 1);
    assert_eq!(r.rejected[0].0, 1, "l'indice del comando scartato");
    assert_eq!(r.accettati(3), 2);
    assert!(w.grid().at(pos(0, 0)).expect("tile").flags.has_road());
    assert!(w.grid().at(pos(1, 0)).expect("tile").flags.has_road());
}

#[test]
fn costruire_segna_il_provider_come_dirty() {
    let mut w = mondo();
    tick(
        &mut w,
        &[Command::PlaceBuilding {
            kind: POZZO,
            origin: pos(5, 5),
        }],
    );
    let id = w.buildings().next().map(|(id, _)| id).expect("il pozzo");
    assert_eq!(w.dirty().coverage, [id]);

    tick(&mut w, &[Command::Demolish { at: pos(5, 5) }]);
    assert!(
        !w.dirty().coverage.contains(&id),
        "un provider demolito non resta nella lista dei dirty"
    );
}
