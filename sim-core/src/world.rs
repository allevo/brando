//! Lo stato del gioco.
//!
//! Struct concreta con `SlotMap` e `Vec`, non un `World` ECS (D1): l'ordine di
//! iterazione delle query di un ECS non e' un contratto stabile, e per i run
//! di bilanciamento un loop stretto su array densi e' ordini di grandezza piu'
//! veloce.

use std::collections::BTreeMap;
use std::sync::Arc;

use slotmap::SlotMap;

use crate::coverage::Coverage;
use crate::data::DataSet;
use crate::grid::Grid;
use crate::ids::{BuildingId, BuildingKindId, HouseId, TileIdx, TilePos};
use crate::network::RoadNetwork;
use crate::production::FoodLedger;
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
    ///
    /// **Oggi nessuno ne legge il contenuto**: il passo 3 ricalcola tutto e
    /// guarda solo [`DirtyFlags::coverage_da_rivedere`]. La lista esiste per
    /// l'invalidazione mirata, che e' l'ottimizzazione vera del passo 3 —
    /// tenerla popolata adesso costa poco e dice quale informazione servira'.
    /// Per questo "tutti dirty" **non** si esprime elencando tutti i provider
    /// ma con [`DirtyFlags::invalida_coverage`]: il giorno in cui la lista
    /// verra' letta, "tutti" e' esattamente il caso da evitare, non da
    /// enumerare.
    pub coverage: Vec<BuildingId>,
    /// La copertura va rivista anche se nessun provider e' nella lista.
    ///
    /// Serve al caso che la sola lista non sa esprimere: demolito l'ultimo
    /// provider, non resta nessuno da segnare come dirty, ma le assegnazioni
    /// esistenti vanno comunque buttate. Senza questo flag le case restavano
    /// servite da un pozzo che non c'e' piu' — un bug trovato dal test di
    /// demolizione della fase 06.
    pub coverage_invalidata: bool,
}

impl DirtyFlags {
    /// Segna un provider come da ricalcolare, senza duplicati e in ordine di
    /// `BuildingId`.
    ///
    /// L'ordine qui e' igiene, non semantica: e' l'ordine di iterazione di
    /// `World::buildings` a decidere chi vince le case contese (fase 06), e
    /// questa lista non viene ancora letta da nessuno. Tenerla ordinata serve
    /// a renderla confrontabile e a rendere l'inserimento una push in coda,
    /// visto che i provider arrivano quasi sempre in ordine crescente.
    pub fn segna_coverage(&mut self, id: BuildingId) {
        self.coverage_invalidata = true;
        if let Err(pos) = self.coverage.binary_search(&id) {
            self.coverage.insert(pos, id);
        }
    }

    /// La copertura va rivista, senza indicare un provider specifico.
    pub fn invalida_coverage(&mut self) {
        self.coverage_invalidata = true;
    }

