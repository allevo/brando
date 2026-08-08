//! Forma grezza delle tabelle, cosi' come stanno nei file RON.
//!
//! I campi sono interi nudi e stringhe: i newtype (`Milli`, `Coins`,
//! `ServiceKind`) compaiono solo dopo la validazione. Cosi' un RON con un
//! numero fuori range produce un errore di validazione leggibile invece di
//! un errore di deserializzazione oscuro, e i file restano leggibili a occhio.

use serde::Deserialize;
use sim_core::Terrain;

#[derive(Debug, Clone, Deserialize)]
pub struct RawRules {
    pub tick_per_mese: u32,
    pub mesi_per_anno: u32,
    pub tesoro_iniziale: i32,
    pub abitanti_per_livello_casa: Vec<u16>,
    pub consumo_cibo_per_abitante: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTerrainTable {
    pub terrains: Vec<RawTerrainDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawTerrainDef {
    pub terrain: Terrain,
    pub costruibile: bool,
    pub attraversabile: bool,
    pub costo_strada: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingTable {
    pub buildings: Vec<RawBuildingDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawBuildingDef {
    pub id: String,
    pub footprint: (u8, u8),
    pub costo: i32,
    pub livelli: u8,
    #[serde(default)]
    pub servizio: Option<RawServiceDef>,
    #[serde(default)]
    pub servizi_richiesti: Vec<String>,
    #[serde(default)]
    pub produzione_per_tick: Option<i32>,
    #[serde(default)]
    pub giacenza_max: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawServiceDef {
    pub kind: String,
    pub raggio_per_livello: Vec<u16>,
    pub capacita_per_livello: Vec<u16>,
}

/// Le tre tabelle appena deserializzate, prima di qualunque controllo.
#[derive(Debug, Clone)]
pub struct RawDataSet {
    pub rules: RawRules,
    pub terrain: RawTerrainTable,
    pub buildings: RawBuildingTable,
}
