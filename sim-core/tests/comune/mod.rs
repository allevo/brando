#![allow(dead_code)]
// Ogni test di integrazione compila questo modulo per conto suo, quindi cio'
// che serve solo a `comandi.rs` risulta morto in `invarianti.rs` e viceversa.

//! Fixture condivise dai test di integrazione di `sim-core`.
//!
//! Il dataset e' costruito a mano invece di caricare le tabelle di
//! produzione: `sim-core` non puo' dipendere da `sim-data` (farebbe un ciclo),
//! e soprattutto un test sulle **meccaniche** non deve rompersi ogni volta che
//! cambia il bilanciamento. I test che riguardano davvero i numeri di
//! produzione stanno in `sim-data`.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_core::data::{BuildingDef, DataSet, Rules, ServiceDef, TerrainDef};
use sim_core::{BuildingKindId, Coins, Command, Grid, Milli, ServiceKind, Terrain, TilePos, World};

pub const CASA: BuildingKindId = BuildingKindId::new(0);
pub const POZZO: BuildingKindId = BuildingKindId::new(1);
pub const FATTORIA: BuildingKindId = BuildingKindId::new(2);
/// Pozzo da una casa sola: serve a far mordere la capacita' nei test, che con
/// capacita' 8 non si osserverebbe.
pub const POZZETTO: BuildingKindId = BuildingKindId::new(3);
/// Tipo che non esiste in tabella: e' cio' che produrra' l'LLM.
pub const INESISTENTE: BuildingKindId = BuildingKindId::new(99);

pub const COSTO_CASA: i32 = 10;
pub const COSTO_POZZO: i32 = 12;
pub const COSTO_FATTORIA: i32 = 40;
pub const COSTO_STRADA_PIANURA: i32 = 2;
pub const TESORO_INIZIALE: i32 = 1000;
pub const ABITANTI_PER_CASA: u16 = 4;

pub fn dataset() -> Arc<DataSet> {
    let rules = Rules {
        tick_per_mese: 30,
        mesi_per_anno: 12,
        tesoro_iniziale: Coins::new(TESORO_INIZIALE),
        abitanti_per_livello_casa: vec![ABITANTI_PER_CASA],
        consumo_cibo_per_abitante: Milli::from_millis(20),
    };

    let mut terrain = BTreeMap::new();
    terrain.insert(
        Terrain::Pianura,
        TerrainDef {
            costruibile: true,
            attraversabile: true,
            costo_strada: Coins::new(COSTO_STRADA_PIANURA),
        },
    );
    terrain.insert(
        Terrain::Acqua,
        TerrainDef {
            costruibile: false,
            attraversabile: false,
            costo_strada: Coins::new(0),
        },
    );
    terrain.insert(
        Terrain::Roccia,
        TerrainDef {
            costruibile: false,
            attraversabile: true,
            costo_strada: Coins::new(6),
        },
    );

    let buildings = vec![
        BuildingDef {
            id: "casa".into(),
            footprint: (1, 1),
            costo: Coins::new(COSTO_CASA),
            livelli: 1,
            servizio: None,
            servizi_richiesti: vec![ServiceKind::Acqua, ServiceKind::Cibo],
            produzione_per_tick: None,
            giacenza_max: None,
        },
        BuildingDef {
            id: "pozzo".into(),
            footprint: (1, 1),
            costo: Coins::new(COSTO_POZZO),
            livelli: 1,
            servizio: Some(ServiceDef {
                kind: ServiceKind::Acqua,
                raggio_per_livello: vec![12],
                capacita_per_livello: vec![8],
            }),
            servizi_richiesti: vec![],
            produzione_per_tick: None,
            giacenza_max: None,
        },
        BuildingDef {
            id: "fattoria".into(),
            footprint: (2, 2),
            costo: Coins::new(COSTO_FATTORIA),
            livelli: 1,
            servizio: Some(ServiceDef {
                kind: ServiceKind::Cibo,
                raggio_per_livello: vec![10],
                capacita_per_livello: vec![6],
            }),
            servizi_richiesti: vec![],
            produzione_per_tick: Some(Milli::from_millis(400)),
            giacenza_max: Some(Milli::from_millis(20_000)),
        },
        BuildingDef {
            id: "pozzetto".into(),
            footprint: (1, 1),
            costo: Coins::new(COSTO_POZZO),
            livelli: 1,
            servizio: Some(ServiceDef {
                kind: ServiceKind::Acqua,
                raggio_per_livello: vec![12],
                capacita_per_livello: vec![1],
            }),
            servizi_richiesti: vec![],
            produzione_per_tick: None,
            giacenza_max: None,
        },
    ];

    Arc::new(DataSet::new(rules, terrain, buildings))
}

/// Mondo di prova: griglia 32x32 di pianura (A4), seed fisso.
pub fn mondo() -> World {
    mondo_con(32, 32)
}

pub fn mondo_con(w: u16, h: u16) -> World {
    let grid = Grid::new(w, h, Terrain::Pianura).expect("dimensioni valide");
    World::new(grid, dataset(), 42)
}

/// Applica i comandi in un solo tick e restituisce il report.
pub fn tick(world: &mut World, cmds: &[Command]) -> sim_core::StepReport {
    sim_core::step(world, cmds)
}

pub fn pos(x: u8, y: u8) -> TilePos {
    TilePos::new(x, y)
}