    pub fn coverage_da_rivedere(&self) -> bool {
        self.coverage_invalidata || !self.coverage.is_empty()
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
    /// Struttura **derivata** dalla griglia: non entra nell'hash canonico
    /// dello stato (fase 08), altrimenti un bug di ricostruzione si
    /// presenterebbe come divergenza di hash invece che come test di
    /// equivalenza fallito.
    pub(crate) roads: RoadNetwork,
    /// Derivata come [`RoadNetwork`], e fuori dall'hash per lo stesso motivo.
    pub(crate) coverage: Coverage,
    /// Contabilita' diagnostica, fuori dall'hash: non influenza nessuna
    /// decisione di gioco.
    pub(crate) food: FoodLedger,
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
        let tiles = grid.len();
        Self {
            tick: 0,
            grid,
            buildings: SlotMap::with_key(),
            houses: SlotMap::with_key(),
            walkers: Vec::new(),
            economy: Economy { tesoro },
            rng: RngSet::from_seed(seed),
            dirty: DirtyFlags::default(),
            roads: RoadNetwork::new(tiles),
            coverage: Coverage::default(),
            food: FoodLedger::default(),
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

    pub const fn roads(&self) -> &RoadNetwork {
        &self.roads
    }

    pub const fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    pub const fn food(&self) -> &FoodLedger {
        &self.food
    }

    /// Somma delle giacenze di tutti i produttori, in millesimi.
    ///
    /// `i64` come il [`FoodLedger`]: e' il termine con cui si chiude
    /// l'uguaglianza di conservazione, e deve poter reggere lo stesso range.
    pub fn giacenza_totale(&self) -> i64 {
        self.buildings
            .values()
            .filter(|b| {
                self.data
                    .def(b.kind)
                    .is_some_and(crate::data::BuildingDef::e_un_produttore)
            })
            .map(|b| i64::from(b.stock.to_millis()))
            .sum()
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

    /// I tile strada 4-adiacenti al footprint di un edificio.
    ///
    /// **Regola di gioco**: un edificio e' agganciato alla rete se almeno un
    /// tile del suo footprint tocca ortogonalmente una strada. E' la regola di
    /// Zeus, dove conta l'ingresso e non l'edificio; in M1 potrebbe diventare
    /// "un tile d'ingresso designato", e allora questa e' la funzione da
    /// cambiare.
    pub fn ingressi_edificio(&self, id: BuildingId) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.ingressi_edificio_in(id, &mut out);
        out
    }

    /// Come [`World::ingressi_edificio`], scrivendo in un buffer riusabile.
    pub fn ingressi_edificio_in(&self, id: BuildingId, out: &mut Vec<TileIdx>) {
        out.clear();
        let Some(b) = self.buildings.get(id) else {
            return;
        };
        let footprint = self.data.def(b.kind).map_or((1, 1), |d| d.footprint);
        self.ingressi_in(b.origin, footprint, out);
    }

    /// Come [`World::ingressi_edificio`], per una casa.
    pub fn ingressi_casa(&self, id: HouseId) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.ingressi_casa_in(id, &mut out);
        out
    }

    /// Come [`World::ingressi_casa`], scrivendo in un buffer riusabile.
    ///
    /// Esiste per il passo 3, che la chiama una volta per casa a ogni
    /// ricalcolo: restituire un `Vec` nuovo ogni volta erano 3.750 allocazioni
    /// per ricalcolo alla scala di riferimento, tutte di due elementi scarsi.
    pub fn ingressi_casa_in(&self, id: HouseId, out: &mut Vec<TileIdx>) {
        out.clear();
        let Some(h) = self.houses.get(id) else {
            return;
        };
        self.ingressi_in(h.origin, (1, 1), out);
    }

    /// I tile strada adiacenti a un footprint, in ordine di `TileIdx`.
    pub fn ingressi(&self, origin: TilePos, footprint: (u8, u8)) -> Vec<TileIdx> {
        let mut out = Vec::new();
        self.ingressi_in(origin, footprint, &mut out);
        out
    }

    /// Come [`World::ingressi`], scrivendo in un buffer riusabile.
    pub fn ingressi_in(&self, origin: TilePos, footprint: (u8, u8), out: &mut Vec<TileIdx>) {
        out.clear();
        for dy in 0..footprint.1 {
            for dx in 0..footprint.0 {
                let (Some(x), Some(y)) = (origin.x.checked_add(dx), origin.y.checked_add(dy))
                else {
                    continue;
                };
                let Some(idx) = self.grid.idx(TilePos::new(x, y)) else {
                    continue;
                };
                for v in self.grid.neighbors4(idx) {
                    if self.grid.get(v).is_some_and(|t| t.flags.has_road()) {
                        out.push(v);
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// Distanza in tile percorsi sulla rete, da un edificio a un altro.
    ///
    /// `None` se non sono connessi o se sono oltre `max`. La distanza si conta
    /// tra i **tile strada di ingresso**: due edifici affacciati sulla stessa
    /// strada distano 0, e un corridoio di N tile tra i due ingressi vale
    /// N - 1. E' distanza percorsa, non euclidea (D2).
    pub fn road_distance(&self, from: BuildingId, to: BuildingId, max: u16) -> Option<u16> {
        let partenze = self.ingressi_edificio(from);
        let arrivi = self.ingressi_edificio(to);
        if partenze.is_empty() || arrivi.is_empty() {
            return None;
        }
        let mut migliore: Option<u16> = None;
        crate::network::bfs_strade(&self.grid, &partenze, max, |t, d| {
            if arrivi.binary_search(&t).is_ok() && migliore.is_none_or(|m| d < m) {
                migliore = Some(d);
            }
        });
        migliore
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

/// Hook di mutazione diretta, dietro la feature `test-util`.
///
/// Non fanno parte dell'API normale di proposito: l'unico canale in scrittura
/// verso il core sono i `Command` (CLAUDE.md, confine core/renderer). Servono
/// al test della fase 08 che verifica che l'hash canonico copra davvero ogni
/// campo dello stato — verifica che, per costruzione, deve poter toccare un
/// campo alla volta.
#[cfg(feature = "test-util")]
impl World {
    pub fn building_mut(&mut self, id: BuildingId) -> Option<&mut Building> {
        self.buildings.get_mut(id)
    }

    pub fn house_mut(&mut self, id: HouseId) -> Option<&mut House> {
        self.houses.get_mut(id)
    }

    pub const fn economy_mut(&mut self) -> &mut Economy {
        &mut self.economy
    }

    /// Consuma un valore dallo stream di un dominio, per verificare che la
    /// posizione dell'RNG entri nell'hash.
    pub fn consuma_rng(&mut self, domain: crate::rng::RngDomain) {
        use rand::RngCore as _;
        self.rng.get(domain).next_u64();
    }
}
