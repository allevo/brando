//! Il dataset di bilanciamento: la forma validata che il core consuma.
//!
//! Le **definizioni** stanno qui e non in `sim-data` per la direzione delle
//! dipendenze: il `World` tiene un `Arc<DataSet>` (A2), e sim-core non puo'
//! dipendere da sim-data. In `sim-data` restano il parsing RON, la
//! validazione e l'I/O — cioe' tutto cio' che il core non deve fare (D4).

use std::collections::BTreeMap;

use crate::data_hash::canonical_hash;
use crate::grid::Terrain;
use crate::ids::BuildingKindId;
use crate::service::ServiceKind;
use crate::units::{Coins, Milli};

/// Costanti globali di simulazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub tick_per_mese: u32,
    pub mesi_per_anno: u32,
    pub tesoro_iniziale: Coins,
    /// Indicizzato per livello di casa (livello 1 = indice 0).
    pub abitanti_per_livello_casa: Vec<u16>,
    pub consumo_cibo_per_abitante: Milli,
}

impl Rules {
    /// Tick in un anno di gioco. Gli obiettivi di scenario si esprimono in
    /// mesi e anni, mai in tick (M1).
    pub const fn tick_per_anno(&self) -> u32 {
        self.tick_per_mese * self.mesi_per_anno
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainDef {
    pub costruibile: bool,
    pub attraversabile: bool,
    pub costo_strada: Coins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDef {
    pub kind: ServiceKind,
    /// Raggio in tile percorsi sulla rete stradale (D2), uno per livello.
    pub raggio_per_livello: Vec<u16>,
    /// Case servite contemporaneamente, uno per livello.
    pub capacita_per_livello: Vec<u16>,
}

impl ServiceDef {
    /// Raggio al livello dato (livello 1 = indice 0), `None` fuori range.
    pub fn raggio(&self, livello: u8) -> Option<u16> {
        self.raggio_per_livello
            .get(usize::from(livello).checked_sub(1)?)
            .copied()
    }

    /// Capacita' al livello dato, `None` fuori range.
    pub fn capacita(&self, livello: u8) -> Option<u16> {
        self.capacita_per_livello
            .get(usize::from(livello).checked_sub(1)?)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// (larghezza, altezza) in tile.
    pub footprint: (u8, u8),
    pub costo: Coins,
    pub livelli: u8,
    pub servizio: Option<ServiceDef>,
    pub servizi_richiesti: Vec<ServiceKind>,
    pub produzione_per_tick: Option<Milli>,
    pub giacenza_max: Option<Milli>,
}

impl BuildingDef {
    /// Un edificio e' una casa se richiede servizi invece di fornirne.
    /// Regola strutturale, non un numero: sta nel codice di proposito.
    pub fn e_una_casa(&self) -> bool {
        self.servizio.is_none() && !self.servizi_richiesti.is_empty()
    }

    pub fn e_un_produttore(&self) -> bool {
        self.produzione_per_tick.is_some()
    }

    /// Numero di tile occupati.
    pub const fn tile_occupati(&self) -> u16 {
        self.footprint.0 as u16 * self.footprint.1 as u16
    }
}

/// Tabelle validate, pronte per il core.
///
/// Vive dietro un `Arc` dentro il `World` (A2): il caricamento e' I/O e resta
/// fuori dal core, ma i sistemi hanno bisogno delle tabelle a ogni tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSet {
    pub rules: Rules,
    /// `BTreeMap` e non `HashMap`: l'ordine di iterazione e' un contratto (D4).
    pub terrain: BTreeMap<Terrain, TerrainDef>,
    /// Indicizzato per [`BuildingKindId`].
    pub buildings: Vec<BuildingDef>,
    /// blake3 del contenuto **validato**, non dei byte dei file: riformattare
    /// un RON o aggiungere un commento non cambia l'hash, cambiare un numero
    /// si. Entra nell'hash dello stato (A2). Lo calcola [`DataSet::new`].
    pub hash: [u8; 32],
}

impl DataSet {
    /// Costruisce e calcola l'hash canonico. L'unico modo di ottenere un
    /// `DataSet`: cosi' l'hash non puo' essere fuori sincrono col contenuto.
    pub fn new(
        rules: Rules,
        terrain: BTreeMap<Terrain, TerrainDef>,
        buildings: Vec<BuildingDef>,
    ) -> Self {
        let hash = canonical_hash(&rules, &terrain, &buildings);
        Self {
            rules,
            terrain,
            buildings,
            hash,
        }
    }

    pub fn def(&self, kind: BuildingKindId) -> Option<&BuildingDef> {
        self.buildings.get(usize::from(kind.get()))
    }

    /// Risolve l'id testuale usato nelle tabelle e negli scenari.
    pub fn kind_by_id(&self, id: &str) -> Option<BuildingKindId> {
        let pos = self.buildings.iter().position(|b| b.id == id)?;
        u16::try_from(pos).ok().map(BuildingKindId::new)
    }

    pub fn terrain(&self, t: Terrain) -> Option<&TerrainDef> {
        self.terrain.get(&t)
    }

    /// Hash in esadecimale, per i messaggi di errore e gli header di replay.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }
}
