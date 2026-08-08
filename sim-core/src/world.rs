//! Lo stato del gioco.
//!
//! Struct concreta con `SlotMap` e `Vec`, non un `World` ECS (D1): l'ordine di
//! iterazione delle query di un ECS non e' un contratto stabile, e per i run
//! di bilanciamento un loop stretto su array densi e' ordini di grandezza piu'
//! veloce.

use std::collections::BTreeMap;
use std::sync::Arc;

use slotmap::SlotMap;

use crate::data::DataSet;
use crate::grid::Grid;
use crate::ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
use crate::rng::RngSet;
use crate::service::ServiceFlags;
use crate::units::{Coins, Milli};

/// Un edificio che fornisce un servizio o produce merce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Building {
    pub kind: BuildingKindId,
    pub origin: TilePos,
    /// Livello di evoluzione, da 1. In M0 resta sempre 1.
    pub level: u8,
    /// Giacenza locale, solo per i produttori (fase 07).
    pub stock: Milli,
}

/// Una casa: l'unita' di simulazione della popolazione (D5).
///
/// Non si simulano individui. In M0 gli abitanti sono un valore fisso dalle
/// `rules` e la casa non evolve: migrazione ed evoluzione sono M1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct House {
    pub origin: TilePos,
    pub level: u8,
    pub abitanti: u16,
    /// Quali servizi la raggiungono in questo tick.
    pub servita: ServiceFlags,
}

/// Walker logistico reale (D3): trasporto merci, carovane, immigranti.
///
/// Vuoto in M0 e per tutto M1: i walker veri arrivano con M3. I portatori
/// d'acqua che il giocatore vedra' camminare sono decorativi, vivono nel
/// renderer e non compaiono mai qui (D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walker {
    pub at: TilePos,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Economy {
    pub tesoro: Coins,
}

/// Cosa va ricalcolato al prossimo tick.
///
/// I flag esistono da subito per scelta esplicita: retrofittarli dopo e'
/// doloroso (CLAUDE.md, ordine del tick). In M0 l'uso e' ingenuo — quando
/// cambiano le strade, tutti i provider tornano dirty — ma la struttura e'
/// quella definitiva.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirtyFlags {
    pub roads: bool,
    /// Provider la cui copertura va ricalcolata. `Vec` ordinato, non un
    /// `HashSet` (D4).
    pub coverage: Vec<BuildingId>,
}

impl DirtyFlags {
    /// Segna un provider come da ricalcolare, senza duplicati e mantenendo
    /// l'ordine per `BuildingId`: l'ordine di iterazione dei provider decide
    /// chi vince le case contese (fase 06), quindi e' semantica di gioco.
    pub fn segna_coverage(&mut self, id: BuildingId) {
        if let Err(pos) = self.coverage.binary_search(&id) {
            self.coverage.insert(pos, id);
        }
    }

    pub fn dimentica_coverage(&mut self, id: BuildingId) {
        if let Ok(pos) = self.coverage.binary_search(&id) {
            self.coverage.remove(pos);
        }
    }
}

/// Lo stato completo della partita.
#[derive(Debug, Clone)]
pub struct World {
    pub(crate) tick: u32,
    pub(crate) grid: Grid,
    /// L'iterazione di uno `SlotMap` e' per indice di slot, quindi
    /// deterministica a parita' di sequenza di inserimenti e rimozioni — che
    /// e' garantita dal log dei comandi (D4). E' un invariante non ovvio da
    /// cui dipende l'hash canonico dello stato.
    pub(crate) buildings: SlotMap<BuildingId, Building>,
    pub(crate) houses: SlotMap<HouseId, House>,
    pub(crate) walkers: Vec<Walker>,
    pub(crate) economy: Economy,
    pub(crate) rng: RngSet,
    pub(crate) dirty: DirtyFlags,
    /// Indici da tile di origine a id. Sono `BTreeMap` e non `HashMap` (D4):
    /// l'ordine di iterazione e' un contratto.
    pub(crate) edifici_per_origine: BTreeMap<TileIdx, BuildingId>,
    pub(crate) case_per_origine: BTreeMap<TileIdx, HouseId>,
    /// Le tabelle di bilanciamento (A2). Sono dentro lo stato e non un
    /// parametro di `step` per non propagarle in ogni funzione interna; il
    /// loro hash entra nell'hash dello stato, cosi' un cambio di
    /// bilanciamento fa fallire il replay subito e per il motivo giusto.
    pub(crate) data: Arc<DataSet>,
}

