//! Caricamento delle tabelle da disco.
//!
//! E' l'unico punto del progetto che fa I/O sui dati di gioco: `sim-core` non
//! ne fa nessuno (D4).

use std::path::{Path, PathBuf};

use crate::raw::{RawBuildingTable, RawDataSet, RawRules, RawTerrainTable};
use crate::validate::{ValidationReport, validate};
use sim_core::data::DataSet;

const FILE_RULES: &str = "rules.ron";
const FILE_TERRAIN: &str = "terrain.ron";
const FILE_BUILDINGS: &str = "buildings.ron";

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("impossibile leggere {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{file} non e' un RON valido: {source}")]
    Ron {
        file: &'static str,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("{0}")]
    Validazione(#[from] ValidationReport),
}

/// Carica le tre tabelle da una directory.
pub fn load_from_dir(dir: &Path) -> Result<DataSet, LoadError> {
    let leggi = |nome: &str| -> Result<String, LoadError> {
        let path = dir.join(nome);
        std::fs::read_to_string(&path).map_err(|source| LoadError::Io { path, source })
    };
    let rules = leggi(FILE_RULES)?;
    let terrain = leggi(FILE_TERRAIN)?;
    let buildings = leggi(FILE_BUILDINGS)?;
    from_ron_str(&rules, &terrain, &buildings)
}

/// Come [`load_from_dir`], ma a partire dal contenuto gia' letto.
///
/// Serve ai test (che compongono tabelle valide e rotte senza duplicare i
/// file su disco) e a chi vorra' incorporare le tabelle nel binario.
pub fn from_ron_str(rules: &str, terrain: &str, buildings: &str) -> Result<DataSet, LoadError> {
    let raw = RawDataSet {
        rules: parse::<RawRules>(FILE_RULES, rules)?,
        terrain: parse::<RawTerrainTable>(FILE_TERRAIN, terrain)?,
        buildings: parse::<RawBuildingTable>(FILE_BUILDINGS, buildings)?,
    };
    Ok(validate(&raw)?)
}

fn parse<T: serde::de::DeserializeOwned>(file: &'static str, s: &str) -> Result<T, LoadError> {
    ron::from_str(s).map_err(|source| LoadError::Ron { file, source })
}
