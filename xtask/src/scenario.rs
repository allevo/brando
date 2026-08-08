//! Scenari di prova per il runner headless.
//!
//! Non sono gli scenari di gioco: quelli hanno obiettivi e condizioni di
//! vittoria e arrivano con `sim-scenario` in M1 (D7). Questi sono situazioni
//! costruite a mano per guardare i numeri girare e per dare un contenuto ai
//! golden replay della fase 08.

use std::sync::Arc;

use sim_core::{Command, DataSet, Grid, Terrain, TilePos, World};

pub struct Scenario {
    pub nome: &'static str,
    pub descrizione: &'static str,
    pub seed: u64,
    pub lato: u16,
    /// Comandi da applicare, con il tick in cui vanno applicati.
    pub comandi: Vec<(u32, Command)>,
}

pub fn per_nome(nome: &str, data: &DataSet) -> Option<Scenario> {
    match nome {
        "minimo" => Some(minimo(data)),
        "fame" => Some(fame(data)),
        _ => None,
    }
}

pub const NOMI: [&str; 2] = ["minimo", "fame"];

fn kind(data: &DataSet, id: &str) -> sim_core::BuildingKindId {
    data.kind_by_id(id)
        .unwrap_or_else(|| panic!("il dataset deve contenere '{id}'"))
}

/// Una strada, un pozzo, una fattoria e quattro case: tutte servite, il cibo
/// avanza. E' la citta' che funziona.
fn minimo(data: &DataSet) -> Scenario {
    let casa = kind(data, "casa");
    let pozzo = kind(data, "pozzo");
    let fattoria = kind(data, "fattoria");

    let mut comandi = Vec::new();
    for x in 1..=16u8 {
        comandi.push((0, Command::PlaceRoad { at: pos(x, 8) }));
    }
    comandi.push((
        1,
        Command::PlaceBuilding {
            kind: pozzo,
            origin: pos(2, 7),
        },
    ));
    comandi.push((
        1,
        Command::PlaceBuilding {
            kind: fattoria,
            origin: pos(4, 6),
        },
    ));
    for i in 0..4u8 {
        comandi.push((
            2,
            Command::PlaceBuilding {
                kind: casa,
                origin: pos(8 + i, 9),
            },
        ));
    }

    Scenario {
        nome: "minimo",
        descrizione: "quattro case servite da un pozzo e una fattoria",
        seed: 42,
        lato: 32,
        comandi,
    }
}

/// Come `minimo`, ma con abbastanza case da mandare la fattoria in deficit.
/// Serve a osservare la giacenza che si esaurisce (fase 07, test 3).
fn fame(data: &DataSet) -> Scenario {
    let casa = kind(data, "casa");
    let mut s = minimo(data);
    s.nome = "fame";
    s.descrizione = "piu' case di quante la fattoria ne sfami";
    for i in 4..8u8 {
        s.comandi.push((
            2,
            Command::PlaceBuilding {
                kind: casa,
                origin: pos(8 + i, 9),
            },
        ));
    }
    s
}

const fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}

impl Scenario {
    pub fn mondo(&self, data: Arc<DataSet>) -> World {
        let grid = Grid::new(self.lato, self.lato, Terrain::Pianura)
            .unwrap_or_else(|e| panic!("griglia dello scenario non valida: {e}"));
        World::new(grid, data, self.seed)
    }

    /// I comandi da applicare a un dato tick, nell'ordine di inserimento:
    /// l'ordine e' parte del contratto di determinismo (D4).
    pub fn comandi_al_tick(&self, tick: u32) -> Vec<Command> {
        self.comandi
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, c)| *c)
            .collect()
    }
}