impl World {
    /// Mondo iniziale: griglia vuota, tesoro dalle `rules`, RNG dal seed.
    pub fn new(grid: Grid, data: Arc<DataSet>, seed: u64) -> Self {
        let tesoro = data.rules.tesoro_iniziale;
        Self {
            tick: 0,
            grid,
            buildings: SlotMap::with_key(),
            houses: SlotMap::with_key(),
            walkers: Vec::new(),
            economy: Economy { tesoro },
            rng: RngSet::from_seed(seed),
            dirty: DirtyFlags::default(),
            edifici_per_origine: BTreeMap::new(),
            case_per_origine: BTreeMap::new(),
            data,
        }
    }

    pub const fn tick(&self) -> u32 {
        self.tick
    }

    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    pub const fn economy(&self) -> &Economy {
        &self.economy
    }

    pub const fn rng(&self) -> &RngSet {
        &self.rng
    }

    pub const fn dirty(&self) -> &DirtyFlags {
        &self.dirty
    }

    pub fn data(&self) -> &DataSet {
        &self.data
    }

    pub fn buildings(&self) -> impl Iterator<Item = (BuildingId, &Building)> {
        self.buildings.iter()
    }

    pub fn building(&self, id: BuildingId) -> Option<&Building> {
        self.buildings.get(id)
    }

    pub fn houses(&self) -> impl Iterator<Item = (HouseId, &House)> {
        self.houses.iter()
    }

    pub fn house(&self, id: HouseId) -> Option<&House> {
        self.houses.get(id)
    }

    pub fn walkers(&self) -> &[Walker] {
        &self.walkers
    }

    pub fn n_edifici(&self) -> usize {
        self.buildings.len()
    }

    pub fn n_case(&self) -> usize {
        self.houses.len()
    }

    /// Popolazione totale.
    pub fn popolazione(&self) -> u32 {
        self.houses.values().map(|h| u32::from(h.abitanti)).sum()
    }

    /// Imposta il terreno di un tile.
    ///
    /// Serve alla **costruzione dello scenario**, prima che la partita
    /// cominci: e' l'unica mutazione dello stato che non passa da un
    /// `Command`, perche' la mappa non e' una mossa del giocatore. Non e' un
    /// canale per il renderer, che verso il core scrive solo comandi.
    /// `false` se la posizione e' fuori dalla mappa.
    pub fn set_terrain(&mut self, pos: TilePos, terrain: crate::grid::Terrain) -> bool {
        match self.grid.at_mut(pos) {
            Some(t) => {
                t.terrain = terrain;
                true
            }
            None => false,
        }
    }

    /// Risolve l'occupante di un tile nel suo id.
    ///
    /// `None` se il tile e' libero. Un tile occupato che non risolve a un id
    /// vivo e' un bug, ed e' l'invariante che coglie gli errori di
    /// demolizione (fase 09).
    pub fn occupante(&self, idx: TileIdx) -> Option<Occupante> {
        let occ = self.grid.get(idx)?.occupante()?;
        if occ.e_casa {
            self.case_per_origine
                .get(&occ.origin)
                .copied()
                .map(Occupante::Casa)
        } else {
            self.edifici_per_origine
                .get(&occ.origin)
                .copied()
                .map(Occupante::Edificio)
        }
    }
}

/// Chi occupa un tile, risolto in un id dello stato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupante {
    Edificio(BuildingId),
    Casa(HouseId),
}
